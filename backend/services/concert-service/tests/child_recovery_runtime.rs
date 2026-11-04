//! Real PG/native-adapter component checks. Ledger lifecycle APIs below are
//! fixture setup, NOT actual canonical Root terminal/HTTP or CR01-CR06 proof.
//! Environment reuse: owner_authority_support/environment.rs at 26579458.
#[path = "owner_authority_support/environment.rs"]
mod environment;
use concert_service::{
    adapters::out::{
        public_profile::PublicProfileReader,
        relationship_authority::guarded::{
            ChildNativeIdentity, ChildQuery, ChildRecoveryReference,
        },
    },
    media_runtime::settlement::MediaTransactionIdentity,
    product_rpc::{
        child_recovery::{ChildRecoverySettings, ChildRecoveryWorker},
        scope_ledger::*,
    },
};
use platform::{
    actor_delegation::{
        owner_binding::OwnerBinding, relationship_read::RelationshipReadSigner,
        ActorDelegationVerifier,
    },
    grpc_auth::Workload,
};
use sqlx::PgPool;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tonic::Status;
use uuid::Uuid;

struct Native {
    reader: PublicProfileReader,
    unavailable: Mutex<Option<Uuid>>,
    hanging: bool,
}
#[async_trait::async_trait]
impl ChildIntentSettler for Native {
    async fn recover_one(&self, reference: &ChildRecoveryReference) -> Result<(), Status> {
        let denied = *self.unavailable.lock().unwrap() == Some(reference.intent_id()?);
        if denied {
            if self.hanging {
                std::future::pending::<()>().await;
            }
            return Err(Status::unavailable(
                "Deliberately unavailable native candidate",
            ));
        }
        // Every successful settlement uses the actual production native owner.
        self.reader.recover_one(reference).await
    }
}
fn keys(id: &str, secret: u8, retained: Vec<ChildRecoveryKey>) -> Arc<ChildRecoveryKeys> {
    Arc::new(
        ChildRecoveryKeys::new(ChildRecoveryKey::new(id, &[secret; 32]).unwrap(), retained)
            .unwrap(),
    )
}
fn native(pool: &PgPool, hanging: bool) -> anyhow::Result<Arc<Native>> {
    // Exact source-bound public constructor copied from Public native fixture
    // at 25a87426. No private reader constructor or synthetic avatar catalog.
    use concert_service::bootstrap::{BootstrapFeatures, ProductReleaseBundle};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap();
    let bundle = root.join("backend/releases/operator-bundle/quality-v6");
    let spatial = root.join("backend/releases/reference-scene-v2/quality-v6");
    let themes = [
        bundle.join("theme-revenge.json"),
        bundle.join("theme-parade.json"),
        bundle.join("theme-danger.json"),
    ];
    let release = ProductReleaseBundle::from_source_bound_files(
        &bundle.join("bootstrap.json"),
        &root.join("contracts/fixtures/assets/selected-static/avatar-manifest.json"),
        &spatial.join("map-manifest.json"),
        [&themes[0], &themes[1], &themes[2]],
        BootstrapFeatures::default(),
        &root.join("frontend/public"),
        &spatial.join("source-binding.json"),
    )
    .map_err(|_| anyhow::anyhow!("Actual source-bound release required"))?;
    let gateway =
        platform::actor_delegation::ActorDelegationSigner::from_seed(Workload::Gateway, &[74; 32])?;
    let signer = RelationshipReadSigner::from_seed(
        &[75; 32],
        &ActorDelegationVerifier::new(Workload::Concert, &[gateway.public_key()])?,
    )?;
    Ok(Arc::new(Native {
        reader: PublicProfileReader::connect(
            pool.clone(),
            release,
            "https://127.0.0.1:1",
            "concert-fixture",
            Some(&environment::tls(Workload::Concert)?),
            signer,
        )?,
        unavailable: Mutex::new(None),
        hanging,
    }))
}
fn binding(expiry: i64) -> OwnerBinding {
    // The real release event is required even for settlement-only reader use.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap();
    let release: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join("backend/releases/operator-bundle/quality-v6/bootstrap.json"))
            .unwrap(),
    )
    .unwrap();
    let event: Uuid = release["event"]["event_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    OwnerBinding {
        actor_id: Uuid::new_v4(),
        event_id: event,
        session_family_id: Uuid::new_v4(),
        request_id: Uuid::new_v4(),
        operation: "listConcertFeed".into(),
        expires_unix_micros: expiry,
        parent_digest: [1; 32],
        parent_body_digest: [2; 32],
        parent_method: "/concert.product.v1.ProductService/Dispatch".into(),
        http_method: "GET".into(),
        http_target: format!("/api/concert/events/{event}/feed"),
        body_digest: [3; 32],
    }
}
async fn register(
    ledger: &PgScopeLedger,
    expires: i64,
) -> anyhow::Result<(ScopeKey, OwnerBinding)> {
    let b = binding(expires);
    let scope = ScopeKey {
        instance: ledger.instance(),
        scope: Uuid::new_v4(),
        binding_digest: b.digest()?,
    };
    ledger.register(&scope, &b).await?;
    ledger.bind_delivery(&scope, &[77; 32]).await?;
    Ok((scope, b))
}
async fn prepared(
    pool: &PgPool,
    ledger: &PgScopeLedger,
    scope: &ScopeKey,
    b: &OwnerBinding,
    lock: i64,
) -> anyhow::Result<ChildRecoveryReference> {
    let mut tx = pool.begin().await?;
    sqlx::query("select pg_advisory_xact_lock($1)")
        .bind(lock)
        .execute(&mut *tx)
        .await?;
    let identity = MediaTransactionIdentity::capture_in(&mut tx).await?;
    let reference = ChildRecoveryReference::LocalPublic {
        query: ChildQuery {
            actor: b.actor_id,
            event: b.event_id,
            subject: b.actor_id,
            family: b.session_family_id,
            request: b.request_id,
            expires_unix_micros: b.expires_unix_micros,
            binding_digest: scope.binding_digest,
        },
        child_nonce: Uuid::new_v4(),
        native: ChildNativeIdentity {
            system_identifier: identity.system_identifier().into(),
            database_oid: identity.database_oid() as u32,
            full_xid: identity.xid().into(),
            backend_pid: identity.backend_pid(),
            backend_started_unix_micros: identity.backend_started_at().timestamp_micros(),
            producer_instance_id: scope.instance,
        },
    };
    ledger.register_child_intent(scope, &reference).await?;
    sqlx::query(&format!(
        "prepare transaction '{}'",
        reference.local_prepared_gid()?
    ))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(reference)
}
async fn finish(ledger: &PgScopeLedger, scope: &ScopeKey) -> anyhow::Result<()> {
    ledger.mark_serialized(scope).await?;
    ledger.seal(scope).await?;
    ledger.finish_root_work(scope).await?;
    Ok(())
}
fn settings(batch: u16) -> ChildRecoverySettings {
    ChildRecoverySettings::new(
        batch,
        Duration::from_millis(50),
        Duration::from_millis(500),
        Duration::from_millis(100),
    )
    .unwrap()
}
async fn counts(pool: &PgPool, instance: Uuid) -> anyhow::Result<(i64, i64)> {
    Ok(sqlx::query_as("select count(*) filter(where completed_at is null),count(*) filter(where completed_at is not null) from concert_product_child_intents where instance_id=$1").bind(instance).fetch_one(pool).await?)
}
async fn writer(pool: &PgPool, lock: i64, blocked: bool) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("set local lock_timeout='25ms'")
        .execute(&mut *tx)
        .await?;
    let result = sqlx::query("select pg_advisory_xact_lock($1)")
        .bind(lock)
        .execute(&mut *tx)
        .await;
    if blocked {
        anyhow::ensure!(
            result
                .err()
                .and_then(|e| e
                    .as_database_error()
                    .and_then(|d| d.code())
                    .map(|v| v.into_owned()))
                .as_deref()
                == Some("55P03"),
            "Native child did not exclude actual writer"
        );
    } else {
        result?;
    }
    tx.rollback().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires frozen owned PG16/native-adapter controller"]
async fn finished_native_children_progress_past_unknown_and_retain_capacity() -> anyhow::Result<()>
{
    let pool = environment::pool("concert").await?;
    let settler = native(&pool, false)?;
    let instance = Uuid::new_v4();
    let ledger = PgScopeLedger::new(
        pool.clone(),
        instance,
        keys("old", 41, vec![]),
        settler.clone(),
    )?;
    let expires = chrono::Utc::now().timestamp_micros() + 60_000_000;
    let mut all = vec![];
    for index in 0..16 {
        let (scope, b) = register(&ledger, expires).await?;
        for child in 0..if index == 0 { 2 } else { 1 } {
            let lock = 810_000 + index * 2 + child;
            let reference = prepared(&pool, &ledger, &scope, &b, lock).await?;
            all.push((scope.clone(), reference, lock));
        }
        finish(&ledger, &scope).await?;
    }
    let first:Uuid=sqlx::query_scalar("select intent_id from concert_product_child_intents where instance_id=$1 order by scope_id,kind,intent_id limit 1").bind(instance).fetch_one(&pool).await?;
    *settler.unavailable.lock().unwrap() = Some(first);
    let restarted = PgScopeLedger::new(
        pool.clone(),
        instance,
        keys("old", 41, vec![]),
        settler.clone(),
    )?;
    let mut worker = ChildRecoveryWorker::new(restarted.clone(), settings(1))?;
    for _ in 0..40 {
        let c = worker.tick().await?;
        assert!(c.attempted <= 1);
    }
    assert_eq!(counts(&pool, instance).await?, (1, 16));
    for (_, reference, lock) in &all {
        writer(&pool, *lock, reference.intent_id()? == first).await?;
    }
    let b = binding(expires);
    let extra = ScopeKey {
        instance,
        scope: Uuid::new_v4(),
        binding_digest: b.digest()?,
    };
    assert_eq!(
        restarted.register(&extra, &b).await.unwrap_err().code(),
        tonic::Code::ResourceExhausted
    );
    *settler.unavailable.lock().unwrap() = None;
    for _ in 0..4 {
        worker.tick().await?;
    }
    assert_eq!(counts(&pool, instance).await?, (0, 17));
    // All children settled still cannot invent either owner/delivery proof.
    assert_eq!(
        restarted.register(&extra, &b).await.unwrap_err().code(),
        tonic::Code::ResourceExhausted
    );
    for (scope, _, lock) in &all {
        assert!(!restarted.delivery_completed(scope).await?);
        writer(&pool, *lock, false).await?;
    }
    pool.close().await;
    Ok(())
}

#[tokio::test]
#[ignore = "requires frozen owned PG16/native-adapter controller"]
async fn active_and_sealed_expired_native_children_remain_quarantined() -> anyhow::Result<()> {
    let pool = environment::pool("concert").await?;
    let settler = native(&pool, false)?;
    let ledger = PgScopeLedger::new(
        pool.clone(),
        Uuid::new_v4(),
        keys("old", 41, vec![]),
        settler.clone(),
    )?;
    let expires = chrono::Utc::now().timestamp_micros() + 500_000;
    let mut all = vec![];
    for state in [ScopePhase::Active, ScopePhase::Sealed] {
        let (scope, b) = register(&ledger, expires).await?;
        let lock = 820_000 + all.len() as i64;
        let reference = prepared(&pool, &ledger, &scope, &b, lock).await?;
        if state == ScopePhase::Sealed {
            ledger.seal(&scope).await?;
        }
        all.push((scope, reference, lock, state));
    }
    tokio::time::sleep(
        Duration::from_micros(
            expires
                .saturating_sub(chrono::Utc::now().timestamp_micros())
                .max(0) as u64,
        ) + Duration::from_millis(5),
    )
    .await;
    let restarted = PgScopeLedger::new(
        pool.clone(),
        ledger.instance(),
        keys("old", 41, vec![]),
        settler.clone(),
    )?;
    let mut worker = ChildRecoveryWorker::new(restarted.clone(), settings(2))?;
    assert_eq!(worker.tick().await?.selected, 0);
    assert_eq!(counts(&pool, ledger.instance()).await?, (2, 0));
    for (scope, _, lock, state) in &all {
        assert_eq!(restarted.resolve_scope(scope).await?, *state);
        writer(&pool, *lock, true).await?;
    }
    // Explicit owned-fixture teardown of intentionally unknown native children;
    // never record an ACK or promote the root as a result of this cleanup.
    for (_, reference, _, _) in &all {
        settler.reader.recover_one(reference).await?;
    }
    assert_eq!(counts(&pool, ledger.instance()).await?, (2, 0));
    pool.close().await;
    Ok(())
}

#[tokio::test]
#[ignore = "requires frozen owned PG16/native-adapter controller"]
async fn retained_key_timeout_and_concurrent_retry_require_real_native_abort() -> anyhow::Result<()>
{
    let pool = environment::pool("concert").await?;
    let settler = native(&pool, true)?;
    let instance = Uuid::new_v4();
    let ledger = PgScopeLedger::new(
        pool.clone(),
        instance,
        keys("old", 41, vec![]),
        settler.clone(),
    )?;
    let expires = chrono::Utc::now().timestamp_micros() + 60_000_000;
    let (scope, b) = register(&ledger, expires).await?;
    let a = prepared(&pool, &ledger, &scope, &b, 830_000).await?;
    let c = prepared(&pool, &ledger, &scope, &b, 830_001).await?;
    finish(&ledger, &scope).await?;
    let wrong = PgScopeLedger::new(
        pool.clone(),
        instance,
        keys("new", 42, vec![]),
        settler.clone(),
    )?;
    let mut worker = ChildRecoveryWorker::new(wrong, settings(2))?;
    let result = worker.tick().await?;
    assert_eq!(
        (result.attempted, result.completed, result.pending),
        (2, 0, 2)
    );
    assert_eq!(counts(&pool, instance).await?, (2, 0));
    writer(&pool, 830_000, true).await?;
    writer(&pool, 830_001, true).await?;
    *settler.unavailable.lock().unwrap() = Some(a.intent_id()?);
    let rotated = PgScopeLedger::new(
        pool.clone(),
        instance,
        keys("new", 42, vec![ChildRecoveryKey::new("old", &[41; 32])?]),
        settler.clone(),
    )?;
    let mut first = ChildRecoveryWorker::new(rotated.clone(), settings(2))?;
    let result = first.tick().await?;
    assert_eq!(
        (result.completed, result.pending, result.timed_out),
        (1, 1, 1)
    );
    assert_eq!(counts(&pool, instance).await?, (1, 1));
    writer(&pool, 830_000, true).await?;
    writer(&pool, 830_001, false).await?;
    *settler.unavailable.lock().unwrap() = None;
    let mut second = ChildRecoveryWorker::new(rotated.clone(), settings(2))?;
    let (a_result, b_result) = tokio::join!(first.tick(), second.tick());
    a_result?;
    b_result?;
    assert_eq!(counts(&pool, instance).await?, (0, 2));
    assert!(!rotated.delivery_completed(&scope).await?);
    assert_ne!(a.intent_id()?, c.intent_id()?);
    writer(&pool, 830_000, false).await?;
    pool.close().await;
    Ok(())
}
