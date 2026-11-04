//! Denial-boundary checks. These do not certify a real moderation producer.
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::{
    admin::{
        moderation::{Change, OwnerStep, UserAction},
        Actor, AuthorityQuery, RoleLease,
    },
    commands::CommandError,
};
use concert_service::{
    config::social_policy_runtime::SocialWorldPolicy,
    moderation_live::ports::{ModerationAuthority, OwnerIntent, TargetRoleLease},
};
use sqlx::PgConnection;
use uuid::Uuid;

struct SnapshotOnly;
#[async_trait]
impl ModerationAuthority for SnapshotOnly {
    async fn check_in(
        &self,
        _: &mut PgConnection,
        _: AuthorityQuery,
        _: DateTime<Utc>,
    ) -> Result<RoleLease, CommandError> {
        panic!("a snapshot must be refused before SQL/role I/O")
    }
    async fn target_role_in(
        &self,
        _: &mut PgConnection,
        _: Actor,
        _: Uuid,
        _: DateTime<Utc>,
    ) -> Result<TargetRoleLease, CommandError> {
        panic!("no acquired Root scope")
    }
    async fn check_disclosure(&self) -> Result<(), CommandError> {
        panic!("no acquired Root scope")
    }
}
fn input() -> (OwnerIntent, Change) {
    let target = Uuid::new_v4();
    (
        OwnerIntent {
            receipt: Uuid::new_v4(),
            actor: Actor {
                actor: Uuid::new_v4(),
                event: Uuid::new_v4(),
            },
            target,
            operation: "moderateConcertUser".into(),
            step: OwnerStep::EventPrivateAuthorityRevocation,
            expected_revision: 1,
        },
        Change::User {
            target,
            expected_revision: 1,
            action: UserAction::Suspend,
            expires_at: None,
            reason: "Boundary fixture".into(),
        },
    )
}
#[tokio::test]
async fn snapshot_role_cannot_start_or_recover_a_social_revocation() {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgresql://localhost/mcr_concert_unused")
        .unwrap();
    let factory = SocialWorldPolicy::compose(pool);
    let leaf = factory.moderation_revocation();
    let (intent, change) = input();
    assert!(matches!(
        leaf.apply(&SnapshotOnly, &intent, &change).await,
        Err(CommandError::Unavailable)
    ));
    assert!(matches!(
        leaf.recover(&SnapshotOnly, &intent, &change).await,
        Err(CommandError::Unavailable)
    ));
}
#[tokio::test]
async fn unsuspend_or_a_different_step_never_revokes_or_restores_social_authority() {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgresql://localhost/mcr_concert_unused")
        .unwrap();
    let leaf = SocialWorldPolicy::compose(pool).moderation_revocation();
    let (mut intent, mut change) = input();
    if let Change::User { action, .. } = &mut change {
        *action = UserAction::Unsuspend;
    }
    assert!(matches!(
        leaf.apply(&SnapshotOnly, &intent, &change).await,
        Err(CommandError::InvalidInput)
    ));
    let (_, suspend) = input();
    intent.target = suspend.target();
    intent.step = OwnerStep::EventRestriction;
    assert!(matches!(
        leaf.apply(&SnapshotOnly, &intent, &suspend).await,
        Err(CommandError::InvalidInput)
    ));
}

#[tokio::test]
async fn another_namespace_or_request_never_reaches_native_storage() {
    use concert_service::{
        adapters::out::relationship_authority::guarded::proto::TransactionStatus,
        product_rpc::{scope_ledger::ProducerTransaction, settlement::NativeTransactionResolver},
        social_live::moderation_revocation::SocialModerationNativeResolver,
    };
    use platform::actor_delegation::owner_binding::OwnerBinding;
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgresql://localhost/mcr_concert_unused")
        .unwrap();
    let instance = Uuid::new_v4();
    let resolver = SocialModerationNativeResolver::new(pool, instance).unwrap();
    let event = Uuid::new_v4();
    let binding = OwnerBinding {
        actor_id: Uuid::new_v4(),
        event_id: event,
        session_family_id: Uuid::new_v4(),
        request_id: Uuid::new_v4(),
        operation: "moderateConcertUser".into(),
        expires_unix_micros: 1,
        parent_digest: [1; 32],
        parent_body_digest: [2; 32],
        body_digest: [3; 32],
        parent_method: "/concert.product.v1.ProductService/Dispatch".into(),
        http_method: "POST".into(),
        http_target: format!(
            "/api/concert/events/{event}/admin/users/{}/moderation",
            Uuid::new_v4()
        ),
    };
    let exact = serde_json::json!({"kind":"MODERATION_PRIVATE_REVOCATION_V1","owner_receipt":Uuid::new_v4(),"command_receipt":Uuid::new_v4(),"intent_digest":"01".repeat(32),"source_revision":2,"scope_id":Uuid::new_v4(),"request_id":binding.request_id});
    let mut wrong_namespace = exact.clone();
    wrong_namespace["kind"] = serde_json::json!("SHARING");
    let mut extra = exact.clone();
    extra["phase"] = serde_json::json!("COMPLETE");
    let mut wrong_request = exact;
    wrong_request["request_id"] = serde_json::json!(Uuid::new_v4());
    for reference in [wrong_namespace, extra, wrong_request] {
        // Syntactically valid identity solely for early refusal. Nothing is
        // registered, stored, executed or claimed as an actual producer effect.
        let identity = ProducerTransaction {
            producer: "CONCERT".into(),
            system_identifier: "1".into(),
            database_oid: 1,
            full_xid: "1".into(),
            backend_pid: 1,
            backend_started_unix_micros: 1,
            effect_binding_digest: [1; 32],
            producer_instance: instance,
            settlement_reference: serde_json::to_vec(&reference).unwrap(),
        };
        let result = resolver.resolve(&binding, &identity).await.unwrap();
        assert_eq!(result.status, TransactionStatus::Unknown);
        assert!(result.receipt_evidence.is_empty());
    }
}
