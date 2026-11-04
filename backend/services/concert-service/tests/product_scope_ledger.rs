//! Actual ledger SQL, not canonical-owner or browser acceptance. Execute only
//! inside the owned fixture created by the product-ledger native runner.
use concert_service::product_rpc::scope_ledger::*;
use platform::{actor_delegation::owner_binding::OwnerBinding, grpc_auth::Workload};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::{
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::PathBuf,
    str::FromStr,
};
use uuid::Uuid;

async fn pool() -> sqlx::PgPool {
    // Ownership checks adapted from owner_media_live_support at 49fb637c;
    // product fixtures use a separate database and never attach by ambient URL.
    let root =
        PathBuf::from(std::env::var("CONCERT_TEST_OWNER_ROOT").expect("owned root required"));
    let meta = std::fs::symlink_metadata(&root).unwrap();
    assert!(
        meta.is_dir() && !meta.file_type().is_symlink() && meta.permissions().mode() & 0o077 == 0
    );
    assert_eq!(
        format!("{}:{}", meta.dev(), meta.ino()),
        std::env::var("CONCERT_TEST_OWNER_IDENTITY").unwrap()
    );
    let path = root.join("mcr-owner.json");
    let marker_meta = std::fs::symlink_metadata(&path).unwrap();
    assert!(
        marker_meta.is_file()
            && !marker_meta.file_type().is_symlink()
            && marker_meta.permissions().mode() & 0o077 == 0
    );
    let marker: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(marker["uid"].as_u64(), Some(meta.uid() as u64));
    assert_eq!(
        marker["token"].as_str(),
        Some(std::env::var("CONCERT_TEST_OWNER_TOKEN").unwrap().as_str())
    );
    let options =
        PgConnectOptions::from_str(&std::env::var("CONCERT_TEST_DATABASE_URL").unwrap()).unwrap();
    assert_eq!(options.get_database(), Some("mcr_product_scope_ledger"));
    assert_eq!(options.get_username(), "mcr_owner");
    assert_eq!(options.get_socket(), Some(&root.join("socket")));
    assert_eq!(options.get_port(), 5432);
    assert!(options.get_options().is_none());
    let pool = PgPoolOptions::new()
        .max_connections(6)
        .connect_with(options)
        .await
        .unwrap();
    let data: String = sqlx::query_scalar("SHOW data_directory")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(data, root.join("data").to_string_lossy());
    pool
}
fn binding() -> OwnerBinding {
    let event = Uuid::new_v4();
    OwnerBinding {
        actor_id: Uuid::new_v4(),
        event_id: event,
        session_family_id: Uuid::new_v4(),
        request_id: Uuid::new_v4(),
        operation: "initConcertUpload".into(),
        expires_unix_micros: chrono::Utc::now().timestamp_micros() + 300_000_000,
        parent_digest: [1; 32],
        parent_body_digest: [2; 32],
        parent_method: "/concert.product.v1.ProductService/Dispatch".into(),
        http_method: "POST".into(),
        http_target: format!("/api/concert/events/{event}/media/uploads"),
        body_digest: [3; 32],
    }
}
async fn registered(pool: &sqlx::PgPool) -> (PgScopeLedger, ScopeKey) {
    let ledger = PgScopeLedger::new(pool.clone(), Uuid::new_v4()).unwrap();
    let binding = binding();
    let key = ScopeKey {
        instance: ledger.instance(),
        scope: Uuid::new_v4(),
        binding_digest: binding.digest().unwrap(),
    };
    ledger.register(&key, &binding).await.unwrap();
    (ledger, key)
}

#[tokio::test]
async fn delivery_is_not_finished_by_serialization_expiry_or_missing_memory() {
    let pool = pool().await;
    let (ledger, key) = registered(&pool).await;
    assert!(ledger.mark_serialized(&key).await.is_err());
    ledger.bind_delivery(&key, &[4; 32]).await.unwrap();
    assert!(ledger.bind_delivery(&key, &[5; 32]).await.is_err());
    assert!(ledger.verify_delivery(&key, &[4; 32]).await.is_err());
    ledger.mark_serialized(&key).await.unwrap();
    assert_eq!(
        ledger.verify_delivery(&key, &[4; 32]).await.unwrap(),
        ScopePhase::Active
    );
    assert!(ledger.verify_delivery(&key, &[5; 32]).await.is_err());
    assert!(ledger.finish_root_work(&key).await.is_err());
    let restarted = PgScopeLedger::new(pool.clone(), key.instance).unwrap();
    assert_eq!(
        restarted.resolve_scope(&key).await.unwrap(),
        ScopePhase::Active
    );
    let missing = ScopeKey {
        scope: Uuid::new_v4(),
        ..key.clone()
    };
    assert_eq!(
        restarted.resolve_scope(&missing).await.unwrap(),
        ScopePhase::Unknown
    );
    assert!(restarted.verify_delivery(&missing, &[4; 32]).await.is_err());
    ledger.seal(&key).await.unwrap();
    ledger.finish_root_work(&key).await.unwrap();
    assert_eq!(
        restarted.resolve_scope(&key).await.unwrap(),
        ScopePhase::RootFinished
    );
    assert!(!restarted.delivery_completed(&key).await.unwrap());
    ledger.record_delivery_completed(&key).await.unwrap();
    assert!(restarted.delivery_completed(&key).await.unwrap());
    assert!(ledger.bind_delivery(&key, &[4; 32]).await.is_err());
    pool.close().await;
}

#[tokio::test]
async fn exact_partial_owner_tracking_and_actual_unsettled_transaction_are_distinct() {
    use concert_service::media_runtime::settlement::{
        transaction_state, MediaTransactionIdentity, MediaTransactionState,
    };
    let pool = pool().await;
    let (ledger, key) = registered(&pool).await;
    let mut tx = pool.begin().await.unwrap();
    let actual = MediaTransactionIdentity::capture_in(&mut tx).await.unwrap();
    let identity = ProducerTransaction {
        producer: "CONCERT".into(),
        system_identifier: actual.system_identifier().into(),
        database_oid: actual.database_oid() as u32,
        full_xid: actual.xid().into(),
        backend_pid: actual.backend_pid(),
        backend_started_unix_micros: actual.backend_started_at().timestamp_micros(),
        effect_binding_digest: key.binding_digest,
        producer_instance: key.instance,
        settlement_reference: b"native-ledger-fixture".to_vec(),
    };
    ledger.register_transaction(&key, &identity).await.unwrap();
    let auth = Uuid::new_v4();
    let friend = Uuid::new_v4();
    ledger
        .record_tracking(&key, &identity, Workload::Auth, auth)
        .await
        .unwrap();
    assert!(ledger
        .confirm_tracking(&key, &identity, auth, friend)
        .await
        .is_err());
    assert!(ledger
        .registration_binding(Workload::Friend, auth, &identity)
        .await
        .is_err());
    assert!(ledger
        .registration_binding(Workload::Auth, Uuid::new_v4(), &identity)
        .await
        .is_err());
    let restored = PgScopeLedger::new(pool.clone(), key.instance).unwrap();
    assert_eq!(
        restored
            .registration_binding(Workload::Auth, auth, &identity)
            .await
            .unwrap()
            .digest()
            .unwrap(),
        key.binding_digest
    );
    let mut altered = identity.clone();
    altered.settlement_reference.push(0);
    assert!(restored
        .registration_binding(Workload::Auth, auth, &altered)
        .await
        .is_err());
    ledger
        .record_tracking(&key, &identity, Workload::Friend, friend)
        .await
        .unwrap();
    ledger
        .confirm_tracking(&key, &identity, auth, friend)
        .await
        .unwrap();
    ledger.bind_delivery(&key, &[6; 32]).await.unwrap();
    ledger.mark_serialized(&key).await.unwrap();
    ledger.seal(&key).await.unwrap();
    assert!(ledger.register_transaction(&key, &identity).await.is_err());
    ledger.finish_root_work(&key).await.unwrap();
    assert_eq!(
        ledger.resolve_scope(&key).await.unwrap(),
        ScopePhase::RootFinished
    );
    assert_eq!(
        transaction_state(&pool, &actual).await,
        MediaTransactionState::InProgress
    );
    tx.rollback().await.unwrap();
    assert_eq!(
        transaction_state(&pool, &actual).await,
        MediaTransactionState::Aborted
    );
    pool.close().await;
}

#[tokio::test]
async fn durable_capacity_does_not_evict_unfinished_or_reopen_sealed_scopes() {
    let pool = pool().await;
    let ledger = PgScopeLedger::new(pool.clone(), Uuid::new_v4()).unwrap();
    let mut first = None;
    for _ in 0..16 {
        let binding = binding();
        let key = ScopeKey {
            instance: ledger.instance(),
            scope: Uuid::new_v4(),
            binding_digest: binding.digest().unwrap(),
        };
        ledger.register(&key, &binding).await.unwrap();
        first.get_or_insert((key, binding));
    }
    let extra = binding();
    let key = ScopeKey {
        instance: ledger.instance(),
        scope: Uuid::new_v4(),
        binding_digest: extra.digest().unwrap(),
    };
    assert_eq!(
        ledger.register(&key, &extra).await.unwrap_err().code(),
        tonic::Code::ResourceExhausted
    );
    let (old, binding) = first.unwrap();
    ledger.seal(&old).await.unwrap();
    assert!(ledger.register(&old, &binding).await.is_err());
    // No protected work/exposed delivery was registered for this fixture scope.
    ledger.finish_root_work(&old).await.unwrap();
    ledger.register(&key, &extra).await.unwrap();
    pool.close().await;
}
