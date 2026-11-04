//! BE15/QA09 actual physical backup/restore process harness. No production activation.
#[path = "canonical_owner_restore_support/capture.rs"]
mod capture;
#[path = "canonical_owner_restore_support/environment.rs"]
mod environment;
#[path = "canonical_owner_restore_support/fault.rs"]
mod fault;
#[path = "canonical_owner_restore_support/fixture.rs"]
mod fixture;
#[path = "canonical_owner_restore_support/notification.rs"]
mod notification;
#[path = "canonical_owner_restore_support/owners.rs"]
mod owners;
#[path = "canonical_owner_restore_support/release.rs"]
mod release;
use concert_service::{
    adapters::out::relationship_authority::guarded::OwnerGuardScope,
    config::canonical_runtime::CanonicalRuntimeCore,
    product_rpc::{
        child_recovery::ChildRecoverySettings,
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

use concert_service::adapters::out::relationship_authority::guarded::proto as authority;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::Write,
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
};

// Private operator handoff: no Debug and no output of capabilities/native IDs.
#[derive(Serialize, Deserialize)]
struct Saved {
    run: Uuid,
    old_pid: u32,
    instance: Uuid,
    case: String,
    actor: Uuid,
    subject: Uuid,
    root_view: Vec<u8>,
    delivery: Vec<u8>,
    prepared: BTreeMap<String, Vec<(String, String)>>,
    intents: Vec<(String, Uuid, String, Vec<u8>)>,
    native: serde_json::Value,
}
fn state_path() -> anyhow::Result<std::path::PathBuf> {
    Ok(environment::root()?.join("restore-private-handoff.json"))
}
fn save(value: &Saved) -> anyhow::Result<()> {
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(state_path()?)?;
    f.write_all(&serde_json::to_vec(value)?)?;
    f.sync_all()?;
    Ok(())
}
fn load(run: Uuid) -> anyhow::Result<Saved> {
    let p = state_path()?;
    let m = std::fs::symlink_metadata(&p)?;
    let root = std::fs::metadata(environment::root()?)?;
    anyhow::ensure!(
        m.is_file()
            && !m.file_type().is_symlink()
            && m.uid() == root.uid()
            && m.permissions().mode() & 0o777 == 0o600
            && m.len() < 1_000_000,
        "private handoff identity"
    );
    let s: Saved = serde_json::from_slice(&std::fs::read(p)?)?;
    anyhow::ensure!(s.run == run, "run identity");
    Ok(s)
}
fn line(s: &str) -> anyhow::Result<()> {
    println!("{s}");
    std::io::stdout().flush()?;
    Ok(())
}
async fn prepared_rows(p: &PgPool) -> anyhow::Result<Vec<(String, String)>> {
    Ok(sqlx::query_as("select gid,transaction::text from pg_prepared_xacts where database=current_database() order by gid").fetch_all(p).await?)
}
async fn prepared_all(
    f: &fixture::Fixture,
) -> anyhow::Result<BTreeMap<String, Vec<(String, String)>>> {
    let mut result = BTreeMap::new();
    for (name, p) in [
        ("auth", &f.auth),
        ("friend", &f.friend),
        ("concert", &f.concert),
        ("profile", &f.profile),
        ("notification", &f.notification),
    ] {
        result.insert(name.into(), prepared_rows(p).await?);
    }
    Ok(result)
}
async fn intents(p: &PgPool, k: &ScopeKey) -> anyhow::Result<Vec<(String, Uuid, String, Vec<u8>)>> {
    Ok(sqlx::query_as("select kind,intent_id,key_id,sealed_reference from concert_product_child_intents where instance_id=$1 and scope_id=$2 order by kind,intent_id").bind(k.instance).bind(k.scope).fetch_all(p).await?)
}
async fn effects(p: &PgPool) -> anyhow::Result<(i64, i64)> {
    Ok(sqlx::query_as("select count(*)::bigint,coalesce(sum(tracked_effects),0)::bigint from qa09_restore_committed_effects").fetch_one(p).await?)
}
async fn root_phase(
    f: &fixture::Fixture,
    k: &ScopeKey,
    expected: ScopePhase,
) -> anyhow::Result<()> {
    assert_eq!(f.owners.core.ledger().resolve_scope(k).await?, expected);
    assert!(!f.owners.core.ledger().delivery_completed(k).await?);
    Ok(())
}
fn key_from(view: &authority::RootReadView) -> anyhow::Result<ScopeKey> {
    let p = view
        .scope
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("scope"))?;
    Ok(ScopeKey {
        instance: p.instance_id.parse()?,
        scope: p.scope_id.parse()?,
        binding_digest: p
            .binding_digest
            .clone()
            .try_into()
            .map_err(|_| anyhow::anyhow!("digest"))?,
    })
}
async fn source(run: Uuid, instance: Uuid, case: &str) -> anyhow::Result<()> {
    let f = fixture::Fixture::new(instance).await?;
    let (actor, family, subject) = f.seed().await?;
    // Explicit fixture effect, never a producer commit witness or domain success
    // default. Real registration + native commit below must remain UNKNOWN.
    sqlx::query("create table qa09_restore_committed_effects(id bigint primary key,tracked_effects bigint not null check(tracked_effects>=0))").execute(&f.concert).await?;
    sqlx::query("insert into qa09_restore_committed_effects values(1,0)")
        .execute(&f.concert)
        .await?;
    let scope = f
        .owners
        .fence
        .acquire(capture::binding(actor, family, f.event, false).await?)
        .await?;
    let k = key(&scope)?;
    let admitted = scope.public_profile_subject(subject).await?;
    let author = f
        .owners
        .public_profile
        .read(&admitted)
        .await
        .map_err(|_| anyhow::anyhow!("real retained Profile read"))?;
    assert_eq!(author.user_id, subject);
    let mut native = serde_json::Value::Null;
    if case != "ACTIVE" {
        let mut actual = f.concert.begin().await?;
        use sha2::Digest;
        scope
            .register_transaction_in(
                &mut actual,
                sha2::Sha256::digest(b"qa09 actual native restore effect").into(),
                Uuid::new_v4().as_bytes().to_vec(),
            )
            .await?;
        native=sqlx::query_scalar("select identity from concert_product_owner_transactions where instance_id=$1 and scope_id=$2").bind(k.instance).bind(k.scope).fetch_one(&f.concert).await?;
        if case == "MISSING_WITNESS" {
            sqlx::query("update qa09_restore_committed_effects set tracked_effects=tracked_effects+1 where id=1").execute(&mut *actual).await?;
            actual.commit().await?;
        } else {
            actual.rollback().await?;
        }
    }
    let cap = serialize(&f, scope.clone()).await?;
    if case != "ACTIVE" {
        f.profile_deny_complete.store(true, Ordering::SeqCst);
        assert!(
            !deliver(&f, cap.clone()).await,
            "a source with unresolved native/child proof cannot deliver"
        );
        root_phase(&f, &k, ScopePhase::RootFinished).await?;
    } else {
        root_phase(&f, &k, ScopePhase::Active).await?;
    }
    let rows = prepared_all(&f).await?;
    let expected_owner = if case == "FINISHED" { 0 } else { 1 };
    assert_eq!(rows["auth"].len(), expected_owner);
    assert_eq!(rows["friend"].len(), expected_owner);
    assert_eq!(
        rows["concert"].len(),
        if case == "FINISHED" { 0 } else { 1 }
    );
    assert_eq!(rows["profile"].len(), 1);
    let saved_intents = intents(&f.concert, &k).await?;
    assert_eq!(saved_intents.len(), 2);
    assert_eq!(
        pending(&f.concert, &k).await?,
        if case == "FINISHED" { 1 } else { 2 }
    );
    assert_eq!(
        effects(&f.concert).await?,
        (1, if case == "MISSING_WITNESS" { 1 } else { 0 })
    );
    save(&Saved {
        run,
        old_pid: std::process::id(),
        instance,
        case: case.into(),
        actor,
        subject,
        root_view: scope.root_read_view().encode_to_vec(),
        delivery: cap.encode_to_vec(),
        prepared: rows,
        intents: saved_intents,
        native,
    })?;
    // Only the controller's actual SIGKILL ends this root. No source teardown
    // or Drop settles prepared state before the full cluster backup.
    line("MCR_RESTORE_SOURCE_READY")?;
    std::future::pending::<()>().await;
    Ok(())
}
// Delegation preserves the actual mTLS owner identity and exact persisted
// registration supplied by production recovery. Only successful typed UNKNOWN
// responses for that same transaction count; no fake settlement is returned.
macro_rules! observe_owner {
    ($name:ident, $service:ident) => {
        struct $name {
            inner: $service::concert_authority::RpcSettlementResolver,
            unknown: Arc<AtomicUsize>,
        }
        #[async_trait::async_trait]
        impl $service::concert_authority::SettlementResolver for $name {
            fn configured(&self, producer: $service::concert_authority::proto::Producer) -> bool {
                $service::concert_authority::SettlementResolver::configured(&self.inner, producer)
            }
            async fn resolve_scope(
                &self,
                query: $service::concert_authority::proto::ResolveScopeRequest,
                budget: &$service::concert_authority::OperationBudget,
            ) -> Result<$service::concert_authority::proto::ResolveScopeResponse, Status> {
                $service::concert_authority::SettlementResolver::resolve_scope(
                    &self.inner,
                    query,
                    budget,
                )
                .await
            }
            async fn resolve_transaction(
                &self,
                query: $service::concert_authority::proto::ResolveTransactionRequest,
                budget: &$service::concert_authority::OperationBudget,
            ) -> Result<$service::concert_authority::proto::ResolveTransactionResponse, Status>
            {
                let expected = query.transaction.clone();
                assert!(!query.registration_id.is_empty());
                let result = $service::concert_authority::SettlementResolver::resolve_transaction(
                    &self.inner,
                    query,
                    budget,
                )
                .await?;
                assert_eq!(result.transaction, expected);
                if result.status
                    == $service::concert_authority::proto::TransactionStatus::Unknown as i32
                    && result.receipt_evidence.is_empty()
                {
                    self.unknown.fetch_add(1, Ordering::SeqCst);
                }
                Ok(result)
            }
        }
    };
}
observe_owner!(ObservedAuth, auth_service);
observe_owner!(ObservedFriend, friend_service);
async fn owner_recover(f: &fixture::Fixture, require_unknown: bool) -> anyhow::Result<(u32, u32)> {
    use auth_service::concert_authority::{
        proto as ap, OperationBudget as ABudget, PgOwnerAuthority as Auth,
        RpcSettlementResolver as AuthResolver,
    };
    use friend_service::concert_authority::{
        proto as fp, OperationBudget as FBudget, PgOwnerAuthority as Friend,
        RpcSettlementResolver as FriendResolver,
    };
    use std::time::Instant;
    // Observe each actual owner resolver independently after the authenticated
    // RPC succeeds. Unavailable cannot satisfy this assertion.
    let auth_unknown = Arc::new(AtomicUsize::new(0));
    let friend_unknown = Arc::new(AtomicUsize::new(0));
    let a = Auth::new(
        f.auth.clone(),
        Arc::new(ObservedAuth {
            inner: AuthResolver::new(
                &environment::tls(Workload::Auth)?,
                "auth-fixture",
                &[(ap::Producer::Concert, f.root_url.clone())],
            )?,
            unknown: auth_unknown.clone(),
        }),
    )?;
    let b = Friend::new(
        f.friend.clone(),
        Arc::new(ObservedFriend {
            inner: FriendResolver::new(
                &environment::tls(Workload::Friend)?,
                "friend-fixture",
                &[(fp::Producer::Concert, f.root_url.clone())],
            )?,
            unknown: friend_unknown.clone(),
        }),
    )?;
    let counts = (
        a.recover(&ABudget::new(Instant::now() + Duration::from_secs(2))?)
            .await?,
        b.recover(&FBudget::new(Instant::now() + Duration::from_secs(2))?)
            .await?,
    );
    if require_unknown {
        assert!(
            auth_unknown.load(Ordering::SeqCst) > 0,
            "Auth exact transaction RPC must successfully return UNKNOWN"
        );
        assert!(
            friend_unknown.load(Ordering::SeqCst) > 0,
            "Friend exact transaction RPC must successfully return UNKNOWN"
        );
    }
    Ok(counts)
}
async fn restored(run: Uuid, instance: Uuid, case: &str) -> anyhow::Result<()> {
    let saved = load(run)?;
    anyhow::ensure!(
        saved.instance == instance && saved.case == case && saved.old_pid != std::process::id(),
        "distinct restored root process"
    );
    let f = fixture::Fixture::new(instance).await?;
    let view = authority::RootReadView::decode(saved.root_view.as_slice())?;
    let k = key_from(&view)?;
    let cap = proto::ResponseDelivery::decode(saved.delivery.as_slice())?;
    assert_eq!(
        prepared_all(&f).await?,
        saved.prepared,
        "physical restore lost exact prepared identities"
    );
    assert_eq!(
        intents(&f.concert, &k).await?,
        saved.intents,
        "physical restore changed encrypted registered children"
    );
    if case != "ACTIVE" {
        let restored_native:serde_json::Value=sqlx::query_scalar("select identity from concert_product_owner_transactions where instance_id=$1 and scope_id=$2").bind(k.instance).bind(k.scope).fetch_one(&f.concert).await?;
        assert_eq!(
            restored_native, saved.native,
            "actual native registration identity must survive the physical restore"
        );
    }
    let expected_effect = (1, if case == "MISSING_WITNESS" { 1 } else { 0 });
    assert_eq!(effects(&f.concert).await?, expected_effect);
    while chrono::Utc::now().timestamp_micros()
        <= view
            .binding
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("binding"))?
            .expires_unix_micros
    {
        tokio::time::sleep(Duration::from_millis(15)).await;
    }
    assert!(
        f.owners.core.registry().lookup_root_read(&view).is_err(),
        "restore cannot recreate expired authority"
    );
    assert!(
        terminal(f.owners.core.clone(), cap.clone()).await.is_err(),
        "empty restored registry cannot invent delivery"
    );
    let foreign = f.compose(Uuid::new_v4(), fixture::KeyMode::Retained)?;
    assert!(foreign.core.ledger().resolve_scope(&k).await.is_err());
    let mut worker = foreign
        .core
        .child_recovery_worker(ChildRecoverySettings::default())?;
    assert_eq!(worker.tick().await?.attempted, 0);
    assert_eq!(
        pending(&f.concert, &k).await?,
        if case == "FINISHED" { 1 } else { 2 }
    );
    assert_eq!(prepared_all(&f).await?, saved.prepared);
    let mut missing = f
        .compose(instance, fixture::KeyMode::Missing)?
        .core
        .child_recovery_worker(ChildRecoverySettings::default())?;
    let failed = missing.tick().await?;
    assert_eq!(failed.completed, 0);
    assert_eq!(
        pending(&f.concert, &k).await?,
        if case == "FINISHED" { 1 } else { 2 }
    );
    assert_eq!(intents(&f.concert, &k).await?, saved.intents);
    assert_eq!(prepared_all(&f).await?, saved.prepared);
    let expected_phase = if case == "ACTIVE" {
        ScopePhase::Active
    } else {
        ScopePhase::RootFinished
    };
    root_phase(&f, &k, expected_phase).await?;
    let retained = f.compose(instance, fixture::KeyMode::Retained)?;
    let mut actual = retained
        .core
        .child_recovery_worker(ChildRecoverySettings::new(
            2,
            Duration::from_secs(2),
            Duration::from_secs(6),
            Duration::from_millis(100),
        )?)?;
    let first = actual.tick().await?;
    if case == "ACTIVE" {
        assert_eq!(first.attempted, 0);
        assert_eq!(
            pending(&f.concert, &k).await?,
            if case == "FINISHED" { 1 } else { 2 }
        );
    } else {
        assert_eq!(first.completed, if case == "FINISHED" { 1 } else { 2 });
        assert_eq!(pending(&f.concert, &k).await?, 0);
    }
    for _ in 0..2 {
        assert_eq!(
            actual.tick().await?.completed,
            0,
            "idempotent drain must not re-ACK or duplicate effects"
        );
        assert_eq!(owner_recover(&f, case == "MISSING_WITNESS").await?, (0, 0));
        assert_eq!(effects(&f.concert).await?, expected_effect);
    }
    let after = prepared_all(&f).await?;
    if case == "FINISHED" {
        assert!(after.values().all(Vec::is_empty));
    } else {
        assert_eq!(after["auth"].len(), 1);
        assert_eq!(after["friend"].len(), 1);
    }
    if case == "ACTIVE" {
        assert_eq!(after, saved.prepared);
    } else {
        assert_eq!(after["concert"].len(), 0);
        assert_eq!(after["profile"].len(), 0);
    }
    if case == "MISSING_WITNESS" {
        use concert_service::product_rpc::{
            scope_ledger::ProducerTransaction,
            settlement::{NativeTransactionResolver, PgNativeAbortResolver},
        };
        let native: ProducerTransaction = serde_json::from_value(saved.native.clone())?;
        let state: Option<String> = sqlx::query_scalar("select pg_xact_status($1::text::xid8)")
            .bind(&native.full_xid)
            .fetch_one(&f.concert)
            .await?;
        assert_eq!(state.as_deref(), Some("committed"));
        let binding_json: serde_json::Value = sqlx::query_scalar(
            "select binding from concert_product_owner_scopes where instance_id=$1 and scope_id=$2",
        )
        .bind(k.instance)
        .bind(k.scope)
        .fetch_one(&f.concert)
        .await?;
        let binding: platform::actor_delegation::owner_binding::OwnerBinding =
            serde_json::from_value(binding_json)?;
        let proof = PgNativeAbortResolver::new(f.concert.clone(), instance)?
            .resolve(&binding, &native)
            .await?;
        assert_eq!(proof.status, authority::TransactionStatus::Unknown);
        assert!(proof.receipt_evidence.is_empty());
        assert!(f.native_queries.load(Ordering::SeqCst)>=2,"actual authenticated owner-to-root settlement RPCs must reach the real native resolver");
        for pool in [&f.auth, &f.friend] {
            let statuses: Vec<String> =
                sqlx::query_scalar("select state from concert_owner_guard_participants")
                    .fetch_all(pool)
                    .await?;
            assert_eq!(
                statuses,
                vec!["PENDING".to_string()],
                "the exact registered missing-witness transaction must remain pending"
            );
        }
    }
    root_phase(&f, &k, expected_phase).await?;
    assert!(terminal(retained.core.clone(), cap).await.is_err());
    // Immutable private references survive even after ACK; a restore/retry never
    // rewrites them into a newly admitted child or renewed content request.
    assert_eq!(intents(&f.concert, &k).await?, saved.intents);
    let result = serde_json::json!({"status":"PASS","case":case,"old_pid":saved.old_pid,"new_pid":std::process::id(),"source_and_restored_distinct":true,"expired_original_authority_refused":true,"lost_registry_delivery_refused":true,"wrong_instance_refused":true,"missing_retained_key_refused":true,"pending_children":pending(&f.concert,&k).await?,"prepared_counts":after.iter().map(|(k,v)|(k.clone(),v.len())).collect::<BTreeMap<_,_>>(),"committed_marker_rows":expected_effect.0,"tracked_effect_count":expected_effect.1,"native_missing_witness_unknown":case=="MISSING_WITNESS","idempotent_recovery":true,"certifies_domain_outbox_or_pickup":false});
    let p = environment::root()?.join("restore-observation.json");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(p)?;
    file.write_all(&serde_json::to_vec(&result)?)?;
    file.sync_all()?;
    line("MCR_RESTORE_OBSERVED")?;
    Ok(())
}
#[tokio::test]
#[ignore = "requires frozen BE15/QA09 physical-backup/restore qualifier and exact owned PG16"]
async fn canonical_owner_restore_process() -> anyhow::Result<()> {
    let run: Uuid = std::env::var("MCR_RESTORE_RUN")?.parse()?;
    let instance: Uuid = std::env::var("MCR_RESTORE_INSTANCE")?.parse()?;
    let case = std::env::var("MCR_RESTORE_CASE")?;
    anyhow::ensure!(
        matches!(case.as_str(), "FINISHED" | "ACTIVE" | "MISSING_WITNESS"),
        "exact restore case"
    );
    match std::env::var("MCR_RESTORE_PHASE")?.as_str() {
        "SOURCE" => source(run, instance, &case).await,
        "RESTORED" => restored(run, instance, &case).await,
        _ => anyhow::bail!("exact restore phase"),
    }
}
