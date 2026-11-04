//! Actual ledger SQL, not canonical-owner or browser acceptance. Execute only
//! inside the owned fixture created by the product-ledger native runner.
use concert_service::product_rpc::scope_ledger::*;
use platform::{actor_delegation::owner_binding::OwnerBinding, grpc_auth::Workload};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::{
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::PathBuf,
    str::FromStr,
    sync::Arc,
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
struct RejectChildRecovery;
#[async_trait::async_trait]
impl ChildIntentSettler for RejectChildRecovery {
    async fn recover_one(
        &self,
        _: &concert_service::adapters::out::relationship_authority::guarded::ChildRecoveryReference,
    ) -> Result<(), tonic::Status> {
        Err(tonic::Status::unavailable("No child fixture configured"))
    }
}
fn keys() -> Arc<ChildRecoveryKeys> {
    Arc::new(
        ChildRecoveryKeys::new(
            ChildRecoveryKey::new("fixture-at-rest", &[43; 32]).unwrap(),
            vec![],
        )
        .unwrap(),
    )
}
fn new_ledger(pool: &sqlx::PgPool, instance: Uuid) -> PgScopeLedger {
    PgScopeLedger::new(
        pool.clone(),
        instance,
        keys(),
        Arc::new(RejectChildRecovery),
    )
    .unwrap()
}
async fn registered(pool: &sqlx::PgPool) -> (PgScopeLedger, ScopeKey) {
    let ledger = new_ledger(pool, Uuid::new_v4());
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
    let restarted = new_ledger(&pool, key.instance);
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
    let restored = new_ledger(&pool, key.instance);
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
    let ledger = new_ledger(&pool, Uuid::new_v4());
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

// Explicit service double exercises durable orchestration, not Profile authority.
struct RecordChildRecovery(Arc<std::sync::Mutex<Vec<Uuid>>>);
#[async_trait::async_trait]
impl ChildIntentSettler for RecordChildRecovery {
    async fn recover_one(
        &self,
        reference: &concert_service::adapters::out::relationship_authority::guarded::ChildRecoveryReference,
    ) -> Result<(), tonic::Status> {
        self.0.lock().unwrap().push(reference.intent_id()?);
        Ok(())
    }
}
#[tokio::test]
async fn encrypted_predispatch_intent_survives_lost_handles_and_requires_actual_recovery_success() {
    use concert_service::adapters::out::relationship_authority::guarded::{
        ChildQuery, ChildRecoveryReference,
    };
    let pool = pool().await;
    let ledger = new_ledger(&pool, Uuid::new_v4());
    let binding = binding();
    let key = ScopeKey {
        instance: ledger.instance(),
        scope: Uuid::new_v4(),
        binding_digest: binding.digest().unwrap(),
    };
    ledger.register(&key, &binding).await.unwrap();
    let query = ChildQuery {
        actor: binding.actor_id,
        event: binding.event_id,
        subject: Uuid::new_v4(),
        family: binding.session_family_id,
        request: binding.request_id,
        expires_unix_micros: binding.expires_unix_micros,
        binding_digest: key.binding_digest,
    };
    let intent = Uuid::new_v4();
    let secret = Uuid::new_v4().simple().to_string();
    let reference = ChildRecoveryReference::ProfilePublic {
        child_id: query.profile_child_id(intent).unwrap(),
        query,
        completion_secret: secret.clone(),
    };
    ledger
        .register_child_intent(&key, &reference)
        .await
        .unwrap();
    ledger
        .register_child_intent(&key, &reference)
        .await
        .unwrap();
    let sealed:Vec<u8>=sqlx::query_scalar("SELECT sealed_reference FROM concert_product_child_intents WHERE instance_id=$1 AND scope_id=$2")
        .bind(key.instance).bind(key.scope).fetch_one(&pool).await.unwrap();
    assert!(!sealed.windows(secret.len()).any(|w| w == secret.as_bytes()));
    let mut altered = reference.clone();
    if let ChildRecoveryReference::ProfilePublic {
        completion_secret, ..
    } = &mut altered
    {
        *completion_secret = Uuid::new_v4().simple().to_string();
    }
    assert!(ledger.register_child_intent(&key, &altered).await.is_err());
    let mut wrong = reference.clone();
    if let ChildRecoveryReference::ProfilePublic { query, .. } = &mut wrong {
        query.actor = Uuid::new_v4();
    }
    assert!(ledger.register_child_intent(&key, &wrong).await.is_err());
    assert!(ledger
        .record_child_completed(&key, &reference)
        .await
        .is_err());
    ledger.bind_delivery(&key, &[55; 32]).await.unwrap();
    ledger.mark_serialized(&key).await.unwrap();
    ledger.seal(&key).await.unwrap();
    assert!(ledger
        .register_child_intent(&key, &reference)
        .await
        .is_err());
    ledger.finish_root_work(&key).await.unwrap();
    // Memory loss is simulated by rebuilding the adapter from only stable keys.
    let restarted = new_ledger(&pool, key.instance);
    assert!(restarted.settle_child_intents(&key).await.is_err());
    assert!(restarted.record_delivery_completed(&key).await.is_err());
    assert!(!restarted.delivery_completed(&key).await.unwrap());
    let calls = Arc::new(std::sync::Mutex::new(vec![]));
    let recovered = PgScopeLedger::new(
        pool.clone(),
        key.instance,
        keys(),
        Arc::new(RecordChildRecovery(calls.clone())),
    )
    .unwrap();
    recovered.settle_child_intents(&key).await.unwrap();
    recovered.settle_child_intents(&key).await.unwrap();
    assert_eq!(*calls.lock().unwrap(), vec![intent]);
    assert!(recovered
        .record_child_completed(&key, &altered)
        .await
        .is_err());
    recovered.record_delivery_completed(&key).await.unwrap();
    assert!(recovered.delivery_completed(&key).await.unwrap());
    pool.close().await;
}

#[tokio::test]
async fn settlement_requires_exact_owner_registration_and_native_abort_after_lost_track_reply() {
    use concert_service::{
        adapters::out::relationship_authority::guarded::proto,
        media_runtime::settlement::MediaTransactionIdentity,
        product_rpc::settlement::{
            NativeTransactionResolver, PgNativeAbortResolver, ProducerSettlementService,
        },
    };
    use platform::grpc_auth::VerifiedCaller;
    use proto::producer_settlement_service_server::ProducerSettlementService as _;
    fn system<T>(value: T, owner: Workload) -> tonic::Request<T> {
        // Service-level double, not a claim of authenticated mTLS execution.
        let mut request = tonic::Request::new(value);
        request.extensions_mut().insert(VerifiedCaller(owner));
        request
    }
    let pool = pool().await;
    let ledger = new_ledger(&pool, Uuid::new_v4());
    let binding = binding();
    let key = ScopeKey {
        instance: ledger.instance(),
        scope: Uuid::new_v4(),
        binding_digest: binding.digest().unwrap(),
    };
    ledger.register(&key, &binding).await.unwrap();
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
        settlement_reference: b"native-abort-fixture".to_vec(),
    };
    ledger.register_transaction(&key, &identity).await.unwrap();
    let auth = Uuid::new_v4();
    let lost_friend = Uuid::new_v4();
    ledger
        .record_tracking(&key, &identity, Workload::Auth, auth)
        .await
        .unwrap();
    let resolver = Arc::new(PgNativeAbortResolver::new(pool.clone(), key.instance).unwrap());
    let service = ProducerSettlementService::new(ledger.clone(), resolver.clone());
    let wire = proto::TransactionIdentity {
        producer: proto::Producer::Concert as i32,
        system_identifier: identity.system_identifier.clone(),
        database_oid: identity.database_oid,
        full_xid: identity.full_xid.clone(),
        backend_pid: identity.backend_pid,
        backend_started_unix_micros: identity.backend_started_unix_micros,
        effect_binding_digest: identity.effect_binding_digest.to_vec(),
        producer_instance_id: key.instance.to_string(),
        settlement_reference: identity.settlement_reference.clone(),
    };
    let request = proto::ResolveTransactionRequest {
        binding: Some(proto::GuardBinding {
            actor_id: binding.actor_id.to_string(),
            event_id: binding.event_id.to_string(),
            session_family_id: binding.session_family_id.to_string(),
            request_id: binding.request_id.to_string(),
            operation: binding.operation.clone(),
            subject_ids: vec![],
            expires_unix_micros: binding.expires_unix_micros,
            parent_digest: binding.parent_digest.to_vec(),
            parent_body_digest: binding.parent_body_digest.to_vec(),
            parent_method: binding.parent_method.clone(),
            http_method: binding.http_method.clone(),
            http_target: binding.http_target.clone(),
            body_digest: binding.body_digest.to_vec(),
        }),
        transaction: Some(wire),
        registration_id: auth.to_string(),
    };
    assert_eq!(
        service
            .resolve_transaction(system(request.clone(), Workload::Gateway))
            .await
            .unwrap_err()
            .code(),
        tonic::Code::Unauthenticated
    );
    assert_eq!(
        service
            .resolve_transaction(system(request.clone(), Workload::Auth))
            .await
            .unwrap()
            .into_inner()
            .status,
        proto::TransactionStatus::Unknown as i32
    );
    ledger.seal(&key).await.unwrap();
    ledger.finish_root_work(&key).await.unwrap();
    assert_eq!(
        service
            .resolve_transaction(system(request.clone(), Workload::Auth))
            .await
            .unwrap()
            .into_inner()
            .status,
        proto::TransactionStatus::InProgress as i32
    );
    let mut missing_ack = request.clone();
    missing_ack.registration_id = lost_friend.to_string();
    assert_eq!(
        service
            .resolve_transaction(system(missing_ack.clone(), Workload::Friend))
            .await
            .unwrap()
            .into_inner()
            .status,
        proto::TransactionStatus::Unknown as i32
    );
    assert!(service
        .resolve_transaction(system(missing_ack.clone(), Workload::Auth))
        .await
        .is_err());
    let mut altered = request.clone();
    altered
        .transaction
        .as_mut()
        .unwrap()
        .settlement_reference
        .push(0);
    assert!(service
        .resolve_transaction(system(altered, Workload::Auth))
        .await
        .is_err());
    tx.rollback().await.unwrap();
    let settled = service
        .resolve_transaction(system(missing_ack, Workload::Friend))
        .await
        .unwrap();
    assert_eq!(settled.metadata().get("cache-control").unwrap(), "no-store");
    assert_eq!(
        settled.into_inner().status,
        proto::TransactionStatus::Aborted as i32
    );
    let scope = proto::ProducerScope {
        instance_id: key.instance.to_string(),
        scope_id: key.scope.to_string(),
        binding_digest: key.binding_digest.to_vec(),
    };
    assert!(
        service
            .resolve_scope(system(
                proto::ResolveScopeRequest {
                    scope: Some(scope.clone())
                },
                Workload::Auth
            ))
            .await
            .unwrap()
            .into_inner()
            .sealed_and_finished
    );
    let missing = proto::ProducerScope {
        scope_id: Uuid::new_v4().to_string(),
        ..scope
    };
    assert!(
        !service
            .resolve_scope(system(
                proto::ResolveScopeRequest {
                    scope: Some(missing)
                },
                Workload::Friend
            ))
            .await
            .unwrap()
            .into_inner()
            .sealed_and_finished
    );
    // A raw native COMMIT has no same-transaction product witness here.
    let mut tx = pool.begin().await.unwrap();
    let actual = MediaTransactionIdentity::capture_in(&mut tx).await.unwrap();
    let committed = ProducerTransaction {
        full_xid: actual.xid().into(),
        backend_pid: actual.backend_pid(),
        backend_started_unix_micros: actual.backend_started_at().timestamp_micros(),
        ..identity
    };
    tx.commit().await.unwrap();
    assert_eq!(
        resolver.resolve(&binding, &committed).await.unwrap().status,
        proto::TransactionStatus::Unknown
    );
    pool.close().await;
}
