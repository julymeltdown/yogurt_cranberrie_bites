mod support;

use auth_service::adapters::out::persistence::user_repository::PgUserRepository;
use auth_service::application::ports::UserRepositoryPort;
use auth_service::domain::user::{User, UserStatus};
use chrono::{Duration, Utc};
use uuid::Uuid;

fn user(status: UserStatus) -> User {
    let id = Uuid::new_v4();
    User {
        id,
        primary_email: format!("{id}@example.test"),
        password_hash: "original-test-hash".into(),
        status,
        last_login_at: Some(Utc::now()),
    }
}

#[tokio::test]
async fn stale_login_snapshot_cannot_restore_a_reset_password() -> anyhow::Result<()> {
    let app = support::start_postgres_app(false).await?;
    let repository = PgUserRepository::new(
        app.pool.clone(),
        std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
    );
    let snapshot = user(UserStatus::Active);
    repository.save(&(&snapshot).into()).await?;
    repository
        .update_password_hash(snapshot.id, "replacement-test-hash")
        .await?;

    assert!(repository
        .save(&(&snapshot.with_last_login(Utc::now())).into())
        .await
        .is_err());
    let current = repository
        .find_by_id(snapshot.id)
        .await?
        .map(auth_service::domain::user::User::from)
        .unwrap();
    assert_eq!(current.password_hash, "replacement-test-hash");
    assert_eq!(current.status, UserStatus::Active);
    Ok(())
}

#[tokio::test]
async fn stale_login_snapshot_cannot_reactivate_a_deleted_user() -> anyhow::Result<()> {
    let app = support::start_postgres_app(false).await?;
    let repository = PgUserRepository::new(
        app.pool.clone(),
        std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
    );
    let snapshot = user(UserStatus::Active);
    repository.save(&(&snapshot).into()).await?;
    assert!(
        repository
            .soft_delete_user(
                snapshot.id,
                snapshot.id,
                "test deletion",
                Utc::now() + Duration::days(30),
            )
            .await?
    );

    assert!(repository
        .save(&(&snapshot.with_last_login(Utc::now())).into())
        .await
        .is_err());
    assert_eq!(
        repository
            .find_by_id(snapshot.id)
            .await?
            .map(auth_service::domain::user::User::from)
            .unwrap()
            .status,
        UserStatus::Deleted
    );
    let deleted: bool =
        sqlx::query_scalar("select deleted_at is not null from users where id = $1")
            .bind(snapshot.id)
            .fetch_one(&app.pool)
            .await?;
    assert!(deleted);
    Ok(())
}

#[tokio::test]
async fn pending_accounts_can_still_be_activated() -> anyhow::Result<()> {
    let app = support::start_postgres_app(false).await?;
    let repository = PgUserRepository::new(
        app.pool.clone(),
        std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
    );
    let pending = user(UserStatus::PendingVerification);
    repository.save(&(&pending).into()).await?;
    repository
        .save(&(&pending.with_status(UserStatus::Active)).into())
        .await?;
    assert_eq!(
        repository
            .find_by_id(pending.id)
            .await?
            .map(auth_service::domain::user::User::from)
            .unwrap()
            .status,
        UserStatus::Active
    );
    Ok(())
}

#[tokio::test]
async fn login_timestamps_cannot_move_backwards() -> anyhow::Result<()> {
    let app = support::start_postgres_app(false).await?;
    let repository = PgUserRepository::new(
        app.pool.clone(),
        std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
    );
    let original = user(UserStatus::Active);
    repository.save(&(&original).into()).await?;
    repository
        .save(&(&original.with_last_login(Utc::now() + Duration::hours(1))).into())
        .await?;
    let latest = repository
        .find_by_id(original.id)
        .await?
        .map(auth_service::domain::user::User::from)
        .unwrap()
        .last_login_at;
    repository.save(&(&original).into()).await?;
    assert_eq!(
        repository
            .find_by_id(original.id)
            .await?
            .map(auth_service::domain::user::User::from)
            .unwrap()
            .last_login_at,
        latest
    );
    Ok(())
}

#[tokio::test]
async fn verified_oauth_can_fill_an_empty_primary_email() -> anyhow::Result<()> {
    let app = support::start_postgres_app(false).await?;
    let repository = PgUserRepository::new(
        app.pool.clone(),
        std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
    );
    let mut snapshot = user(UserStatus::Active);
    snapshot.primary_email.clear();
    repository.save(&(&snapshot).into()).await?;
    snapshot.primary_email = "verified@example.test".into();
    repository.save(&(&snapshot).into()).await?;
    assert_eq!(
        repository
            .find_by_id(snapshot.id)
            .await?
            .map(auth_service::domain::user::User::from)
            .unwrap()
            .primary_email,
        "verified@example.test",
    );
    Ok(())
}
