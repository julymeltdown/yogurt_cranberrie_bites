//! Executes the production `RuntimeHooks::succeeded` write order with recording
//! steps. Synthetic steps only: this acquires no owner authority, writes no
//! ledger rows and does not certify a canonical scope.
use super::*;
use sqlx::Connection as _;

#[derive(Default)]
struct Order(std::sync::Mutex<Vec<&'static str>>);
impl Order {
    fn push(&self, step: &'static str) {
        self.0.lock().unwrap().push(step);
    }
    fn seen(&self) -> Vec<&'static str> {
        self.0.lock().unwrap().clone()
    }
}
struct RecordingRetention(Arc<Order>);
#[async_trait::async_trait]
impl SourceRetention for RecordingRetention {
    async fn retain_sources(
        &self,
        _: &mut PgConnection,
        _: SocialReadSources,
        _: Option<DateTime<Utc>>,
    ) -> Result<(), Status> {
        self.0.push("retain");
        Ok(())
    }
}
struct RecordingWitness(Arc<Order>);
#[async_trait::async_trait]
impl SocialTransactionHooks for RecordingWitness {
    async fn register(&self, _: &mut PgConnection) -> Result<(), RelationshipError> {
        Ok(())
    }
    async fn succeeded(
        &self,
        _: &mut PgConnection,
        _: SocialEffect,
    ) -> Result<(), RelationshipError> {
        self.0.push("witness");
        Ok(())
    }
}
struct NoRegistration;
#[async_trait::async_trait]
impl TransactionRegistration for NoRegistration {
    async fn register(&self, _: &mut PgConnection) -> Result<(), RelationshipError> {
        Ok(())
    }
    fn for_projection(&self) -> Arc<dyn TransactionRegistration> {
        Arc::new(NoRegistration)
    }
}
fn hooks(order: &Arc<Order>, retained: Option<SocialReadSources>) -> RuntimeHooks {
    RuntimeHooks {
        witness: Arc::new(RecordingWitness(order.clone())),
        retention: Arc::new(RecordingRetention(order.clone())),
        registration: Arc::new(NoRegistration),
        write_gate: Arc::new(ReadOnlyGate::default()),
        retained,
    }
}
fn effect(projection: bool) -> SocialEffect {
    SocialEffect {
        operation: "listSocialBlocks",
        resource: Uuid::new_v4(),
        revision: 1,
        projection,
    }
}
async fn connection() -> PgConnection {
    let url = std::env::var("CONCERT_TEST_DATABASE_URL").expect("owned database url required");
    PgConnection::connect(&url).await.unwrap()
}

/// The witness insert holds `FOR SHARE` on the root scope row until commit, and
/// child-intent registration takes `FOR UPDATE` on that row. Reversing these two
/// writes in `RuntimeHooks::succeeded` reproduced the production 503; see the
/// lock constraint pinned by `product_scope_ledger`.
#[tokio::test]
#[ignore = "owned native database connection required"]
async fn projecting_writes_retain_the_child_before_the_effect_witness() {
    let mut c = connection().await;
    let order = Arc::new(Order::default());
    hooks(
        &order,
        Some(SocialReadSources::Metadata { subjects: vec![] }),
    )
    .succeeded(&mut c, effect(true))
    .await
    .unwrap();
    assert_eq!(
        order.seen(),
        ["retain", "witness"],
        "a projecting transaction must register its child intent before the witness takes the scope share lock"
    );
}

/// A command's write transaction records only the effect; its child is retained
/// by the separately authorized post-command projection.
#[tokio::test]
#[ignore = "owned native database connection required"]
async fn command_writes_record_only_the_effect_witness() {
    let mut c = connection().await;
    let order = Arc::new(Order::default());
    hooks(
        &order,
        Some(SocialReadSources::Metadata { subjects: vec![] }),
    )
    .succeeded(&mut c, effect(false))
    .await
    .unwrap();
    assert_eq!(order.seen(), ["witness"]);
}
