//! External actual-owner qualification. This does not invent an owner view,
//! import an owner's implementation, or claim a root/Gateway HTTP journey.
use super::{
    guard::{binding_digest, wire_identity},
    native,
    read_reference::ReadReference,
    wire::{lease, proto},
};
use platform::{
    actor_delegation::{
        concert_self_profile::ConcertSelfProfileSigner, relationship_read::RelationshipReadSigner,
        request_for, ActorDelegationVerifier,
    },
    grpc_auth::{ServiceIdentityClientInterceptor, Workload},
    grpc_tls::TlsMaterial,
};
use sha2::{Digest, Sha256};
use std::{path::PathBuf, time::Duration};
use tonic::{service::interceptor::InterceptedService, Request};
use uuid::Uuid;
fn required(name: &str) -> anyhow::Result<String> {
    Ok(std::env::var(name)?)
}
fn tls(role: Workload) -> anyhow::Result<TlsMaterial> {
    let p = PathBuf::from(required("MCR_SELF_PROFILE_ROOT")?).join("pki");
    TlsMaterial::load(
        p.join(format!("{}.crt", role.as_str())),
        p.join(format!("{}.key", role.as_str())),
        p.join("ca.crt"),
    )
}
fn signed<T>(message: T, actor: Uuid) -> anyhow::Result<Request<T>> {
    let mut request = request_for(message, &actor.to_string())?;
    request.set_timeout(Duration::from_secs(4));
    Ok(request)
}
#[tokio::test]
#[ignore = "requires owned PG16 and separately frozen real Auth/Friend/Profile mTLS processes"]
async fn actual_owner_read_track_and_native_abort_settlement() -> anyhow::Result<()> {
    let pool = native::pool().await?;
    let actor = Uuid::parse_str(&required("MCR_SELF_PROFILE_ACTOR")?)?;
    let family = Uuid::parse_str(&required("MCR_SELF_PROFILE_FAMILY")?)?;
    let event = Uuid::parse_str(&required("MCR_SELF_PROFILE_EVENT")?)?;
    anyhow::ensure!(
        !actor.is_nil() && !family.is_nil() && !event.is_nil(),
        "explicit fixture identities required"
    );
    // Actual canonical rows must already be created by the private fixture
    // controller. The client cannot fabricate a missing profile or live session.
    let existing: (String, String) =
        sqlx::query_as("select nickname,bio from profiles where user_id=$1 and deleted_at is null")
            .bind(actor)
            .fetch_one(&pool)
            .await?;
    let gateway = ActorDelegationVerifier::from_env(false, Workload::Profile)?;
    let root_signer = RelationshipReadSigner::from_env(&gateway)?;
    let self_signer = ConcertSelfProfileSigner::from_env(&gateway)?;
    let root_tls = tls(Workload::Concert)?;
    let token = required("CONCERT_SERVICE_CREDENTIAL")?;
    let interceptor = || ServiceIdentityClientInterceptor::new(Workload::Concert, &token);
    let auth_channel = root_tls
        .endpoint(&required("MCR_SELF_PROFILE_AUTH_ENDPOINT")?, Workload::Auth)?
        .connect()
        .await?;
    let friend_channel = root_tls
        .endpoint(
            &required("MCR_SELF_PROFILE_FRIEND_ENDPOINT")?,
            Workload::Friend,
        )?
        .connect()
        .await?;
    let profile_channel = root_tls
        .endpoint(
            &required("MCR_SELF_PROFILE_PROFILE_ENDPOINT")?,
            Workload::Profile,
        )?
        .connect()
        .await?;
    let mut auth = lease::auth_owner_authority_service_client::AuthOwnerAuthorityServiceClient::new(
        root_signer.client(
            InterceptedService::new(auth_channel, interceptor()?),
            Workload::Auth,
        )?,
    );
    let mut friend =
        lease::friend_owner_authority_service_client::FriendOwnerAuthorityServiceClient::new(
            root_signer.client(
                InterceptedService::new(friend_channel, interceptor()?),
                Workload::Friend,
            )?,
        );
    let mut profile =
        proto::concert_self_profile_service_client::ConcertSelfProfileServiceClient::new(
            self_signer.client(InterceptedService::new(profile_channel, interceptor()?)),
        );
    let binding = lease::GuardBinding {
        actor_id: actor.to_string(),
        event_id: event.to_string(),
        session_family_id: family.to_string(),
        request_id: Uuid::new_v4().to_string(),
        operation: "getConcertParticipant".into(),
        subject_ids: Vec::new(),
        expires_unix_micros: chrono::Utc::now().timestamp_micros() + 4_500_000,
        // Test-only trusted root envelope. Actual Auth/Friend facts/leases and
        // Track/settlement are real; upstream Gateway admission is not claimed.
        parent_digest: Sha256::digest(b"native fixture root envelope").to_vec(),
        parent_body_digest: Sha256::digest(b"native fixture framed parent").to_vec(),
        parent_method: "/concert.participant.v1.ConcertParticipantService/GetParticipant".into(),
        http_method: "GET".into(),
        http_target: format!("/api/concert/events/{event}/me"),
        body_digest: Sha256::digest(b"").to_vec(),
    };
    let root_scope = lease::ProducerScope {
        instance_id: Uuid::new_v4().to_string(),
        scope_id: Uuid::new_v4().to_string(),
        binding_digest: binding_digest(&binding)?.to_vec(),
    };
    let request = lease::AcquireRequest {
        binding: Some(binding.clone()),
        response_scope: Some(root_scope),
    };
    let a = auth
        .acquire(signed(request.clone(), actor)?)
        .await?
        .into_inner();
    let f = friend.acquire(signed(request, actor)?).await?.into_inner();
    anyhow::ensure!(
        a.session_live
            && a.subjects
                .iter()
                .any(|s| s.user_id == actor.to_string() && s.active && s.security_revision > 0),
        "actual Auth fact required"
    );
    anyhow::ensure!(
        f.subjects
            .iter()
            .any(|s| s.user_id == actor.to_string() && s.active),
        "actual Friend fact required"
    );
    let scope = proto::SelfScope {
        binding: Some(binding.clone()),
        auth_view: a.view,
        friend_view: f.view,
    };
    let query = proto::ReadSelfRequest {
        scope: Some(scope.clone()),
        subject_id: actor.to_string(),
    };
    let response = profile.read(signed(query.clone(), actor)?).await?;
    assert_eq!(
        response.metadata().get("cache-control").unwrap(),
        "no-store"
    );
    let response = response.into_inner();
    let content = response
        .content
        .ok_or_else(|| anyhow::anyhow!("real SELF content required"))?;
    assert_eq!(content.user_id, actor.to_string());
    assert_eq!(content.display_name, existing.0);
    assert_eq!(
        content.bio,
        if existing.1.is_empty() {
            None
        } else {
            Some(existing.1)
        }
    );
    assert!(content.version.unwrap().revision > 0);
    assert!(
        response.observed_at_unix_micros > 0
            && response.expires_unix_micros <= binding.expires_unix_micros
    );
    let reference: Uuid = sqlx::query_scalar(
        "select reference_id from concert_self_profile_read_references where binding_proto=$1",
    )
    .bind(prost::Message::encode_to_vec(&binding))
    .fetch_one(&pool)
    .await?;
    let read = ReadReference::load(&pool, reference).await?.unwrap();
    let auth_channel = tls(Workload::Auth)?
        .endpoint(
            &required("MCR_SELF_PROFILE_PROFILE_ENDPOINT")?,
            Workload::Profile,
        )?
        .connect()
        .await?;
    let mut settlement =
        lease::producer_settlement_service_client::ProducerSettlementServiceClient::new(
            InterceptedService::new(
                auth_channel,
                ServiceIdentityClientInterceptor::new(
                    Workload::Auth,
                    &required("AUTH_SERVICE_CREDENTIAL")?,
                )?,
            ),
        );
    let mut request = Request::new(lease::ResolveTransactionRequest {
        binding: Some(binding.clone()),
        transaction: Some(wire_identity(&read.identity, reference)),
        registration_id: Uuid::new_v4().to_string(),
    });
    request.set_timeout(Duration::from_secs(3));
    let settled = settlement.resolve_transaction(request).await?.into_inner();
    assert_eq!(settled.status, lease::TransactionStatus::Aborted as i32);
    assert_eq!(settled.receipt_evidence.len(), 32);
    let before: i64 =
        sqlx::query_scalar("select count(*) from concert_self_profile_read_references")
            .fetch_one(&pool)
            .await?;
    let mut wrong = query.clone();
    wrong.subject_id = Uuid::new_v4().to_string();
    assert_eq!(
        profile
            .read(signed(wrong, actor)?)
            .await
            .unwrap_err()
            .code(),
        tonic::Code::PermissionDenied
    );
    let mut wrong = query;
    wrong
        .scope
        .as_mut()
        .unwrap()
        .friend_view
        .as_mut()
        .unwrap()
        .view_secret = vec![0; 32];
    assert_eq!(
        profile
            .read(signed(wrong, actor)?)
            .await
            .unwrap_err()
            .code(),
        tonic::Code::PermissionDenied
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from concert_self_profile_read_references")
            .fetch_one(&pool)
            .await?,
        before
    );
    // No fake root completion is sent. The controller must prove that these
    // real owner guards remain quarantined until actual root settlement, and
    // clean only its own isolated fixture without calling that production PASS.
    pool.close().await;
    Ok(())
}
