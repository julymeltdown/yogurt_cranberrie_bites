//! CF02 actual Participant/typed-child shared composition. No production mounting.
#[path = "shared_participant_composition_support/mod.rs"]
mod support;
use concert_service::{
    adapters::out::relationship_authority::guarded::proto as authority,
    config::canonical_runtime::CanonicalRuntimeCore,
    participant::{self, proto as pp},
    product_rpc::{
        child_recovery::ChildRecoverySettings,
        delivery::{DeliveryService, POLICY},
        proto,
        scope_ledger::{ScopeKey, ScopePhase},
    },
};
use http_body_util::{BodyExt, Full};
use platform::{
    actor_delegation::{
        response_scope::{ResponseScope, ResponseScopeToken, RetainedScopeLayer},
        HEADER,
    },
    grpc_auth::{CallerRegistry, Workload},
    grpc_method_policy::{guard, guard_without_delegation},
};
use prost::Message;
use std::{
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use support::{environment, fixture};
use tonic::{codegen::Bytes, Status};
use tower::{Layer, ServiceExt};
use uuid::Uuid;
// Retained-body typed delivery helpers adapted from CF01 at f4349d5a.
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

async fn eof(core: CanonicalRuntimeCore, cap: proto::ResponseDelivery) -> bool {
    consume(ResponseScopeToken::new(Arc::new(Terminal {
        core,
        cap,
        calls: Arc::new(AtomicUsize::new(0)),
    })))
    .await
}
fn key(cap: &proto::ResponseDelivery) -> anyhow::Result<ScopeKey> {
    Ok(ScopeKey {
        instance: cap.instance_id.parse()?,
        scope: cap.scope_id.parse()?,
        binding_digest: cap
            .binding_digest
            .clone()
            .try_into()
            .map_err(|_| anyhow::anyhow!("binding digest"))?,
    })
}
fn producer(cap: &proto::ResponseDelivery) -> authority::ProducerScope {
    authority::ProducerScope {
        instance_id: cap.instance_id.clone(),
        scope_id: cap.scope_id.clone(),
        binding_digest: cap.binding_digest.clone(),
    }
}
async fn pending(pool: &sqlx::PgPool, k: &ScopeKey) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar("select count(*) from concert_product_child_intents where instance_id=$1 and scope_id=$2 and completed_at is null").bind(k.instance).bind(k.scope).fetch_one(pool).await?)
}
async fn invoke(
    m: &fixture::Mounted,
    f: &fixture::Fixture,
    patch: Option<serde_json::Value>,
) -> anyhow::Result<(pp::ParticipantResponse, serde_json::Value)> {
    let now = chrono::Utc::now().timestamp_micros();
    let (method, verb) = if patch.is_some() {
        ("UpdatePreferences", "PATCH")
    } else {
        ("GetParticipant", "GET")
    };
    let path = format!("/concert.participant.v1.ConcertParticipantService/{method}");
    let query = pp::ParticipantRequest {
        user_id: f.actor.to_string(),
        event_id: f.event.to_string(),
        session_family_id: f.family.to_string(),
        request_id: Uuid::new_v4().to_string(),
        http_method: verb.into(),
        http_target: format!("/api/concert/events/{}/me", f.event),
        request_json: patch
            .map(|v| serde_json::to_vec(&v))
            .transpose()?
            .unwrap_or_default(),
        release_digest: f.product.source_digest().into(),
        authority_expires_unix_micros: now + 1_950_000,
        auth_observed_at_unix_micros: now,
        profile_observed_at_unix_micros: now,
    };
    let bytes = query.encode_to_vec();
    let mut framed = vec![0];
    framed.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    framed.extend(bytes);
    let signer =
        platform::actor_delegation::ActorDelegationSigner::from_seed(Workload::Gateway, &[74; 32])?;
    let proof = signer.issue(
        Workload::Concert,
        &path,
        f.actor,
        None,
        &framed,
        chrono::Utc::now().timestamp(),
    );
    let service = guard(
        &participant::POLICY,
        m.service.clone().server(),
        m.callers.clone(),
        m.gateway.clone(),
    )?;
    let request = axum::http::Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/grpc")
        .header("te", "trailers")
        .header("x-service-caller", "gateway")
        .header("x-service-token", "gateway-fixture")
        .header(HEADER, proof)
        .body(axum::body::Body::from(framed))?;
    let response = RetainedScopeLayer.layer(service).oneshot(request).await?;
    let headers = response.headers().clone();
    let body = response
        .into_body()
        .collect()
        .await
        .map_err(|_| anyhow::anyhow!("real Participant retained body"))?;
    let status = headers
        .get("grpc-status")
        .or_else(|| body.trailers().and_then(|t| t.get("grpc-status")))
        .map(|x| x.to_str().unwrap_or("invalid"))
        .unwrap_or("0");
    anyhow::ensure!(status == "0", "actual Participant RPC failed with {status}");
    let data = body.to_bytes();
    anyhow::ensure!(data.len() >= 5, "actual Participant frame");
    let result = pp::ParticipantResponse::decode(&data[5..])?;
    anyhow::ensure!(
        result.failure.is_none(),
        "actual Participant producer returned a failure envelope"
    );
    let document = serde_json::from_slice(&result.response_json)?;
    Ok((result, document))
}
async fn negative_composition(f: &fixture::Fixture, instance: Uuid) -> anyhow::Result<()> {
    let digest = f.product.source_digest();
    assert!(
        fixture::configured(instance, &"ab".repeat(32))?
            .compose_shared(f.concert.clone(), f.product.clone(), fixture::additional()?)
            .is_err(),
        "release origin mismatch"
    );
    let mut wrong = fixture::additional()?;
    wrong.profile_endpoint = "https://other-profile.invalid:55555".into();
    assert!(
        fixture::configured(instance, digest)?
            .compose_shared(f.concert.clone(), f.product.clone(), wrong)
            .is_err(),
        "SELF/public owner mismatch"
    );
    for bad in [
        "http://notification.invalid:55555",
        "https://notification.invalid",
        "https://user:secret@notification.invalid:55555",
        "https://notification.invalid:55555/private",
        "https://notification.invalid:55555?token=private",
    ] {
        let mut wrong = fixture::additional()?;
        wrong.notification_endpoint = bad.into();
        let error = fixture::configured(instance, digest)?
            .compose_shared(f.concert.clone(), f.product.clone(), wrong)
            .err()
            .ok_or_else(|| anyhow::anyhow!("invalid endpoint accepted"))?
            .to_string();
        assert!(!error.contains("user:secret") && !error.contains("token=private"));
    }
    // Pure lazy pools are never connected. Refuse foreign/closed origins before
    // any live owner construction or transport dispatch.
    let foreign = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@127.0.0.1:65534/foreign_store")?;
    assert!(fixture::configured(instance, digest)?
        .compose_shared(foreign.clone(), f.product.clone(), fixture::additional()?)
        .is_err());
    foreign.close().await;
    let closed = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@127.0.0.1:65534/mcr_concert_cf02")?;
    closed.close().await;
    assert!(fixture::configured(instance, digest)?
        .compose_shared(closed, f.product.clone(), fixture::additional()?)
        .is_err());
    let gateway = support::owners::gateway(Workload::Concert)?;
    let relationship = support::owners::signer()?;
    let self_key = fixture::self_signer()?;
    for decision_key in [
        platform::actor_delegation::ActorDelegationSigner::from_seed(Workload::Gateway, &[74; 32])?
            .public_key(),
        relationship.public_key(),
        self_key.public_key(),
        platform::actor_delegation::ActorDelegationSigner::local_parity(Workload::Profile)
            .public_key(),
    ] {
        assert!(concert_service::config::participant_runtime::ParticipantRuntimeSettings::from_validated(fixture::raw_settings(instance,digest,None)?,environment::tls(Workload::Concert)?,gateway.clone(),relationship.clone(),self_key.clone(),&[decision_key]).is_err(),"dedicated decision role must be bound to this Gateway and outbound key set");
    }
    assert_eq!(f.prepared().await?, vec![0, 0, 0, 0]);
    Ok(())
}
#[tokio::test]
#[ignore = "requires frozen CF02 executable and exact owned PG16/mTLS controller"]
async fn shared_participant_core_and_real_profile_native() -> anyhow::Result<()> {
    let f = fixture::Fixture::new().await?;
    let instance = Uuid::new_v4();
    negative_composition(&f, instance).await?;
    let shared = fixture::configured(instance, f.product.source_digest())?.compose_shared(
        f.concert.clone(),
        f.product.clone(),
        fixture::additional()?,
    )?;
    assert!(Arc::ptr_eq(
        &shared.participant.canonical.registry(),
        &shared.owners.core.registry()
    ));
    assert!(Arc::ptr_eq(&shared.participant.fence, &shared.owners.fence));
    assert_eq!(shared.participant.canonical.ledger().instance(), instance);
    assert_eq!(shared.owners.core.ledger().instance(), instance);
    let product = shared.owners;
    let mounted = fixture::Mounted::new(shared.participant).await?;
    assert!(Arc::ptr_eq(&mounted.fence, &product.fence));
    let (read, document) = invoke(&mounted, &f, None).await?;
    assert_eq!(document["profile"]["display_name"], "CF02 actual canonical");
    let cap = read
        .delivery
        .ok_or_else(|| anyhow::anyhow!("actual Participant delivery"))?;
    let k = key(&cap)?;
    let original = mounted
        .core
        .registry()
        .lookup_for_settlement(&producer(&cap))?;
    let through_product = product
        .core
        .registry()
        .lookup_for_settlement(&producer(&cap))?;
    assert!(
        Arc::ptr_eq(&original, &through_product),
        "actual Participant scope must be the exact Product registry scope"
    );
    // Test-owned additional callback attaches a real typed public child to that
    // same actual Participant scope. No second scope/core/fence is constructed.
    let subject = original.public_profile_subject(f.subject).await?;
    let author = product
        .public_profile
        .read(&subject)
        .await
        .map_err(|_| anyhow::anyhow!("actual typed Profile child"))?;
    assert_eq!(author.user_id, f.subject);
    assert_eq!(f.prepared().await?, vec![1, 1, 1, 1]);
    assert_eq!(pending(&f.concert, &k).await?, 2);
    assert_eq!(
        product.core.ledger().resolve_scope(&k).await?,
        ScopePhase::Active
    );
    assert!(
        !mounted.core.ledger().delivery_completed(&k).await?,
        "inner serialization is not outer terminal proof"
    );
    // Actual negative Profile completion: native owner/Local work settles in
    // insertion order; the remaining public Profile must use common recovery.
    f.deny_complete.store(true, Ordering::SeqCst);
    assert!(!eof(product.core.clone(), cap.clone()).await);
    assert_eq!(
        mounted.core.ledger().resolve_scope(&k).await?,
        ScopePhase::RootFinished
    );
    assert_eq!(f.prepared().await?, vec![0, 0, 0, 1]);
    assert_eq!(pending(&f.concert, &k).await?, 1);
    let ciphertext:Vec<Vec<u8>>=sqlx::query_scalar("select sealed_reference from concert_product_child_intents where instance_id=$1 and scope_id=$2 order by intent_id").bind(k.instance).bind(k.scope).fetch_all(&f.concert).await?;
    while chrono::Utc::now().timestamp_micros() <= original.binding().deadline().expires_micros() {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(product
        .core
        .registry()
        .lookup_root_read(&original.root_read_view())
        .is_err());
    f.deny_complete.store(false, Ordering::SeqCst);
    let missing = fixture::configured_with_keys(
        instance,
        f.product.source_digest(),
        Some(serde_json::json!([{"id":"cf02-other","secret_hex":"63".repeat(32)}])),
    )?
    .compose_shared(f.concert.clone(), f.product.clone(), fixture::additional()?)?;
    let mut cannot_decrypt = missing
        .owners
        .core
        .child_recovery_worker(ChildRecoverySettings::default())?;
    let missing_counts = cannot_decrypt.tick().await?;
    assert_eq!(missing_counts.attempted, 1);
    assert_eq!(missing_counts.completed, 0);
    assert_eq!(pending(&f.concert, &k).await?, 1);
    assert_eq!(f.prepared().await?, vec![0, 0, 0, 1]);
    f.deny_complete.store(false, Ordering::SeqCst);
    let mut recovery = mounted
        .core
        .child_recovery_worker(ChildRecoverySettings::default())?;
    assert_eq!(
        recovery.tick().await?.completed,
        1,
        "shared mode must use actual CF01 composite, never ParticipantOnlyChildren"
    );
    assert_eq!(recovery.tick().await?.completed, 0);
    assert_eq!(f.prepared().await?, vec![0, 0, 0, 0]);
    assert_eq!(pending(&f.concert, &k).await?, 0);
    let after:Vec<Vec<u8>>=sqlx::query_scalar("select sealed_reference from concert_product_child_intents where instance_id=$1 and scope_id=$2 order by intent_id").bind(k.instance).bind(k.scope).fetch_all(&f.concert).await?;
    assert_eq!(after, ciphertext);
    assert!(eof(product.core.clone(), cap.clone()).await);
    assert!(eof(mounted.core.clone(), cap).await);
    assert!(mounted.core.ledger().delivery_completed(&k).await?);
    // Actual coordinated Profile mutation exercises dedicated SELF and decision
    // keys, immutable journal/native witness and the same closed union.
    let (patch,document)=invoke(&mounted,&f,Some(serde_json::json!({"expected_revision":1,"display_name":"CF02 shared canonical","bio":"CF02 committed through real Profile"}))).await?;
    assert_eq!(document["revision"], 2);
    assert_eq!(document["profile"]["display_name"], "CF02 shared canonical");
    let patch_cap = patch
        .delivery
        .ok_or_else(|| anyhow::anyhow!("actual PATCH delivery"))?;
    assert!(eof(product.core.clone(), patch_cap).await);
    let nickname: String = sqlx::query_scalar("select nickname from profiles where user_id=$1")
        .bind(f.actor)
        .fetch_one(&f.profile)
        .await?;
    assert_eq!(nickname, "CF02 shared canonical");
    let decisions: Vec<String> =
        sqlx::query_scalar("select decision from concert_participant_profile_operations")
            .fetch_all(&f.concert)
            .await?;
    assert_eq!(decisions, vec!["COMMIT".to_string()]);
    assert_eq!(f.prepared().await?, vec![0, 0, 0, 0]);
    mounted.stop().await?;
    // Existing standalone composition still has its original dedicated core and
    // Participant native resolver. No shared factory is needed for C03 alone.
    let standalone = fixture::configured(Uuid::new_v4(), f.product.source_digest())?
        .compose(f.concert.clone(), f.product.clone())?;
    let single = fixture::Mounted::new(standalone).await?;
    let (read, document) = invoke(&single, &f, None).await?;
    assert_eq!(document["profile"]["display_name"], "CF02 shared canonical");
    assert!(
        eof(
            single.core.clone(),
            read.delivery
                .ok_or_else(|| anyhow::anyhow!("standalone delivery"))?
        )
        .await
    );
    assert_eq!(f.prepared().await?, vec![0, 0, 0, 0]);
    single.stop().await?;
    println!("CF02 actual shared Participant/core/typed Profile/native recovery, closed union coordinated PATCH, original EOF and standalone C03 PASS");
    Ok(())
}
