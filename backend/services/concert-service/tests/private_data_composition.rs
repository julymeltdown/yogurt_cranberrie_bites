//! Composition negatives only; actual seven-category qualification belongs to
//! the domain owners/coordinator and remains a separate native runtime gate.
use async_trait::async_trait;
use concert_service::{
    account_private_data::{
        PrivateDataCategory, PrivateDataCategoryOwner, PrivateDataCategoryReceipt,
    },
    config::{
        account_deletion_runtime::{
            AccountDeletionRuntime, AccountDeletionSettings, AuthDeletionCallers,
            LiveDeletionOwners,
        },
        private_data_composition::PrivateDataComposition,
    },
    world::account_deletion::{
        DeletionAuthority, DeletionFailure, VerifiedDeletionSource, WorldDeletionCommand,
        WorldDeletionTarget,
    },
};
use std::sync::{Arc, Mutex};
use tokio::time::Instant;
use uuid::Uuid;

struct Refuse;
#[async_trait]
impl PrivateDataCategoryOwner for Refuse {
    async fn apply_category(
        &self,
        _: &WorldDeletionCommand,
        _: &VerifiedDeletionSource,
        _: Instant,
    ) -> Result<PrivateDataCategoryReceipt, DeletionFailure> {
        Err(DeletionFailure::Unavailable)
    }
}
#[test]
fn closed_registration_preserves_core_and_rejects_duplicate_domain() {
    let composition = PrivateDataComposition::awaiting_domain_providers();
    assert_eq!(composition.configured_categories(), 1);
    assert!(composition
        .register(
            PrivateDataCategory::CorePreferencesAndAvatar,
            Arc::new(Refuse)
        )
        .is_err());
    let composition = PrivateDataComposition::awaiting_domain_providers()
        .register(PrivateDataCategory::IntroText, Arc::new(Refuse))
        .unwrap();
    assert_eq!(composition.configured_categories(), 2);
    assert!(composition
        .register(PrivateDataCategory::IntroText, Arc::new(Refuse))
        .is_err());
}
struct RecordingDeny(Arc<Mutex<Option<(WorldDeletionCommand, Instant)>>>);
#[async_trait]
impl DeletionAuthority for RecordingDeny {
    async fn verify(
        &self,
        command: &WorldDeletionCommand,
        deadline: Instant,
    ) -> Result<VerifiedDeletionSource, DeletionFailure> {
        *self.0.lock().unwrap() = Some((command.clone(), deadline));
        Err(DeletionFailure::Unavailable)
    }
}
#[tokio::test]
async fn actual_coordinator_receives_exact_command_source_and_original_deadline(
) -> anyhow::Result<()> {
    let seen = Arc::new(Mutex::new(None));
    let authority = Arc::new(RecordingDeny(seen.clone()));
    let pool =
        sqlx::postgres::PgPoolOptions::new().connect_lazy("postgres://unused/mcr_composition")?;
    let coordinator =
        PrivateDataComposition::awaiting_domain_providers().compose(pool.clone(), authority)?;
    let command = WorldDeletionCommand {
        saga_id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        actor_id: Uuid::new_v4(),
        target: WorldDeletionTarget::PrivateData,
        retention_until: chrono::DateTime::from_timestamp(1_793_815_000, 777_888_999).unwrap(),
    };
    let deadline = Instant::now() + std::time::Duration::from_secs(3);
    assert!(matches!(
        coordinator.apply(command.clone(), deadline).await,
        Err(DeletionFailure::Unavailable)
    ));
    let observed = seen.lock().unwrap().take().unwrap();
    assert!(observed.0 == command);
    assert_eq!(
        observed.0.retention_until.timestamp_subsec_nanos(),
        777_888_999
    );
    assert_eq!(observed.1, deadline);
    // The rejecting canonical source prevents any database effect or positive
    // receipt. No fixture source/category success is manufactured by this test.
    pool.close().await;
    Ok(())
}
#[tokio::test]
async fn off_does_not_construct_any_private_data_authority_or_owner() -> anyhow::Result<()> {
    let pool =
        sqlx::postgres::PgPoolOptions::new().connect_lazy("postgres://unused/mcr_composition")?;
    let result = AccountDeletionRuntime::compose_with_private_data(
        AccountDeletionSettings::Off,
        pool.clone(),
        "",
        None,
        "",
        AuthDeletionCallers::from_credentials("composition-test-auth", None)?,
        LiveDeletionOwners::default(),
        |_, _| panic!("off must not construct a provider or outbound authority"),
    )?;
    assert!(result.is_none());
    pool.close().await;
    Ok(())
}
