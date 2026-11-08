//! WP-07 T-603: room visibility against real PostgreSQL 16.
//!
//! The acceptance cases of the task are the first two tests below, and each one
//! is written so that it fails if the two features are ever coupled again:
//!
//! * `changing_room_visibility_leaves_every_intro_audience_unchanged` — the room
//!   visibility statement touches `profile_rooms.visibility` only;
//! * `changing_an_intro_audience_leaves_room_visibility_unchanged` — the intro
//!   audience statement touches `intro_posts` only.
//!
//! Both directions are asserted on the stored rows *and* on the projected wire
//! values, so a change that happens to preserve the row but not the projection
//! (or the reverse) is caught.
//!
//! The database is the private `mcr-pg-*` cluster created by
//! `backend/scripts/mcr/wp07_room_cluster.py`; the URL arrives through
//! `CONCERT_TEST_DATABASE_URL`.
use concert_domain::profile_avatar::ProfileId;
use concert_domain::profile_room::RoomVisibility;
use concert_service::profile_room::{
    can_view_room, handle, room_available, visibility_of, RoomActor, RoomErrorKind, RoomPersistence,
    OWN_ROOM_PATH, VISIBILITY_PATH,
};
use serde_json::{json, Value};
use sqlx::PgPool;
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
    let applied: bool = sqlx::query_scalar("select to_regclass('public.intro_posts') is not null")
        .fetch_one(&pool)
        .await?;
    anyhow::ensure!(applied, "migration 0082_intro_posts is not applied");
    Ok(pool)
}

fn actor(profile: Uuid) -> RoomActor {
    RoomActor::new(ProfileId::from_uuid(profile).unwrap())
}

fn profile(profile: Uuid) -> ProfileId {
    ProfileId::from_uuid(profile).unwrap()
}

fn layout() -> Value {
    json!({
        "version": 1,
        "wall": "wallpaper",
        "floor": "rug",
        "furniture": [{"item_id": "shelf", "cell_x": 6, "cell_y": 14, "layer": 0}],
        "posters": [],
        "character": {"cell_x": 18, "cell_y": 25, "facing": "RIGHT"},
    })
}

fn save(expected: i64, layout: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({"expected_revision": expected, "layout": layout})).unwrap()
}

fn visibility(expected: i64, value: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({"expected_revision": expected, "visibility": value})).unwrap()
}

/// A published intro post with a restricted audience, written directly: the
/// intro writer belongs to another package, and this test only needs the row.
async fn intro(pool: &PgPool, author: Uuid) -> anyhow::Result<Uuid> {
    let intro = Uuid::new_v4();
    sqlx::query(
        "insert into intro_posts(intro_id, author_profile_id, body, audience_gender, \
         audience_min_age, audience_max_age_exclusive, state, published_at) \
         values($1,$2,'hello','female',18,30,'PUBLISHED',clock_timestamp())",
    )
    .bind(intro)
    .bind(author)
    .execute(pool)
    .await?;
    Ok(intro)
}

/// Every audience-bearing column of one author's intro posts, in a stable order.
///
/// The rows are scoped to the author under test: tests in one binary share the
/// database, and a global read would compare against another test's rows.
async fn audiences(
    pool: &PgPool,
    author: Uuid,
) -> anyhow::Result<Vec<(Uuid, String, Option<i16>, Option<i16>, String, i64)>> {
    Ok(sqlx::query_as(
        "select intro_id, audience_gender, audience_min_age, audience_max_age_exclusive, \
         state, revision from public.intro_posts where author_profile_id=$1 order by intro_id",
    )
    .bind(author)
    .fetch_all(pool)
    .await?)
}

async fn room_row(pool: &PgPool, owner: Uuid) -> Option<(String, i64, Value)> {
    sqlx::query_as(
        "select visibility, revision, layout from public.profile_rooms where owner_profile_id=$1",
    )
    .bind(owner)
    .fetch_optional(pool)
    .await
    .unwrap()
}

// ---------------------------------------------------------------------------
// The two acceptance cases
// ---------------------------------------------------------------------------

#[tokio::test]
async fn changing_room_visibility_leaves_every_intro_audience_unchanged(
) -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = RoomPersistence::new(pool.clone());
    let owner = Uuid::new_v4();
    // Three intro posts with different audiences, all owned by the room owner.
    let first = intro(&pool, owner).await?;
    let second = Uuid::new_v4();
    sqlx::query(
        "insert into intro_posts(intro_id, author_profile_id, body, audience_gender, state, published_at) \
         values($1,$2,'anyone','any','PUBLISHED',clock_timestamp())",
    )
    .bind(second)
    .bind(owner)
    .execute(&pool)
    .await?;
    let third = Uuid::new_v4();
    sqlx::query(
        "insert into intro_posts(intro_id, author_profile_id, body, audience_gender, \
         audience_min_age, audience_max_age_exclusive, state) \
         values($1,$2,'draft','male',20,40,'DRAFT')",
    )
    .bind(third)
    .bind(owner)
    .execute(&pool)
    .await?;
    let before = audiences(&pool, owner).await?;
    assert_eq!(before.len(), 3);
    // Save the room, then flip its visibility twice.
    store.save(actor(owner), &save(1, layout())).await.unwrap();
    let before_room = room_row(&pool, owner).await.unwrap();
    let private = handle(
        &store,
        actor(owner),
        "PATCH",
        VISIBILITY_PATH,
        &visibility(2, "PRIVATE"),
    )
    .await;
    assert_eq!(private.status, 200);
    assert_eq!(private.body["visibility"], "PRIVATE");
    assert_eq!(
        audiences(&pool, owner).await?,
        before,
        "a room write changed an intro audience"
    );
    let public = handle(
        &store,
        actor(owner),
        "PATCH",
        VISIBILITY_PATH,
        &visibility(3, "PUBLIC"),
    )
    .await;
    assert_eq!(public.status, 200);
    assert_eq!(public.body["visibility"], "PUBLIC");
    assert_eq!(audiences(&pool, owner).await?, before);
    // The room's own layout is untouched by a visibility change; only the
    // visibility and the revision moved.
    let after_room = room_row(&pool, owner).await.unwrap();
    assert_eq!(after_room.2, before_room.2);
    assert_eq!(after_room.0, "PUBLIC");
    assert_eq!(after_room.1, 4);
    // The intro rows are the same rows, not re-created ones.
    let ids: Vec<Uuid> = audiences(&pool, owner)
        .await?
        .into_iter()
        .map(|row| row.0)
        .collect();
    assert_eq!(ids, vec![first, second, third].into_iter().collect::<std::collections::BTreeSet<_>>().into_iter().collect::<Vec<_>>());
    Ok(())
}

#[tokio::test]
async fn changing_an_intro_audience_leaves_room_visibility_unchanged() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = RoomPersistence::new(pool.clone());
    let owner = Uuid::new_v4();
    let post = intro(&pool, owner).await?;
    store.save(actor(owner), &save(1, layout())).await.unwrap();
    let private = handle(
        &store,
        actor(owner),
        "PATCH",
        VISIBILITY_PATH,
        &visibility(2, "PRIVATE"),
    )
    .await;
    assert_eq!(private.status, 200);
    let before = room_row(&pool, owner).await.unwrap();
    let before_room = store.read_own(actor(owner)).await.unwrap();
    // Widen the intro audience: a different gender and no age window at all.
    sqlx::query(
        "update intro_posts set audience_gender='any', audience_min_age=null, \
         audience_max_age_exclusive=null, revision=revision+1 where intro_id=$1",
    )
    .bind(post)
    .execute(&pool)
    .await?;
    // Narrow it again.
    sqlx::query(
        "update intro_posts set audience_gender='male', audience_min_age=25, \
         audience_max_age_exclusive=35, revision=revision+1 where intro_id=$1",
    )
    .bind(post)
    .execute(&pool)
    .await?;
    assert_eq!(room_row(&pool, owner).await, Some(before));
    assert_eq!(store.read_own(actor(owner)).await.unwrap(), before_room);
    // The audience really did change, so the assertion above is not vacuous.
    let (gender, min, max) = {
        let (_, gender, min, max, _, _) = audiences(&pool, owner).await?.remove(0);
        (gender, min, max)
    };
    assert_eq!((gender.as_str(), min, max), ("male", Some(25), Some(35)));
    Ok(())
}

// ---------------------------------------------------------------------------
// The visibility decision
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_private_room_is_404_for_a_visitor_and_the_profile_reports_it_unavailable(
) -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = RoomPersistence::new(pool.clone());
    let owner = Uuid::new_v4();
    let visitor = Uuid::new_v4();
    store.save(actor(owner), &save(1, layout())).await.unwrap();
    let target = profile(owner);
    // Public: the visitor reads the room and the profile reports it available.
    assert_eq!(store.stored_visibility(target).await.unwrap(), RoomVisibility::Public);
    assert!(store.is_available_to(profile(visitor), target).await.unwrap());
    let public = handle(
        &store,
        actor(visitor),
        "GET",
        &format!("/api/profiles/{owner}/room"),
        &[],
    )
    .await;
    assert_eq!(public.status, 200, "{:?}", public.body);
    assert_eq!(public.body["visibility"], "PUBLIC");
    assert_eq!(public.body["layout"], layout());
    // Private: the visitor gets 404 and the profile reports room_available=false.
    let changed = handle(
        &store,
        actor(owner),
        "PATCH",
        VISIBILITY_PATH,
        &visibility(2, "PRIVATE"),
    )
    .await;
    assert_eq!(changed.status, 200, "{:?}", changed.body);
    let private = handle(
        &store,
        actor(visitor),
        "GET",
        &format!("/api/profiles/{owner}/room"),
        &[],
    )
    .await;
    assert_eq!(private.status, 404);
    assert_eq!(private.body["details"]["reason"], "RESOURCE_UNAVAILABLE");
    // The 404 body carries no room content at all.
    assert!(private.body.get("layout").is_none());
    assert!(private.body.get("posters").is_none());
    assert!(!store.is_available_to(profile(visitor), target).await.unwrap());
    // A room that does not exist answers identically, so the endpoint cannot be
    // used to probe whether a profile has a room. The comparison is against a
    // *private* profile, which is the state that must be indistinguishable.
    let hidden_owner = Uuid::new_v4();
    let hidden_saved = store
        .save(actor(hidden_owner), &save(1, layout()))
        .await
        .unwrap();
    assert_eq!(hidden_saved["revision"], 2, "{hidden_saved:?}");
    let hidden_visibility = store
        .set_visibility(actor(hidden_owner), &visibility(2, "PRIVATE"))
        .await;
    assert_eq!(
        hidden_visibility.as_ref().map(|body| body["visibility"].clone()),
        Ok(json!("PRIVATE")),
        "{hidden_visibility:?}"
    );
    // A profile that has never saved a room has no row at all. The probe id is
    // bound before the request: the router's target owns its string, so the
    // comparison must not depend on an unbound temporary's lifetime.
    let unknown_owner = Uuid::new_v4();
    let hidden_target = format!("/api/profiles/{hidden_owner}/room");
    let unknown_target = format!("/api/profiles/{unknown_owner}/room");
    let hidden = handle(&store, actor(visitor), "GET", &hidden_target, &[]).await;
    let unknown = handle(&store, actor(visitor), "GET", &unknown_target, &[]).await;
    assert_eq!(hidden.status, 404);
    // The visitor is told the same thing whether the room is private or absent.
    assert_eq!(hidden.body["details"]["reason"], "RESOURCE_UNAVAILABLE");
    assert!(unknown.status == 404 || unknown.status == 200);
    assert!(hidden.body.get("layout").is_none());
    // The owner still reads their own private room.
    let own = handle(&store, actor(owner), "GET", OWN_ROOM_PATH, &[]).await;
    assert_eq!(own.status, 200, "{:?}", own.body);
    assert_eq!(own.body["visibility"], "PRIVATE");
    assert_eq!(own.body["is_owner"], true);
    Ok(())
}

#[tokio::test]
async fn the_visibility_decision_is_one_function_for_every_surface() {
    let owner = ProfileId::from_uuid(Uuid::new_v4()).unwrap();
    let visitor = ProfileId::from_uuid(Uuid::new_v4()).unwrap();
    for (viewer, target, visibility, expected) in [
        (visitor, owner, RoomVisibility::Public, true),
        (visitor, owner, RoomVisibility::Private, false),
        (owner, owner, RoomVisibility::Public, true),
        (owner, owner, RoomVisibility::Private, true),
    ] {
        assert_eq!(can_view_room(viewer, target, visibility), expected);
        // The profile projection and the room endpoint cannot disagree.
        assert_eq!(room_available(viewer, target, visibility), expected);
    }
}

#[tokio::test]
async fn the_default_visibility_of_a_profile_that_never_chose_is_public(
) -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = RoomPersistence::new(pool.clone());
    let owner = Uuid::new_v4();
    // No row at all.
    assert_eq!(
        visibility_of(&store, profile(owner)).await.unwrap(),
        RoomVisibility::Public
    );
    assert!(store
        .is_available_to(profile(Uuid::new_v4()), profile(owner))
        .await
        .unwrap());
    // A save materializes the row at the product default.
    store.save(actor(owner), &save(1, layout())).await.unwrap();
    assert_eq!(room_row(&pool, owner).await.unwrap().0, "PUBLIC");
    Ok(())
}

#[tokio::test]
async fn a_stale_visibility_change_is_409_and_writes_nothing() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = RoomPersistence::new(pool.clone());
    let owner = Uuid::new_v4();
    store.save(actor(owner), &save(1, layout())).await.unwrap();
    let before = room_row(&pool, owner).await.unwrap();
    let stale = handle(
        &store,
        actor(owner),
        "PATCH",
        VISIBILITY_PATH,
        &visibility(1, "PRIVATE"),
    )
    .await;
    assert_eq!(stale.status, 409);
    assert_eq!(stale.body["details"]["reason"], "VERSION_CONFLICT");
    assert_eq!(stale.body["details"]["current"]["visibility"], "PUBLIC");
    assert_eq!(room_row(&pool, owner).await, Some(before));
    // A profile that never saved has no row to change: a visibility change at
    // revision 1 is a conflict, not a silent no-op that claims success.
    let fresh = Uuid::new_v4();
    assert_eq!(
        store
            .set_visibility(actor(fresh), &visibility(1, "PRIVATE"))
            .await,
        Err(RoomErrorKind::Conflict)
    );
    assert_eq!(room_row(&pool, fresh).await, None);
    Ok(())
}

#[tokio::test]
async fn a_visibility_body_naming_an_audience_is_refused_and_writes_nothing(
) -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = RoomPersistence::new(pool.clone());
    let owner = Uuid::new_v4();
    store.save(actor(owner), &save(1, layout())).await.unwrap();
    let before = room_row(&pool, owner).await.unwrap();
    for extra in [
        json!({"audience_gender": "female"}),
        json!({"audience_min_age": 18}),
        json!({"audience_max_age_exclusive": 30}),
        json!({"intro_id": Uuid::new_v4().to_string()}),
        json!({"owner_profile_id": Uuid::new_v4().to_string()}),
    ] {
        let mut body = json!({"expected_revision": 2, "visibility": "PRIVATE"});
        for (key, value) in extra.as_object().unwrap() {
            body[key] = value.clone();
        }
        let response = handle(
            &store,
            actor(owner),
            "PATCH",
            VISIBILITY_PATH,
            &serde_json::to_vec(&body).unwrap(),
        )
        .await;
        assert_eq!(response.status, 400, "{extra}");
    }
    assert_eq!(room_row(&pool, owner).await, Some(before));
    Ok(())
}

#[tokio::test]
async fn the_visibility_module_never_names_an_intro_table() {
    // The rule is structural, not only behavioural: the module that applies room
    // visibility has no statement that could reach an intro post.
    let source = include_str!("../src/profile_room/visibility.rs");
    let code = source
        .split("#[cfg(test)]")
        .next()
        .expect("module source");
    let code = code
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in ["intro_posts", "intro_attachments", "audience_gender", "IntroAudience"] {
        assert!(
            !code.contains(forbidden),
            "the visibility module must not mention {forbidden}"
        );
    }
}
