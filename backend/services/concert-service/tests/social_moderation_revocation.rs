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
