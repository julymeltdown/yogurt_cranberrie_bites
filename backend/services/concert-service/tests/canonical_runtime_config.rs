//! CR07 constructor identity and negative service-boundary checks.
//! Uses closed SQLx pools: no ambient database, native proof or owner success.
use authority::producer_settlement_service_server::ProducerSettlementService;
use concert_service::{
    adapters::out::relationship_authority::guarded::{proto as authority, ChildRecoveryReference},
    config::canonical_runtime::CanonicalRuntimeCore,
    product_rpc::{
        child_recovery::ChildRecoverySettings,
        delivery::DeliveryService,
        proto::{self, product_delivery_service_server::ProductDeliveryService},
        scope_ledger::{ChildIntentSettler, ChildRecoveryKey, ChildRecoveryKeys},
        settlement::PgNativeAbortResolver,
    },
};
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions, PgSslMode},
    PgPool,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tonic::{Code, Request, Status};
use uuid::Uuid;

struct UnavailableChildren(AtomicUsize);
#[async_trait::async_trait]
impl ChildIntentSettler for UnavailableChildren {
    async fn recover_one(&self, _: &ChildRecoveryReference) -> Result<(), Status> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(Status::unavailable("CR07 negative-only child adapter"))
    }
}
fn keys() -> anyhow::Result<Arc<ChildRecoveryKeys>> {
    Ok(Arc::new(ChildRecoveryKeys::new(
        ChildRecoveryKey::new("cr07-current", &[91; 32])?,
        vec![ChildRecoveryKey::new("cr07-retained", &[92; 32])?],
    )?))
}
async fn closed_pool() -> PgPool {
    let options = PgConnectOptions::new()
        .host("127.0.0.1")
        .port(1)
        .username("mcr_cr07")
        .password("unused-closed-pool")
        .database("mcr_concert_cr07")
        .ssl_mode(PgSslMode::Disable);
    let pool = PgPoolOptions::new().connect_lazy_with(options);
    // Every test closes the lazy pool before composing: a bug cannot connect
    // using inherited PG defaults or certify a fake successful database call.
    pool.close().await;
    pool
}

#[tokio::test]
async fn nil_instance_is_rejected_without_database_or_child_dispatch() -> anyhow::Result<()> {
    let children = Arc::new(UnavailableChildren(AtomicUsize::new(0)));
    let result =
        CanonicalRuntimeCore::new(closed_pool().await, Uuid::nil(), keys()?, children.clone());
    let error = result.err().expect("nil producer instance must fail");
    assert_eq!(error.code(), Code::InvalidArgument);
    assert_eq!(children.0.load(Ordering::SeqCst), 0);
    Ok(())
}

#[tokio::test]
async fn feature_clones_share_the_existing_registry_and_instance() -> anyhow::Result<()> {
    let instance = Uuid::new_v4();
    let children = Arc::new(UnavailableChildren(AtomicUsize::new(0)));
    let core = CanonicalRuntimeCore::new(closed_pool().await, instance, keys()?, children.clone())?;
    let participant = core.clone();
    let product = core.clone();
    assert_eq!(participant.ledger().instance(), instance);
    assert_eq!(product.ledger().instance(), instance);
    assert!(Arc::ptr_eq(&participant.registry(), &product.registry()));
    assert!(Arc::ptr_eq(&core.registry(), &participant.registry()));
    // No feature creates an authority grant or new scope merely by composing.
    assert!(participant
        .registry()
        .lookup_root_read(&authority::RootReadView::default())
        .is_err());
    assert!(product
        .registry()
        .lookup_root_read(&authority::RootReadView::default())
        .is_err());
    assert!(core
        .child_recovery_worker(ChildRecoverySettings::default())
        .is_ok());
    assert_eq!(children.0.load(Ordering::SeqCst), 0);
    Ok(())
}

#[tokio::test]
async fn core_factories_preserve_actual_verified_caller_requirements() -> anyhow::Result<()> {
    let pool = closed_pool().await;
    let instance = Uuid::new_v4();
    let children = Arc::new(UnavailableChildren(AtomicUsize::new(0)));
    let core = CanonicalRuntimeCore::new(pool.clone(), instance, keys()?, children.clone())?;
    let service = core.settlement_service(Arc::new(PgNativeAbortResolver::new(pool, instance)?));
    let mut scope = Request::new(authority::ResolveScopeRequest::default());
    scope
        .metadata_mut()
        .insert("x-service-caller", "auth".parse()?);
    scope
        .metadata_mut()
        .insert("x-service-token", "unverified-raw-header".parse()?);
    let error = service
        .resolve_scope(scope)
        .await
        .err()
        .expect("raw metadata is not a verified owner");
    assert_eq!(error.code(), Code::Unauthenticated);
    let mut transaction = Request::new(authority::ResolveTransactionRequest::default());
    transaction
        .metadata_mut()
        .insert("x-service-caller", "friend".parse()?);
    let error = service
        .resolve_transaction(transaction)
        .await
        .err()
        .expect("unverified caller cannot resolve native work");
    assert_eq!(error.code(), Code::Unauthenticated);
    let delivery = DeliveryService::new(core.delivery());
    let mut request = Request::new(proto::ResponseDelivery::default());
    request
        .metadata_mut()
        .insert("x-service-caller", "gateway".parse()?);
    let error = delivery
        .complete_response(request)
        .await
        .err()
        .expect("raw Gateway header cannot complete delivery");
    assert_eq!(error.code(), Code::Unauthenticated);
    assert_eq!(children.0.load(Ordering::SeqCst), 0);
    Ok(())
}
