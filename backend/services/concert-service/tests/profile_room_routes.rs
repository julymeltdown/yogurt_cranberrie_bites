//! WP-07 T-605: the five personal-room routes.
//!
//! The route table is closed to exactly the five frozen operations of
//! http-contract.md §2.4. Every case is asserted twice where it matters: once
//! against the pure parser (so a refusal is proved without a database) and once
//! through the handler against real PostgreSQL 16 (so the refusal is proved not
//! to write).
//!
//! The database is the private `mcr-pg-*` cluster created by
//! `backend/scripts/mcr/wp07_room_cluster.py`; the URL arrives through
//! `CONCERT_TEST_DATABASE_URL`.
use concert_domain::profile_avatar::ProfileId;
use concert_service::profile_room::{
    handle, names_event_path, names_intro_path, route, RoomActor, RoomPersistence, RoomRoute,
    OWN_ROOM_PATH, POSTERS_PATH, ROUTES, VISIBILITY_PATH,
};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::io::Cursor;
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
    let applied: bool = sqlx::query_scalar("select to_regclass('public.profile_rooms') is not null")
        .fetch_one(&pool)
        .await?;
    anyhow::ensure!(applied, "migration 0081_profile_rooms is not applied");
    Ok(pool)
}

fn actor(profile: Uuid) -> RoomActor {
    RoomActor::new(ProfileId::from_uuid(profile).unwrap())
}

fn layout() -> Value {
    json!({
        "version": 1,
        "wall": "panel",
        "floor": "concrete",
        "furniture": [{"item_id": "bed", "cell_x": 4, "cell_y": 20, "layer": 0}],
        "posters": [],
        "character": {"cell_x": 12, "cell_y": 26, "facing": "LEFT"},
    })
}

fn save(expected: i64) -> Vec<u8> {
    serde_json::to_vec(&json!({"expected_revision": expected, "layout": layout()})).unwrap()
}

async fn stored(pool: &PgPool, profile: Uuid) -> Option<(i64, Value)> {
    sqlx::query_as("select revision, layout from public.profile_rooms where owner_profile_id=$1")
        .bind(profile)
        .fetch_optional(pool)
        .await
        .unwrap()
}

async fn media_count(pool: &PgPool, owner: Uuid) -> i64 {
    sqlx::query_scalar("select count(*) from public.profile_media where owner_profile_id=$1")
        .bind(owner)
        .fetch_one(pool)
        .await
        .unwrap()
}

// ---------------------------------------------------------------------------
// The closed route table
// ---------------------------------------------------------------------------

#[test]
fn the_route_table_is_exactly_the_five_frozen_operations() {
    assert_eq!(
        ROUTES,
        &[
            ("GET", "/api/profiles/{profileId}/room", "getProfileRoom"),
            ("GET", "/api/me/room", "getOwnRoom"),
            ("PUT", "/api/me/room", "saveOwnRoom"),
            ("PATCH", "/api/me/room/visibility", "setOwnRoomVisibility"),
            ("POST", "/api/me/room/posters", "uploadRoomPoster"),
        ]
    );
    assert_eq!(OWN_ROOM_PATH, "/api/me/room");
    assert_eq!(VISIBILITY_PATH, "/api/me/room/visibility");
    assert_eq!(POSTERS_PATH, "/api/me/room/posters");
    let profile = Uuid::new_v4();
    assert_eq!(
        route("GET", &format!("/api/profiles/{profile}/room")),
        Ok(RoomRoute::ProfileRoom(
            ProfileId::from_uuid(profile).unwrap()
        ))
    );
    assert_eq!(route("GET", OWN_ROOM_PATH), Ok(RoomRoute::Own));
    assert_eq!(route("PUT", OWN_ROOM_PATH), Ok(RoomRoute::Save));
    assert_eq!(route("PATCH", VISIBILITY_PATH), Ok(RoomRoute::Visibility));
    assert_eq!(route("POST", POSTERS_PATH), Ok(RoomRoute::Poster));
    // The operation ids are the frozen ones, and the read routes are not writes.
    assert_eq!(RoomRoute::Own.operation(), "getOwnRoom");
    assert_eq!(RoomRoute::Save.operation(), "saveOwnRoom");
    assert_eq!(RoomRoute::Visibility.operation(), "setOwnRoomVisibility");
    assert_eq!(RoomRoute::Poster.operation(), "uploadRoomPoster");
    assert_eq!(
        RoomRoute::ProfileRoom(ProfileId::from_uuid(profile).unwrap()).operation(),
        "getProfileRoom"
    );
    assert!(!RoomRoute::Own.mutation());
    assert!(!RoomRoute::ProfileRoom(ProfileId::from_uuid(profile).unwrap()).mutation());
    assert!(RoomRoute::Save.mutation());
    assert!(RoomRoute::Visibility.mutation());
    assert!(RoomRoute::Poster.mutation());
    // No route names an event, a participant or an intro post.
    for (_, path, _) in ROUTES {
        assert!(!path.contains("events"));
        assert!(!path.contains("participant"));
        assert!(!path.contains("intro"));
    }
}

#[test]
fn any_target_containing_an_events_segment_is_rejected() {
    for target in [
        "/api/events/00000000-0000-4000-8000-000000000701/me/room",
        "/api/concert/events/00000000-0000-4000-8000-000000000701/me/room",
        "/api/me/room/events/x",
        "/api/me/events/room",
        "/events/",
        "/api/me/room?next=/events/1",
        // Percent-encoded spellings decode to the same path and are refused.
        "/api/me%2Fevents%2Froom",
        "/api/me%2fevents%2froom",
        "/api/me/events%2Froom",
    ] {
        assert!(names_event_path(target), "{target}");
        for method in ["GET", "PUT", "PATCH", "POST"] {
            assert!(route(method, target).is_err(), "{method} {target}");
        }
    }
    // The five real targets are not event paths.
    for path in [OWN_ROOM_PATH, VISIBILITY_PATH, POSTERS_PATH] {
        assert!(!names_event_path(path));
    }
    // A path that merely contains the letters is not an event path.
    assert!(!names_event_path("/api/me/events-room"));
    assert!(!names_event_path("/api/me/room-events"));
}

#[test]
fn any_target_containing_an_intro_segment_is_rejected() {
    for target in [
        "/api/me/intro/room",
        "/api/intro-posts/00000000-0000-4000-8000-000000000701/room",
        "/api/me/room/intro",
        "/api/me%2Fintro%2Froom",
        "/api/me/intro",
        "/api/me/intros",
        "/api/me/room?intro=1",
    ] {
        assert!(names_intro_path(target), "{target}");
        for method in ["GET", "PUT", "PATCH", "POST"] {
            assert!(route(method, target).is_err(), "{method} {target}");
        }
    }
    // The five real targets name no intro post.
    for path in [OWN_ROOM_PATH, VISIBILITY_PATH, POSTERS_PATH] {
        assert!(!names_intro_path(path));
    }
    // A word that merely starts with "intro" is not an intro post.
    assert!(!names_intro_path("/api/me/introduce"));
    assert!(!names_intro_path("/api/me/room-intro"));
}

#[test]
fn every_other_method_path_and_query_is_refused() {
    let profile = Uuid::new_v4();
    for (method, target) in [
        ("POST", OWN_ROOM_PATH),
        ("PATCH", OWN_ROOM_PATH),
        ("DELETE", OWN_ROOM_PATH),
        ("GET", VISIBILITY_PATH),
        ("PUT", VISIBILITY_PATH),
        ("GET", POSTERS_PATH),
        ("DELETE", POSTERS_PATH),
        ("GET", "/api/me/rooms"),
        ("GET", "/api/me/room/"),
        ("GET", "/api/rooms"),
        ("GET", "/api/me/profile"),
        ("GET", "/api/me/room?revision=1"),
        ("PUT", "/api/me/room?expected=1"),
        ("get", OWN_ROOM_PATH),
        ("GET", ""),
        ("GET", "/"),
        // The visitor read is the only parameterised route, and its segment must
        // be a canonical non-nil profile id.
        ("GET", "/api/profiles/not-a-uuid/room"),
        ("GET", "/api/profiles/00000000-0000-0000-0000-000000000000/room"),
        ("GET", &format!("/api/profiles/{}/room/extra", profile)),
        ("GET", &format!("/api/profiles/{}/rooms", profile)),
        ("POST", &format!("/api/profiles/{}/room", profile)),
        ("GET", &format!("/api/profiles/{}/room?x=1", profile)),
    ] {
        assert!(route(method, target).is_err(), "{method} {target}");
    }
    // A control character cannot be smuggled into a target.
    assert!(route("GET", "/api/me/room\n").is_err());
    assert!(route("GET", &format!("/api/me/room{}", "\u{0}")).is_err());
    // An uppercase UUID is not the canonical spelling.
    assert!(route("GET", &format!("/api/profiles/{}/room", profile.to_string().to_uppercase())).is_err());
}

// ---------------------------------------------------------------------------
// The handler
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_five_operations_serve_their_frozen_shapes() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = RoomPersistence::new(pool.clone());
    let owner = Uuid::new_v4();
    let visitor = Uuid::new_v4();
    // 2.4.2 getOwnRoom
    let own = handle(&store, actor(owner), "GET", OWN_ROOM_PATH, &[]).await;
    assert_eq!(own.status, 200);
    assert_eq!(own.body["is_owner"], true);
    // 2.4.3 saveOwnRoom
    let saved = handle(&store, actor(owner), "PUT", OWN_ROOM_PATH, &save(1)).await;
    assert_eq!(saved.status, 200);
    assert_eq!(saved.body["revision"], 2);
    assert_eq!(saved.body["layout"], layout());
    // 2.4.4 setOwnRoomVisibility
    let visibility = handle(
        &store,
        actor(owner),
        "PATCH",
        VISIBILITY_PATH,
        &serde_json::to_vec(&json!({"expected_revision": 2, "visibility": "PUBLIC"})).unwrap(),
    )
    .await;
    assert_eq!(visibility.status, 200);
    assert_eq!(visibility.body["visibility"], "PUBLIC");
    // 2.4.5 uploadRoomPoster
    let image = image::RgbaImage::from_fn(64, 64, |x, y| {
        image::Rgba([(x % 256) as u8, (y % 256) as u8, 64, 255])
    });
    let mut bytes = Vec::new();
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
        .unwrap();
    let encoded = {
        const ALPHABET: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for chunk in bytes.chunks(3) {
            let b = [
                chunk[0],
                chunk.get(1).copied().unwrap_or(0),
                chunk.get(2).copied().unwrap_or(0),
            ];
            out.push(ALPHABET[(b[0] >> 2) as usize] as char);
            out.push(ALPHABET[(((b[0] & 0b11) << 4) | (b[1] >> 4)) as usize] as char);
            out.push(if chunk.len() > 1 {
                ALPHABET[(((b[1] & 0b1111) << 2) | (b[2] >> 6)) as usize] as char
            } else {
                '='
            });
            out.push(if chunk.len() > 2 {
                ALPHABET[(b[2] & 0b111111) as usize] as char
            } else {
                '='
            });
        }
        out
    };
    let poster = handle(
        &store,
        actor(owner),
        "POST",
        POSTERS_PATH,
        &serde_json::to_vec(&json!({"preset": "FINE", "width": 64, "height": 64,
            "content_type": "image/png", "bytes_base64": encoded}))
        .unwrap(),
    )
    .await;
    assert_eq!(poster.status, 200, "{:?}", poster.body);
    assert_eq!(poster.body["preset"], "FINE");
    // 2.4.1 getProfileRoom
    let room = handle(
        &store,
        actor(visitor),
        "GET",
        &format!("/api/profiles/{owner}/room"),
        &[],
    )
    .await;
    assert_eq!(room.status, 200, "{:?}", room.body);
    assert_eq!(room.body["layout"], layout());
    // The poster is not placed on the wall by an upload: the layout still has no
    // poster until a save places it.
    assert_eq!(room.body["posters"], json!([]));
    Ok(())
}

#[tokio::test]
async fn an_unknown_route_is_refused_before_any_producer_work() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = RoomPersistence::new(pool.clone());
    let owner = Uuid::new_v4();
    for (method, target) in [
        ("GET", "/api/me/room/intro"),
        ("GET", "/api/concert/events/00000000-0000-4000-8000-000000000701/me/room"),
        ("POST", "/api/me/room"),
        ("DELETE", "/api/me/room"),
        ("GET", "/api/me/room?expected_revision=1"),
    ] {
        let response = handle(&store, actor(owner), method, target, &save(1)).await;
        assert_eq!(response.status, 400, "{method} {target}");
        assert_eq!(response.body["details"]["reason"], "INVALID_INPUT");
    }
    // Nothing was written, not even for a well-formed body.
    assert_eq!(stored(&pool, owner).await, None);
    assert_eq!(media_count(&pool, owner).await, 0);
    Ok(())
}

#[tokio::test]
async fn a_read_with_a_body_is_refused_and_writes_nothing() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = RoomPersistence::new(pool.clone());
    let owner = Uuid::new_v4();
    let response = handle(&store, actor(owner), "GET", OWN_ROOM_PATH, &save(1)).await;
    assert_eq!(response.status, 400);
    assert_eq!(stored(&pool, owner).await, None);
    Ok(())
}

#[tokio::test]
async fn an_invalid_body_is_400_and_writes_nothing() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = RoomPersistence::new(pool.clone());
    let owner = Uuid::new_v4();
    for body in [
        b"".to_vec(),
        b"{}".to_vec(),
        b"[]".to_vec(),
        save(0),
        serde_json::to_vec(&json!({"expected_revision": 1, "layout": {"version": 1}})).unwrap(),
    ] {
        let response = handle(&store, actor(owner), "PUT", OWN_ROOM_PATH, &body).await;
        assert_eq!(
            response.status,
            400,
            "{}",
            String::from_utf8_lossy(&body)
        );
        assert_eq!(response.body["details"]["reason"], "INVALID_INPUT");
    }
    for body in [
        b"".to_vec(),
        b"{}".to_vec(),
        serde_json::to_vec(&json!({"expected_revision": 1, "visibility": "FRIENDS"})).unwrap(),
        serde_json::to_vec(&json!({"expected_revision": 1, "visibility": "PUBLIC",
            "audience_gender": "any"}))
        .unwrap(),
    ] {
        let response = handle(&store, actor(owner), "PATCH", VISIBILITY_PATH, &body).await;
        assert_eq!(response.status, 400);
    }
    assert_eq!(stored(&pool, owner).await, None);
    Ok(())
}

#[tokio::test]
async fn a_body_over_the_bound_is_413_before_it_is_parsed() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = RoomPersistence::new(pool.clone());
    let owner = Uuid::new_v4();
    let oversized = vec![b'x'; 1_500_001];
    for (method, target) in [
        ("PUT", OWN_ROOM_PATH),
        ("PATCH", VISIBILITY_PATH),
        ("POST", POSTERS_PATH),
    ] {
        let response = handle(&store, actor(owner), method, target, &oversized).await;
        assert_eq!(response.status, 413, "{method} {target}");
        assert_eq!(response.body["details"]["reason"], "PAYLOAD_TOO_LARGE");
    }
    assert_eq!(stored(&pool, owner).await, None);
    assert_eq!(media_count(&pool, owner).await, 0);
    Ok(())
}

#[tokio::test]
async fn a_stale_write_is_409_with_the_current_room_and_writes_nothing() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = RoomPersistence::new(pool.clone());
    let owner = Uuid::new_v4();
    handle(&store, actor(owner), "PUT", OWN_ROOM_PATH, &save(1)).await;
    let before = stored(&pool, owner).await.unwrap();
    let conflict = handle(&store, actor(owner), "PUT", OWN_ROOM_PATH, &save(1)).await;
    assert_eq!(conflict.status, 409);
    assert_eq!(conflict.body["details"]["reason"], "VERSION_CONFLICT");
    assert_eq!(conflict.body["details"]["current"]["revision"], 2);
    assert_eq!(conflict.body["details"]["current"]["layout"], before.1);
    assert_eq!(stored(&pool, owner).await, Some(before));
    // The same applies to a stale visibility change.
    let visibility = handle(
        &store,
        actor(owner),
        "PATCH",
        VISIBILITY_PATH,
        &serde_json::to_vec(&json!({"expected_revision": 1, "visibility": "PRIVATE"})).unwrap(),
    )
    .await;
    assert_eq!(visibility.status, 409);
    assert_eq!(visibility.body["details"]["current"]["visibility"], "PUBLIC");
    Ok(())
}

#[tokio::test]
async fn two_profiles_never_share_a_room() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = RoomPersistence::new(pool.clone());
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    handle(&store, actor(first), "PUT", OWN_ROOM_PATH, &save(1)).await;
    // The second profile still starts at its own revision 1 default room.
    let read = handle(&store, actor(second), "GET", OWN_ROOM_PATH, &[]).await;
    assert_eq!(read.status, 200);
    assert_eq!(read.body["revision"], 1);
    assert_ne!(read.body["layout"], layout());
    assert_eq!(stored(&pool, second).await, None);
    Ok(())
}

// ---------------------------------------------------------------------------
// The route module source has no event or intro coupling
// ---------------------------------------------------------------------------

#[test]
fn the_route_source_has_no_event_or_intro_parameter() {
    let source = include_str!("../src/profile_room/http.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();
    // The only event- and intro-shaped text allowed is the rejection guard itself.
    assert!(source.contains("names_event_path"));
    assert!(source.contains("names_intro_path"));
    for forbidden in ["EventId", "ParticipantKey", "intro_id", "event_id"] {
        assert!(
            !source.contains(forbidden),
            "room routes must not mention {forbidden}"
        );
    }
}
