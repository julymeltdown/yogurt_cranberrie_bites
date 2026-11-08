//! WP-06 T-502: profile post commands against real PostgreSQL 16.
//!
//! The database is the runner-owned private `mcr-pg-*` cluster created by
//! `backend/scripts/mcr/wp06_profile_posts_cluster.py`. The URL arrives through
//! `CONCERT_TEST_DATABASE_URL`.
//!
//! The hard constraints this file owns:
//!
//! * a post with an `INTRO_POST` media id is **rejected** (V-06-d);
//! * a foreign media id is rejected without revealing whether it exists
//!   (V-06-c) — the answer is the same `404` an absent id gets;
//! * the stored bytes are the normalized photograph, not a pixel conversion
//!   (V-07-b), and this module contains no conversion call at all;
//! * publishing works with a default room and no intro post (V-07-a);
//! * no poster record is created as a side effect (V-07-c);
//! * at most four attachments, unique ids, `READY` state, description 0–500.
use concert_application::media::{InitUpload, Mime, Purpose};
use concert_service::adapters::out::media::codec::BoundedImageSanitizer;
use concert_service::profile_media::{ProfileMediaId, ProfileMediaStore, UploadPurpose};
use concert_service::profile_posts::{
    create_profile_post, delete_profile_post, CreateProfilePost, PostVisibility, ProfilePostError,
    MAX_ATTACHMENTS, MAX_DESCRIPTION,
};
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use platform::cpu::WorkerPool;
use serde_json::json;
use sha2::{Digest, Sha256};
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
        .max_connections(6)
        .connect_with(options)
        .await?;
    let applied: bool =
        sqlx::query_scalar("select to_regclass('public.profile_posts') is not null")
            .fetch_one(&pool)
            .await?;
    anyhow::ensure!(applied, "migration 0084_profile_posts is not applied");
    Ok(pool)
}

fn media_store(pool: PgPool) -> ProfileMediaStore {
    ProfileMediaStore::new(
        pool,
        Arc::new(BoundedImageSanitizer::new(
            WorkerPool::new(1).unwrap().class(0).unwrap(),
        )),
    )
}

fn photograph(width: u32, height: u32) -> Vec<u8> {
    let mut image = RgbImage::new(width, height);
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        *pixel = Rgb([(x * 7 % 256) as u8, (y * 11 % 256) as u8, ((x + y) % 256) as u8]);
    }
    let mut bytes = Vec::new();
    DynamicImage::ImageRgb8(image)
        .write_to(&mut std::io::Cursor::new(&mut bytes), ImageFormat::Png)
        .unwrap();
    bytes
}

/// Upload and finalize one asset in `context`.
async fn ready_media(
    store: &ProfileMediaStore,
    owner: Uuid,
    context: UploadPurpose,
) -> anyhow::Result<Uuid> {
    let bytes = photograph(24, 18);
    let intent = store
        .init(
            owner,
            context,
            InitUpload {
                purpose: context.purpose(),
                mime: Mime::Png,
                byte_length: bytes.len(),
                sha256: Sha256::digest(&bytes).into(),
            },
        )
        .await
        .unwrap();
    let id = ProfileMediaId::from_uuid(intent.media_id).unwrap();
    let revision = store
        .put(owner, context, id, Mime::Png, bytes)
        .await
        .unwrap();
    store
        .finalize(owner, context, id, revision)
        .await
        .unwrap();
    Ok(intent.media_id)
}

fn command(description: &str, media_ids: Vec<Uuid>, visibility: PostVisibility) -> CreateProfilePost {
    CreateProfilePost {
        description: description.to_owned(),
        media_ids,
        visibility,
        expires_at: None,
    }
}

async fn post_count(pool: &PgPool, author: Uuid) -> i64 {
    sqlx::query_scalar("select count(*) from public.profile_posts where author_profile_id=$1")
        .bind(author)
        .fetch_one(pool)
        .await
        .unwrap()
}

// ---------------------------------------------------------------------------
// Acceptance: a post with an INTRO_POST media id is rejected
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_post_with_an_intro_post_media_id_is_rejected() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = media_store(pool.clone());
    let author = Uuid::new_v4();
    // The author uploads a photograph through the intro composer. It is theirs,
    // it is READY, and it is purpose INTRO_POST.
    let intro_media = ready_media(&store, author, UploadPurpose::IntroPost).await?;
    let result = create_profile_post(
        &pool,
        author,
        &command("republishing a restricted attachment", vec![intro_media], PostVisibility::Public),
    )
    .await;
    assert_eq!(result, Err(ProfilePostError::AttachmentNotPublishable));
    // No post was created, and no association exists.
    assert_eq!(post_count(&pool, author).await, 0);
    let attached: i64 = sqlx::query_scalar(
        "select count(*) from public.profile_post_attachments where media_id=$1",
    )
    .bind(intro_media)
    .fetch_one(&pool)
    .await?;
    assert_eq!(attached, 0);
    // The database refuses the same row independently of this module: even a
    // direct insert of the association is refused by the purpose trigger.
    let post = Uuid::new_v4();
    sqlx::query(
        "insert into public.profile_posts(post_id, author_profile_id, description) \
         values($1,$2,'direct')",
    )
    .bind(post)
    .bind(author)
    .execute(&pool)
    .await?;
    let error = sqlx::query(
        "insert into public.profile_post_attachments(post_id, media_id, position) values($1,$2,0)",
    )
    .bind(post)
    .bind(intro_media)
    .execute(&pool)
    .await
    .expect_err("the purpose trigger must refuse an intro attachment");
    let constraint = match &error {
        sqlx::Error::Database(database) => database.constraint().map(str::to_owned),
        _ => None,
    };
    assert_eq!(constraint.as_deref(), Some("profile_post_attachments_purpose"));
    Ok(())
}

#[tokio::test]
async fn a_room_poster_or_profile_image_media_id_is_also_rejected() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = media_store(pool.clone());
    let author = Uuid::new_v4();
    for context in [UploadPurpose::RoomPoster, UploadPurpose::ProfileImage] {
        let media = ready_media(&store, author, context).await?;
        assert_eq!(
            create_profile_post(
                &pool,
                author,
                &command("not a profile post asset", vec![media], PostVisibility::Public),
            )
            .await,
            Err(ProfilePostError::AttachmentNotPublishable),
            "{context:?}"
        );
    }
    assert_eq!(post_count(&pool, author).await, 0);
    Ok(())
}

// ---------------------------------------------------------------------------
// Acceptance: a foreign media id is rejected without revealing existence
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_foreign_media_id_is_rejected_without_revealing_whether_it_exists(
) -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = media_store(pool.clone());
    let author = Uuid::new_v4();
    let stranger = Uuid::new_v4();
    // The stranger owns a perfectly valid PROFILE_POST asset.
    let foreign = ready_media(&store, stranger, UploadPurpose::ProfilePost).await?;
    // The author owns nothing with that id.
    let absent = Uuid::new_v4();
    let foreign_result = create_profile_post(
        &pool,
        author,
        &command("someone else's photo", vec![foreign], PostVisibility::Public),
    )
    .await;
    let absent_result = create_profile_post(
        &pool,
        author,
        &command("a photo that does not exist", vec![absent], PostVisibility::Public),
    )
    .await;
    // The two answers are identical: no existence oracle.
    assert_eq!(foreign_result, absent_result);
    assert_eq!(foreign_result, Err(ProfilePostError::Unavailable));
    assert_eq!(ProfilePostError::Unavailable.status(), 404);
    assert_eq!(ProfilePostError::Unavailable.reason(), "RESOURCE_UNAVAILABLE");
    // The stranger's asset is untouched and still READY.
    let state: String =
        sqlx::query_scalar("select state from public.profile_media where media_id=$1")
            .bind(foreign)
            .fetch_one(&pool)
            .await?;
    assert_eq!(state, "READY");
    assert_eq!(post_count(&pool, author).await, 0);
    Ok(())
}

#[tokio::test]
async fn a_mixed_body_with_one_foreign_id_creates_no_post() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = media_store(pool.clone());
    let author = Uuid::new_v4();
    let stranger = Uuid::new_v4();
    let own = ready_media(&store, author, UploadPurpose::ProfilePost).await?;
    let foreign = ready_media(&store, stranger, UploadPurpose::ProfilePost).await?;
    assert_eq!(
        create_profile_post(
            &pool,
            author,
            &command("mixed", vec![own, foreign], PostVisibility::Public),
        )
        .await,
        Err(ProfilePostError::Unavailable)
    );
    // The whole command is one transaction: the valid attachment did not land.
    assert_eq!(post_count(&pool, author).await, 0);
    let attached: i64 = sqlx::query_scalar(
        "select count(*) from public.profile_post_attachments where media_id=$1",
    )
    .bind(own)
    .fetch_one(&pool)
    .await?;
    assert_eq!(attached, 0);
    Ok(())
}

// ---------------------------------------------------------------------------
// Acceptance: publishing works with a default room and no intro post
// ---------------------------------------------------------------------------

#[tokio::test]
async fn publishing_works_with_a_default_room_and_no_intro_post() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = media_store(pool.clone());
    let author = Uuid::new_v4();
    // The profile has never published an intro post.
    let intros: i64 =
        sqlx::query_scalar("select count(*) from public.intro_posts where author_profile_id=$1")
            .bind(author)
            .fetch_one(&pool)
            .await?;
    assert_eq!(intros, 0, "the fixture must start with no intro post");
    // Its room is in the default (undecorated, private) state.
    sqlx::query("insert into public.profile_rooms(owner_profile_id, layout) values($1, '{}')")
        .bind(author)
        .execute(&pool)
        .await?;
    let room_before: (i64, serde_json::Value, String) = sqlx::query_as(
        "select revision, layout, visibility from public.profile_rooms where owner_profile_id=$1",
    )
    .bind(author)
    .fetch_one(&pool)
    .await?;
    let media = ready_media(&store, author, UploadPurpose::ProfilePost).await?;
    let created = create_profile_post(
        &pool,
        author,
        &command("my first photo", vec![media], PostVisibility::Public),
    )
    .await
    .unwrap();
    assert_eq!(created.media_ids, vec![media]);
    assert_eq!(created.visibility, PostVisibility::Public);
    assert_eq!(created.revision, 1);
    // The room is untouched by publishing a photograph.
    let room_after: (i64, serde_json::Value, String) = sqlx::query_as(
        "select revision, layout, visibility from public.profile_rooms where owner_profile_id=$1",
    )
    .bind(author)
    .fetch_one(&pool)
    .await?;
    assert_eq!(room_before, room_after);
    // Still no intro post.
    let intros: i64 =
        sqlx::query_scalar("select count(*) from public.intro_posts where author_profile_id=$1")
            .bind(author)
            .fetch_one(&pool)
            .await?;
    assert_eq!(intros, 0);
    Ok(())
}

#[tokio::test]
async fn no_poster_record_is_created_as_a_side_effect() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = media_store(pool.clone());
    let author = Uuid::new_v4();
    let media = ready_media(&store, author, UploadPurpose::ProfilePost).await?;
    create_profile_post(
        &pool,
        author,
        &command("artwork", vec![media], PostVisibility::Public),
    )
    .await
    .unwrap();
    // No ROOM_POSTER asset exists for this author, and the room has no poster.
    let posters: i64 = sqlx::query_scalar(
        "select count(*) from public.profile_media where owner_profile_id=$1 and purpose='ROOM_POSTER'",
    )
    .bind(author)
    .fetch_one(&pool)
    .await?;
    assert_eq!(posters, 0, "publishing a photograph must not mint a poster");
    let rooms: i64 =
        sqlx::query_scalar("select count(*) from public.profile_rooms where owner_profile_id=$1")
            .bind(author)
            .fetch_one(&pool)
            .await?;
    assert_eq!(rooms, 0, "publishing a photograph must not create a room");
    // The media purpose is unchanged: still PROFILE_POST, never ROOM_POSTER.
    let purpose: String =
        sqlx::query_scalar("select purpose from public.profile_media where media_id=$1")
            .bind(media)
            .fetch_one(&pool)
            .await?;
    assert_eq!(purpose, "PROFILE_POST");
    Ok(())
}

// ---------------------------------------------------------------------------
// Acceptance: the stored bytes are the normalized photograph
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_stored_bytes_are_the_normalized_photograph_not_a_conversion(
) -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = media_store(pool.clone());
    let author = Uuid::new_v4();
    let source = photograph(120, 90);
    let intent = store
        .init(
            author,
            UploadPurpose::ProfilePost,
            InitUpload {
                purpose: Purpose::ProfilePost,
                mime: Mime::Png,
                byte_length: source.len(),
                sha256: Sha256::digest(&source).into(),
            },
        )
        .await
        .unwrap();
    let id = ProfileMediaId::from_uuid(intent.media_id).unwrap();
    let revision = store
        .put(author, UploadPurpose::ProfilePost, id, Mime::Png, source.clone())
        .await
        .unwrap();
    store
        .finalize(author, UploadPurpose::ProfilePost, id, revision)
        .await
        .unwrap();
    create_profile_post(
        &pool,
        author,
        &command("photograph", vec![intent.media_id], PostVisibility::Public),
    )
    .await
    .unwrap();
    // The bytes the post serves are the sanitized photograph: same pixels, same
    // dimensions, no palette reduction and no downscale.
    let stored: Vec<u8> =
        sqlx::query_scalar("select sanitized from public.profile_media where media_id=$1")
            .bind(intent.media_id)
            .fetch_one(&pool)
            .await?;
    let source_pixels = image::load_from_memory(&source).unwrap().to_rgb8();
    let stored_pixels = image::load_from_memory(&stored).unwrap().to_rgb8();
    assert_eq!(source_pixels.dimensions(), (120, 90));
    assert_eq!(stored_pixels.dimensions(), source_pixels.dimensions());
    assert_eq!(stored_pixels.into_raw(), source_pixels.into_raw());
    // The attachment projects the same dimensions, not a poster's 128 px cap.
    let (width, height): (i32, i32) = sqlx::query_as(
        "select width, height from public.profile_media where media_id=$1",
    )
    .bind(intent.media_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!((width, height), (120, 90));
    Ok(())
}

#[tokio::test]
async fn the_command_module_contains_no_pixel_conversion() -> anyhow::Result<()> {
    // The structural half of V-07-b: the post command module has no image code
    // at all, so no future edit can quietly add a conversion here. Comments are
    // stripped because the module documentation legitimately names what it does
    // not do.
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/profile_posts/commands.rs"),
    )?;
    let code: String = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in [
        "posterize",
        "palette",
        "quantiz",
        "resize",
        "DynamicImage",
        "image::",
        "sanitize",
        "convert",
    ] {
        assert!(
            !code.contains(forbidden),
            "the profile post command must contain no pixel conversion: {forbidden}"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Attachment count, uniqueness, readiness, description length
// ---------------------------------------------------------------------------

#[tokio::test]
async fn up_to_four_attachments_are_accepted_and_five_are_refused() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = media_store(pool.clone());
    let author = Uuid::new_v4();
    let mut ids = Vec::new();
    for _ in 0..5 {
        ids.push(ready_media(&store, author, UploadPurpose::ProfilePost).await?);
    }
    assert_eq!(MAX_ATTACHMENTS, 4);
    let created = create_profile_post(
        &pool,
        author,
        &command("four", ids[..4].to_vec(), PostVisibility::Public),
    )
    .await
    .unwrap();
    assert_eq!(created.media_ids, ids[..4].to_vec());
    let positions: Vec<i16> = sqlx::query_scalar(
        "select position from public.profile_post_attachments where post_id=$1 order by position",
    )
    .bind(created.post_id)
    .fetch_all(&pool)
    .await?;
    assert_eq!(positions, vec![0, 1, 2, 3]);
    // Five is refused by the parser and by the command.
    let five = ids.clone();
    let body = serde_json::to_vec(&json!({
        "description": "five",
        "media_ids": five,
        "visibility": "PUBLIC",
        "expires_at": null,
    }))
    .unwrap();
    assert_eq!(CreateProfilePost::parse(&body), Err(ProfilePostError::Invalid));
    // The database refuses a fifth slot independently.
    let error = sqlx::query(
        "insert into public.profile_post_attachments(post_id, media_id, position) values($1,$2,4)",
    )
    .bind(created.post_id)
    .bind(ids[4])
    .execute(&pool)
    .await
    .expect_err("a fifth attachment must be refused");
    let constraint = match &error {
        sqlx::Error::Database(database) => database.constraint().map(str::to_owned),
        _ => None,
    };
    assert_eq!(constraint.as_deref(), Some("profile_post_attachments_position"));
    Ok(())
}

#[tokio::test]
async fn a_duplicate_media_id_is_refused() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = media_store(pool.clone());
    let author = Uuid::new_v4();
    let media = ready_media(&store, author, UploadPurpose::ProfilePost).await?;
    let body = serde_json::to_vec(&json!({
        "description": "twice",
        "media_ids": [media, media],
        "visibility": "PUBLIC",
        "expires_at": null,
    }))
    .unwrap();
    assert_eq!(CreateProfilePost::parse(&body), Err(ProfilePostError::Invalid));
    assert_eq!(post_count(&pool, author).await, 0);
    Ok(())
}

#[tokio::test]
async fn a_media_id_that_is_not_ready_is_refused() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = media_store(pool.clone());
    let author = Uuid::new_v4();
    let bytes = photograph(8, 8);
    // A PENDING asset: init only.
    let pending = store
        .init(
            author,
            UploadPurpose::ProfilePost,
            InitUpload {
                purpose: Purpose::ProfilePost,
                mime: Mime::Png,
                byte_length: bytes.len(),
                sha256: Sha256::digest(&bytes).into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        create_profile_post(
            &pool,
            author,
            &command("pending", vec![pending.media_id], PostVisibility::Public),
        )
        .await,
        Err(ProfilePostError::AttachmentNotPublishable)
    );
    // An UPLOADED but unfinalized asset is refused too.
    let id = ProfileMediaId::from_uuid(pending.media_id).unwrap();
    store
        .put(author, UploadPurpose::ProfilePost, id, Mime::Png, bytes)
        .await
        .unwrap();
    assert_eq!(
        create_profile_post(
            &pool,
            author,
            &command("uploaded", vec![pending.media_id], PostVisibility::Public),
        )
        .await,
        Err(ProfilePostError::AttachmentNotPublishable)
    );
    assert_eq!(post_count(&pool, author).await, 0);
    Ok(())
}

#[tokio::test]
async fn the_description_is_bounded_at_five_hundred_code_points() -> anyhow::Result<()> {
    assert_eq!(MAX_DESCRIPTION, 500);
    // A 500-code-point description parses; 501 does not. The count is in Unicode
    // code points, so a multi-byte character counts once.
    let at_limit = "가".repeat(500);
    let body = serde_json::to_vec(&json!({
        "description": at_limit,
        "media_ids": [],
        "visibility": "PUBLIC",
        "expires_at": null,
    }))
    .unwrap();
    let parsed = CreateProfilePost::parse(&body).unwrap();
    assert_eq!(parsed.description.chars().count(), 500);
    let over = "가".repeat(501);
    let body = serde_json::to_vec(&json!({
        "description": over,
        "media_ids": [],
        "visibility": "PUBLIC",
        "expires_at": null,
    }))
    .unwrap();
    assert_eq!(CreateProfilePost::parse(&body), Err(ProfilePostError::Invalid));
    Ok(())
}

#[tokio::test]
async fn the_create_body_is_closed() -> anyhow::Result<()> {
    // Every field is required and an unknown field is refused, never ignored.
    for body in [
        json!({"description": "x", "media_ids": [], "visibility": "PUBLIC"}),
        json!({"description": "x", "media_ids": [], "visibility": "PUBLIC", "expires_at": null, "intro_id": null}),
        json!({"description": "x", "media_ids": [], "visibility": "FRIENDS", "expires_at": null}),
        json!({"description": "x", "media_ids": [], "visibility": "PUBLIC", "expires_at": "tomorrow"}),
        json!({"description": 5, "media_ids": [], "visibility": "PUBLIC", "expires_at": null}),
        json!({"description": "x", "media_ids": ["not-a-uuid"], "visibility": "PUBLIC", "expires_at": null}),
        json!({"description": "x", "media_ids": [], "visibility": "PUBLIC", "expires_at": null, "audience": null}),
    ] {
        assert_eq!(
            CreateProfilePost::parse(&serde_json::to_vec(&body).unwrap()),
            Err(ProfilePostError::Invalid),
            "{body}"
        );
    }
    // A text-only post is valid: a profile post does not require a photograph.
    let body = json!({"description": "just words", "media_ids": [], "visibility": "PUBLIC", "expires_at": null});
    let parsed = CreateProfilePost::parse(&serde_json::to_vec(&body).unwrap()).unwrap();
    assert!(parsed.media_ids.is_empty());
    Ok(())
}

#[tokio::test]
async fn a_private_post_and_an_expiring_post_are_stored_as_asked() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = Uuid::new_v4();
    let expires = chrono::Utc::now() + chrono::Duration::hours(1);
    let created = create_profile_post(
        &pool,
        author,
        &CreateProfilePost {
            description: "private".into(),
            media_ids: Vec::new(),
            visibility: PostVisibility::Private,
            expires_at: Some(expires),
        },
    )
    .await
    .unwrap();
    assert_eq!(created.visibility, PostVisibility::Private);
    let (visibility, state, stored_expiry): (String, String, Option<chrono::DateTime<chrono::Utc>>) =
        sqlx::query_as(
            "select visibility, state, expires_at from public.profile_posts where post_id=$1",
        )
        .bind(created.post_id)
        .fetch_one(&pool)
        .await?;
    assert_eq!(visibility, "PRIVATE");
    assert_eq!(state, "PUBLISHED");
    assert_eq!(stored_expiry.map(|value| value.timestamp()), Some(expires.timestamp()));
    Ok(())
}

// ---------------------------------------------------------------------------
// Delete
// ---------------------------------------------------------------------------

#[tokio::test]
async fn delete_requires_the_current_revision_and_the_author() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = Uuid::new_v4();
    let stranger = Uuid::new_v4();
    let created = create_profile_post(
        &pool,
        author,
        &command("mine", Vec::new(), PostVisibility::Public),
    )
    .await
    .unwrap();
    // A stale revision is a conflict and writes nothing.
    assert_eq!(
        delete_profile_post(&pool, author, created.post_id, 7).await,
        Err(ProfilePostError::Conflict)
    );
    assert_eq!(ProfilePostError::Conflict.status(), 409);
    // A stranger gets the same answer as an absent post.
    assert_eq!(
        delete_profile_post(&pool, stranger, created.post_id, 1).await,
        Err(ProfilePostError::Unavailable)
    );
    assert_eq!(
        delete_profile_post(&pool, stranger, Uuid::new_v4(), 1).await,
        Err(ProfilePostError::Unavailable)
    );
    // The author deletes at the right revision.
    let deleted = delete_profile_post(&pool, author, created.post_id, 1)
        .await
        .unwrap();
    assert_eq!(deleted.revision, 2);
    assert_eq!(
        deleted.to_wire(),
        json!({"resource_id": created.post_id, "revision": 2, "state": "DELETED"})
    );
    // A second delete is unavailable: the post is gone.
    assert_eq!(
        delete_profile_post(&pool, author, created.post_id, 2).await,
        Err(ProfilePostError::Unavailable)
    );
    Ok(())
}

#[tokio::test]
async fn deleting_a_post_removes_the_association_and_keeps_the_media_file(
) -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = media_store(pool.clone());
    let author = Uuid::new_v4();
    let media = ready_media(&store, author, UploadPurpose::ProfilePost).await?;
    let first = create_profile_post(
        &pool,
        author,
        &command("first", vec![media], PostVisibility::Public),
    )
    .await
    .unwrap();
    let second = create_profile_post(
        &pool,
        author,
        &command("second", vec![media], PostVisibility::Public),
    )
    .await
    .unwrap();
    delete_profile_post(&pool, author, first.post_id, 1)
        .await
        .unwrap();
    // The first post's association is gone; the second post's is intact.
    let remaining: i64 = sqlx::query_scalar(
        "select count(*) from public.profile_post_attachments where media_id=$1",
    )
    .bind(media)
    .fetch_one(&pool)
    .await?;
    assert_eq!(remaining, 1);
    let owner_post: Uuid = sqlx::query_scalar(
        "select post_id from public.profile_post_attachments where media_id=$1",
    )
    .bind(media)
    .fetch_one(&pool)
    .await?;
    assert_eq!(owner_post, second.post_id);
    // The file itself survives because a live post still references it.
    let state: String =
        sqlx::query_scalar("select state from public.profile_media where media_id=$1")
            .bind(media)
            .fetch_one(&pool)
            .await?;
    assert_eq!(state, "READY");
    Ok(())
}
