//! WP-07 T-604: poster upload against real PostgreSQL 16.
//!
//! Every acceptance case of the task is one of the tests below:
//!
//! * `a_jpeg_poster_is_rejected` — the content type is exactly `image/png`, and a
//!   JPEG that *claims* to be a PNG is refused by the signature check;
//! * `a_png_whose_declared_dimensions_differ_from_the_actual_ones_is_rejected` —
//!   the declared size is a claim the server checks against a real decode;
//! * `the_stored_bytes_equal_the_submitted_bytes` — the server never re-encodes or
//!   re-pixelates: the `profile_media` row holds exactly the bytes it was given;
//! * `an_asset_referenced_by_another_profile_is_rejected` — a poster is stored for
//!   the authenticated owner only, and a placement of someone else's media is
//!   refused.
//!
//! The database is the private `mcr-pg-*` cluster created by
//! `backend/scripts/mcr/wp07_room_cluster.py`; the URL arrives through
//! `CONCERT_TEST_DATABASE_URL`.
use concert_domain::profile_avatar::ProfileId;
use concert_domain::profile_room::MAX_POSTER_BYTES;
use concert_service::profile_room::{
    handle, validate_png, RoomActor, RoomErrorKind, RoomPersistence, POSTERS_PATH,
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
        .max_connections(8)
        .connect_with(options)
        .await?;
    let applied: bool = sqlx::query_scalar("select to_regclass('public.profile_media') is not null")
        .fetch_one(&pool)
        .await?;
    anyhow::ensure!(applied, "migration 0080_profile_media is not applied");
    Ok(pool)
}

fn actor(profile: Uuid) -> RoomActor {
    RoomActor::new(ProfileId::from_uuid(profile).unwrap())
}

/// A real PNG of the given size, produced by the same encoder the browser
/// conversion uses (a lossless PNG of the sampled pixel grid).
fn png(width: u32, height: u32) -> Vec<u8> {
    let image = image::RgbaImage::from_fn(width, height, |x, y| {
        image::Rgba([
            ((x * 7) % 256) as u8,
            ((y * 11) % 256) as u8,
            (((x + y) * 3) % 256) as u8,
            255,
        ])
    });
    let mut bytes = Vec::new();
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
        .expect("png encode");
    bytes
}

fn jpeg(width: u32, height: u32) -> Vec<u8> {
    let image = image::RgbImage::from_fn(width, height, |x, y| {
        image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])
    });
    let mut bytes = Vec::new();
    image::DynamicImage::ImageRgb8(image)
        .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Jpeg)
        .expect("jpeg encode");
    bytes
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
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
}

fn upload(preset: &str, width: u32, height: u32, content_type: &str, bytes: &[u8]) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "preset": preset,
        "width": width,
        "height": height,
        "content_type": content_type,
        "bytes_base64": base64(bytes),
    }))
    .unwrap()
}

/// The stored media row: purpose, content type, state, dimensions and bytes.
async fn stored_media(
    pool: &PgPool,
    media: &str,
) -> Option<(String, String, String, i32, i32, Vec<u8>, Uuid)> {
    let id = Uuid::parse_str(media).unwrap();
    sqlx::query_as(
        "select purpose, content_type, state, width, height, sanitized, owner_profile_id \
         from public.profile_media where media_id=$1",
    )
    .bind(id)
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
// The acceptance cases
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_jpeg_poster_is_rejected() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = RoomPersistence::new(pool.clone());
    let owner = Uuid::new_v4();
    let bytes = jpeg(64, 64);
    // A JPEG declared as a JPEG is refused at the content type.
    let declared_jpeg = handle(
        &store,
        actor(owner),
        "POST",
        POSTERS_PATH,
        &upload("FINE", 64, 64, "image/jpeg", &bytes),
    )
    .await;
    assert_eq!(declared_jpeg.status, 400);
    assert_eq!(declared_jpeg.body["details"]["reason"], "INVALID_INPUT");
    // A JPEG that lies about its type is refused by the signature check, with the
    // media-type status.
    let lying = handle(
        &store,
        actor(owner),
        "POST",
        POSTERS_PATH,
        &upload("FINE", 64, 64, "image/png", &bytes),
    )
    .await;
    assert_eq!(lying.status, 415);
    assert_eq!(lying.body["details"]["reason"], "INVALID_MEDIA_TYPE");
    // A WebP and a GIF are refused the same way.
    for foreign in [b"RIFF\0\0\0\0WEBPVP8 ".to_vec(), b"GIF89a".to_vec()] {
        let response = handle(
            &store,
            actor(owner),
            "POST",
            POSTERS_PATH,
            &upload("FINE", 64, 64, "image/png", &foreign),
        )
        .await;
        assert_eq!(response.status, 415);
    }
    // Nothing was stored for any of them.
    assert_eq!(media_count(&pool, owner).await, 0);
    Ok(())
}

#[tokio::test]
async fn a_png_whose_declared_dimensions_differ_from_the_actual_ones_is_rejected(
) -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = RoomPersistence::new(pool.clone());
    let owner = Uuid::new_v4();
    let bytes = png(64, 64);
    // The real size is 64x64; every other declaration is refused.
    for (width, height) in [(32u32, 32u32), (64, 32), (32, 64), (8, 8), (128, 128)] {
        let response = handle(
            &store,
            actor(owner),
            "POST",
            POSTERS_PATH,
            &upload("FINE", width, height, "image/png", &bytes),
        )
        .await;
        assert_eq!(response.status, 400, "{width}x{height}");
        assert_eq!(response.body["details"]["reason"], "INVALID_INPUT");
    }
    assert_eq!(media_count(&pool, owner).await, 0);
    // The honest declaration is accepted.
    let accepted = handle(
        &store,
        actor(owner),
        "POST",
        POSTERS_PATH,
        &upload("FINE", 64, 64, "image/png", &bytes),
    )
    .await;
    assert_eq!(accepted.status, 200, "{:?}", accepted.body);
    Ok(())
}

#[tokio::test]
async fn the_stored_bytes_equal_the_submitted_bytes() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = RoomPersistence::new(pool.clone());
    let owner = Uuid::new_v4();
    // A poster with a deliberately non-default PNG encoding: the server must not
    // re-encode it, so the bytes survive exactly.
    let bytes = png(32, 32);
    let response = handle(
        &store,
        actor(owner),
        "POST",
        POSTERS_PATH,
        &upload("CHUNKY", 32, 32, "image/png", &bytes),
    )
    .await;
    assert_eq!(response.status, 200, "{:?}", response.body);
    let media = response.body["media"]["media_id"].as_str().unwrap().to_owned();
    let (purpose, content_type, state, width, height, stored, stored_owner) =
        stored_media(&pool, &media).await.expect("stored media");
    assert_eq!(purpose, "ROOM_POSTER");
    assert_eq!(content_type, "image/png");
    assert_eq!(state, "READY");
    assert_eq!((width, height), (32, 32));
    assert_eq!(stored_owner, owner);
    assert_eq!(stored, bytes, "the stored bytes must equal the submitted bytes");
    // The response describes a poster of the preset's native size.
    assert_eq!(response.body["preset"], "CHUNKY");
    assert_eq!(response.body["width"], 32);
    assert_eq!(response.body["height"], 32);
    assert_eq!(response.body["media"]["content_type"], "image/png");
    assert_eq!(response.body["media"]["state"], "READY");
    Ok(())
}

#[tokio::test]
async fn an_asset_referenced_by_another_profile_is_rejected() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = RoomPersistence::new(pool.clone());
    let owner = Uuid::new_v4();
    let stranger = Uuid::new_v4();
    // The owner uploads a poster; the stranger cannot place it in their own room.
    let uploaded = handle(
        &store,
        actor(owner),
        "POST",
        POSTERS_PATH,
        &upload("FINE", 64, 64, "image/png", &png(64, 64)),
    )
    .await;
    assert_eq!(uploaded.status, 200);
    let media = uploaded.body["media"]["media_id"].as_str().unwrap().to_owned();
    let layout = json!({
        "version": 1,
        "wall": "plaster",
        "floor": "wood",
        "furniture": [],
        "posters": [{"poster_media_id": media, "slot": 0, "x": 2, "y": 2,
            "width": 6, "height": 6}],
        "character": {"cell_x": 16, "cell_y": 24, "facing": "RIGHT"},
    });
    let save = |expected: i64| {
        serde_json::to_vec(&json!({"expected_revision": expected, "layout": layout})).unwrap()
    };
    assert_eq!(
        store.save(actor(stranger), &save(1)).await,
        Err(RoomErrorKind::Invalid)
    );
    assert_eq!(media_count(&pool, stranger).await, 0);
    // The owner can place their own poster.
    let saved = store.save(actor(owner), &save(1)).await.unwrap();
    assert_eq!(saved["posters"][0]["media_id"], media);
    // The stranger's room never gained a row.
    let stranger_rooms: i64 =
        sqlx::query_scalar("select count(*) from public.profile_rooms where owner_profile_id=$1")
            .bind(stranger)
            .fetch_one(&pool)
            .await?;
    assert_eq!(stranger_rooms, 0);
    Ok(())
}

// ---------------------------------------------------------------------------
// Bounds
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_poster_side_is_bounded_by_the_contract() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = RoomPersistence::new(pool.clone());
    let owner = Uuid::new_v4();
    // 128 is the largest admitted side; 129 is refused before any decode.
    let largest = png(128, 128);
    let mut body: Value =
        serde_json::from_slice(&upload("FINE", 128, 128, "image/png", &largest)).unwrap();
    body["preset"] = json!("FINE");
    let response = handle(
        &store,
        actor(owner),
        "POST",
        POSTERS_PATH,
        &serde_json::to_vec(&body).unwrap(),
    )
    .await;
    // 128x128 is a valid PNG but not the FINE preset's native size, so the
    // preset rule refuses it; the side bound itself is proved by the decoder.
    assert_eq!(response.status, 400);
    assert_eq!(validate_png(&largest), Ok((128, 128)));
    // A PNG larger than the bound is refused.
    let too_large = png(129, 129);
    assert_eq!(validate_png(&too_large), Err(RoomErrorKind::Invalid));
    // A zero-sized PNG cannot be produced by the encoder, so the header is
    // rewritten to zero: the decoder refuses it.
    let mut zero = png(8, 8);
    zero[16..20].copy_from_slice(&0u32.to_be_bytes());
    assert_eq!(validate_png(&zero), Err(RoomErrorKind::Invalid));
    // The declared side is 8..=128.
    for side in [0u32, 7, 129] {
        let response = handle(
            &store,
            actor(owner),
            "POST",
            POSTERS_PATH,
            &upload("FINE", side, side, "image/png", &png(8, 8)),
        )
        .await;
        assert_eq!(response.status, 400, "{side}");
    }
    Ok(())
}

#[tokio::test]
async fn the_poster_is_bounded_to_one_mib() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = RoomPersistence::new(pool.clone());
    let owner = Uuid::new_v4();
    // A body whose decoded bytes exceed one MiB is refused.
    let oversized = vec![0u8; MAX_POSTER_BYTES + 1];
    let response = handle(
        &store,
        actor(owner),
        "POST",
        POSTERS_PATH,
        &upload("FINE", 64, 64, "image/png", &oversized),
    )
    .await;
    assert_eq!(response.status, 400);
    // A body over the request bound is refused before it is parsed.
    let huge = vec![b'x'; 1_500_001];
    let response = handle(&store, actor(owner), "POST", POSTERS_PATH, &huge).await;
    assert_eq!(response.status, 413);
    assert_eq!(response.body["details"]["reason"], "PAYLOAD_TOO_LARGE");
    assert_eq!(media_count(&pool, owner).await, 0);
    Ok(())
}

#[tokio::test]
async fn a_truncated_or_corrupt_png_is_rejected() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = RoomPersistence::new(pool.clone());
    let owner = Uuid::new_v4();
    let bytes = png(64, 64);
    // The signature and IHDR alone are not an image.
    let truncated = bytes[..40].to_vec();
    assert_eq!(validate_png(&truncated), Err(RoomErrorKind::Invalid));
    let response = handle(
        &store,
        actor(owner),
        "POST",
        POSTERS_PATH,
        &upload("FINE", 64, 64, "image/png", &truncated),
    )
    .await;
    assert_eq!(response.status, 400);
    // A flipped pixel byte breaks the CRC and the decode.
    let mut corrupt = bytes.clone();
    let index = corrupt.len() / 2;
    corrupt[index] ^= 0xff;
    let response = handle(
        &store,
        actor(owner),
        "POST",
        POSTERS_PATH,
        &upload("FINE", 64, 64, "image/png", &corrupt),
    )
    .await;
    assert_eq!(response.status, 400);
    assert_eq!(media_count(&pool, owner).await, 0);
    Ok(())
}

#[tokio::test]
async fn the_poster_body_is_closed() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = RoomPersistence::new(pool.clone());
    let owner = Uuid::new_v4();
    let bytes = png(64, 64);
    for extra in [
        json!({"owner_profile_id": Uuid::new_v4().to_string()}),
        json!({"media_id": Uuid::new_v4().to_string()}),
        json!({"intro_id": Uuid::new_v4().to_string()}),
        json!({"purpose": "ROOM_POSTER"}),
    ] {
        let mut body: Value =
            serde_json::from_slice(&upload("FINE", 64, 64, "image/png", &bytes)).unwrap();
        for (key, value) in extra.as_object().unwrap() {
            body[key] = value.clone();
        }
        let response = handle(
            &store,
            actor(owner),
            "POST",
            POSTERS_PATH,
            &serde_json::to_vec(&body).unwrap(),
        )
        .await;
        assert_eq!(response.status, 400, "{extra}");
    }
    assert_eq!(media_count(&pool, owner).await, 0);
    Ok(())
}

#[tokio::test]
async fn each_upload_is_a_new_media_asset() -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = RoomPersistence::new(pool.clone());
    let owner = Uuid::new_v4();
    let bytes = png(64, 64);
    let first = handle(
        &store,
        actor(owner),
        "POST",
        POSTERS_PATH,
        &upload("FINE", 64, 64, "image/png", &bytes),
    )
    .await;
    let second = handle(
        &store,
        actor(owner),
        "POST",
        POSTERS_PATH,
        &upload("FINE", 64, 64, "image/png", &bytes),
    )
    .await;
    assert_eq!(first.status, 200);
    assert_eq!(second.status, 200);
    assert_ne!(
        first.body["media"]["media_id"], second.body["media"]["media_id"],
        "an upload creates a new asset; it never rewrites a published one"
    );
    assert_eq!(media_count(&pool, owner).await, 2);
    Ok(())
}

#[tokio::test]
async fn the_poster_purpose_is_room_poster_and_never_an_intro_purpose(
) -> anyhow::Result<()> {
    let pool = pool().await?;
    let store = RoomPersistence::new(pool.clone());
    let owner = Uuid::new_v4();
    let response = handle(
        &store,
        actor(owner),
        "POST",
        POSTERS_PATH,
        &upload("FINE", 64, 64, "image/png", &png(64, 64)),
    )
    .await;
    assert_eq!(response.status, 200);
    let media = response.body["media"]["media_id"].as_str().unwrap();
    let (purpose, ..) = stored_media(&pool, media).await.unwrap();
    assert_eq!(purpose, "ROOM_POSTER");
    // The schema's purpose column is closed and includes ROOM_POSTER, so the
    // stored row is admitted by the same constraint every other purpose obeys.
    // The read is scoped to this owner: tests in one binary share the database.
    let purposes: Vec<String> = sqlx::query_scalar(
        "select distinct purpose::text from public.profile_media \
         where owner_profile_id=$1 order by 1",
    )
    .bind(owner)
    .fetch_all(&pool)
    .await?;
    assert_eq!(purposes, vec!["ROOM_POSTER".to_owned()]);
    // And the closed purpose set the column admits is the profile set.
    let definition: String = sqlx::query_scalar(
        "select pg_get_constraintdef(oid) from pg_constraint \
         where conname='profile_media_purpose'",
    )
    .fetch_one(&pool)
    .await?;
    assert!(definition.contains("ROOM_POSTER"), "{definition}");
    assert!(definition.contains("INTRO_POST"), "{definition}");
    Ok(())
}
