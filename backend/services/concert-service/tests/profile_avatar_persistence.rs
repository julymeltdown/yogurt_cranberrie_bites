//! WP-08 T-702: profile-owned avatar persistence against real PostgreSQL 16.
//!
//! The database is the runner-owned private `mcr-pg-*` cluster created by
//! `backend/scripts/mcr/wp08_avatar_cluster.py` (the
//! `migration_smoke.py` harness pattern). The URL arrives through
//! `CONCERT_TEST_DATABASE_URL` and must name the `mcr_owner` role.
//!
//! Every case below is one of the hard constraints of the work package:
//!
//! * a non-owner write changes nothing (`403`);
//! * concurrent writes at the same revision produce one success and one
//!   conflict;
//! * a revision conflict returns the current config and writes nothing;
//! * the stored row has no event or participant column;
//! * a stored config the catalog no longer admits fails closed, with no
//!   fallback character substituted.
use concert_application::bootstrap::{
    BootstrapDefaults, BootstrapRelease, ValidatedProductRelease,
};
use concert_domain::profile_avatar::{config_value, parse_config, ProfileId};
use concert_domain::{AvatarSelection, Revision, Theme};
use concert_service::profile_avatar::{
    http_status, ProfileAvatarActor, ProfileAvatarError, ProfileAvatarPersistence,
};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Fixture
// ---------------------------------------------------------------------------

async fn pool() -> anyhow::Result<PgPool> {
    let url = std::env::var("CONCERT_TEST_DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("CONCERT_TEST_DATABASE_URL is required"))?;
    let options: sqlx::postgres::PgConnectOptions = url.parse()?;
    anyhow::ensure!(
        options.get_username() == "mcr_owner",
        "fixture must use the owned cluster"
    );
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(8)
        .connect_with(options)
        .await?;
    let applied: bool =
        sqlx::query_scalar("select to_regclass('public.profile_avatars') is not null")
            .fetch_one(&pool)
            .await?;
    anyhow::ensure!(applied, "migration 0090_profile_avatars is not applied");
    Ok(pool)
}

/// The release fixture: the frozen C03 bootstrap document with its avatar
/// manifest, projected into a validated release exactly as the concert avatar
/// test does. `synthetic` is the explicit metadata-test policy.
fn release(selection: AvatarSelection) -> Arc<ValidatedProductRelease> {
    let document: Value = serde_json::from_str(include_str!(
        "../../../../contracts/fixtures/core/bootstrap-open.json"
    ))
    .expect("frozen bootstrap fixture");
    let defaults = BootstrapDefaults {
        theme: Theme::Revenge,
        avatar: selection,
    };
    Arc::new(
        ValidatedProductRelease::from_validated(
            BootstrapRelease::new(defaults, serde_json::to_vec(&document).unwrap()).unwrap(),
            "a".repeat(64),
        )
        .unwrap(),
    )
}

fn default_selection() -> AvatarSelection {
    let document: Value = serde_json::from_str(include_str!(
        "../../../../contracts/fixtures/core/bootstrap-open.json"
    ))
    .unwrap();
    parse_config(&serde_json::to_vec(&document["avatar"]["config"]).unwrap()).unwrap()
}

fn persistence(pool: PgPool) -> ProfileAvatarPersistence {
    ProfileAvatarPersistence::from_synthetic_test(pool, release(default_selection()), true)
        .expect("fixture release must project")
}

fn actor(profile: Uuid) -> ProfileAvatarActor {
    ProfileAvatarActor::new(ProfileId::from_uuid(profile).unwrap())
}

fn command(expected: i64, config: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({"expected_revision": expected, "config": config})).unwrap()
}

/// A second admitted config: the layered selection with no accessory.
fn alternate() -> Value {
    let mut config = config_value(&default_selection());
    config["item_ids"]["accessory"] = Value::Null;
    config
}

async fn stored(pool: &PgPool, profile: Uuid) -> Option<(i64, Value)> {
    sqlx::query_as("select revision, config from public.profile_avatars where owner_profile_id=$1")
        .bind(profile)
        .fetch_optional(pool)
        .await
        .unwrap()
}

// ---------------------------------------------------------------------------
// Read
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_profile_that_never_saved_reads_the_release_default_at_revision_one(
) -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = persistence(pool.clone());
    let profile = Uuid::new_v4();
    let state = store.read(actor(profile)).await.unwrap();
    assert_eq!(state.revision, Revision::initial());
    assert_eq!(state.selection, default_selection());
    assert_eq!(state.to_wire(), json!({"revision": 1, "config": config_value(&default_selection())}));
    // A read never creates a row: it cannot authorize a write.
    assert_eq!(stored(&pool, profile).await, None);
    Ok(())
}

// ---------------------------------------------------------------------------
// Write and reload
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_save_advances_the_revision_once_and_reloads_the_same_config(
) -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = persistence(pool.clone());
    let profile = Uuid::new_v4();
    let saved = store
        .set_avatar(actor(profile), &command(1, alternate()))
        .await
        .unwrap();
    assert_eq!(saved.revision, Revision::new(2).unwrap());
    assert_eq!(saved.to_wire(), json!({"revision": 2, "config": alternate()}));
    let (revision, config) = stored(&pool, profile).await.unwrap();
    assert_eq!(revision, 2);
    assert_eq!(config, alternate());
    // The reload is the same configuration the save returned.
    assert_eq!(store.read(actor(profile)).await.unwrap(), saved);
    Ok(())
}

#[tokio::test]
async fn a_revision_conflict_writes_nothing_and_keeps_the_current_config(
) -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = persistence(pool.clone());
    let profile = Uuid::new_v4();
    store
        .set_avatar(actor(profile), &command(1, alternate()))
        .await
        .unwrap();
    let before = stored(&pool, profile).await.unwrap();
    // Revision 1 is stale now; the current revision is 2.
    assert_eq!(
        store.set_avatar(actor(profile), &command(1, alternate())).await,
        Err(ProfileAvatarError::Conflict)
    );
    assert_eq!(stored(&pool, profile).await, Some(before.clone()));
    // A conflict is a 409 with the current config available on re-read.
    assert_eq!(http_status(ProfileAvatarError::Conflict), 409);
    let current = store.read(actor(profile)).await.unwrap();
    assert_eq!(current.revision, Revision::new(2).unwrap());
    assert_eq!(current.to_wire()["config"], before.1);
    Ok(())
}

#[tokio::test]
async fn concurrent_writes_at_the_same_revision_are_one_success_and_one_conflict(
) -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = Arc::new(persistence(pool.clone()));
    let profile = Uuid::new_v4();
    // Both writers believe the current revision is 1.
    let first = Arc::clone(&store);
    let second = Arc::clone(&store);
    let (left, right) = tokio::join!(
        async move { first.set_avatar(actor(profile), &command(1, alternate())).await },
        async move { second.set_avatar(actor(profile), &command(1, alternate())).await },
    );
    let outcomes = [left, right];
    let successes = outcomes
        .iter()
        .filter(|outcome| matches!(outcome, Ok(state) if state.revision == Revision::new(2).unwrap()))
        .count();
    let conflicts = outcomes
        .iter()
        .filter(|outcome| **outcome == Err(ProfileAvatarError::Conflict))
        .count();
    assert_eq!(
        (successes, conflicts),
        (1, 1),
        "one success and one conflict: {outcomes:?}"
    );
    // Exactly one commit landed.
    let (revision, config) = stored(&pool, profile).await.unwrap();
    assert_eq!(revision, 2);
    assert_eq!(config, alternate());
    Ok(())
}

#[tokio::test]
async fn many_concurrent_writers_at_one_revision_commit_exactly_once() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = Arc::new(persistence(pool.clone()));
    let profile = Uuid::new_v4();
    let mut tasks = Vec::new();
    for _ in 0..8 {
        let store = Arc::clone(&store);
        tasks.push(tokio::spawn(async move {
            store
                .set_avatar(actor(profile), &command(1, alternate()))
                .await
        }));
    }
    let mut successes = 0;
    let mut conflicts = 0;
    for task in tasks {
        match task.await.unwrap() {
            Ok(state) => {
                assert_eq!(state.revision, Revision::new(2).unwrap());
                successes += 1;
            }
            Err(ProfileAvatarError::Conflict) => conflicts += 1,
            Err(other) => panic!("unexpected persistence error: {other:?}"),
        }
    }
    assert_eq!((successes, conflicts), (1, 7));
    assert_eq!(stored(&pool, profile).await.unwrap().0, 2);
    Ok(())
}

// ---------------------------------------------------------------------------
// Ownership: a non-owner write changes nothing
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_non_owner_write_cannot_reach_another_profiles_avatar() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = persistence(pool.clone());
    let owner = Uuid::new_v4();
    let stranger = Uuid::new_v4();
    let owned = store
        .set_avatar(actor(owner), &command(1, alternate()))
        .await
        .unwrap();
    let before = stored(&pool, owner).await.unwrap();
    // The stranger's own write, at the owner's revision, lands on the
    // stranger's row and leaves the owner's avatar byte-identical.
    let stranger_state = store
        .set_avatar(actor(stranger), &command(1, alternate()))
        .await
        .unwrap();
    assert_eq!(stranger_state.profile, actor(stranger).owner);
    assert_eq!(store.read(actor(owner)).await.unwrap(), owned);
    assert_eq!(stored(&pool, owner).await, Some(before.clone()));
    // The stranger cannot even express a write to the owner's row: the only
    // owner a request can bind is the authenticated one, and the trigger
    // refuses re-pointing a stored avatar at another profile.
    let repointed = sqlx::query(
        "update public.profile_avatars set owner_profile_id=$2, revision=revision+1 \
         where owner_profile_id=$1",
    )
    .bind(owner)
    .bind(stranger)
    .execute(&pool)
    .await;
    let error = repointed.expect_err("owner re-point must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("profile_avatars_owner_immutable")
    );
    assert_eq!(stored(&pool, owner).await, Some(before));
    Ok(())
}

#[tokio::test]
async fn the_http_projection_of_a_denied_write_is_403() {
    // The persistence has no "denied" branch for a caller-supplied owner
    // because it has no such parameter. The route layer decides ownership from
    // the verified session, and its refusal is the frozen 403.
    assert_eq!(http_status(ProfileAvatarError::Denied), 403);
    assert_eq!(http_status(ProfileAvatarError::Unauthenticated), 401);
}

// ---------------------------------------------------------------------------
// Schema: no event, no participant
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_profile_avatar_table_has_no_event_or_participant_column(
) -> anyhow::Result<()> {
    let pool = pool().await?;
    let columns: Vec<String> = sqlx::query_scalar(
        "select column_name::text from information_schema.columns \
         where table_schema='public' and table_name='profile_avatars' order by column_name",
    )
    .fetch_all(&pool)
    .await?;
    assert_eq!(
        columns,
        vec![
            "config".to_owned(),
            "created_at".to_owned(),
            "owner_profile_id".to_owned(),
            "revision".to_owned(),
            "updated_at".to_owned(),
        ]
    );
    for forbidden in ["event_id", "user_id", "participant_id"] {
        assert!(!columns.iter().any(|column| column == forbidden));
    }
    Ok(())
}

#[tokio::test]
async fn the_profile_avatar_table_refuses_a_non_object_config_and_a_nil_owner(
) -> anyhow::Result<()> {
    let pool = pool().await?;
    let error = sqlx::query("insert into profile_avatars(owner_profile_id, config) values($1, '[]')")
        .bind(Uuid::new_v4())
        .execute(&pool)
        .await
        .expect_err("array config must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("profile_avatars_config_object")
    );
    let error = sqlx::query("insert into profile_avatars(owner_profile_id, config) values($1, '{}')")
        .bind(Uuid::nil())
        .execute(&pool)
        .await
        .expect_err("nil owner must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("profile_avatars_owner_profile_id_check")
    );
    let error = sqlx::query("insert into profile_avatars(owner_profile_id, config, revision) values($1, '{}', 0)")
        .bind(Uuid::new_v4())
        .execute(&pool)
        .await
        .expect_err("revision 0 must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("profile_avatars_revision")
    );
    Ok(())
}

#[tokio::test]
async fn the_avatar_revision_must_advance_exactly_once() -> anyhow::Result<()> {
    let pool = pool().await?;
    let profile = Uuid::new_v4();
    sqlx::query("insert into profile_avatars(owner_profile_id, config) values($1, '{}')")
        .bind(profile)
        .execute(&pool)
        .await?;
    let error = sqlx::query("update profile_avatars set revision=5 where owner_profile_id=$1")
        .bind(profile)
        .execute(&pool)
        .await
        .expect_err("a skipped revision must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("profile_avatars_revision_step")
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Fail closed: no fallback character
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_stored_config_the_catalog_no_longer_admits_fails_closed(
) -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = persistence(pool.clone());
    let profile = Uuid::new_v4();
    // A row written outside the catalog: a valid closed object that is not an
    // admitted avatar config.
    sqlx::query("insert into profile_avatars(owner_profile_id, config) values($1, $2)")
        .bind(profile)
        .bind(json!({"contract_version": "1.0.0", "kind": "avatar_config"}))
        .execute(&pool)
        .await?;
    assert_eq!(
        store.read(actor(profile)).await,
        Err(ProfileAvatarError::InvalidState)
    );
    // It is reported as an error, never replaced by the release default.
    assert_eq!(http_status(ProfileAvatarError::InvalidState), 400);
    Ok(())
}

#[tokio::test]
async fn an_unadmitted_config_is_refused_before_any_write() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = persistence(pool.clone());
    let profile = Uuid::new_v4();
    let mut unknown = config_value(&default_selection());
    unknown["item_ids"]["hair"] = json!("not_in_the_manifest");
    assert_eq!(
        store.set_avatar(actor(profile), &command(1, unknown)).await,
        Err(ProfileAvatarError::Invalid)
    );
    assert_eq!(stored(&pool, profile).await, None);
    // A body that is not a closed SetAvatarRequest at all.
    assert_eq!(
        store.set_avatar(actor(profile), b"{}").await,
        Err(ProfileAvatarError::Invalid)
    );
    assert_eq!(stored(&pool, profile).await, None);
    Ok(())
}

// ---------------------------------------------------------------------------
// Product construction refuses a synthetic release
// ---------------------------------------------------------------------------

#[tokio::test]
async fn product_construction_refuses_a_fixture_release() -> anyhow::Result<()> {
    let pool = pool().await?;
    assert_eq!(
        ProfileAvatarPersistence::from_product(pool.clone(), release(default_selection())).err(),
        Some(ProfileAvatarError::Unavailable)
    );
    assert_eq!(
        ProfileAvatarPersistence::from_synthetic_test(pool, release(default_selection()), false)
            .err(),
        Some(ProfileAvatarError::Unavailable)
    );
    Ok(())
}

fn constraint(error: &sqlx::Error) -> Option<String> {
    match error {
        sqlx::Error::Database(database) => database.constraint().map(str::to_owned),
        _ => None,
    }
}
