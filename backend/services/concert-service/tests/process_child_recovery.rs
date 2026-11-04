//! CR01: actual Root process death and durable native child recovery.
//! Source provenance: process_child_recovery_support/provenance.json.
//! Compilation/runtime status is recorded separately in docs/mcr/backend/.
#[path = "process_child_recovery_support/capture.rs"]
mod capture;
#[path = "process_child_recovery_support/environment.rs"]
mod environment;
#[path = "process_child_recovery_support/fault.rs"]
mod fault;
#[path = "process_child_recovery_support/fixture.rs"]
mod fixture;
#[path = "process_child_recovery_support/owners.rs"]
mod owners;
#[path = "process_child_recovery_support/release.rs"]
mod release;

use concert_service::{
    adapters::out::relationship_authority::guarded::{proto as authority, OwnerGuardScope},
    product_rpc::{
        self,
        child_recovery::{ChildRecoverySettings, ChildRecoveryWorker},
        delivery::{DeliveryPublisher, DeliveryService, POLICY as DELIVERY_POLICY},
        proto,
        scope_ledger::{ScopeKey, ScopePhase},
        scoped::{ProductRepresentation, ScopedProductOwner, ScopedProductProducer},
        ProductRpcService, VerifiedProduct,
    },
};
use http_body_util::{BodyExt, Full};
use platform::{
    actor_delegation::{
        response_scope::{ResponseScope, ResponseScopeToken, RetainedScopeLayer},
        ActorDelegationVerifier, HEADER,
    },
    grpc_auth::{CallerRegistry, Workload},
    grpc_method_policy::{guard, guard_without_delegation},
};
use prost::Message;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::{
    io::Write,
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::PathBuf,
    sync::{
        atomic::{AtomicI32, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tonic::{codegen::Bytes, Status};
use tower::{Layer, ServiceExt};
use uuid::Uuid;

const TEST_MODE: &str = "MCR_PROCESS_MODE";
const DISPATCH: &str = "/concert.product.v1.ProductService/Dispatch";
const COMPLETE: &str = "/concert.product.v1.ProductDeliveryService/CompleteResponse";

// No Debug: this file contains private capabilities and must never reach reports/stdout.
#[derive(Serialize, Deserialize)]
struct Handoff {
    run: Uuid,
    old_pid: u32,
    instance: Uuid,
    subject: Uuid,
    root_view: Vec<u8>,
    delivery: Vec<u8>,
    prepared: Vec<Prepared>,
}
#[derive(Serialize, Deserialize)]
struct Prepared {
    owner: String,
    gid: String,
    xid: String,
}
fn state_path(run: Uuid) -> anyhow::Result<PathBuf> {
    Ok(environment::root()?.join(format!("process-cr01-{}.json", run.simple())))
}
fn save(state: &Handoff) -> anyhow::Result<()> {
    let path = state_path(state.run)?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(&serde_json::to_vec(state)?)?;
    file.sync_all()?;
    Ok(())
}
fn load(run: Uuid) -> anyhow::Result<Handoff> {
    let path = state_path(run)?;
    let meta = std::fs::symlink_metadata(&path)?;
    let root = std::fs::metadata(environment::root()?)?;
    anyhow::ensure!(
        meta.is_file()
            && !meta.file_type().is_symlink()
            && meta.uid() == root.uid()
            && meta.permissions().mode() & 0o777 == 0o600,
        "private owned handoff required"
    );
    let state: Handoff = serde_json::from_slice(&std::fs::read(path)?)?;
    anyhow::ensure!(state.run == run, "run identity mismatch");
    Ok(state)
}
fn frame(message: &impl Message) -> Vec<u8> {
    let bytes = message.encode_to_vec();
    let mut frame = vec![0];
    frame.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    frame.extend(bytes);
    frame
}
fn decode<M: Message + Default>(bytes: &[u8]) -> anyhow::Result<M> {
    anyhow::ensure!(
        bytes.len() >= 5 && bytes[0] == 0,
        "one uncompressed gRPC frame required"
    );
    let len = u32::from_be_bytes(bytes[1..5].try_into()?) as usize;
    anyhow::ensure!(len + 5 == bytes.len(), "exactly one gRPC frame required");
    Ok(M::decode(&bytes[5..])?)
}
fn request(method: &str, body: Vec<u8>) -> anyhow::Result<axum::http::Request<axum::body::Body>> {
    Ok(axum::http::Request::builder()
        .method("POST")
        .uri(method)
        .header("content-type", "application/grpc")
        .header("te", "trailers")
        .header("x-service-caller", "gateway")
        .header("x-service-token", "gateway-fixture")
        .header("grpc-timeout", "1800000u")
        .body(axum::body::Body::from(body))?)
}
fn grpc_code(
    headers: &axum::http::HeaderMap,
    trailers: Option<&axum::http::HeaderMap>,
) -> anyhow::Result<i32> {
    Ok(trailers
        .and_then(|h| h.get("grpc-status"))
        .or_else(|| headers.get("grpc-status"))
        .ok_or_else(|| anyhow::anyhow!("missing explicit gRPC terminal status"))?
        .to_str()?
        .parse()?)
}
async fn complete(
    publisher: DeliveryPublisher,
    capability: proto::ResponseDelivery,
) -> anyhow::Result<i32> {
    let service = guard_without_delegation(
        &DELIVERY_POLICY,
        proto::product_delivery_service_server::ProductDeliveryServiceServer::new(
            DeliveryService::new(publisher),
        ),
        CallerRegistry::new().register(Workload::Gateway, "gateway-fixture")?,
    )?;
    let service = platform::call_deadline::CallDeadlineLayer.layer(service);
    let response = service
        .oneshot(request(COMPLETE, frame(&capability))?)
        .await?;
    let headers = response.headers().clone();
    let body = response.into_body().collect().await?;
    grpc_code(&headers, body.trailers())
}
struct ReadProducer {
    reader: Arc<concert_service::adapters::out::public_profile::PublicProfileReader>,
    subject: Uuid,
    captured: Mutex<Option<Arc<OwnerGuardScope>>>,
}
#[async_trait::async_trait]
impl ScopedProductProducer for ReadProducer {
    async fn dispatch(
        &self,
        _: &VerifiedProduct,
        scope: Arc<OwnerGuardScope>,
        _: proto::ProductRequest,
    ) -> Result<ProductRepresentation, Status> {
        let subject = scope.public_profile_subject(self.subject).await?;
        let author = self
            .reader
            .read(&subject)
            .await
            .map_err(|_| Status::unavailable("actual Profile read required"))?;
        // Assert actual native content privately, then exercise the production
        // closed negative Product projection rather than inventing a Feed DTO.
        if author.user_id != self.subject {
            return Err(Status::internal("wrong actual author"));
        }
        let document: serde_json::Value = serde_json::from_slice(&author.json)
            .map_err(|_| Status::internal("actual author JSON required"))?;
        let expected = self.subject.to_string();
        if document["user_id"].as_str() != Some(expected.as_str())
            || String::from_utf8_lossy(&author.json).contains("private-url-canary")
        {
            return Err(Status::internal(
                "actual author identity/privacy assertion failed",
            ));
        }
        *self
            .captured
            .lock()
            .map_err(|_| Status::internal("scope capture"))? = Some(scope);
        Err(Status::permission_denied(
            "private CR01 producer diagnostic",
        ))
    }
}
// In-process Gateway terminal-body fixture: actual EOF invokes the real guarded
// DeliveryService. External Gateway network HTTP remains explicitly unqualified.
struct Terminal {
    publisher: DeliveryPublisher,
    capability: proto::ResponseDelivery,
    observed: Arc<AtomicI32>,
}
impl ResponseScope for Terminal {
    fn finish_response(self: Arc<Self>) -> futures::future::BoxFuture<'static, Result<(), Status>> {
        Box::pin(async move {
            let result = complete(self.publisher.clone(), self.capability.clone()).await;
            if let Ok(code) = &result {
                self.observed.store(*code, Ordering::SeqCst);
            }
            match result {
                Ok(14) => Err(Status::unavailable("expected negative Auth Complete")),
                _ => Err(Status::internal("terminal callback must fail Unavailable")),
            }
        })
    }
}
fn scope_key(value: &authority::ProducerScope) -> anyhow::Result<ScopeKey> {
    Ok(ScopeKey {
        instance: value.instance_id.parse()?,
        scope: value.scope_id.parse()?,
        binding_digest: value.binding_digest.as_slice().try_into()?,
    })
}
async fn prepared(pool: &PgPool) -> anyhow::Result<Vec<(String, String)>> {
    Ok(sqlx::query_as("select gid,transaction::text from pg_prepared_xacts where database=current_database() and owner=current_user order by gid")
        .fetch_all(pool).await?)
}
// Native writer probes use real UPDATE statements, a 25ms lock timeout and
// explicit rollback on BOTH success and error. They never commit or alter a receipt.
async fn local_writer(f: &fixture::Fixture, subject: Uuid, blocked: bool) -> anyhow::Result<()> {
    let mut tx = f.concert.begin().await?;
    let result=async {
        sqlx::query("set local lock_timeout='25ms'").execute(&mut *tx).await?;
        sqlx::query("update public.concert_avatar_settings set preset_id='photo01_front_02',revision=revision+1 where event_id=$1 and user_id=$2")
            .bind(f.event).bind(subject).execute(&mut *tx).await
    }.await;
    let rollback = tx.rollback().await;
    let code = result
        .as_ref()
        .err()
        .and_then(|e| e.as_database_error())
        .and_then(|e| e.code())
        .map(|v| v.into_owned());
    rollback?;
    if blocked {
        anyhow::ensure!(
            code.as_deref() == Some("55P03"),
            "Local writer must fail with SQLSTATE 55P03"
        );
    } else {
        anyhow::ensure!(
            result?.rows_affected() == 1,
            "Local writer did not resume after actual ACK"
        );
    }
    Ok(())
}
async fn profile_writer(f: &fixture::Fixture, subject: Uuid, blocked: bool) -> anyhow::Result<()> {
    let mut tx = f.profile.begin().await?;
    let result = async {
        sqlx::query("set local lock_timeout='25ms'")
            .execute(&mut *tx)
            .await?;
        sqlx::query("update public.profiles set account_visibility='PRIVATE' where user_id=$1")
            .bind(subject)
            .execute(&mut *tx)
            .await
    }
    .await;
    let rollback = tx.rollback().await;
    let code = result
        .as_ref()
        .err()
        .and_then(|e| e.as_database_error())
        .and_then(|e| e.code())
        .map(|v| v.into_owned());
    rollback?;
    if blocked {
        anyhow::ensure!(
            code.as_deref() == Some("55P03"),
            "Profile writer must fail with SQLSTATE 55P03"
        );
    } else {
        anyhow::ensure!(
            result?.rows_affected() == 1,
            "Profile writer did not resume after actual ACK"
        );
    }
    Ok(())
}
async fn writers(f: &fixture::Fixture, subject: Uuid, blocked: bool) -> anyhow::Result<()> {
    local_writer(f, subject, blocked).await?;
    profile_writer(f, subject, blocked).await
}
// model::json stores response_scope as a JSON STRING containing Base64 protobuf,
// not an object. SQL decodes that string; Rust compares the full ProducerScope.
fn owner_scope_matches(encoded: &[u8], key: &ScopeKey) -> anyhow::Result<bool> {
    let scope = authority::ProducerScope::decode(encoded)?;
    Ok(scope.instance_id == key.instance.to_string()
        && scope.scope_id == key.scope.to_string()
        && scope.binding_digest.as_slice() == key.binding_digest)
}
// Actual Enroll updates subjects on the SAME owner lease, not one PREPARE per subject.
async fn enrolled_owner(
    pool: &PgPool,
    key: &ScopeKey,
    actor: Uuid,
    subject: Uuid,
) -> anyhow::Result<()> {
    let rows:Vec<(String,Vec<Uuid>,Vec<u8>)>=sqlx::query_as(
        "select prepared_gid,enrolled_subjects,decode(response_scope#>>'{}','base64') from public.concert_owner_guard_leases where binding_digest=$1 and state='ACTIVE'")
        .bind(key.binding_digest.as_slice()).fetch_all(pool).await?;
    let mut exact = Vec::new();
    for (gid, subjects, encoded) in rows {
        if owner_scope_matches(&encoded, key)? {
            exact.push((gid, subjects));
        }
    }
    anyhow::ensure!(exact.len() == 1, "exact actual owner lease required");
    anyhow::ensure!(
        exact[0].1.len() == 2 && exact[0].1.contains(&actor) && exact[0].1.contains(&subject),
        "actual owner did not enroll actor and subject on its lease"
    );
    let native = prepared(pool).await?;
    anyhow::ensure!(
        native.len() == 1 && native[0].0 == exact[0].0,
        "actual owner lease PREPARE mismatch"
    );
    Ok(())
}
async fn completed_owner(pool: &PgPool, key: &ScopeKey, expected_gid: &str) -> anyhow::Result<()> {
    let rows:Vec<(String,Vec<u8>,Vec<u8>)>=sqlx::query_as(
        "select prepared_gid,decode(response_scope#>>'{}','base64'),decode(scope_settlement#>>'{}','base64') from public.concert_owner_guard_leases where binding_digest=$1 and state='COMPLETED' and completed_at is not null and scope_settlement is not null")
        .bind(key.binding_digest.as_slice()).fetch_all(pool).await?;
    let mut exact = Vec::new();
    for (gid, encoded, settlement) in rows {
        if owner_scope_matches(&encoded, key)? {
            let proof = authority::ResolveScopeResponse::decode(settlement.as_slice())?;
            let proof_scope = proof
                .scope
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("owner root proof missing scope"))?;
            anyhow::ensure!(
                proof.sealed_and_finished
                    && owner_scope_matches(&proof_scope.encode_to_vec(), key)?,
                "actual owner completed receipt lacks exact RootFinished proof"
            );
            exact.push(gid);
        }
    }
    anyhow::ensure!(
        exact.len() == 1 && exact[0] == expected_gid,
        "actual original owner lease lacks durable completed/root-proof receipt"
    );
    Ok(())
}
async fn wait_past_saved_expiry(view: &authority::RootReadView) -> anyhow::Result<()> {
    // No rewritten view, deadline, authority or content grant. This is only the
    // time selection threshold of actual owner recover(), never a release proof.
    let expires = view
        .binding
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("saved binding missing"))?
        .expires_unix_micros;
    let after = expires
        .checked_add(10_000)
        .ok_or_else(|| anyhow::anyhow!("saved expiry overflow"))?;
    let remaining = after
        .saturating_sub(chrono::Utc::now().timestamp_micros())
        .max(0);
    anyhow::ensure!(
        remaining <= 1_800_000,
        "wait exceeds original 1.8-second request bound"
    );
    if remaining > 0 {
        tokio::time::sleep(Duration::from_micros(remaining as u64)).await;
    }
    anyhow::ensure!(
        chrono::Utc::now().timestamp_micros() >= after,
        "saved original expiry not passed"
    );
    Ok(())
}
async fn pending(f: &fixture::Fixture, key: &ScopeKey) -> anyhow::Result<(i64, i64)> {
    Ok(sqlx::query_as("select count(*) filter(where completed_at is null),count(*) filter(where completed_at is not null) from concert_product_child_intents where instance_id=$1 and scope_id=$2 and binding_digest=$3")
        .bind(key.instance).bind(key.scope).bind(key.binding_digest.as_slice()).fetch_one(&f.concert).await?)
}
async fn invariant(f: &fixture::Fixture, key: &ScopeKey) -> anyhow::Result<()> {
    anyhow::ensure!(
        f.ledger.resolve_scope(key).await? == ScopePhase::RootFinished,
        "true RootFinished required"
    );
    anyhow::ensure!(
        !f.ledger.delivery_completed(key).await?,
        "must not fabricate DeliveryCompleted"
    );
    let tx:i64=sqlx::query_scalar("select count(*) from concert_product_owner_transactions where instance_id=$1 and scope_id=$2")
        .bind(key.instance).bind(key.scope).fetch_one(&f.concert).await?;
    anyhow::ensure!(tx == 0, "CR01 requires no producer transaction");
    Ok(())
}
fn line(value: &str) -> anyhow::Result<()> {
    // Leading newline isolates the fixed protocol line from libtest progress text.
    println!("\n{value}");
    std::io::stdout().flush()?;
    Ok(())
}
async fn old(run: Uuid, instance: Uuid) -> anyhow::Result<()> {
    let f = fixture::Fixture::new(instance, true).await?;
    for pool in [&f.auth, &f.friend, &f.concert, &f.profile] {
        anyhow::ensure!(
            prepared(pool).await?.is_empty(),
            "clean isolated databases required"
        );
    }
    let (actor, family, subject) = f.seed().await?;
    let producer = Arc::new(ReadProducer {
        reader: f.reader.clone(),
        subject,
        captured: Mutex::new(None),
    });
    let publisher = DeliveryPublisher::new(f.ledger.clone(), f.registry.clone());
    let owner = ScopedProductOwner::new(
        Arc::new(f.fence.clone()),
        publisher.clone(),
        producer.clone(),
    );
    let signer = capture::gateway()?;
    let service = guard(
        &product_rpc::POLICY,
        proto::product_service_server::ProductServiceServer::new(ProductRpcService::new(Arc::new(
            owner,
        ))),
        CallerRegistry::new().register(Workload::Gateway, "gateway-fixture")?,
        ActorDelegationVerifier::new(Workload::Concert, &[signer.public_key()])?,
    )?;
    let service =
        platform::call_deadline::CallDeadlineLayer.layer(RetainedScopeLayer.layer(service));
    let now = chrono::Utc::now().timestamp_micros();
    let message = proto::ProductRequest {
        user_id: actor.to_string(),
        event_id: f.event.to_string(),
        session_family_id: family.to_string(),
        request_id: Uuid::new_v4().to_string(),
        http_method: "GET".into(),
        http_target: format!("/api/concert/events/{}/feed", f.event),
        http_body: Vec::new(),
        content_type: String::new(),
        idempotency_key: String::new(),
        authority_expires_unix_micros: now + 1_800_000,
        auth_observed_at_unix_micros: now,
        profile_observed_at_unix_micros: now,
        access_expires_unix_micros: now + 60_000_000,
        ..Default::default()
    };
    let bytes = frame(&message);
    let proof = signer.issue(
        Workload::Concert,
        DISPATCH,
        actor,
        None,
        &bytes,
        chrono::Utc::now().timestamp(),
    );
    let mut req = request(DISPATCH, bytes)?;
    req.headers_mut().insert(HEADER, proof.parse()?);
    let response = service.oneshot(req).await?;
    let headers = response.headers().clone();
    // Actual Concert gRPC body EOF marks serialization through production Serialized.
    let body = response.into_body().collect().await?;
    anyhow::ensure!(
        grpc_code(&headers, body.trailers())? == 0,
        "guarded Product RPC denied"
    );
    let output: proto::ProductResponse = decode(&body.to_bytes())?;
    anyhow::ensure!(
        output.status == 403 && output.content_type == "application/json",
        "actual closed produced refusal required"
    );
    let document: serde_json::Value = serde_json::from_slice(&output.body)?;
    anyhow::ensure!(
        document["details"]["reason"] == "PERMISSION_DENIED",
        "actual negative Product contract required"
    );
    anyhow::ensure!(
        !String::from_utf8_lossy(&output.body).contains("private CR01 producer diagnostic")
            && !String::from_utf8_lossy(&output.body).contains("private-url-canary")
            && document.get("user_id").is_none(),
        "produced error disclosed private producer content"
    );
    let capability = output
        .delivery
        .ok_or_else(|| anyhow::anyhow!("delivery missing"))?;
    let scope = producer
        .captured
        .lock()
        .unwrap()
        .take()
        .ok_or_else(|| anyhow::anyhow!("scope missing"))?;
    let view = scope.root_read_view();
    let key = scope_key(&scope.producer_scope())?;
    anyhow::ensure!(
        f.ledger.resolve_scope(&key).await? == ScopePhase::Active,
        "EOF prematurely finished root"
    );
    let observed = Arc::new(AtomicI32::new(-1));
    let terminal = Arc::new(Terminal {
        publisher: publisher.clone(),
        capability: capability.clone(),
        observed: observed.clone(),
    });
    let outer = tower::service_fn(move |_: ()| {
        let token = ResponseScopeToken::new(terminal.clone());
        let bytes = output.body.clone();
        async move {
            let mut response = axum::http::Response::new(Full::new(Bytes::from(bytes)));
            response.extensions_mut().insert(token);
            Ok::<_, std::convert::Infallible>(response)
        }
    });
    let response = RetainedScopeLayer.layer(outer).oneshot(()).await?;
    anyhow::ensure!(
        response.into_body().collect().await.is_err(),
        "negative terminal fault missing"
    );
    anyhow::ensure!(
        observed.load(Ordering::SeqCst) == 14,
        "terminal callback did not return Unavailable"
    );
    invariant(&f, &key).await?;
    anyhow::ensure!(
        pending(&f, &key).await? == (2, 0),
        "both real children must remain pending"
    );
    let kinds:Vec<String>=sqlx::query_scalar("select kind from concert_product_child_intents where instance_id=$1 and scope_id=$2 order by kind")
        .bind(instance).bind(key.scope).fetch_all(&f.concert).await?;
    anyhow::ensure!(
        kinds == vec!["LOCAL_PUBLIC".to_string(), "PROFILE_PUBLIC".to_string()],
        "wrong real child kinds"
    );
    let receipt: i64 = sqlx::query_scalar(
        "select count(*) from concert_public_profile_children where state='REGISTERED'",
    )
    .fetch_one(&f.profile)
    .await?;
    anyhow::ensure!(receipt == 1, "actual Profile native receipt missing");
    enrolled_owner(&f.auth, &key, actor, subject).await?;
    enrolled_owner(&f.friend, &key, actor, subject).await?;
    writers(&f, subject, true).await?; // Before exact old-process kill.
    let mut saved = Vec::new();
    for (name, pool) in [
        ("auth", &f.auth),
        ("friend", &f.friend),
        ("concert", &f.concert),
        ("profile", &f.profile),
    ] {
        let rows = prepared(pool).await?;
        anyhow::ensure!(rows.len() == 1, "exact real prepared owner/child required");
        for (gid, xid) in rows {
            saved.push(Prepared {
                owner: name.into(),
                gid,
                xid,
            });
        }
    }
    save(&Handoff {
        run,
        old_pid: std::process::id(),
        instance,
        subject,
        root_view: view.encode_to_vec(),
        delivery: capability.encode_to_vec(),
        prepared: saved,
    })?;
    line("MCR_CR01_OLD_READY")?;
    // Keep f, its actual Arc scopes, reader and servers alive until controller SIGKILL.
    std::future::pending::<()>().await;
    drop(scope);
    drop(f);
    Ok(())
}

async fn old_content_denied(
    f: &fixture::Fixture,
    view: &authority::RootReadView,
    subject: Uuid,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        f.registry.lookup_root_read(view).is_err(),
        "lost root view resurrected"
    );
    use concert_service::product_rpc::root_read::{ConcertRootReadService, POLICY};
    use platform::actor_delegation::{
        concert_read::PostConcertRootReadVerifier, ActorDelegationSigner,
    };
    let signer = ActorDelegationSigner::from_seed(Workload::Post, &[76; 32])?;
    let verifier = PostConcertRootReadVerifier::new(
        &[signer.public_key()],
        &ActorDelegationVerifier::new(Workload::Concert, &[capture::gateway()?.public_key()])?,
    )?;
    let server = guard(
        &POLICY,
        ConcertRootReadService::new(f.registry.clone(), f.reader.clone()).server(),
        CallerRegistry::new().register(Workload::Post, "post-fixture")?,
        verifier.into_inner(),
    )?;
    let method = "/authority.lease.v1.ConcertRootReadService/CheckSubject";
    let input = authority::CheckSubjectRequest {
        root: Some(view.clone()),
        subject_id: subject.to_string(),
    };
    let actor: Uuid = view
        .binding
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("binding missing"))?
        .actor_id
        .parse()?;
    let bytes = frame(&input);
    // Sign the negative callback frame without altering the OLD root's deadline/binding.
    let proof = signer.issue(
        Workload::Concert,
        method,
        actor,
        None,
        &bytes,
        chrono::Utc::now().timestamp(),
    );
    let mut req = request(method, bytes)?;
    req.headers_mut()
        .insert("x-service-caller", "post".parse()?);
    req.headers_mut()
        .insert("x-service-token", "post-fixture".parse()?);
    req.headers_mut().insert(HEADER, proof.parse()?);
    let response = platform::call_deadline::CallDeadlineLayer
        .layer(server)
        .oneshot(req)
        .await?;
    let headers = response.headers().clone();
    let body = response.into_body().collect().await?;
    anyhow::ensure!(
        grpc_code(&headers, body.trailers())? != 0 && body.to_bytes().is_empty(),
        "old root callback disclosed content"
    );
    Ok(())
}
async fn restart(run: Uuid, instance: Uuid) -> anyhow::Result<()> {
    let state = load(run)?;
    anyhow::ensure!(
        state.instance == instance && state.old_pid != std::process::id(),
        "physical restart identity required"
    );
    let f = fixture::Fixture::new(instance, false).await?;
    let view = authority::RootReadView::decode(state.root_view.as_slice())?;
    let capability = proto::ResponseDelivery::decode(state.delivery.as_slice())?;
    let key = scope_key(
        view.scope
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("scope missing"))?,
    )?;
    invariant(&f, &key).await?;
    anyhow::ensure!(
        pending(&f, &key).await? == (2, 0),
        "pending children lost at restart"
    );
    for observation in &state.prepared {
        let pool = match observation.owner.as_str() {
            "auth" => &f.auth,
            "friend" => &f.friend,
            "concert" => &f.concert,
            "profile" => &f.profile,
            _ => anyhow::bail!("unknown observed owner"),
        };
        anyhow::ensure!(
            prepared(pool)
                .await?
                .iter()
                .any(|(gid, xid)| gid == &observation.gid && xid == &observation.xid),
            "prepared child/owner did not survive exact process death"
        );
    }
    writers(&f, state.subject, true).await?; // After physical restart, before recovery.
    old_content_denied(&f, &view, state.subject).await?;
    let publisher = DeliveryPublisher::new(f.ledger.clone(), f.registry.clone());
    anyhow::ensure!(
        complete(publisher.clone(), capability.clone()).await? == 14,
        "empty-registry delivery must remain Unavailable"
    );
    // Fresh worker state: no cursor restoration or in-memory candidate injection.
    let settings = ChildRecoverySettings::new(
        2,
        Duration::from_secs(2),
        Duration::from_secs(6),
        Duration::from_millis(100),
    )?;
    let mut worker = ChildRecoveryWorker::new(f.ledger.clone(), settings)?;
    let mut completed = 0;
    for _ in 0..8 {
        let counts = worker.tick().await?;
        completed += counts.completed;
        if pending(&f, &key).await? == (0, 2) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    anyhow::ensure!(
        completed == 2 && pending(&f, &key).await? == (0, 2),
        "actual production recovery did not ACK both children"
    );
    for pool in [&f.concert, &f.profile] {
        anyhow::ensure!(
            prepared(pool).await?.is_empty(),
            "actual child PREPARE survived recovery"
        );
    }
    writers(&f, state.subject, false).await?; // After real durable child ACKs, roll back both writes.
    let settled: i64 = sqlx::query_scalar(
        "select count(*) from concert_public_profile_children where state='SETTLED'",
    )
    .fetch_one(&f.profile)
    .await?;
    anyhow::ensure!(settled == 1, "actual Profile settled receipt missing");
    invariant(&f, &key).await?;
    old_content_denied(&f, &view, state.subject).await?;
    anyhow::ensure!(
        complete(publisher.clone(), capability.clone()).await? == 14,
        "child ACK must not manufacture delivery completion"
    );
    // Child-only recovery is followed by actual Auth/Friend store recovery.
    // Original expiry is unchanged; genuine RootFinished over the new Root mTLS
    // service, not expiry or absence, is the owner settlement proof.
    wait_past_saved_expiry(&view).await?;
    f.recover_owners().await?;
    for (name, pool) in [("auth", &f.auth), ("friend", &f.friend)] {
        let original = state
            .prepared
            .iter()
            .find(|observation| observation.owner == name)
            .ok_or_else(|| anyhow::anyhow!("original owner PREPARE observation missing"))?;
        completed_owner(pool, &key, &original.gid).await?;
    }
    for pool in [&f.auth, &f.friend, &f.concert, &f.profile] {
        anyhow::ensure!(
            prepared(pool).await?.is_empty(),
            "all four native guard sets must settle normally"
        );
    }
    anyhow::ensure!(
        pending(&f, &key).await? == (0, 2),
        "owner reconciliation changed child ACKs"
    );
    invariant(&f, &key).await?;
    old_content_denied(&f, &view, state.subject).await?;
    anyhow::ensure!(
        complete(publisher, capability).await? == 14,
        "all four native guards settled must still not complete lost-registry Delivery"
    );
    line("MCR_CR01_RESTART_OBSERVED")?;
    Ok(())
}
#[tokio::test]
#[ignore = "requires owned PG16/mTLS and source-bound prebuilt process controller; NOT_RUN"]
async fn process_root_child_recovery_cr01() -> anyhow::Result<()> {
    let run: Uuid = std::env::var("MCR_PROCESS_RUN")?.parse()?;
    let instance: Uuid = std::env::var("MCR_PROCESS_INSTANCE")?.parse()?;
    anyhow::ensure!(
        !run.is_nil() && !instance.is_nil(),
        "explicit nonzero run/instance required"
    );
    // Controller bounds old-process startup until READY. Restart also has a Rust bound.
    // The old process intentionally awaits forever after fixed READY.
    match std::env::var(TEST_MODE)?.as_str() {
        "old" => old(run, instance).await,
        "restart" => tokio::time::timeout(Duration::from_secs(90), restart(run, instance)).await?,
        _ => anyhow::bail!("unsupported mode; only bounded CR01 is authored"),
    }
}
