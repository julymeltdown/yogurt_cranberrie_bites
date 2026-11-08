//! WP-08 T-703: the two profile avatar routes.
//!
//! The route table is closed to `GET /api/me/avatar` and `PUT /api/me/avatar`.
//! Every case here is asserted twice where it matters: once against the pure
//! parser (so a refusal is proved without a database) and once through the
//! handler against real PostgreSQL 16 (so the refusal is proved not to write).
//!
//! The database is the private `mcr-pg-*` cluster created by
//! `backend/scripts/mcr/wp08_avatar_cluster.py`; the URL arrives through
//! `CONCERT_TEST_DATABASE_URL`.
use concert_application::bootstrap::{
    BootstrapDefaults, BootstrapRelease, ValidatedProductRelease,
};
use concert_domain::profile_avatar::{config_value, parse_config, ProfileId};
use concert_domain::{AvatarSelection, Theme};
use concert_service::profile_avatar::{
    handle, names_event_path, route, ProfileAvatarActor, ProfileAvatarPersistence,
    ProfileAvatarRoute, AVATAR_PATH, ROUTES,
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
        .max_connections(4)
        .connect_with(options)
        .await?;
    let applied: bool =
        sqlx::query_scalar("select to_regclass('public.profile_avatars') is not null")
            .fetch_one(&pool)
            .await?;
    anyhow::ensure!(applied, "migration 0090_profile_avatars is not applied");
    Ok(pool)
}

fn default_selection() -> AvatarSelection {
    let document: Value = serde_json::from_str(include_str!(
        "../../../../contracts/fixtures/core/bootstrap-open.json"
    ))
    .unwrap();
    parse_config(&serde_json::to_vec(&document["avatar"]["config"]).unwrap()).unwrap()
}

fn persistence(pool: PgPool) -> Arc<ProfileAvatarPersistence> {
    let document: Value = serde_json::from_str(include_str!(
        "../../../../contracts/fixtures/core/bootstrap-open.json"
    ))
    .unwrap();
    let release = Arc::new(
        ValidatedProductRelease::from_validated(
            BootstrapRelease::new(
                BootstrapDefaults {
                    theme: Theme::Revenge,
                    avatar: default_selection(),
                },
                serde_json::to_vec(&document).unwrap(),
            )
            .unwrap(),
            "a".repeat(64),
        )
        .unwrap(),
    );
    Arc::new(
        ProfileAvatarPersistence::from_synthetic_test(pool, release, true)
            .expect("fixture release must project"),
    )
}

fn actor(profile: Uuid) -> ProfileAvatarActor {
    ProfileAvatarActor::new(ProfileId::from_uuid(profile).unwrap())
}

fn command(expected: i64, config: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({"expected_revision": expected, "config": config})).unwrap()
}

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
// The closed route table
// ---------------------------------------------------------------------------

#[test]
fn the_route_table_is_exactly_the_two_frozen_operations() {
    assert_eq!(
        ROUTES,
        &[
            ("GET", "/api/me/avatar", "getOwnProfileAvatar"),
            ("PUT", "/api/me/avatar", "setOwnProfileAvatar"),
        ]
    );
    assert_eq!(AVATAR_PATH, "/api/me/avatar");
    assert_eq!(route("GET", AVATAR_PATH), Ok(ProfileAvatarRoute::Get));
    assert_eq!(route("PUT", AVATAR_PATH), Ok(ProfileAvatarRoute::Put));
    assert_eq!(ProfileAvatarRoute::Get.operation(), "getOwnProfileAvatar");
    assert_eq!(ProfileAvatarRoute::Put.operation(), "setOwnProfileAvatar");
    assert!(!ProfileAvatarRoute::Get.mutation());
    assert!(ProfileAvatarRoute::Put.mutation());
    // No route names an event or a participant.
    for (_, path, _) in ROUTES {
        assert!(!path.contains("events"));
        assert!(!path.contains("participant"));
        assert!(!path.contains(':'));
    }
}

#[test]
fn any_target_containing_an_events_segment_is_rejected() {
    for target in [
        "/api/events/00000000-0000-4000-8000-000000000701/me/avatar",
        "/api/concert/events/00000000-0000-4000-8000-000000000701/me/avatar",
        "/api/me/avatar/events/x",
        "/api/me/events/avatar",
        "/events/",
        "/api/me/avatar?next=/events/1",
        // Percent-encoded spellings decode to the same path and are refused.
        "/api/me%2Fevents%2Favatar",
        "/api/me%2fevents%2favatar",
        "/api/me/events%2Favatar",
    ] {
        assert!(names_event_path(target), "{target}");
        assert!(
            route("GET", target).is_err(),
            "GET {target} must be refused"
        );
        assert!(
            route("PUT", target).is_err(),
            "PUT {target} must be refused"
        );
    }
    // The two real targets are not event paths.
    assert!(!names_event_path(AVATAR_PATH));
    assert!(!names_event_path("/api/me/avatar?"));
    // A path that merely contains the letters is not an event path.
    assert!(!names_event_path("/api/me/events-avatar"));
    assert!(!names_event_path("/api/me/avatar-events"));
}

#[test]
fn every_other_method_path_and_query_is_refused() {
    for (method, target) in [
        ("POST", AVATAR_PATH),
        ("PATCH", AVATAR_PATH),
        ("DELETE", AVATAR_PATH),
        ("GET", "/api/me/avatars"),
        ("GET", "/api/me/avatar/"),
        ("GET", "/api/avatar"),
        ("GET", "/api/me/profile"),
        ("GET", "/api/me/avatar?revision=1"),
        ("PUT", "/api/me/avatar?expected=1"),
        ("get", AVATAR_PATH),
        ("GET", ""),
        ("GET", "/"),
    ] {
        assert!(route(method, target).is_err(), "{method} {target}");
    }
    // A control character cannot be smuggled into a target.
    assert!(route("GET", "/api/me/avatar\n").is_err());
    assert!(route("GET", &format!("/api/me/avatar{}", "\u{0}")).is_err());
}

// ---------------------------------------------------------------------------
// The handler
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_read_returns_the_frozen_avatar_state() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = persistence(pool.clone());
    let profile = Uuid::new_v4();
    let response = handle(&store, actor(profile), "GET", AVATAR_PATH, &[]).await;
    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        json!({"revision": 1, "config": config_value(&default_selection())})
    );
    assert_eq!(
        response.body.as_object().unwrap().keys().collect::<Vec<_>>(),
        vec!["config", "revision"]
    );
    Ok(())
}

#[tokio::test]
async fn a_write_saves_and_a_read_reloads_the_same_configuration() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = persistence(pool.clone());
    let profile = Uuid::new_v4();
    let written = handle(
        &store,
        actor(profile),
        "PUT",
        AVATAR_PATH,
        &command(1, alternate()),
    )
    .await;
    assert_eq!(written.status, 200);
    assert_eq!(written.body, json!({"revision": 2, "config": alternate()}));
    let read = handle(&store, actor(profile), "GET", AVATAR_PATH, &[]).await;
    assert_eq!(read.status, 200);
    assert_eq!(read.body, written.body);
    Ok(())
}

#[tokio::test]
async fn a_stale_write_is_409_with_the_current_config_and_writes_nothing(
) -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = persistence(pool.clone());
    let profile = Uuid::new_v4();
    handle(
        &store,
        actor(profile),
        "PUT",
        AVATAR_PATH,
        &command(1, alternate()),
    )
    .await;
    let before = stored(&pool, profile).await.unwrap();
    let conflict = handle(
        &store,
        actor(profile),
        "PUT",
        AVATAR_PATH,
        &command(1, alternate()),
    )
    .await;
    assert_eq!(conflict.status, 409);
    assert_eq!(conflict.body["details"]["reason"], "VERSION_CONFLICT");
    // The current config travels with the conflict so the client can reconcile.
    assert_eq!(conflict.body["details"]["current"]["revision"], 2);
    assert_eq!(conflict.body["details"]["current"]["config"], before.1);
    assert_eq!(stored(&pool, profile).await, Some(before));
    Ok(())
}

#[tokio::test]
async fn an_event_target_is_400_and_writes_nothing() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = persistence(pool.clone());
    let profile = Uuid::new_v4();
    for target in [
        "/api/concert/events/00000000-0000-4000-8000-000000000701/me/avatar",
        "/api/events/00000000-0000-4000-8000-000000000701/me/avatar",
        "/api/me/avatar/events/1",
    ] {
        for method in ["GET", "PUT"] {
            let response = handle(&store, actor(profile), method, target, &command(1, alternate()))
                .await;
            assert_eq!(response.status, 400, "{method} {target}");
            assert_eq!(response.body["details"]["reason"], "INVALID_INPUT");
        }
    }
    // Nothing was written, not even for the well-formed body.
    assert_eq!(stored(&pool, profile).await, None);
    Ok(())
}

#[tokio::test]
async fn a_read_with_a_body_is_refused_and_writes_nothing() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = persistence(pool.clone());
    let profile = Uuid::new_v4();
    let response = handle(
        &store,
        actor(profile),
        "GET",
        AVATAR_PATH,
        &command(1, alternate()),
    )
    .await;
    assert_eq!(response.status, 400);
    assert_eq!(stored(&pool, profile).await, None);
    Ok(())
}

#[tokio::test]
async fn an_invalid_body_is_400_and_writes_nothing() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = persistence(pool.clone());
    let profile = Uuid::new_v4();
    for body in [
        b"".to_vec(),
        b"{}".to_vec(),
        b"[]".to_vec(),
        command(0, alternate()),
        command(1, json!({"kind": "avatar_config"})),
        command(1, json!({"contract_version": "1.0.0", "kind": "avatar_config"})),
    ] {
        let response = handle(&store, actor(profile), "PUT", AVATAR_PATH, &body).await;
        assert_eq!(response.status, 400, "{}", String::from_utf8_lossy(&body));
        assert_eq!(response.body["details"]["reason"], "INVALID_INPUT");
    }
    assert_eq!(stored(&pool, profile).await, None);
    Ok(())
}

#[tokio::test]
async fn a_write_that_names_its_own_owner_is_refused() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = persistence(pool.clone());
    let owner = Uuid::new_v4();
    let other = Uuid::new_v4();
    for extra in [
        json!({"profile_id": other.to_string()}),
        json!({"owner_profile_id": other.to_string()}),
        json!({"event_id": "00000000-0000-4000-8000-000000000701"}),
        json!({"user_id": other.to_string()}),
    ] {
        let mut body = json!({"expected_revision": 1, "config": alternate()});
        for (key, value) in extra.as_object().unwrap() {
            body[key] = value.clone();
        }
        let response = handle(
            &store,
            actor(owner),
            "PUT",
            AVATAR_PATH,
            &serde_json::to_vec(&body).unwrap(),
        )
        .await;
        assert_eq!(response.status, 400, "{extra}");
    }
    // Neither profile gained a row.
    assert_eq!(stored(&pool, owner).await, None);
    assert_eq!(stored(&pool, other).await, None);
    Ok(())
}

#[tokio::test]
async fn two_profiles_never_share_an_avatar() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = persistence(pool.clone());
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    handle(
        &store,
        actor(first),
        "PUT",
        AVATAR_PATH,
        &command(1, alternate()),
    )
    .await;
    // The second profile still starts at its own revision 1 default.
    let read = handle(&store, actor(second), "GET", AVATAR_PATH, &[]).await;
    assert_eq!(read.status, 200);
    assert_eq!(read.body["revision"], 1);
    assert_eq!(
        read.body["config"],
        config_value(&default_selection()),
        "a second profile must not see the first profile's avatar"
    );
    assert_eq!(stored(&pool, second).await, None);
    Ok(())
}

// ---------------------------------------------------------------------------
// The route module source has no event coupling
// ---------------------------------------------------------------------------

#[test]
fn the_route_source_has_no_event_parameter_or_event_path() {
    let source = include_str!("../src/profile_avatar/http.rs");
    // The only event-shaped text allowed is the rejection guard itself.
    assert!(source.contains("names_event_path"));
    for forbidden in ["event_id", "EventId", "ParticipantKey", "participant_id"] {
        assert!(
            !source.contains(forbidden),
            "profile avatar routes must not mention {forbidden}"
        );
    }
}
