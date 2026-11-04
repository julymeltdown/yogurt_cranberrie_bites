//! CF01 native composition, retained delivery and original Notification recovery.
//! Test-owned guarded RPC/body transport is not public Gateway HTTP acceptance.
#[path = "canonical_owner_composition_support/capture.rs"]
mod capture;
#[path = "canonical_owner_composition_support/environment.rs"]
mod environment;
#[path = "canonical_owner_composition_support/fixture.rs"]
mod fixture;
#[path = "canonical_owner_composition_support/notification.rs"]
mod notification;
#[path = "canonical_owner_composition_support/owners.rs"]
mod owners;
#[path = "canonical_owner_composition_support/release.rs"]
mod release;
use concert_service::{
    adapters::out::relationship_authority::guarded::{ChildRecoveryReference, OwnerGuardScope},
    config::canonical_runtime::CanonicalRuntimeCore,
    product_rpc::{
        child_recovery::ChildRecoverySettings,
        child_settlement::NotificationInboxIntentSettler,
        delivery::{DeliveryService, POLICY},
        proto,
        scope_ledger::{ScopeKey, ScopePhase},
    },
};
use http_body_util::{BodyExt, Full};
use platform::{
    actor_delegation::response_scope::{ResponseScope, ResponseScopeToken, RetainedScopeLayer},
    grpc_auth::{CallerRegistry, Workload},
    grpc_method_policy::guard_without_delegation,
};
use prost::Message;
use sqlx::PgPool;
use std::{
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tonic::{codegen::Bytes, Status};
use tower::{Layer, ServiceExt};
use uuid::Uuid;
async fn prepared(p: &PgPool) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar(
        "select count(*) from pg_prepared_xacts where database=current_database()",
    )
    .fetch_one(p)
    .await?)
}
fn key(scope: &OwnerGuardScope) -> anyhow::Result<ScopeKey> {
    let p = scope.producer_scope();
    Ok(ScopeKey {
        instance: p.instance_id.parse()?,
        scope: p.scope_id.parse()?,
        binding_digest: p
            .binding_digest
            .try_into()
            .map_err(|_| anyhow::anyhow!("digest"))?,
    })
}
async fn pending(p: &PgPool, k: &ScopeKey) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar("select count(*) from concert_product_child_intents where instance_id=$1 and scope_id=$2 and completed_at is null").bind(k.instance).bind(k.scope).fetch_one(p).await?)
}
async fn terminal(core: CanonicalRuntimeCore, cap: proto::ResponseDelivery) -> Result<(), Status> {
    let service = guard_without_delegation(
        &POLICY,
        proto::product_delivery_service_server::ProductDeliveryServiceServer::new(
            DeliveryService::new(core.delivery()),
        ),
        CallerRegistry::new()
            .register(Workload::Gateway, "gateway-fixture")
            .map_err(|_| Status::internal("caller"))?,
    )
    .map_err(|_| Status::internal("guard"))?;
    let bytes = cap.encode_to_vec();
    let mut frame = vec![0];
    frame.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    frame.extend(bytes);
    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/concert.product.v1.ProductDeliveryService/CompleteResponse")
        .header("content-type", "application/grpc")
        .header("te", "trailers")
        .header("x-service-caller", "gateway")
        .header("x-service-token", "gateway-fixture")
        .body(axum::body::Body::from(frame))
        .map_err(|_| Status::internal("request"))?;
    let response = service
        .oneshot(request)
        .await
        .map_err(|_| Status::unavailable("delivery transport"))?;
    let headers = response.headers().clone();
    let body = response
        .into_body()
        .collect()
        .await
        .map_err(|_| Status::unavailable("delivery body"))?;
    let trailers = body.trailers();
    let status = headers
        .get("grpc-status")
        .or_else(|| trailers.and_then(|h| h.get("grpc-status")))
        .and_then(|v| v.to_str().ok())
        .unwrap_or("0");
    if status != "0" {
        return Err(Status::unavailable("actual delivery refused"));
    }
    let bytes = body.to_bytes();
    if bytes.len() < 5 {
        return Err(Status::internal("delivery frame"));
    }
    let reply = proto::CompleteResponseResult::decode(&bytes[5..])
        .map_err(|_| Status::internal("delivery reply"))?;
    if !reply.completed {
        return Err(Status::unavailable("actual delivery incomplete"));
    }
    Ok(())
}
struct Terminal {
    core: CanonicalRuntimeCore,
    cap: proto::ResponseDelivery,
    calls: Arc<AtomicUsize>,
}
impl ResponseScope for Terminal {
    fn finish_response(
        self: Arc<Self>,
    ) -> Pin<Box<dyn Future<Output = Result<(), Status>> + Send + 'static>> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            terminal(self.core.clone(), self.cap.clone()).await
        })
    }
}
async fn consume(token: ResponseScopeToken) -> bool {
    let service = tower::service_fn(move |_: ()| {
        let token = token.clone();
        async move {
            let mut response =
                axum::http::Response::new(Full::new(Bytes::from_static(b"safe fixture body")));
            response.extensions_mut().insert(token);
            Ok::<_, std::convert::Infallible>(response)
        }
    });
    RetainedScopeLayer
        .layer(service)
        .oneshot(())
        .await
        .unwrap()
        .into_body()
        .collect()
        .await
        .is_ok()
}
async fn serialize(
    f: &fixture::Fixture,
    scope: Arc<OwnerGuardScope>,
) -> anyhow::Result<proto::ResponseDelivery> {
    let (cap, token) = f.owners.core.delivery().publish(scope).await?;
    anyhow::ensure!(
        consume(token).await,
        "actual inner body serialization failed"
    );
    Ok(cap)
}
async fn deliver(f: &fixture::Fixture, cap: proto::ResponseDelivery) -> bool {
    consume(ResponseScopeToken::new(Arc::new(Terminal {
        core: f.owners.core.clone(),
        cap,
        calls: Arc::new(AtomicUsize::new(0)),
    })))
    .await
}

#[tokio::test]
#[ignore = "requires owned PG16/mTLS CF01 qualifier and frozen committed source"]
async fn actual_factory_retains_profile_and_local_children_until_outer_body_eof(
) -> anyhow::Result<()> {
    let f = fixture::Fixture::new().await?;
    let (actor, family, subject) = f.seed().await?;
    let binding = capture::binding(actor, family, f.event, false).await?;
    let scope = f.owners.fence.acquire(binding).await?;
    let k = key(&scope)?;
    assert_eq!(k.instance, f.owners.core.ledger().instance());
    assert!(Arc::ptr_eq(
        &scope,
        &f.owners
            .core
            .registry()
            .lookup_for_settlement(&scope.producer_scope())?
    ));
    let admitted = scope.public_profile_subject(subject).await?;
    let author = f
        .owners
        .public_profile
        .read(&admitted)
        .await
        .map_err(|_| anyhow::anyhow!("actual public Profile read"))?;
    assert_eq!(author.user_id, subject);
    assert!(!String::from_utf8_lossy(&author.json).contains("private-url-canary"));
    assert_eq!(
        (
            prepared(&f.auth).await?,
            prepared(&f.friend).await?,
            prepared(&f.concert).await?,
            prepared(&f.profile).await?
        ),
        (1, 1, 1, 1)
    );
    assert_eq!(pending(&f.concert, &k).await?, 2);
    let cap = serialize(&f, scope.clone()).await?;
    assert_eq!(
        f.owners.core.ledger().resolve_scope(&k).await?,
        ScopePhase::Active
    );
    assert_eq!(
        pending(&f.concert, &k).await?,
        2,
        "inner serialization cannot settle children"
    );
    // Dropping an unpolled outer body cannot execute its terminal callback.
    let calls = Arc::new(AtomicUsize::new(0));
    let terminal = Arc::new(Terminal {
        core: f.owners.core.clone(),
        cap: cap.clone(),
        calls: calls.clone(),
    });
    let service = tower::service_fn(move |_: ()| {
        let t = terminal.clone();
        async move {
            let mut r = axum::http::Response::new(Full::new(Bytes::from_static(b"unread")));
            r.extensions_mut().insert(ResponseScopeToken::new(t));
            Ok::<_, std::convert::Infallible>(r)
        }
    });
    drop(RetainedScopeLayer.layer(service).oneshot(()).await?);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(pending(&f.concert, &k).await?, 2);
    assert!(deliver(&f, cap.clone()).await);
    assert!(deliver(&f, cap).await);
    assert_eq!(
        (
            prepared(&f.auth).await?,
            prepared(&f.friend).await?,
            prepared(&f.concert).await?,
            prepared(&f.profile).await?
        ),
        (0, 0, 0, 0)
    );
    assert_eq!(pending(&f.concert, &k).await?, 0);
    assert!(f.owners.core.ledger().delivery_completed(&k).await?);
    assert!(admitted.check().await.is_err());
    println!("CF01 actual factory Profile/Local/native owner retention, unread body negative, terminal body EOF and idempotent delivery PASS");
    Ok(())
}

#[tokio::test]
#[ignore = "requires owned PG16/mTLS CF01 qualifier and frozen committed source"]
async fn fresh_factory_recovers_original_notification_binding_without_target_policy(
) -> anyhow::Result<()> {
    let f = fixture::Fixture::new().await?;
    let (actor, family, _) = f.seed().await?;
    let scope = f
        .owners
        .fence
        .acquire(capture::binding(actor, family, f.event, true).await?)
        .await?;
    let k = key(&scope)?;
    use concert_service::notification_live::proto::{
        execute_request::Input, CountInput, InboxOperation,
    };
    let error = f
        .owners
        .notification
        .execute(
            scope.clone(),
            InboxOperation::UnreadCount,
            Input::Count(CountInput {}),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code(), tonic::Code::Unavailable);
    assert_eq!(
        error.message(),
        "Notification root target policy unavailable"
    );
    let reference = f
        .observation
        .reference
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| anyhow::anyhow!("actual Execute not observed"))?;
    assert_eq!(pending(&f.concert, &k).await?, 1);
    assert!(
        f.owners
            .notification
            .recover_notification_inbox(&reference)
            .await
            .is_err(),
        "ACTIVE root is not cleanup proof"
    );
    assert_eq!(f.observation.complete_calls.load(Ordering::SeqCst), 0);
    f.observation.deny_complete.store(true, Ordering::SeqCst);
    let cap = serialize(&f, scope.clone()).await?;
    assert!(
        !deliver(&f, cap.clone()).await,
        "unavailable completion must withhold delivery success"
    );
    assert_eq!(
        f.owners.core.ledger().resolve_scope(&k).await?,
        ScopePhase::RootFinished
    );
    assert_eq!(pending(&f.concert, &k).await?, 1);
    assert!(!f.owners.core.ledger().delivery_completed(&k).await?);
    let original_expiry = reference.query().expires_unix_micros;
    while chrono::Utc::now().timestamp_micros() <= original_expiry {
        tokio::time::sleep(Duration::from_millis(15)).await;
    }
    f.observation.deny_complete.store(false, Ordering::SeqCst);
    let reconstructed = f.compose(k.instance)?;
    assert!(!Arc::ptr_eq(
        &reconstructed.core.registry(),
        &f.owners.core.registry()
    ));
    assert!(
        reconstructed
            .core
            .registry()
            .lookup_for_settlement(&scope.producer_scope())
            .is_err(),
        "reconstruction cannot rebuild authority"
    );
    let before = f.observation.complete_calls.load(Ordering::SeqCst);
    let foreign = f.compose(Uuid::new_v4())?;
    assert!(foreign
        .notification
        .recover_notification_inbox(&reference)
        .await
        .is_err());
    for field in 0..5 {
        let mut bad = reference.clone();
        let ChildRecoveryReference::NotificationInbox { query, .. } = &mut bad else {
            unreachable!()
        };
        match field {
            0 => {
                query.actor = Uuid::new_v4();
                query.subject = query.actor
            }
            1 => query.event = Uuid::new_v4(),
            2 => query.family = Uuid::new_v4(),
            3 => query.request = Uuid::new_v4(),
            _ => query.binding_digest = [96; 32],
        }
        assert!(reconstructed
            .notification
            .recover_notification_inbox(&bad)
            .await
            .is_err());
    }
    assert_eq!(
        f.observation.complete_calls.load(Ordering::SeqCst),
        before,
        "foreign instance/reference must not dispatch Complete"
    );
    let mut worker = reconstructed
        .core
        .child_recovery_worker(ChildRecoverySettings::default())?;
    let counts = worker.tick().await?;
    assert_eq!(counts.completed, 1);
    assert_eq!(counts.pending, 0);
    assert_eq!(pending(&f.concert, &k).await?, 0);
    use sqlx::Row;
    let row = sqlx::query(
        "select binding_proto,sealed from concert_notification_authority_intents where child_id=$1",
    )
    .bind(reference.intent_id()?)
    .fetch_one(&f.notification)
    .await?;
    assert!(row.get::<bool, _>("sealed"));
    let bytes: Vec<u8> = row.get("binding_proto");
    let restored =
        notification_service::concert::transport::lease::GuardBinding::decode(bytes.as_slice())?;
    assert_eq!(restored.expires_unix_micros, original_expiry);
    assert_eq!(restored.actor_id, actor.to_string());
    assert_eq!(restored.session_family_id, family.to_string());
    assert_eq!(restored.operation, "getConcertNotificationUnreadCount");
    assert!(restored.subject_ids.is_empty());
    let n: i64 = sqlx::query_scalar("select count(*) from concert_notification_native_references")
        .fetch_one(&f.notification)
        .await?;
    assert_eq!(
        n, 0,
        "NoEffect is close-before-dispatch proof, not an inbox effect"
    );
    // The original live registry is still required for the exact delivery token.
    assert!(terminal(reconstructed.core.clone(), cap.clone())
        .await
        .is_err());
    assert!(deliver(&f, cap).await);
    assert!(f.owners.core.ledger().delivery_completed(&k).await?);
    let mut wrong_secret = reference.clone();
    let ChildRecoveryReference::NotificationInbox {
        completion_secret, ..
    } = &mut wrong_secret
    else {
        unreachable!()
    };
    *completion_secret = "97".repeat(32);
    assert!(reconstructed
        .notification
        .recover_notification_inbox(&wrong_secret)
        .await
        .is_err());
    assert_eq!(
        (
            prepared(&f.auth).await?,
            prepared(&f.friend).await?,
            prepared(&f.concert).await?,
            prepared(&f.notification).await?
        ),
        (0, 0, 0, 0)
    );
    println!("CF01 actual factory original expired Notification binding, real close-before-dispatch NoEffect, wrong instance/reference/secret refusal and retained delivery PASS; public target policy/inbox NOT_RUN");
    Ok(())
}
