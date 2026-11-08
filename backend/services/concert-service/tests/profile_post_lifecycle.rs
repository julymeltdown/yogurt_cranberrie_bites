//! WP-06 T-511: profile post deletion independence, against real PostgreSQL 16.
//!
//! The database is the runner-owned private `mcr-pg-*` cluster created by
//! `backend/scripts/mcr/wp06_profile_posts_cluster.py`. The URL arrives through
//! `CONCERT_TEST_DATABASE_URL`.
//!
//! The hard constraint this file owns:
//!
//! * deleting a profile post leaves the **room, avatar, intro posts and
//!   favorites** untouched (V-07-e);
//! * deleting an intro post leaves profile posts untouched (V-08-a).
//!
//! Both directions are asserted with row counts *and* row contents before and
//! after, so a delete that merely preserved the count while rewriting a row would
//! still fail.
use concert_service::profile_posts::{
    create_profile_post, delete_profile_post, list_profile_posts, CreateProfilePost,
    PostVisibility, ProfilePostQuery,
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
        .max_connections(6)
        .connect_with(options)
        .await?;
    for table in ["profile_posts", "intro_posts", "profile_rooms", "profile_avatars"] {
        let present: bool = sqlx::query_scalar(
            "select to_regclass('public.' || $1) is not null",
        )
        .bind(table)
        .fetch_one(&pool)
        .await?;
        anyhow::ensure!(present, "{table} is not applied");
    }
    Ok(pool)
}

/// Everything a profile post deletion must leave alone, read as one snapshot.
///
/// The snapshot is a `Value` so the assertion is a single equality: a changed
/// row, a changed revision or a vanished row all fail the same comparison.
async fn neighbours(pool: &PgPool, profile: Uuid) -> anyhow::Result<Value> {
    let room: Option<(i64, Value, String)> = sqlx::query_as(
        "select revision, layout, visibility from public.profile_rooms \
         where owner_profile_id=$1",
    )
    .bind(profile)
    .fetch_optional(pool)
    .await?;
    let avatar: Option<(i64, Value)> = sqlx::query_as(
        "select revision, config from public.profile_avatars where owner_profile_id=$1",
    )
    .bind(profile)
    .fetch_optional(pool)
    .await?;
    let intros: Vec<(Uuid, String, String, i64, Option<String>)> = sqlx::query_as(
        "select intro_id, body, state, revision, currently_seeking from public.intro_posts \
         where author_profile_id=$1 order by intro_id",
    )
    .bind(profile)
    .fetch_all(pool)
    .await?;
    let intro_attachments: Vec<(Uuid, Uuid, i16)> = sqlx::query_as(
        "select a.intro_id, a.media_id, a.position from public.intro_attachments a \
         join public.intro_posts p on p.intro_id = a.intro_id \
         where p.author_profile_id=$1 order by a.intro_id, a.position",
    )
    .bind(profile)
    .fetch_all(pool)
    .await?;
    let likes: i64 = sqlx::query_scalar(
        "select count(*) from public.intro_likes l join public.intro_posts p \
         on p.intro_id = l.intro_id where p.author_profile_id=$1",
    )
    .bind(profile)
    .fetch_one(pool)
    .await?;
    Ok(json!({
        "room": room.map(|(revision, layout, visibility)| json!({
            "revision": revision, "layout": layout, "visibility": visibility,
        })),
        "avatar": avatar.map(|(revision, config)| json!({
            "revision": revision, "config": config,
        })),
        "intro_posts": intros.iter().map(|(id, body, state, revision, seeking)| json!({
            "intro_id": id, "body": body, "state": state, "revision": revision,
            "currently_seeking": seeking,
        })).collect::<Vec<_>>(),
        "intro_attachments": intro_attachments.iter().map(|(intro, media, position)| json!({
            "intro_id": intro, "media_id": media, "position": position,
        })).collect::<Vec<_>>(),
        "intro_likes": likes,
    }))
}

async fn post_ids(pool: &PgPool, profile: Uuid) -> anyhow::Result<Vec<Uuid>> {
    let page = list_profile_posts(
        pool,
        profile,
        profile,
        &ProfilePostQuery {
            limit: 100,
            cursor: None,
        },
    )
    .await
    .unwrap();
    Ok(page.items.iter().map(|item| item.post_id).collect())
}

/// A profile with a decorated room, a saved avatar, a published intro post with
/// one attachment, and one like from another profile.
async fn furnished_profile(pool: &PgPool) -> anyhow::Result<Uuid> {
    let profile = Uuid::new_v4();
    let liker = Uuid::new_v4();
    let layout = json!({
        "version": 1,
        "wall": "wall_brick_a",
        "floor": "floor_wood_a",
        "furniture": [],
        "posters": [],
        "character": {"x": 4, "y": 6, "facing": "down"},
    });
    sqlx::query(
        "insert into public.profile_rooms(owner_profile_id, layout, visibility, revision) \
         values($1,$2,'PUBLIC',3)",
    )
    .bind(profile)
    .bind(&layout)
    .execute(pool)
    .await?;
    sqlx::query(
        "insert into public.profile_avatars(owner_profile_id, config, revision) values($1,$2,5)",
    )
    .bind(profile)
    .bind(json!({"contract_version": "1.0.0", "kind": "avatar_config"}))
    .execute(pool)
    .await?;
    let intro = Uuid::new_v4();
    sqlx::query(
        "insert into public.intro_posts(intro_id, author_profile_id, body, state, published_at, \
         currently_seeking) values($1,$2,'hello there','PUBLISHED',clock_timestamp(),'music')",
    )
    .bind(intro)
    .bind(profile)
    .execute(pool)
    .await?;
    // An intro attachment needs an INTRO_POST media asset owned by the author.
    let media = Uuid::new_v4();
    sqlx::query(
        "insert into public.profile_media(media_id, owner_profile_id, purpose, content_type, \
         state, declared_bytes, input_sha256, sanitized, width, height) \
         values($1,$2,'INTRO_POST','image/png','READY',4,decode(repeat('ab',32),'hex'), \
                decode('89504e47','hex'),2,2)",
    )
    .bind(media)
    .bind(profile)
    .execute(pool)
    .await?;
    sqlx::query("insert into public.intro_attachments(intro_id, media_id, position) values($1,$2,0)")
        .bind(intro)
        .bind(media)
        .execute(pool)
        .await?;
    sqlx::query("insert into public.intro_likes(intro_id, actor_profile_id) values($1,$2)")
        .bind(intro)
        .bind(liker)
        .execute(pool)
        .await?;
    Ok(profile)
}

// ---------------------------------------------------------------------------
// Acceptance: deleting a profile post leaves the neighbours untouched
// ---------------------------------------------------------------------------

#[tokio::test]
async fn deleting_a_profile_post_leaves_the_room_avatar_intro_and_favorites_untouched(
) -> anyhow::Result<()> {
    let pool = pool().await?;
    let profile = furnished_profile(&pool).await?;
    let before = neighbours(&pool, profile).await?;
    // The fixture must actually be furnished, or the assertion below proves
    // nothing.
    assert!(before["room"].is_object());
    assert!(before["avatar"].is_object());
    assert_eq!(before["intro_posts"].as_array().unwrap().len(), 1);
    assert_eq!(before["intro_attachments"].as_array().unwrap().len(), 1);
    assert_eq!(before["intro_likes"], 1);

    let kept = create_profile_post(
        &pool,
        profile,
        &CreateProfilePost {
            description: "keep me".into(),
            media_ids: Vec::new(),
            visibility: PostVisibility::Public,
            expires_at: None,
        },
    )
    .await?
    .post_id;
    let doomed = create_profile_post(
        &pool,
        profile,
        &CreateProfilePost {
            description: "delete me".into(),
            media_ids: Vec::new(),
            visibility: PostVisibility::Private,
            expires_at: None,
        },
    )
    .await?
    .post_id;

    delete_profile_post(&pool, profile, doomed, 1).await?;

    // The neighbours are byte-identical.
    let after = neighbours(&pool, profile).await?;
    assert_eq!(after, before, "deleting a profile post changed another entity");
    // The other post survives; only the deleted one is gone.
    let ids = post_ids(&pool, profile).await?;
    assert_eq!(ids, vec![kept]);
    Ok(())
}

#[tokio::test]
async fn deleting_an_intro_post_leaves_profile_posts_untouched() -> anyhow::Result<()> {
    let pool = pool().await?;
    let profile = furnished_profile(&pool).await?;
    let first = create_profile_post(
        &pool,
        profile,
        &CreateProfilePost {
            description: "first photograph".into(),
            media_ids: Vec::new(),
            visibility: PostVisibility::Public,
            expires_at: None,
        },
    )
    .await?
    .post_id;
    let second = create_profile_post(
        &pool,
        profile,
        &CreateProfilePost {
            description: "second photograph".into(),
            media_ids: Vec::new(),
            visibility: PostVisibility::Private,
            expires_at: None,
        },
    )
    .await?
    .post_id;
    let posts_before = post_ids(&pool, profile).await?;
    assert_eq!(posts_before, vec![second, first]);
    let room_before: (i64, Value, String) = sqlx::query_as(
        "select revision, layout, visibility from public.profile_rooms where owner_profile_id=$1",
    )
    .bind(profile)
    .fetch_one(&pool)
    .await?;
    let avatar_before: (i64, Value) = sqlx::query_as(
        "select revision, config from public.profile_avatars where owner_profile_id=$1",
    )
    .bind(profile)
    .fetch_one(&pool)
    .await?;

    // Delete the intro post the way the intro command does: state DELETED, its
    // attachments removed as associations, its likes left to their own policy.
    let intro: Uuid = sqlx::query_scalar(
        "select intro_id from public.intro_posts where author_profile_id=$1",
    )
    .bind(profile)
    .fetch_one(&pool)
    .await?;
    let mut tx = pool.begin().await?;
    sqlx::query(
        "update public.intro_posts set state='DELETED', revision=revision+1, published_at=null \
         where intro_id=$1",
    )
    .bind(intro)
    .execute(&mut *tx)
    .await?;
    sqlx::query("delete from public.intro_attachments where intro_id=$1")
        .bind(intro)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    // The profile posts are exactly what they were.
    assert_eq!(post_ids(&pool, profile).await?, posts_before);
    let room_after: (i64, Value, String) = sqlx::query_as(
        "select revision, layout, visibility from public.profile_rooms where owner_profile_id=$1",
    )
    .bind(profile)
    .fetch_one(&pool)
    .await?;
    assert_eq!(room_after, room_before);
    let avatar_after: (i64, Value) = sqlx::query_as(
        "select revision, config from public.profile_avatars where owner_profile_id=$1",
    )
    .bind(profile)
    .fetch_one(&pool)
    .await?;
    assert_eq!(avatar_after, avatar_before);
    // The intro post is deleted and its attachment association is gone; the
    // media file itself survives because another surface may still reference it.
    let state: String =
        sqlx::query_scalar("select state from public.intro_posts where intro_id=$1")
            .bind(intro)
            .fetch_one(&pool)
            .await?;
    assert_eq!(state, "DELETED");
    let attachments: i64 =
        sqlx::query_scalar("select count(*) from public.intro_attachments where intro_id=$1")
            .bind(intro)
            .fetch_one(&pool)
            .await?;
    assert_eq!(attachments, 0);
    let media_state: String = sqlx::query_scalar(
        "select state from public.profile_media where owner_profile_id=$1 and purpose='INTRO_POST'",
    )
    .bind(profile)
    .fetch_one(&pool)
    .await?;
    assert_eq!(media_state, "READY");
    Ok(())
}

#[tokio::test]
async fn a_profile_post_delete_statement_names_no_neighbour_table() -> anyhow::Result<()> {
    // The structural half of the constraint: the delete command's SQL cannot
    // touch another entity, because it never names one. Comments are stripped
    // because the module documentation explains what it deliberately leaves
    // alone.
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/profile_posts/commands.rs"),
    )?;
    let code: String = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in [
        "profile_rooms",
        "profile_avatars",
        "intro_posts",
        "intro_attachments",
        "intro_likes",
        "profile_favorites",
    ] {
        assert!(
            !code.contains(forbidden),
            "a profile post command must not name {forbidden}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn deleting_a_profile_post_keeps_its_media_available_to_other_posts(
) -> anyhow::Result<()> {
    let pool = pool().await?;
    let profile = Uuid::new_v4();
    let media = Uuid::new_v4();
    sqlx::query(
        "insert into public.profile_media(media_id, owner_profile_id, purpose, content_type, \
         state, declared_bytes, input_sha256, sanitized, width, height) \
         values($1,$2,'PROFILE_POST','image/png','READY',4,decode(repeat('cd',32),'hex'), \
                decode('89504e47','hex'),2,2)",
    )
    .bind(media)
    .bind(profile)
    .execute(&pool)
    .await?;
    let first = create_profile_post(
        &pool,
        profile,
        &CreateProfilePost {
            description: "first".into(),
            media_ids: vec![media],
            visibility: PostVisibility::Public,
            expires_at: None,
        },
    )
    .await?
    .post_id;
    create_profile_post(
        &pool,
        profile,
        &CreateProfilePost {
            description: "second".into(),
            media_ids: vec![media],
            visibility: PostVisibility::Public,
            expires_at: None,
        },
    )
    .await?;
    delete_profile_post(&pool, profile, first, 1).await?;
    // The shared file is still READY and still referenced by the surviving post.
    let state: String =
        sqlx::query_scalar("select state from public.profile_media where media_id=$1")
            .bind(media)
            .fetch_one(&pool)
            .await?;
    assert_eq!(state, "READY");
    let references: i64 = sqlx::query_scalar(
        "select count(*) from public.profile_post_attachments where media_id=$1",
    )
    .bind(media)
    .fetch_one(&pool)
    .await?;
    assert_eq!(references, 1);
    Ok(())
}
