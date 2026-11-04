//! Component qualification: production canonical owners/child machine and body
//! layer, not ProductRpcService's global notification routing or Gateway HTTP.
use super::{capture, environment, fixture::Fixture};
use concert_service::{
    adapters::out::relationship_authority::guarded::{GuardBinding, OwnerGuardScope},
    notification_live::NotificationRequest,
    product_rpc::{
        delivery::{DeliveryService, POLICY},
        proto,
    },
};
use http_body_util::BodyExt;
use platform::{
    actor_delegation::{
        authority_deadline::AuthorityRpcDeadline,
        response_scope::{ResponseScopeToken, RetainedScopeLayer},
    },
    grpc_auth::{CallerRegistry, Workload},
    grpc_method_policy::guard_without_delegation,
};
use prost::Message;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::{sync::Arc, time::Duration};
use tower::{Layer, ServiceExt};
use uuid::Uuid;

fn request(actor: Uuid, family: Uuid, operation: usize) -> proto::ProductRequest {
    let event = Uuid::new_v4();
    let now = chrono::Utc::now().timestamp_micros();
    let (method, suffix, body) = match operation {
        0 => ("GET", String::new(), vec![]),
        1 => ("GET", "/unread-count".into(), vec![]),
        2 => (
            "PATCH",
            format!("/{}/read", Uuid::new_v4()),
            b"{\"expected_revision\":1}".to_vec(),
        ),
        _ => ("PATCH", "/read-all".into(), b"{}".to_vec()),
    };
    proto::ProductRequest {
        user_id: actor.to_string(),
        event_id: event.to_string(),
        session_family_id: family.to_string(),
        request_id: Uuid::new_v4().to_string(),
        http_method: method.into(),
        http_target: format!("/api/concert/events/{event}/me/notifications{suffix}"),
        http_body: body,
        content_type: if operation >= 2 {
            "application/json".into()
        } else {
            String::new()
        },
        idempotency_key: if operation >= 2 {
            Uuid::new_v4().to_string()
        } else {
            String::new()
        },
        authority_expires_unix_micros: now + 1_800_000,
        auth_observed_at_unix_micros: now,
        profile_observed_at_unix_micros: now,
        access_expires_unix_micros: now + 60_000_000,
        ..Default::default()
    }
}
async fn acquire(
    f: &Fixture,
    r: &proto::ProductRequest,
) -> anyhow::Result<(
    platform::actor_delegation::exchange::VerifiedParentAuthority,
    Arc<OwnerGuardScope>,
)> {
    let parent = capture::parent(r).await?;
    let deadline =
        AuthorityRpcDeadline::new(r.authority_expires_unix_micros, Duration::from_secs(2))?;
    let b = GuardBinding::from_parent(
        &parent,
        Uuid::parse_str(&r.user_id)?,
        Uuid::parse_str(&r.event_id)?,
        Uuid::parse_str(&r.session_family_id)?,
        Uuid::parse_str(&r.request_id)?,
        NotificationRequest::parse(r)?.operation().id(),
        &r.http_method,
        &r.http_target,
        Sha256::digest(&r.http_body).into(),
        deadline,
    )?;
    let scope = f.fence.acquire(b).await?;
    Ok((parent, scope))
}
async fn pending(pool: &PgPool) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar(
        "select count(*) from pg_prepared_xacts where database=current_database()",
    )
    .fetch_one(pool)
    .await?)
}
async fn children(f: &Fixture, scope: &OwnerGuardScope, completed: bool) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar("select count(*) from concert_product_child_intents where scope_id=$1 and (completed_at is not null)=$2").bind(Uuid::parse_str(&scope.producer_scope().scope_id)?).bind(completed).fetch_one(&f.concert).await?)
}
fn frame(m: &impl Message) -> Vec<u8> {
    let b = m.encode_to_vec();
    let mut f = vec![0];
    f.extend_from_slice(&(b.len() as u32).to_be_bytes());
    f.extend(b);
    f
}
async fn body_eof(token: ResponseScopeToken, body: Vec<u8>) -> anyhow::Result<()> {
    let service = tower::service_fn(move |_| {
        let token = token.clone();
        let body = body.clone();
        async move {
            let mut r = axum::http::Response::new(axum::body::Body::from(body));
            r.extensions_mut().insert(token);
            Ok::<_, std::convert::Infallible>(r)
        }
    });
    RetainedScopeLayer
        .layer(service)
        .oneshot(())
        .await?
        .into_body()
        .collect()
        .await
        .map_err(|e| anyhow::anyhow!("component body: {e}"))?;
    Ok(())
}
async fn complete(f: &Fixture, capability: proto::ResponseDelivery) -> anyhow::Result<()> {
    let service = guard_without_delegation(
        &POLICY,
        proto::product_delivery_service_server::ProductDeliveryServiceServer::new(
            DeliveryService::new(f.publisher.clone()),
        ),
        CallerRegistry::new().register(Workload::Gateway, "gateway-fixture")?,
    )?;
    let service = platform::call_deadline::CallDeadlineLayer.layer(service);
    for _ in 0..2 {
        let r = axum::http::Request::builder()
            .method("POST")
            .uri("/concert.product.v1.ProductDeliveryService/CompleteResponse")
            .header("content-type", "application/grpc")
            .header("te", "trailers")
            .header("x-service-caller", "gateway")
            .header("x-service-token", "gateway-fixture")
            .body(axum::body::Body::from(frame(&capability)))?;
        let response = service.clone().oneshot(r).await?;
        let b = response.into_body().collect().await?;
        anyhow::ensure!(
            b.trailers()
                .and_then(|h| h.get("grpc-status"))
                .map_or(true, |v| v == "0"),
            "actual delivery denied"
        );
        let bytes = b.to_bytes();
        anyhow::ensure!(bytes.len() >= 5, "actual delivery response missing");
        assert!(proto::CompleteResponseResult::decode(&bytes[5..])?.completed);
    }
    Ok(())
}
#[tokio::test]
#[ignore = "requires named frozen owned PG16/mTLS lease"]
async fn missing_target_policy_denies_all_four_under_real_owners_and_terminal_body(
) -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let (actor, family, _) = environment::identities(&f.auth, 0).await?;
    for op in 0..4 {
        let r = request(actor, family, op);
        let (parent, scope) = acquire(&f, &r).await?;
        let publisher = f.publisher.clone();
        let runtime = f.runtime.clone();
        let owned = scope.clone();
        let (representation, delivery, token) = scope
            .run_owned(async move {
                parent
                    .scope(async move {
                        let (delivery, token) = publisher.publish(owned.clone()).await?;
                        let result = runtime.dispatch_for_scope(owned, r).await?;
                        Ok((result, delivery, token))
                    })
                    .await
            })
            .await?;
        assert_eq!(representation.status, 503);
        let value: serde_json::Value = serde_json::from_slice(&representation.body)?;
        assert_eq!(value["details"]["reason"], "DEPENDENCY_UNAVAILABLE");
        assert!(!String::from_utf8(representation.body.clone())?.contains("root target policy"));
        assert_eq!(children(&f, &scope, false).await?, 1);
        assert_eq!(children(&f, &scope, true).await?, 0);
        assert_eq!((pending(&f.auth).await?, pending(&f.friend).await?), (1, 1));
        assert!(
            scope.finish_response().await.is_err(),
            "unconsumed component body is not serialized"
        );
        let intents: i64 =
            sqlx::query_scalar("select count(*) from concert_notification_authority_intents")
                .fetch_one(&f.notification)
                .await?;
        assert_eq!(
            intents, op as i64,
            "missing provider is refused before local owner intent, including empty/count"
        );
        body_eof(token, representation.body).await?;
        assert_eq!(
            children(&f, &scope, false).await?,
            1,
            "component EOF only marks serialized"
        );
        assert_eq!((pending(&f.auth).await?, pending(&f.friend).await?), (1, 1));
        complete(&f, delivery).await?;
        assert_eq!(children(&f, &scope, true).await?, 1);
        assert_eq!(children(&f, &scope, false).await?, 0);
        assert_eq!((pending(&f.auth).await?, pending(&f.friend).await?), (0, 0));
    }
    println!("all four actual Notification RPCs fail closed without targets; real canonical Auth/Friend/mTLS, durable predispatch child, actual component body EOF only marks serialized, authenticated terminal Delivery closes no-effect child and owners; no production target or Gateway HTTP acceptance");
    Ok(())
}
#[tokio::test]
#[ignore = "requires named frozen owned PG16/mTLS lease"]
async fn sealed_parent_mutations_and_missing_task_local_never_register_child() -> anyhow::Result<()>
{
    let f = Fixture::new().await?;
    let (actor, family, _) = environment::identities(&f.auth, 0).await?;
    let r = request(actor, family, 0);
    let (parent, scope) = acquire(&f, &r).await?;
    let runtime = f.runtime.clone();
    let owned = scope.clone();
    let publisher = f.publisher.clone();
    let (delivery, token) = scope
        .run_owned(async move {
            let (delivery, token) = publisher.publish(owned.clone()).await?;
            assert!(runtime
                .dispatch_for_scope(owned.clone(), r.clone())
                .await
                .is_err());
            parent
                .scope(async move {
                    for field in 0..9 {
                        let mut altered = r.clone();
                        match field {
                            0 => altered.user_id = Uuid::new_v4().to_string(),
                            1 => altered.event_id = Uuid::new_v4().to_string(),
                            2 => altered.session_family_id = Uuid::new_v4().to_string(),
                            3 => altered.request_id = Uuid::new_v4().to_string(),
                            4 => altered.http_method = "PATCH".into(),
                            5 => altered.http_target.push_str("?limit=1"),
                            6 => altered.http_body = b"{}".to_vec(),
                            7 => altered.authority_expires_unix_micros += 1,
                            _ => altered.idempotency_key = Uuid::new_v4().to_string(),
                        };
                        assert!(runtime
                            .dispatch_for_scope(owned.clone(), altered)
                            .await
                            .is_err());
                    }
                    Ok((delivery, token))
                })
                .await
        })
        .await?;
    assert_eq!(children(&f, &scope, false).await?, 0);
    let count: i64 =
        sqlx::query_scalar("select count(*) from concert_notification_authority_intents")
            .fetch_one(&f.notification)
            .await?;
    assert_eq!(count, 0);
    body_eof(token, b"{}".to_vec()).await?;
    complete(&f, delivery).await?;
    assert_eq!((pending(&f.auth).await?, pending(&f.friend).await?), (0, 0));
    Ok(())
}
#[tokio::test]
#[ignore = "requires named frozen owned PG16/mTLS lease; expected quarantine until fixture teardown"]
async fn dropped_body_and_expiry_do_not_finish_registered_child() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let (actor, family, _) = environment::identities(&f.auth, 0).await?;
    let r = request(actor, family, 1);
    let original = r.clone();
    let (parent, scope) = acquire(&f, &r).await?;
    let owned = scope.clone();
    let publisher = f.publisher.clone();
    let runtime = f.runtime.clone();
    let (result, token) = scope
        .run_owned(async move {
            parent
                .scope(async move {
                    let (_, token) = publisher.publish(owned.clone()).await?;
                    let result = runtime.dispatch_for_scope(owned, r).await?;
                    Ok((result, token))
                })
                .await
        })
        .await?;
    assert_eq!(result.status, 503);
    drop(result);
    drop(token);
    let stop = scope.binding().deadline().stop() + Duration::from_millis(25);
    tokio::time::sleep_until(stop).await;
    assert!(f
        .runtime
        .dispatch_for_scope(scope.clone(), original)
        .await
        .is_err());
    assert_eq!(children(&f, &scope, false).await?, 1);
    assert_eq!(children(&f, &scope, true).await?, 0);
    let phase: String =
        sqlx::query_scalar("select phase from concert_product_owner_scopes where scope_id=$1")
            .bind(Uuid::parse_str(&scope.producer_scope().scope_id)?)
            .fetch_one(&f.concert)
            .await?;
    assert_eq!(phase, "ACTIVE");
    assert_eq!((pending(&f.auth).await?, pending(&f.friend).await?), (1, 1));
    println!("dropped unconsumed component body plus expired original scope leaves actual Root/child and both canonical owners quarantined; fixture PostgreSQL teardown is not settlement evidence");
    Ok(())
}
