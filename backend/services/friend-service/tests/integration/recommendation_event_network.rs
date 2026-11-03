//! Production registration guard, generated client/server and real Friend PG.
//! Test-CA mTLS, local parity signing keys and Profile doubles do not qualify
//! public REST, production CA/key rotation, provider or multi-process acceptance.
use super::recommendation_event_atomicity::{hide_event, seed_impression};
use crate::support::start_postgres_app;
use anyhow::Result;
use friend_service::adapters::r#in::grpc::method_policy::FRIEND_METHOD_POLICY;
use friend_service::adapters::r#in::grpc::server::{proto, FriendGrpcServer};
use platform::actor_delegation::{
    request_for, ActorDelegationClient, ActorDelegationSigner, ActorDelegationVerifier,
};
use platform::grpc_auth::{CallerRegistry, ServiceIdentityClientInterceptor, Workload};
use platform::grpc_method_policy::guard;
use platform::grpc_tls::TlsMaterial;
use prost::Message;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::{Channel, Server};
use tonic::Code;
use uuid::Uuid;

fn tls_material(workload: Workload) -> Result<TlsMaterial> {
    let leaf = test_support::tls::issue(workload.as_str())?;
    TlsMaterial::load(leaf.cert, leaf.key, leaf.ca)
}

async fn effects(pool: &sqlx::PgPool) -> Result<serde_json::Value> {
    Ok(sqlx::query_scalar("select jsonb_build_object('events',coalesce((select jsonb_agg(to_jsonb(e) order by event_id) from people_recommendation_events e),'[]'::jsonb),'refresh',coalesce((select jsonb_agg(to_jsonb(j) order by viewer_id) from people_recommendation_refresh_jobs j),'[]'::jsonb))")
        .fetch_one(pool).await?)
}

async fn call(
    channel: Channel,
    workload: Workload,
    credential: &str,
    delegation: Option<(ActorDelegationSigner, Workload)>,
    actor: Uuid,
    message: proto::RecordRecommendationEventRequest,
) -> Result<Result<bool, tonic::Status>> {
    let identity = ServiceIdentityClientInterceptor::new(workload, credential)?;
    let response = match delegation {
        Some((signer, audience)) => {
            proto::friend_service_client::FriendServiceClient::with_interceptor(
                ActorDelegationClient::new(channel, signer, audience),
                identity,
            )
            .record_people_recommendation_event(request_for(message, &actor.to_string())?)
            .await
        }
        None => {
            proto::friend_service_client::FriendServiceClient::with_interceptor(channel, identity)
                .record_people_recommendation_event(message)
                .await
        }
    };
    Ok(response.map(|response| response.into_inner().accepted))
}

fn wire_body(message: &proto::RecordRecommendationEventRequest) -> Vec<u8> {
    let encoded = message.encode_to_vec();
    let mut body = vec![0]; // Uncompressed unary gRPC message frame.
    body.extend_from_slice(&u32::try_from(encoded.len()).unwrap().to_be_bytes());
    body.extend_from_slice(&encoded);
    body
}

async fn call_token(
    channel: Channel,
    token: &str,
    message: proto::RecordRecommendationEventRequest,
) -> Result<Result<bool, tonic::Status>> {
    let identity =
        ServiceIdentityClientInterceptor::new(Workload::Gateway, "network-gateway-credential")?;
    let mut request = tonic::Request::new(message);
    request
        .metadata_mut()
        .insert(platform::actor_delegation::HEADER, token.parse()?);
    Ok(
        proto::friend_service_client::FriendServiceClient::with_interceptor(channel, identity)
            .record_people_recommendation_event(request)
            .await
            .map(|response| response.into_inner().accepted),
    )
}

#[tokio::test]
async fn guarded_recommendation_rpc_denies_effects_and_preserves_signed_retry_identity(
) -> Result<()> {
    let app = start_postgres_app(false).await?;
    let actor = Uuid::new_v4();
    let victim = Uuid::new_v4();
    let mut event = hide_event(actor);
    event.actor_id = Some(actor);
    seed_impression(&app.pool, &event).await;
    let message = proto::RecordRecommendationEventRequest {
        event_id: event.event_id.to_string(),
        schema_version: i32::from(event.schema_version),
        occurred_at: event.occurred_at.to_rfc3339(),
        viewer_id: actor.to_string(),
        subject_type: "USER".into(),
        subject_id: event.subject_id.to_string(),
        actor_id: actor.to_string(),
        surface: "PEOPLE_RECOMMENDATION".into(),
        position: 0,
        request_id: event.request_id.clone().unwrap(),
        action: "HIDE".into(),
        source: event.source.clone(),
        model_version: event.model_version.clone(),
        impression_id: event.impression_id.unwrap().to_string(),
        metadata_json: "{}".into(),
        ..Default::default()
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let (shutdown, stopped) = tokio::sync::oneshot::channel();
    let registry = CallerRegistry::new()
        .register(Workload::Gateway, "network-gateway-credential")?
        .register(Workload::Post, "network-post-credential")?;
    let server_tls = tls_material(Workload::Friend)?;
    let gateway_tls = tls_material(Workload::Gateway)?;
    let post_tls = tls_material(Workload::Post)?;
    let server = tokio::spawn(
        Server::builder()
            .tls_config(server_tls.server_config()?)?
            .add_service(guard(
                &FRIEND_METHOD_POLICY,
                proto::friend_service_server::FriendServiceServer::new(FriendGrpcServer::new(
                    app.state.clone(),
                )),
                registry,
                ActorDelegationVerifier::local_parity(Workload::Friend),
            )?)
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async {
                let _ = stopped.await;
            }),
    );
    let outcome: Result<()> = async {
        let endpoint = format!("https://{address}");
        let channel = gateway_tls.connect(&endpoint, Workload::Friend).await?;
        let post_channel = post_tls.connect(&endpoint, Workload::Friend).await?;
        let signer = ActorDelegationSigner::local_parity(Workload::Gateway);
        let before = effects(&app.pool).await?;
        for (workload, credential, delegation, delegated_actor, expected) in [
            (
                Workload::Gateway,
                "network-gateway-credential",
                None,
                actor,
                Code::Unauthenticated,
            ),
            (
                Workload::Post,
                "network-post-credential",
                None,
                actor,
                Code::PermissionDenied,
            ),
            (
                Workload::Gateway,
                "network-post-credential",
                None,
                actor,
                Code::Unauthenticated,
            ),
            (
                Workload::Gateway,
                "network-gateway-credential",
                Some((signer.clone(), Workload::Friend)),
                victim,
                Code::PermissionDenied,
            ),
            (
                Workload::Gateway,
                "network-gateway-credential",
                Some((signer.clone(), Workload::Post)),
                actor,
                Code::PermissionDenied,
            ),
            (
                Workload::Gateway,
                "network-gateway-credential",
                Some((
                    ActorDelegationSigner::from_seed(Workload::Gateway, &[42; 32])?,
                    Workload::Friend,
                )),
                actor,
                Code::Unauthenticated,
            ),
        ] {
            let status = call(
                if workload == Workload::Post {
                    post_channel.clone()
                } else {
                    channel.clone()
                },
                workload,
                credential,
                delegation,
                delegated_actor,
                message.clone(),
            )
            .await?
            .unwrap_err();
            assert_eq!(status.code(), expected);
            assert_eq!(
                effects(&app.pool).await?,
                before,
                "denied call changed owner rows"
            );
        }
        // TLS trust alone is insufficient: the Post leaf must not authenticate
        // a Gateway caller even with the Gateway's valid credential/delegation.
        let status = call(
            post_channel,
            Workload::Gateway,
            "network-gateway-credential",
            Some((signer.clone(), Workload::Friend)),
            actor,
            message.clone(),
        )
        .await?
        .unwrap_err();
        assert_eq!(status.code(), Code::Unauthenticated);
        assert_eq!(effects(&app.pool).await?, before);
        let mut wrong_actor_field = message.clone();
        wrong_actor_field.actor_id = victim.to_string();
        let status = call(
            channel.clone(),
            Workload::Gateway,
            "network-gateway-credential",
            Some((signer.clone(), Workload::Friend)),
            actor,
            wrong_actor_field,
        )
        .await?
        .unwrap_err();
        assert_eq!(status.code(), Code::PermissionDenied);
        assert_eq!(effects(&app.pool).await?, before);

        let method = "/friend.v1.FriendService/RecordPeopleRecommendationEvent";
        let now = chrono::Utc::now().timestamp();
        let body = wire_body(&message);
        let mut wrong_resource = message.clone();
        wrong_resource.subject_id = victim.to_string();
        for (label, token, request, expected) in [
            (
                "expired",
                signer.issue(Workload::Friend, method, actor, None, &body, now - 3600),
                message.clone(),
                Code::Unauthenticated,
            ),
            (
                "future issuance",
                signer.issue(Workload::Friend, method, actor, None, &body, now + 3600),
                message.clone(),
                Code::Unauthenticated,
            ),
            (
                "wrong method",
                signer.issue(
                    Workload::Friend,
                    "/friend.v1.FriendService/GetPeopleRecommendations",
                    actor,
                    None,
                    &body,
                    now,
                ),
                message.clone(),
                Code::PermissionDenied,
            ),
            (
                "resource body changed",
                signer.issue(Workload::Friend, method, actor, None, &body, now),
                wrong_resource,
                Code::PermissionDenied,
            ),
            (
                "wrong operation",
                signer.issue(
                    Workload::Friend,
                    method,
                    actor,
                    Some(Uuid::new_v4()),
                    &body,
                    now,
                ),
                message.clone(),
                Code::PermissionDenied,
            ),
        ] {
            let status = call_token(channel.clone(), &token, request)
                .await?
                .unwrap_err();
            assert_eq!(status.code(), expected, "{label}");
            assert_eq!(
                effects(&app.pool).await?,
                before,
                "{label} changed owner rows"
            );
        }

        assert!(
            call(
                channel.clone(),
                Workload::Gateway,
                "network-gateway-credential",
                Some((signer.clone(), Workload::Friend)),
                actor,
                message.clone()
            )
            .await??
        );
        let committed = effects(&app.pool).await?;
        assert_eq!(committed["events"].as_array().unwrap().len(), 1);
        assert_eq!(committed["refresh"].as_array().unwrap().len(), 1);
        // The signer issues a fresh delegation nonce. Application identity is
        // independently retained by the owner, not by the authentication cache.
        assert!(
            !call(
                channel.clone(),
                Workload::Gateway,
                "network-gateway-credential",
                Some((signer.clone(), Workload::Friend)),
                actor,
                message.clone()
            )
            .await??
        );
        assert_eq!(effects(&app.pool).await?, committed);
        let token = signer.issue(
            Workload::Friend,
            method,
            actor,
            None,
            &body,
            chrono::Utc::now().timestamp(),
        );
        assert!(!call_token(channel.clone(), &token, message.clone()).await??);
        let replay = call_token(channel.clone(), &token, message.clone())
            .await?
            .unwrap_err();
        assert_eq!(replay.code(), Code::Unauthenticated);
        assert_eq!(effects(&app.pool).await?, committed);
        let mut conflict = message;
        conflict.action = "CLICK".into();
        let status = call(
            channel,
            Workload::Gateway,
            "network-gateway-credential",
            Some((signer, Workload::Friend)),
            actor,
            conflict,
        )
        .await?
        .unwrap_err();
        assert_eq!(status.code(), Code::AlreadyExists);
        assert_eq!(
            status
                .metadata()
                .get(platform::mutation_context::ERROR_CODE)
                .unwrap()
                .to_str()?,
            "conflict"
        );
        assert_eq!(effects(&app.pool).await?, committed);
        Ok(())
    }
    .await;
    let _ = shutdown.send(());
    tokio::time::timeout(std::time::Duration::from_secs(5), server).await???;
    app.shutdown().await?;
    outcome
}
