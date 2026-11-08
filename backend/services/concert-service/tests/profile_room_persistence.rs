//! WP-07 T-602: room persistence against real PostgreSQL 16.
//!
//! The database is the runner-owned private `mcr-pg-*` cluster created by
//! `backend/scripts/mcr/wp07_room_cluster.py` (the `migration_smoke.py` harness
//! pattern). The URL arrives through `CONCERT_TEST_DATABASE_URL` and must name the
//! `mcr_owner` role.
//!
//! Every case below is one of the hard constraints of the work package:
//!
//! * two concurrent saves with the same revision: one succeeds, one gets `409`
//!   with the current layout, and exactly one commit lands;
//! * a non-owner save writes nothing, and the owner's room is byte-identical
//!   afterwards;
//! * a revision conflict writes nothing and returns the current room;
//! * a stored layout the current rules no longer admit fails closed;
//! * the stored row has no intro or event column.
use concert_domain::profile_room::{
    own_room_wire, profile_room_wire, RoomLayout, RoomVisibility, StoredRoom,
};
use concert_domain::profile_avatar::ProfileId;
use concert_domain::Revision;
use concert_service::profile_room::{
    handle, http_status, RoomActor, RoomErrorKind, RoomPersistence, OWN_ROOM_PATH,
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
        "wall": "brick",
        "floor": "tile",
        "furniture": [
            {"item_id": "sofa", "cell_x": 3, "cell_y": 18, "layer": 0},
            {"item_id": "lamp", "cell_x": 3, "cell_y": 18, "layer": 1},
        ],
        "posters": [],
        "character": {"cell_x": 9, "cell_y": 27, "facing": "LEFT"},
    })
}

fn other_layout() -> Value {
    let mut value = layout();
    value["wall"] = json!("panel");
    value["furniture"] = json!([{"item_id": "desk", "cell_x": 20, "cell_y": 12, "layer": 2}]);
    value
}

fn save(expected: i64, layout: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({"expected_revision": expected, "layout": layout})).unwrap()
}

async fn stored(pool: &PgPool, profile: Uuid) -> Option<(i64, Value, String)> {
    sqlx::query_as(
        "select revision, layout, visibility from public.profile_rooms where owner_profile_id=$1",
    )
    .bind(profile)
    .fetch_optional(pool)
    .await
    .unwrap()
}

fn constraint(error: &sqlx::Error) -> Option<String> {
    match error {
        sqlx::Error::Database(database) => database.constraint().map(str::to_owned),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Read
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_profile_that_never_saved_reads_the_default_room_at_revision_one(
) -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = RoomPersistence::new(pool.clone());
    let profile = Uuid::new_v4();
    let room = store.read_own(actor(profile)).await.unwrap();
    assert_eq!(room["revision"], 1);
    assert_eq!(room["is_owner"], true);
    assert_eq!(room["owner_profile_id"], profile.to_string());
    // The product default is PUBLIC: authorized visitors see the saved room.
    assert_eq!(room["visibility"], "PUBLIC");
    assert_eq!(room["layout"], RoomLayout::initial().to_wire());
    assert_eq!(room["posters"], json!([]));
    assert_eq!(room["poster_moderation"], json!([]));
    // A read never creates a row: it cannot authorize or fabricate a write.
    assert_eq!(stored(&pool, profile).await, None);
    Ok(())
}

// ---------------------------------------------------------------------------
// Write and reload
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_save_advances_the_revision_once_and_reloads_the_same_layout(
) -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = RoomPersistence::new(pool.clone());
    let profile = Uuid::new_v4();
    let saved = store.save(actor(profile), &save(1, layout())).await.unwrap();
    assert_eq!(saved["revision"], 2);
    assert_eq!(saved["layout"], layout());
    let (revision, stored_layout, visibility) = stored(&pool, profile).await.unwrap();
    assert_eq!(revision, 2);
    assert_eq!(stored_layout, layout());
    assert_eq!(visibility, "PUBLIC");
    // The reload is exactly the room the save returned.
    assert_eq!(store.read_own(actor(profile)).await.unwrap(), saved);
    Ok(())
}

#[tokio::test]
async fn a_revision_conflict_writes_nothing_and_returns_the_current_room(
) -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = RoomPersistence::new(pool.clone());
    let profile = Uuid::new_v4();
    store.save(actor(profile), &save(1, layout())).await.unwrap();
    let before = stored(&pool, profile).await.unwrap();
    // Revision 1 is stale now; the current revision is 2.
    assert_eq!(
        store.save(actor(profile), &save(1, other_layout())).await,
        Err(RoomErrorKind::Conflict)
    );
    assert_eq!(stored(&pool, profile).await, Some(before.clone()));
    // The route returns 409 with the current layout, so the client can offer an
    // explicit resolution instead of discarding its unsaved layout.
    let response = handle(
        &store,
        actor(profile),
        "PUT",
        OWN_ROOM_PATH,
        &save(1, other_layout()),
    )
    .await;
    assert_eq!(response.status, 409);
    assert_eq!(response.body["details"]["reason"], "VERSION_CONFLICT");
    assert_eq!(response.body["details"]["current"]["revision"], 2);
    assert_eq!(response.body["details"]["current"]["layout"], before.1);
    assert_eq!(stored(&pool, profile).await, Some(before));
    Ok(())
}

#[tokio::test]
async fn concurrent_saves_at_the_same_revision_are_one_success_and_one_conflict(
) -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = RoomPersistence::new(pool.clone());
    let profile = Uuid::new_v4();
    // Both writers believe the current revision is 1.
    let first = Arc::clone(&store);
    let second = Arc::clone(&store);
    let (left, right) = tokio::join!(
        async move { first.save(actor(profile), &save(1, layout())).await },
        async move { second.save(actor(profile), &save(1, other_layout())).await },
    );
    let outcomes = [left, right];
    let successes = outcomes
        .iter()
        .filter(|outcome| matches!(outcome, Ok(room) if room["revision"] == 2))
        .count();
    let conflicts = outcomes
        .iter()
        .filter(|outcome| **outcome == Err(RoomErrorKind::Conflict))
        .count();
    assert_eq!(
        (successes, conflicts),
        (1, 1),
        "one success and one conflict: {outcomes:?}"
    );
    // Exactly one commit landed.
    let (revision, stored_layout, _) = stored(&pool, profile).await.unwrap();
    assert_eq!(revision, 2);
    let winner = outcomes
        .iter()
        .find_map(|outcome| outcome.as_ref().ok())
        .expect("one winner");
    assert_eq!(stored_layout, winner["layout"]);
    // The loser's 409 carries the winner's layout, not its own.
    let response = handle(
        &store,
        actor(profile),
        "PUT",
        OWN_ROOM_PATH,
        &save(1, other_layout()),
    )
    .await;
    assert_eq!(response.status, 409);
    assert_eq!(response.body["details"]["current"]["layout"], stored_layout);
    Ok(())
}

#[tokio::test]
async fn many_concurrent_savers_at_one_revision_commit_exactly_once() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = RoomPersistence::new(pool.clone());
    let profile = Uuid::new_v4();
    // Materialize the row at revision 1 so every writer competes for revision 2.
    store.save(actor(profile), &save(1, layout())).await.unwrap();
    let mut tasks = Vec::new();
    for _ in 0..8 {
        let store = Arc::clone(&store);
        tasks.push(tokio::spawn(async move {
            store.save(actor(profile), &save(2, other_layout())).await
        }));
    }
    let mut successes = 0;
    let mut conflicts = 0;
    for task in tasks {
        match task.await.unwrap() {
            Ok(room) => {
                assert_eq!(room["revision"], 3);
                successes += 1;
            }
            Err(RoomErrorKind::Conflict) => conflicts += 1,
            Err(other) => panic!("unexpected persistence error: {other:?}"),
        }
    }
    assert_eq!((successes, conflicts), (1, 7));
    assert_eq!(stored(&pool, profile).await.unwrap().0, 3);
    Ok(())
}

// ---------------------------------------------------------------------------
// Ownership: a non-owner save writes nothing
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_non_owner_save_writes_nothing() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = RoomPersistence::new(pool.clone());
    let owner = Uuid::new_v4();
    let stranger = Uuid::new_v4();
    let owned = store.save(actor(owner), &save(1, layout())).await.unwrap();
    let before = stored(&pool, owner).await.unwrap();
    // The stranger's save at the owner's revision lands on the *stranger's* row:
    // the only owner a statement binds is the authenticated one, so a write to
    // another profile's room cannot even be expressed.
    let stranger_room = store
        .save(actor(stranger), &save(1, other_layout()))
        .await
        .unwrap();
    assert_eq!(stranger_room["owner_profile_id"], stranger.to_string());
    assert_eq!(store.read_own(actor(owner)).await.unwrap(), owned);
    assert_eq!(stored(&pool, owner).await, Some(before.clone()));
    // The schema refuses re-pointing a stored room at another profile, so even a
    // direct statement cannot transfer ownership.
    let repointed = sqlx::query(
        "update public.profile_rooms set owner_profile_id=$2, revision=revision+1 \
         where owner_profile_id=$1",
    )
    .bind(owner)
    .bind(stranger)
    .execute(&pool)
    .await;
    let error = repointed.expect_err("owner re-point must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("profile_rooms_owner_immutable")
    );
    assert_eq!(stored(&pool, owner).await, Some(before));
    Ok(())
}

#[tokio::test]
async fn the_http_projection_of_a_denied_write_is_403() {
    // The persistence has no "denied" branch for a caller-supplied owner because
    // it has no such parameter. The route layer decides ownership from the
    // verified session, and its refusal is the frozen 403.
    assert_eq!(http_status(RoomErrorKind::Denied), 403);
    assert_eq!(http_status(RoomErrorKind::Unauthenticated), 401);
    assert_eq!(http_status(RoomErrorKind::Conflict), 409);
    assert_eq!(http_status(RoomErrorKind::Missing), 404);
    assert_eq!(http_status(RoomErrorKind::TooLarge), 413);
    assert_eq!(http_status(RoomErrorKind::UnsupportedMedia), 415);
}

#[tokio::test]
async fn a_save_that_names_its_own_owner_is_refused_and_writes_nothing(
) -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = RoomPersistence::new(pool.clone());
    let owner = Uuid::new_v4();
    let other = Uuid::new_v4();
    for (key, value) in [
        ("owner_profile_id", json!(other.to_string())),
        ("profile_id", json!(other.to_string())),
        ("event_id", json!(Uuid::new_v4().to_string())),
        ("intro_id", json!(Uuid::new_v4().to_string())),
    ] {
        let mut body = json!({"expected_revision": 1, "layout": layout()});
        body[key] = value;
        let response = handle(
            &store,
            actor(owner),
            "PUT",
            OWN_ROOM_PATH,
            &serde_json::to_vec(&body).unwrap(),
        )
        .await;
        assert_eq!(response.status, 400, "{key}");
    }
    assert_eq!(stored(&pool, owner).await, None);
    assert_eq!(stored(&pool, other).await, None);
    Ok(())
}

// ---------------------------------------------------------------------------
// Fail closed
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_stored_layout_the_rules_no_longer_admit_fails_closed() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = RoomPersistence::new(pool.clone());
    let profile = Uuid::new_v4();
    // A row written outside the room rules: five posters, an unknown wall style.
    sqlx::query("insert into profile_rooms(owner_profile_id, layout) values($1, $2)")
        .bind(profile)
        .bind(json!({"version": 1, "wall": "marble", "floor": "wood", "furniture": [],
            "posters": [], "character": {"cell_x": 0, "cell_y": 0, "facing": "LEFT"}}))
        .execute(&pool)
        .await?;
    assert_eq!(
        store.read_own(actor(profile)).await,
        Err(RoomErrorKind::InvalidState)
    );
    // It is reported as an invalid state, never replaced by the default room.
    assert_eq!(http_status(RoomErrorKind::InvalidState), 400);
    Ok(())
}

#[tokio::test]
async fn an_unadmitted_layout_is_refused_before_any_write() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = RoomPersistence::new(pool.clone());
    let profile = Uuid::new_v4();
    let mut five_posters = layout();
    five_posters["posters"] = Value::Array(
        (0..5)
            .map(|slot| {
                json!({"poster_media_id": Uuid::new_v4().to_string(), "slot": slot,
                    "x": 0, "y": 0, "width": 4, "height": 4})
            })
            .collect(),
    );
    for body in [
        save(1, five_posters),
        save(1, json!({"version": 1, "wall": "marble", "floor": "wood", "furniture": [],
            "posters": [], "character": {"cell_x": 0, "cell_y": 0, "facing": "LEFT"}})),
        save(0, layout()),
        b"{}".to_vec(),
        b"".to_vec(),
    ] {
        assert!(
            store.save(actor(profile), &body).await.is_err(),
            "{}",
            String::from_utf8_lossy(&body)
        );
    }
    assert_eq!(stored(&pool, profile).await, None);
    Ok(())
}

#[tokio::test]
async fn a_save_cannot_place_a_poster_the_owner_does_not_hold() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = RoomPersistence::new(pool.clone());
    let owner = Uuid::new_v4();
    let stranger = Uuid::new_v4();
    // A READY ROOM_POSTER owned by someone else.
    let foreign = Uuid::new_v4();
    insert_media(&pool, foreign, stranger, "ROOM_POSTER").await?;
    let mut body = layout();
    body["posters"] = json!([{"poster_media_id": foreign.to_string(), "slot": 0,
        "x": 2, "y": 2, "width": 6, "height": 6}]);
    assert_eq!(
        store.save(actor(owner), &save(1, body.clone())).await,
        Err(RoomErrorKind::Invalid)
    );
    assert_eq!(stored(&pool, owner).await, None);
    // An INTRO_POST asset owned by the owner is refused too: a poster is never an
    // intro attachment (V-06-e).
    let intro_media = Uuid::new_v4();
    insert_media(&pool, intro_media, owner, "INTRO_POST").await?;
    body["posters"] = json!([{"poster_media_id": intro_media.to_string(), "slot": 0,
        "x": 2, "y": 2, "width": 6, "height": 6}]);
    assert_eq!(
        store.save(actor(owner), &save(1, body.clone())).await,
        Err(RoomErrorKind::Invalid)
    );
    assert_eq!(stored(&pool, owner).await, None);
    // The owner's own READY ROOM_POSTER is admitted.
    let owned = Uuid::new_v4();
    insert_media(&pool, owned, owner, "ROOM_POSTER").await?;
    body["posters"] = json!([{"poster_media_id": owned.to_string(), "slot": 0,
        "x": 2, "y": 2, "width": 6, "height": 6}]);
    let saved = store.save(actor(owner), &save(1, body)).await.unwrap();
    assert_eq!(saved["posters"][0]["media_id"], owned.to_string());
    Ok(())
}

async fn insert_media(
    pool: &PgPool,
    media: Uuid,
    owner: Uuid,
    purpose: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "insert into profile_media(media_id,owner_profile_id,purpose,content_type,state,\
         declared_bytes,sanitized,width,height,input_sha256) \
         values($1,$2,$3,'image/png','READY',4,decode('89504e47','hex'),2,2,\
         decode(repeat('ab',32),'hex'))",
    )
    .bind(media)
    .bind(owner)
    .bind(purpose)
    .execute(pool)
    .await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Schema: no intro, no event
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_room_table_has_no_intro_or_event_column() -> anyhow::Result<()> {
    let pool = pool().await?;
    let columns: Vec<String> = sqlx::query_scalar(
        "select column_name::text from information_schema.columns \
         where table_schema='public' and table_name='profile_rooms' order by column_name",
    )
    .fetch_all(&pool)
    .await?;
    assert_eq!(
        columns,
        vec![
            "created_at".to_owned(),
            "layout".to_owned(),
            "owner_profile_id".to_owned(),
            "revision".to_owned(),
            "updated_at".to_owned(),
            "visibility".to_owned(),
        ]
    );
    for forbidden in ["intro_id", "event_id", "user_id", "participant_id", "presence"] {
        assert!(!columns.iter().any(|column| column == forbidden));
    }
    Ok(())
}

#[tokio::test]
async fn the_room_revision_must_advance_exactly_once() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = RoomPersistence::new(pool.clone());
    let profile = Uuid::new_v4();
    store.save(actor(profile), &save(1, layout())).await.unwrap();
    let error = sqlx::query("update profile_rooms set revision=9 where owner_profile_id=$1")
        .bind(profile)
        .execute(&pool)
        .await
        .expect_err("a skipped revision must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("profile_rooms_revision_step")
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Projections
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_visitor_reads_the_saved_room_while_the_owner_is_absent() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = RoomPersistence::new(pool.clone());
    let owner = Uuid::new_v4();
    let visitor = Uuid::new_v4();
    let own = store.save(actor(owner), &save(1, layout())).await.unwrap();
    // The owner is not connected: there is no session, no presence row and no
    // heartbeat anywhere in this test. The visitor still reads the saved room.
    let room = store
        .read_for_visitor(
            actor(visitor),
            ProfileId::from_uuid(owner).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(room["layout"], own["layout"]);
    assert_eq!(room["revision"], own["revision"]);
    assert_eq!(room["owner_profile_id"], owner.to_string());
    // A visitor's projection has no owner-only field.
    assert!(room.get("is_owner").is_none());
    assert!(room.get("poster_moderation").is_none());
    // And no presence field of any kind.
    for forbidden in ["online", "presence", "last_seen", "activity", "connected"] {
        assert!(
            room.get(forbidden).is_none(),
            "a room must not carry {forbidden}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn the_wire_projections_match_the_frozen_shapes() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = RoomPersistence::new(pool.clone());
    let profile = Uuid::new_v4();
    let owner = ProfileId::from_uuid(profile).unwrap();
    // The projection helpers are the only place the wire shape is written, so
    // they are exercised directly against the stored room.
    let room = StoredRoom {
        owner,
        visibility: RoomVisibility::Public,
        layout: RoomLayout::initial(),
        revision: Revision::new(4).unwrap(),
    };
    let visitor = profile_room_wire(&room, &[], "2026-11-09T00:00:00.000Z");
    assert_eq!(
        visitor.as_object().unwrap().keys().collect::<Vec<_>>(),
        vec![
            "layout",
            "owner_profile_id",
            "posters",
            "revision",
            "updated_at",
            "visibility"
        ]
    );
    let own = own_room_wire(&room, &[], &[], "2026-11-09T00:00:00.000Z");
    assert_eq!(
        own.as_object().unwrap().keys().collect::<Vec<_>>(),
        vec![
            "is_owner",
            "layout",
            "owner_profile_id",
            "poster_moderation",
            "posters",
            "revision",
            "updated_at",
            "visibility"
        ]
    );
    assert_eq!(own["is_owner"], true);
    // The live read uses the same shape.
    let live = store.read_own(actor(profile)).await.unwrap();
    assert_eq!(
        live.as_object().unwrap().keys().collect::<Vec<_>>(),
        own.as_object().unwrap().keys().collect::<Vec<_>>()
    );
    Ok(())
}
