//! Requires a newly owned native PG16 fixture. Missing isolation evidence is failure, never skip.
use chrono::{DateTime, Utc};
use profile_application::concert_identity::*;
use profile_service::adapters::out::persistence::concert_identity::PgConcertProfileRepository;
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool,
};
use std::{os::unix::fs::MetadataExt, path::PathBuf, str::FromStr, sync::Arc, time::Duration};
use uuid::Uuid;

fn options() -> PgConnectOptions {
    let raw = std::env::var("MCR_PROFILE_OWNER_TEST_DATABASE_URL")
        .expect("explicit owned MCR database is required");
    let opts = PgConnectOptions::from_str(&raw)
        .unwrap_or_else(|_| panic!("invalid owned test connection"));
    let root = PathBuf::from(
        std::env::var("MCR_PROFILE_OWNER_TEST_ROOT").expect("owned fixture root required"),
    );
    let root = root.canonicalize().expect("owned fixture root exists");
    assert!(
        root.to_string_lossy().starts_with("/private/tmp/mcr-pg-")
            || root.to_string_lossy().starts_with("/tmp/mcr-pg-"),
        "dedicated native fixture required"
    );
    let metadata = root.symlink_metadata().expect("owned fixture metadata");
    assert!(!metadata.file_type().is_symlink());
    assert_eq!(
        metadata.dev().to_string(),
        std::env::var("MCR_PROFILE_OWNER_TEST_DEV").expect("owned fixture device")
    );
    assert_eq!(
        metadata.ino().to_string(),
        std::env::var("MCR_PROFILE_OWNER_TEST_INO").expect("owned fixture inode")
    );
    let marker: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("mcr-owner.json")).expect("owned marker"))
            .expect("valid owner marker");
    assert_eq!(marker["uid"].as_u64(), Some(metadata.uid() as u64));
    assert_eq!(
        marker["token"].as_str(),
        Some(
            std::env::var("MCR_PROFILE_OWNER_TEST_TOKEN")
                .expect("owned fixture token")
                .as_str()
        )
    );
    assert_eq!(
        opts.get_socket(),
        Some(&root.join("socket")),
        "exact private socket required"
    );
    assert_eq!(opts.get_port(), 5432);
    assert!(opts.get_options().is_none(), "no hidden connection options");
    assert_eq!(opts.get_username(), "mcr_owner");
    assert_eq!(opts.get_database(), Some("mcr_profile_identity_owner"));
    opts
}
async fn owner_pool() -> PgPool {
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(Duration::from_secs(2))
        .connect_with(options())
        .await
        .unwrap_or_else(|_| panic!("owned PostgreSQL fixture unavailable"));
    let version: i32 = sqlx::query_scalar("select current_setting('server_version_num')::int")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!((160000..170000).contains(&version), "PG16 fixture required");
    pool
}
async fn reader_pool(no_access: bool) -> PgPool {
    PgPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(Duration::from_secs(2))
        .after_connect(move |connection, _| {
            Box::pin(async move {
                let role = if no_access {
                    "set role mcr_profile_identity_noaccess"
                } else {
                    "set role mcr_profile_identity_reader"
                };
                sqlx::query(role).execute(&mut *connection).await?;
                sqlx::query("set default_transaction_read_only = on")
                    .execute(&mut *connection)
                    .await?;
                Ok(())
            })
        })
        .connect_with(options())
        .await
        .unwrap_or_else(|_| panic!("owned reader fixture unavailable"))
}
async fn seed(pool: &PgPool, nickname: &str, bio: &str, private: bool) -> Uuid {
    let user = Uuid::new_v4();
    sqlx::query("insert into profiles (user_id,username,nickname,bio,avatar_url,account_visibility,follower_count,post_count,following_count,delete_reason) values ($1,$2,$3,$4,$5,$6,971,972,973,$7)")
        .bind(user).bind(format!("private-handle-{user}" )).bind(nickname).bind(bio)
        .bind("https://private.invalid/legacy-avatar?token=private-jwt-canary")
        .bind(if private {"PRIVATE"}else{"PUBLIC"}).bind("private-deletion-reason-canary")
        .execute(pool).await.expect("seed owned profile fixture");
    user
}
fn service(pool: PgPool) -> ReadConcertSelfProfile {
    ReadConcertSelfProfile::new(Arc::new(PgConcertProfileRepository::new(pool)))
}
fn query(user: Uuid) -> ConcertProfileQuery {
    ConcertProfileQuery::new(user).unwrap()
}

#[tokio::test]
async fn active_private_self_is_minimal_and_updates_are_uncached() {
    let owner = owner_pool().await;
    let reader = reader_pool(false).await;
    let user = seed(&owner, "Mira", "Own bio", true).await;
    let foreign = seed(&owner, "Foreign", "Foreign bio", false).await;
    // This role lacks SELECT on private columns and every write privilege. Successful reads
    // exercise the actual minimum SQL projection, not a broad DTO filtered after selection.
    let read = service(reader.clone());
    let before: DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
        .fetch_one(&owner)
        .await
        .unwrap();
    let profile = read.execute(query(user)).await.unwrap();
    let after: DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
        .fetch_one(&owner)
        .await
        .unwrap();
    assert_eq!(profile.user_id(), user);
    assert_eq!(profile.display_name(), "Mira");
    assert_eq!(profile.bio(), Some("Own bio"));
    assert!(before <= profile.observed_at() && profile.observed_at() <= after);
    assert_eq!(format!("{profile:?}"), "ConcertSelfProfile([REDACTED])");
    assert_eq!(
        read.execute(query(foreign)).await.unwrap().display_name(),
        "Foreign"
    );
    assert_eq!(
        read.execute(query(user)).await.unwrap().display_name(),
        "Mira"
    );
    sqlx::query("update profiles set nickname='Changed',bio='' where user_id=$1")
        .bind(user)
        .execute(&owner)
        .await
        .unwrap();
    let updated = read.execute(query(user)).await.unwrap();
    assert_eq!(updated.display_name(), "Changed");
    assert_eq!(updated.bio(), None);
    assert!(updated.observed_at() >= profile.observed_at());
    assert!(
        sqlx::query("select username,avatar_url,delete_reason from profiles where user_id=$1")
            .bind(user)
            .fetch_optional(&reader)
            .await
            .is_err(),
        "fixture must deny broad private projection"
    );
    assert!(
        sqlx::query("update profiles set nickname='Forbidden' where user_id=$1")
            .bind(user)
            .execute(&reader)
            .await
            .is_err(),
        "read path role cannot mutate"
    );
    reader.close().await;
    owner.close().await;
}
#[tokio::test]
async fn missing_soft_deleted_and_committed_fence_are_indistinguishable() {
    let owner = owner_pool().await;
    let reader = reader_pool(false).await;
    let read = service(reader.clone());
    let missing = Uuid::new_v4();
    let deleted = seed(&owner, "Deleted", "secret", false).await;
    let fenced = seed(&owner, "Fenced", "secret", true).await;
    let live = seed(&owner, "Live", "visible to self", true).await;
    sqlx::query("update profiles set deleted_at=now() where user_id=$1")
        .bind(deleted)
        .execute(&owner)
        .await
        .unwrap();
    sqlx::query("insert into profile_user_deletion_tombstones(user_id,saga_id,actor_id,retention_until) values($1,$2,$3,now()+interval '1 day')")
        .bind(fenced).bind(Uuid::new_v4()).bind(Uuid::new_v4()).execute(&owner).await.unwrap();
    // A committed saga fence excludes content even before its row cleanup finishes.
    for user in [missing, deleted, fenced] {
        assert_eq!(
            read.execute(query(user)).await,
            Err(ConcertProfileError::ContentUnavailable)
        );
    }
    assert_eq!(
        read.execute(query(live)).await.unwrap().display_name(),
        "Live"
    );
    let error = sqlx::query("update profiles set nickname='Stale' where user_id=$1")
        .bind(fenced)
        .execute(&owner)
        .await
        .unwrap_err();
    assert_eq!(
        error.as_database_error().and_then(|e| e.constraint()),
        Some("profile_user_deletion_fence")
    );
    sqlx::query("update profiles set deleted_at=now() where user_id=$1")
        .bind(fenced)
        .execute(&owner)
        .await
        .expect("owning deletion cleanup remains allowed");
    assert_eq!(
        read.execute(query(fenced)).await,
        Err(ConcertProfileError::ContentUnavailable)
    );
    reader.close().await;
    owner.close().await;
}
#[tokio::test]
async fn malformed_stored_content_is_not_truncated_or_cleaned() {
    let owner = owner_pool().await;
    let reader = reader_pool(false).await;
    let read = service(reader.clone());
    for (name, bio) in [
        (" ".into(), "valid".into()),
        ("a".repeat(41), "valid".into()),
        ("bad\nname".into(), "valid".into()),
        ("Name".into(), "a".repeat(161)),
        ("Name".into(), "raw\nproof".into()),
    ] {
        let user = seed(&owner, &name, &bio, true).await;
        let error = read.execute(query(user)).await.unwrap_err();
        assert_eq!(error, ConcertProfileError::ContentUnavailable);
        assert_eq!(error.to_string(), "Profile content is unavailable");
    }
    let user = seed(&owner, &"🎸".repeat(40), &"가".repeat(160), true).await;
    let profile = read.execute(query(user)).await.unwrap();
    assert_eq!(profile.display_name().chars().count(), 40);
    assert_eq!(profile.bio().unwrap().chars().count(), 160);
    reader.close().await;
    owner.close().await;
}
#[tokio::test]
async fn repository_bounds_transfer_without_truncating_large_stored_values() {
    let owner = owner_pool().await;
    let reader = reader_pool(false).await;
    let repository = PgConcertProfileRepository::new(reader.clone());
    let read = service(reader.clone());
    for (name, bio) in [
        ("a".repeat(41), "valid".into()),
        ("🎸".repeat(41), "valid".into()),
        ("Name".into(), "a".repeat(161)),
        ("Name".into(), "🎸".repeat(161)),
        ("n".repeat(2 * 1024 * 1024), "valid".into()),
        ("Name".into(), "b".repeat(2 * 1024 * 1024)),
    ] {
        let user = seed(&owner, &name, &bio, true).await;
        // Assert at the repository boundary, before application length checks can hide
        // unbounded SQL transfer. Invalid source content must not become a truncated row.
        assert!(repository
            .find_self_content(query(user))
            .await
            .unwrap()
            .is_none());
        assert_eq!(
            read.execute(query(user)).await,
            Err(ConcertProfileError::ContentUnavailable)
        );
        let lengths: (i32, i32) = sqlx::query_as(
            "select octet_length(nickname), octet_length(bio) from profiles where user_id=$1",
        )
        .bind(user)
        .fetch_one(&owner)
        .await
        .unwrap();
        assert_eq!(lengths, (name.len() as i32, bio.len() as i32));
    }
    // Both maxima use four-byte scalars; the byte predicates must not exclude valid UTF-8.
    let name = "🎸".repeat(40);
    let bio = "🎵".repeat(160);
    let user = seed(&owner, &name, &bio, true).await;
    let record = repository
        .find_self_content(query(user))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.nickname, name);
    assert_eq!(record.bio, bio);
    let profile = read.execute(query(user)).await.unwrap();
    assert_eq!(profile.display_name(), name);
    assert_eq!(profile.bio(), Some(bio.as_str()));
    reader.close().await;
    owner.close().await;
}
#[tokio::test]
async fn statement_snapshot_preserves_atomic_content_and_deletion_visibility() {
    let owner = owner_pool().await;
    let reader = reader_pool(false).await;
    let read = service(reader.clone());
    let user = seed(&owner, "Before", "before", true).await;
    let mut writer = owner.begin().await.unwrap();
    sqlx::query("update profiles set nickname='After',bio='after' where user_id=$1")
        .bind(user)
        .execute(&mut *writer)
        .await
        .unwrap();
    let before = read.execute(query(user)).await.unwrap();
    assert_eq!(
        (before.display_name(), before.bio()),
        ("Before", Some("before"))
    );
    writer.commit().await.unwrap();
    let after = read.execute(query(user)).await.unwrap();
    assert_eq!(
        (after.display_name(), after.bio()),
        ("After", Some("after"))
    );
    let mut deletion = owner.begin().await.unwrap();
    sqlx::query("select profile_lock_subject_ids($1::uuid[])")
        .bind(vec![user])
        .execute(&mut *deletion)
        .await
        .unwrap();
    sqlx::query("insert into profile_user_deletion_tombstones(user_id,saga_id,actor_id,retention_until) values($1,$2,$3,now()+interval '1 day')")
        .bind(user).bind(Uuid::new_v4()).bind(Uuid::new_v4()).execute(&mut *deletion).await.unwrap();
    assert_eq!(
        read.execute(query(user)).await.unwrap().display_name(),
        "After"
    );
    deletion.commit().await.unwrap();
    assert_eq!(
        read.execute(query(user)).await,
        Err(ConcertProfileError::ContentUnavailable)
    );
    reader.close().await;
    owner.close().await;
}
#[tokio::test]
async fn real_database_permission_failure_and_closed_pool_are_safe() {
    let reader = reader_pool(true).await;
    let read = service(reader.clone());
    let error = read.execute(query(Uuid::new_v4())).await.unwrap_err();
    assert_eq!(error, ConcertProfileError::DependencyUnavailable);
    assert_eq!(error.to_string(), "Profile owner is unavailable");
    reader.close().await;
    assert_eq!(
        read.execute(query(Uuid::new_v4())).await,
        Err(ConcertProfileError::DependencyUnavailable)
    );
}
#[tokio::test]
async fn owner_database_unavailable_is_not_cached_content() {
    // Harness invokes ONLY this test after stopping its owned server. Connecting lazily
    // ensures the failure is the real repository acquisition path, with no default fallback.
    let pool = PgPoolOptions::new()
        .acquire_timeout(Duration::from_millis(600))
        .connect_lazy_with(options());
    assert_eq!(
        service(pool.clone()).execute(query(Uuid::new_v4())).await,
        Err(ConcertProfileError::DependencyUnavailable)
    );
    pool.close().await;
}
