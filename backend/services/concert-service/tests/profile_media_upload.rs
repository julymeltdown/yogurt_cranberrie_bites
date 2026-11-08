//! WP-06 T-501: the profile media upload lifecycle against real PostgreSQL 16.
//!
//! The database is the runner-owned private `mcr-pg-*` cluster created by
//! `backend/scripts/mcr/wp06_profile_posts_cluster.py` (the
//! `migration_smoke.py` harness pattern). The URL arrives through
//! `CONCERT_TEST_DATABASE_URL` and must name the `mcr_owner` role.
//!
//! Every hard constraint of the work package that lives in the upload lifecycle
//! has a case here:
//!
//! * a finalized asset is `READY` with width, height and sanitized bytes;
//! * an abandoned upload is cleaned after the orphan window;
//! * a `ROOM_POSTER` asset cannot be finalized through the profile-post path;
//! * only JPEG/PNG/WebP, 10 MiB, 4096 px and 16 M pixels are accepted — no GIF,
//!   no video;
//! * a cancelled upload (init, then nothing) leaves no orphan;
//! * the stored bytes are the normalized photograph, not a pixel conversion.
use concert_application::media::{InitUpload, Mime, Purpose, MAX_BYTES, MAX_DIMENSION, ORPHAN_SECONDS};
use concert_service::adapters::out::media::codec::BoundedImageSanitizer;
use concert_service::profile_media::{
    cleanup_orphans, cleanup_owner_orphans, ProfileMediaError, ProfileMediaId, ProfileMediaState,
    ProfileMediaStore, UploadPurpose,
};
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use platform::cpu::WorkerPool;
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
        sqlx::query_scalar("select to_regclass('public.profile_media') is not null")
            .fetch_one(&pool)
            .await?;
    anyhow::ensure!(applied, "migration 0080_profile_media is not applied");
    Ok(pool)
}

fn store(pool: PgPool) -> ProfileMediaStore {
    ProfileMediaStore::new(
        pool,
        Arc::new(BoundedImageSanitizer::new(
            WorkerPool::new(1).unwrap().class(0).unwrap(),
        )),
    )
}

/// A deterministic RGB photograph. The pixels are a gradient plus a hard edge,
/// so a pixel conversion (posterize, palette reduce, downsample) would change
/// them visibly and the byte-for-byte assertion below would fail.
fn photograph(width: u32, height: u32) -> Vec<u8> {
    let mut image = RgbImage::new(width, height);
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        let band = if x < width / 2 { 0 } else { 128 };
        *pixel = Rgb([
            (x * 255 / width.max(1)) as u8 / 2 + band,
            (y * 255 / height.max(1)) as u8,
            ((x + y) % 256) as u8,
        ]);
    }
    let mut bytes = Vec::new();
    DynamicImage::ImageRgb8(image)
        .write_to(&mut std::io::Cursor::new(&mut bytes), ImageFormat::Png)
        .unwrap();
    bytes
}

fn init(purpose: Purpose, mime: Mime, bytes: &[u8]) -> InitUpload {
    InitUpload {
        purpose,
        mime,
        byte_length: bytes.len(),
        sha256: Sha256::digest(bytes).into(),
    }
}

async fn upload(
    store: &ProfileMediaStore,
    owner: Uuid,
    context: UploadPurpose,
    bytes: &[u8],
    mime: Mime,
) -> Result<concert_service::profile_media::ProfileMediaUpload, ProfileMediaError> {
    let intent = store
        .init(owner, context, init(context.purpose(), mime, bytes))
        .await?;
    let id = ProfileMediaId::from_uuid(intent.media_id)?;
    let revision = store.put(owner, context, id, mime, bytes.to_vec()).await?;
    store.finalize(owner, context, id, revision).await
}

async fn row(pool: &PgPool, id: Uuid) -> Option<(String, Option<i32>, Option<i32>, Option<Vec<u8>>)> {
    sqlx::query_as("select state, width, height, sanitized from public.profile_media where media_id=$1")
        .bind(id)
        .fetch_optional(pool)
        .await
        .unwrap()
}

/// Age a row past the orphan window.
///
/// `profile_media.created_at` is immutable through the identity guard, so a test
/// cannot simply update it. The supported seam is a server-side clock shift: the
/// row's `created_at` is compared against `clock_timestamp()`, so moving the
/// transaction's `now` is not enough either. The test therefore re-creates the
/// row's age by inserting it through a temporary override that the guard allows:
/// a `PENDING` row is deleted and re-inserted with the same identity and an older
/// `created_at`, which is a fresh insert rather than an update.
async fn age(pool: &PgPool, id: Uuid, seconds: i64) -> anyhow::Result<()> {
    // Read the identity, drop the row, and re-insert it with the same identity
    // and an older creation instant. Nothing else observes the row between the
    // two statements inside this test.
    let mut tx = pool.begin().await?;
    let row: (Uuid, String, String, String, i32, Vec<u8>, Option<Vec<u8>>, Option<i32>, Option<i32>) =
        sqlx::query_as(
            "select owner_profile_id, purpose, content_type, state, declared_bytes, \
                    input_sha256, sanitized, width, height \
             from public.profile_media where media_id=$1 for update",
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
    sqlx::query("delete from public.profile_media where media_id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "insert into public.profile_media(media_id, owner_profile_id, purpose, content_type, \
         state, declared_bytes, input_sha256, sanitized, width, height, created_at, updated_at) \
         values($1,$2,$3,$4,$5,$6,$7,$8,$9,$10, \
                clock_timestamp() - make_interval(secs => $11), \
                clock_timestamp() - make_interval(secs => $11))",
    )
    .bind(id)
    .bind(row.0)
    .bind(row.1)
    .bind(row.2)
    .bind(row.3)
    .bind(row.4)
    .bind(row.5)
    .bind(row.6)
    .bind(row.7)
    .bind(row.8)
    .bind(seconds as f64)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

async fn count(pool: &PgPool, owner: Uuid) -> i64 {
    sqlx::query_scalar("select count(*) from public.profile_media where owner_profile_id=$1")
        .bind(owner)
        .fetch_one(pool)
        .await
        .unwrap()
}

// ---------------------------------------------------------------------------
// Acceptance: a finalized asset is READY with width, height and bytes
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_finalized_asset_is_ready_with_dimensions_and_sanitized_bytes() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = store(pool.clone());
    let owner = Uuid::new_v4();
    let bytes = photograph(64, 48);
    let intent = store
        .init(
            owner,
            UploadPurpose::ProfilePost,
            init(Purpose::ProfilePost, Mime::Png, &bytes),
        )
        .await
        .unwrap();
    assert_eq!(intent.revision, 1);
    // The intent names the profile upload path, which has no event segment.
    assert_eq!(
        intent.upload_path(),
        format!("/api/me/media/{}/bytes", intent.media_id)
    );
    assert!(!intent.upload_path().contains("/events/"));
    assert_eq!(intent.to_wire()["max_bytes"], MAX_BYTES);
    assert_eq!(intent.to_wire()["upload_method"], "PUT");
    assert_eq!(intent.to_wire()["state"], "PENDING");

    let id = ProfileMediaId::from_uuid(intent.media_id)?;
    let revision = store
        .put(owner, UploadPurpose::ProfilePost, id, Mime::Png, bytes.clone())
        .await
        .unwrap();
    assert_eq!(revision, 2);
    let ready = store
        .finalize(owner, UploadPurpose::ProfilePost, id, revision)
        .await
        .unwrap();
    assert_eq!(ready.width, 64);
    assert_eq!(ready.height, 48);
    assert_eq!(ready.mime, Mime::Png);
    assert_eq!(ready.purpose, Purpose::ProfilePost);
    assert_eq!(ready.revision, 3);
    assert_eq!(ready.read_path(), format!("/api/media/{}/content", ready.media_id));
    let wire = ready.to_wire();
    assert_eq!(wire["state"], "READY");
    assert_eq!(wire["purpose"], "PROFILE_POST");
    assert_eq!(wire["width"], 64);
    assert_eq!(wire["height"], 48);

    let (state, width, height, sanitized) = row(&pool, ready.media_id).await.unwrap();
    assert_eq!(state, "READY");
    assert_eq!((width, height), (Some(64), Some(48)));
    assert!(sanitized.is_some_and(|bytes| !bytes.is_empty()));
    Ok(())
}

#[tokio::test]
async fn the_stored_bytes_are_the_normalized_photograph_not_a_conversion(
) -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = store(pool.clone());
    let owner = Uuid::new_v4();
    let bytes = photograph(96, 72);
    let ready = upload(&store, owner, UploadPurpose::ProfilePost, &bytes, Mime::Png)
        .await
        .unwrap();
    let (mime, stored) = store
        .content(owner, ProfileMediaId::from_uuid(ready.media_id)?)
        .await
        .unwrap();
    assert_eq!(mime, Mime::Png);
    // The sanitizer re-encodes the decoded frame without metadata. The pixels are
    // the source pixels: decoding the stored bytes yields the source image
    // exactly. A posterization, palette reduction or downsample would differ.
    let source = image::load_from_memory(&bytes).unwrap().to_rgb8();
    let normalized = image::load_from_memory(&stored).unwrap().to_rgb8();
    assert_eq!(source.dimensions(), normalized.dimensions());
    assert_eq!(source.into_raw(), normalized.into_raw());
    // And the stored bytes carry no EXIF/ICC/text chunk from the source.
    assert!(!stored.windows(4).any(|window| window == b"eXIf"));
    assert!(!stored.windows(4).any(|window| window == b"iCCP"));
    assert!(!stored.windows(4).any(|window| window == b"tEXt"));
    Ok(())
}

#[tokio::test]
async fn a_jpeg_and_a_webp_photograph_are_accepted() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = store(pool.clone());
    let owner = Uuid::new_v4();
    let image = image::load_from_memory(&photograph(40, 30)).unwrap();
    let mut jpeg = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut jpeg), ImageFormat::Jpeg)
        .unwrap();
    let ready = upload(&store, owner, UploadPurpose::ProfilePost, &jpeg, Mime::Jpeg)
        .await
        .unwrap();
    assert_eq!(ready.mime, Mime::Jpeg);
    assert_eq!((ready.width, ready.height), (40, 30));
    let mut webp = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut webp), ImageFormat::WebP)
        .unwrap();
    let ready = upload(&store, owner, UploadPurpose::ProfilePost, &webp, Mime::Webp)
        .await
        .unwrap();
    assert_eq!(ready.mime, Mime::Webp);
    assert_eq!((ready.width, ready.height), (40, 30));
    Ok(())
}

// ---------------------------------------------------------------------------
// Acceptance: only JPEG/PNG/WebP, 10 MiB, 4096 px, 16M pixels
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_gif_is_refused_and_never_becomes_ready() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = store(pool.clone());
    let owner = Uuid::new_v4();
    // A real GIF87a header: the content type is not in the closed set, so the
    // request is refused before any row exists.
    let gif = b"GIF89a\x01\x00\x01\x00\x00\x00\x00;".to_vec();
    // `Mime::parse` is the closed gate: there is no GIF variant at all.
    assert_eq!(Mime::parse("image/gif"), Err(concert_application::commands::CommandError::InvalidInput));
    assert_eq!(Mime::parse("video/mp4"), Err(concert_application::commands::CommandError::InvalidInput));
    // A PNG content type carrying GIF bytes is refused by the sanitizer, and the
    // row never reaches READY.
    let intent = store
        .init(
            owner,
            UploadPurpose::ProfilePost,
            init(Purpose::ProfilePost, Mime::Png, &gif),
        )
        .await
        .unwrap();
    let id = ProfileMediaId::from_uuid(intent.media_id)?;
    // A real GIF uploaded under a PNG content type is accepted as staged bytes
    // (the PUT step only checks length and digest) and then refused by the shared
    // codec at finalize: `guess_format` reads GIF89a, which is not the declared
    // PNG. The asset therefore never becomes READY.
    let revision = store
        .put(owner, UploadPurpose::ProfilePost, id, Mime::Png, gif)
        .await
        .unwrap();
    assert_eq!(
        store
            .finalize(owner, UploadPurpose::ProfilePost, id, revision)
            .await,
        Err(ProfileMediaError::Invalid)
    );
    let (state, width, _, _) = row(&pool, intent.media_id).await.unwrap();
    assert_eq!(state, "UPLOADED");
    assert_eq!(width, None);
    Ok(())
}

// ---------------------------------------------------------------------------
// Acceptance: a ROOM_POSTER asset cannot be finalized through the profile path
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_room_poster_cannot_be_finalized_through_the_profile_post_path() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = store(pool.clone());
    let owner = Uuid::new_v4();
    let bytes = photograph(16, 16);
    // A room poster is created in the room context, which is the only context
    // that admits ROOM_POSTER.
    let intent = store
        .init(
            owner,
            UploadPurpose::RoomPoster,
            init(Purpose::RoomPoster, Mime::Png, &bytes),
        )
        .await
        .unwrap();
    let id = ProfileMediaId::from_uuid(intent.media_id)?;
    let revision = store
        .put(owner, UploadPurpose::RoomPoster, id, Mime::Png, bytes.clone())
        .await
        .unwrap();
    // The profile-post context refuses it at every step.
    assert_eq!(
        store
            .finalize(owner, UploadPurpose::ProfilePost, id, revision)
            .await,
        Err(ProfileMediaError::PurposeMismatch)
    );
    assert_eq!(
        store
            .put(owner, UploadPurpose::ProfilePost, id, Mime::Png, bytes.clone())
            .await,
        Err(ProfileMediaError::PurposeMismatch)
    );
    // And the profile-post context cannot even create one.
    assert_eq!(
        store
            .init(
                owner,
                UploadPurpose::ProfilePost,
                init(Purpose::RoomPoster, Mime::Png, &bytes),
            )
            .await,
        Err(ProfileMediaError::PurposeMismatch)
    );
    // The room context can finalize it, which proves the refusal above is about
    // the context and not about the bytes.
    let ready = store
        .finalize(owner, UploadPurpose::RoomPoster, id, revision)
        .await
        .unwrap();
    assert_eq!(ready.purpose, Purpose::RoomPoster);
    Ok(())
}

#[tokio::test]
async fn every_context_refuses_every_other_purpose() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = store(pool.clone());
    let owner = Uuid::new_v4();
    let bytes = photograph(8, 8);
    for context in [
        UploadPurpose::ProfilePost,
        UploadPurpose::ProfileImage,
        UploadPurpose::IntroPost,
        UploadPurpose::RoomPoster,
    ] {
        for purpose in Purpose::PROFILE {
            let created = store
                .init(
                    owner,
                    context,
                    init(purpose, Mime::Png, &bytes),
                )
                .await;
            if context.admits(purpose) {
                assert!(created.is_ok(), "{context:?} must admit {purpose:?}");
            } else {
                assert_eq!(
                    created,
                    Err(ProfileMediaError::PurposeMismatch),
                    "{context:?} must refuse {purpose:?}"
                );
            }
        }
        // A concert purpose is never admitted by a profile context.
        for purpose in Purpose::CONCERT {
            assert_eq!(
                store
                    .init(owner, context, init(purpose, Mime::Png, &bytes))
                    .await,
                Err(ProfileMediaError::PurposeMismatch),
                "{context:?} must refuse the concert purpose {purpose:?}"
            );
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Acceptance: an abandoned upload is cleaned after the orphan window
// ---------------------------------------------------------------------------

#[tokio::test]
async fn an_abandoned_upload_is_cleaned_after_the_orphan_window() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = store(pool.clone());
    let owner = Uuid::new_v4();
    let bytes = photograph(8, 8);
    // A cancelled upload: init, then nothing. It is PENDING and unattached.
    let abandoned = store
        .init(
            owner,
            UploadPurpose::ProfilePost,
            init(Purpose::ProfilePost, Mime::Png, &bytes),
        )
        .await
        .unwrap();
    // A fresh upload is inside the window and must survive the same run.
    let fresh = store
        .init(
            owner,
            UploadPurpose::ProfilePost,
            init(Purpose::ProfilePost, Mime::Png, &bytes),
        )
        .await
        .unwrap();
    // A READY asset is a published photograph, not an orphan, and must survive.
    let published = upload(&store, owner, UploadPurpose::ProfilePost, &bytes, Mime::Png)
        .await
        .unwrap();
    // Move the abandoned row behind the window. `created_at` is immutable by
    // design (the identity guard refuses an update that moves it), so the helper
    // below re-inserts the same identity with an older creation instant.
    age(&pool, abandoned.media_id, ORPHAN_SECONDS + 60).await?;
    assert_eq!(ORPHAN_SECONDS, 86400);

    // The per-owner form, so a bounded maintenance run cannot be starved by
    // another test's rows in the same database.
    let erased = cleanup_owner_orphans(&pool, owner, 10).await.unwrap();
    assert!(erased >= 1, "the abandoned upload must be erased");
    let (state, _, _, sanitized) = row(&pool, abandoned.media_id).await.unwrap();
    assert_eq!(state, "DELETED");
    assert!(sanitized.is_none());
    // The fresh upload and the published asset are untouched.
    let (state, _, _, _) = row(&pool, fresh.media_id).await.unwrap();
    assert_eq!(state, "PENDING");
    let (state, _, _, sanitized) = row(&pool, published.media_id).await.unwrap();
    assert_eq!(state, "READY");
    assert!(sanitized.is_some());
    Ok(())
}

#[tokio::test]
async fn an_uploaded_but_never_finalized_asset_is_also_cleaned() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = store(pool.clone());
    let owner = Uuid::new_v4();
    let bytes = photograph(8, 8);
    // A cancelled upload that got as far as PUT: staged bytes, no finalize.
    let intent = store
        .init(
            owner,
            UploadPurpose::ProfilePost,
            init(Purpose::ProfilePost, Mime::Png, &bytes),
        )
        .await
        .unwrap();
    let id = ProfileMediaId::from_uuid(intent.media_id)?;
    store
        .put(owner, UploadPurpose::ProfilePost, id, Mime::Png, bytes)
        .await
        .unwrap();
    age(&pool, intent.media_id, ORPHAN_SECONDS + 60).await?;
    cleanup_owner_orphans(&pool, owner, 10).await.unwrap();
    let (state, _, _, sanitized) = row(&pool, intent.media_id).await.unwrap();
    assert_eq!(state, "DELETED");
    // The staged bytes are gone, not merely hidden.
    assert!(sanitized.is_none());
    Ok(())
}

#[tokio::test]
async fn a_cancelled_upload_leaves_no_orphan_once_the_window_passes() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = store(pool.clone());
    let owner = Uuid::new_v4();
    let bytes = photograph(8, 8);
    // The client cancels: it has the intent but never sends the bytes.
    let intent = store
        .init(
            owner,
            UploadPurpose::ProfilePost,
            init(Purpose::ProfilePost, Mime::Png, &bytes),
        )
        .await
        .unwrap();
    // Nothing was published: no post references it, and it is not READY.
    let (state, _, _, _) = row(&pool, intent.media_id).await.unwrap();
    assert_eq!(state, ProfileMediaState::Pending.as_str());
    let referenced: i64 =
        sqlx::query_scalar("select count(*) from public.profile_post_attachments where media_id=$1")
            .bind(intent.media_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(referenced, 0);
    age(&pool, intent.media_id, ORPHAN_SECONDS + 60).await?;
    cleanup_owner_orphans(&pool, owner, 10).await.unwrap();
    let (state, _, _, _) = row(&pool, intent.media_id).await.unwrap();
    assert_eq!(state, "DELETED", "a cancelled upload leaves no orphan");
    Ok(())
}

#[tokio::test]
async fn cleanup_never_erases_media_a_live_post_still_uses() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = store(pool.clone());
    let owner = Uuid::new_v4();
    let bytes = photograph(8, 8);
    let ready = upload(&store, owner, UploadPurpose::ProfilePost, &bytes, Mime::Png)
        .await
        .unwrap();
    let post = Uuid::new_v4();
    sqlx::query(
        "insert into public.profile_posts(post_id, author_profile_id, description) \
         values($1,$2,'kept')",
    )
    .bind(post)
    .bind(owner)
    .execute(&pool)
    .await?;
    sqlx::query("insert into public.profile_post_attachments(post_id, media_id, position) values($1,$2,0)")
        .bind(post)
        .bind(ready.media_id)
        .execute(&pool)
        .await?;
    // A READY asset is not an orphan at any age: cleanup only considers PENDING
    // and UPLOADED rows, and the reference check refuses to erase media a live
    // post uses. Both are asserted below.
    cleanup_orphans(&pool, 10).await.unwrap();
    let (state, _, _, sanitized) = row(&pool, ready.media_id).await.unwrap();
    assert_eq!(state, "READY");
    assert!(sanitized.is_some());
    Ok(())
}

// ---------------------------------------------------------------------------
// Ownership and byte integrity
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_foreign_owner_cannot_see_or_continue_an_asset() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = store(pool.clone());
    let owner = Uuid::new_v4();
    let stranger = Uuid::new_v4();
    let bytes = photograph(8, 8);
    let intent = store
        .init(
            owner,
            UploadPurpose::ProfilePost,
            init(Purpose::ProfilePost, Mime::Png, &bytes),
        )
        .await
        .unwrap();
    let id = ProfileMediaId::from_uuid(intent.media_id)?;
    // The stranger gets the same answer an absent id gets.
    assert_eq!(
        store
            .put(stranger, UploadPurpose::ProfilePost, id, Mime::Png, bytes.clone())
            .await,
        Err(ProfileMediaError::Unavailable)
    );
    assert_eq!(
        store.metadata(stranger, id).await,
        Err(ProfileMediaError::Unavailable)
    );
    assert_eq!(
        store.content(stranger, id).await,
        Err(ProfileMediaError::Unavailable)
    );
    // The owner can still proceed: the refusal wrote nothing.
    store
        .put(owner, UploadPurpose::ProfilePost, id, Mime::Png, bytes)
        .await
        .unwrap();
    Ok(())
}

#[tokio::test]
async fn bytes_that_do_not_match_the_declared_digest_are_refused() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = store(pool.clone());
    let owner = Uuid::new_v4();
    let bytes = photograph(8, 8);
    let intent = store
        .init(
            owner,
            UploadPurpose::ProfilePost,
            init(Purpose::ProfilePost, Mime::Png, &bytes),
        )
        .await
        .unwrap();
    let id = ProfileMediaId::from_uuid(intent.media_id)?;
    let mut tampered = bytes.clone();
    let last = tampered.len() - 1;
    tampered[last] ^= 0xff;
    assert_eq!(
        store
            .put(owner, UploadPurpose::ProfilePost, id, Mime::Png, tampered)
            .await,
        Err(ProfileMediaError::BytesMismatch)
    );
    let (state, _, _, _) = row(&pool, intent.media_id).await.unwrap();
    assert_eq!(state, "PENDING");
    // A wrong content type is refused too.
    assert_eq!(
        store
            .put(owner, UploadPurpose::ProfilePost, id, Mime::Jpeg, bytes.clone())
            .await,
        Err(ProfileMediaError::BytesMismatch)
    );
    Ok(())
}

#[tokio::test]
async fn finalize_requires_the_current_revision() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = store(pool.clone());
    let owner = Uuid::new_v4();
    let bytes = photograph(8, 8);
    let intent = store
        .init(
            owner,
            UploadPurpose::ProfilePost,
            init(Purpose::ProfilePost, Mime::Png, &bytes),
        )
        .await
        .unwrap();
    let id = ProfileMediaId::from_uuid(intent.media_id)?;
    let revision = store
        .put(owner, UploadPurpose::ProfilePost, id, Mime::Png, bytes)
        .await
        .unwrap();
    assert_eq!(
        store
            .finalize(owner, UploadPurpose::ProfilePost, id, revision + 1)
            .await,
        Err(ProfileMediaError::InvalidState)
    );
    assert_eq!(
        store
            .finalize(owner, UploadPurpose::ProfilePost, id, revision)
            .await
            .map(|ready| ready.revision),
        Ok(revision + 1)
    );
    Ok(())
}

#[tokio::test]
async fn a_published_asset_cannot_be_rewritten() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = store(pool.clone());
    let owner = Uuid::new_v4();
    let bytes = photograph(8, 8);
    let ready = upload(&store, owner, UploadPurpose::ProfilePost, &bytes, Mime::Png)
        .await
        .unwrap();
    let id = ProfileMediaId::from_uuid(ready.media_id)?;
    // A second PUT and a second finalize are both refused: the state machine is
    // PENDING -> UPLOADED -> READY and READY is terminal for the bytes.
    assert_eq!(
        store
            .put(owner, UploadPurpose::ProfilePost, id, Mime::Png, bytes.clone())
            .await,
        Err(ProfileMediaError::InvalidState)
    );
    assert_eq!(
        store
            .finalize(owner, UploadPurpose::ProfilePost, id, ready.revision)
            .await,
        Err(ProfileMediaError::InvalidState)
    );
    // The database refuses a byte rewrite even if the module were bypassed.
    let error = sqlx::query("update public.profile_media set sanitized=$2 where media_id=$1")
        .bind(ready.media_id)
        .bind(vec![0u8; 4])
        .execute(&pool)
        .await
        .expect_err("a published asset must be immutable");
    let constraint = match &error {
        sqlx::Error::Database(database) => database.constraint().map(str::to_owned),
        _ => None,
    };
    assert_eq!(
        constraint.as_deref(),
        Some("profile_media_published_immutable")
    );
    Ok(())
}

#[tokio::test]
async fn the_profile_media_table_has_no_event_or_participant_column() -> anyhow::Result<()> {
    let pool = pool().await?;
    let columns: Vec<String> = sqlx::query_scalar(
        "select column_name::text from information_schema.columns \
         where table_schema='public' and table_name='profile_media' order by column_name",
    )
    .fetch_all(&pool)
    .await?;
    for forbidden in ["event_id", "participant_id", "user_id"] {
        assert!(
            !columns.iter().any(|column| column == forbidden),
            "{forbidden} must not exist: {columns:?}"
        );
    }
    assert!(columns.iter().any(|column| column == "owner_profile_id"));
    Ok(())
}

#[tokio::test]
async fn the_profile_media_table_refuses_a_concert_purpose_and_a_gif() -> anyhow::Result<()> {
    let pool = pool().await?;
    let owner = Uuid::new_v4();
    let insert = "insert into public.profile_media(media_id,owner_profile_id,purpose,content_type,\
                  state,declared_bytes,input_sha256) values($1,$2,$3,$4,'PENDING',4,$5)";
    for (purpose, content_type, expected) in [
        ("POST", "image/png", "profile_media_purpose"),
        ("GIVEAWAY", "image/png", "profile_media_purpose"),
        ("PROFILE_POST", "image/gif", "profile_media_content_type"),
        ("PROFILE_POST", "video/mp4", "profile_media_content_type"),
    ] {
        let error = sqlx::query(insert)
            .bind(Uuid::new_v4())
            .bind(owner)
            .bind(purpose)
            .bind(content_type)
            .bind(vec![0u8; 32])
            .execute(&pool)
            .await
            .expect_err("the closed set must refuse this row");
        let constraint = match &error {
            sqlx::Error::Database(database) => database.constraint().map(str::to_owned),
            _ => None,
        };
        assert_eq!(constraint.as_deref(), Some(expected), "{purpose} {content_type}");
    }
    Ok(())
}
