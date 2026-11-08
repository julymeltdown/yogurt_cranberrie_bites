//! WP-06 T-503: profile post reads against real PostgreSQL 16.
//!
//! The database is the runner-owned private `mcr-pg-*` cluster created by
//! `backend/scripts/mcr/wp06_profile_posts_cluster.py`. The URL arrives through
//! `CONCERT_TEST_DATABASE_URL`.
//!
//! The hard constraint this file owns:
//!
//! * a private post is **absent from a visitor's list** and **`404` on direct
//!   read**, and the **author sees it**;
//! * pagination happens **inside the query**, so a page is never post-filtered
//!   to fewer than `limit` while reporting a cursor from the unfiltered set.
use concert_service::profile_posts::{
    create_profile_post, list_profile_posts, read_profile_post, CreateProfilePost, PostVisibility,
    ProfilePostError, ProfilePostQuery, DEFAULT_LIMIT, MAX_LIMIT,
};
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
    let applied: bool =
        sqlx::query_scalar("select to_regclass('public.profile_posts') is not null")
            .fetch_one(&pool)
            .await?;
    anyhow::ensure!(applied, "migration 0084_profile_posts is not applied");
    Ok(pool)
}

async fn post(
    pool: &PgPool,
    author: Uuid,
    description: &str,
    visibility: PostVisibility,
) -> anyhow::Result<Uuid> {
    Ok(create_profile_post(
        pool,
        author,
        &CreateProfilePost {
            description: description.to_owned(),
            media_ids: Vec::new(),
            visibility,
            expires_at: None,
        },
    )
    .await
    .unwrap()
    .post_id)
}

fn query(limit: i64, cursor: Option<String>) -> ProfilePostQuery {
    ProfilePostQuery { limit, cursor }
}

// ---------------------------------------------------------------------------
// Acceptance: a private post is absent for a visitor and present for the author
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_private_post_is_absent_from_a_visitors_list_and_404_on_direct_read(
) -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = Uuid::new_v4();
    let visitor = Uuid::new_v4();
    let public = post(&pool, author, "everyone can see this", PostVisibility::Public).await?;
    let private = post(&pool, author, "only I can see this", PostVisibility::Private).await?;

    // The visitor's list contains the public post and not the private one.
    let page = list_profile_posts(&pool, visitor, author, &query(DEFAULT_LIMIT, None)).await?;
    let ids: Vec<Uuid> = page.items.iter().map(|item| item.post_id).collect();
    assert!(ids.contains(&public));
    assert!(!ids.contains(&private), "a private post must be absent");
    // The visitor's direct read of the private post is the same answer an absent
    // id gets: 404 RESOURCE_UNAVAILABLE, with no content.
    let direct = read_profile_post(&pool, visitor, private).await;
    assert_eq!(direct, Err(ProfilePostError::Unavailable));
    assert_eq!(ProfilePostError::Unavailable.status(), 404);
    assert_eq!(ProfilePostError::Unavailable.reason(), "RESOURCE_UNAVAILABLE");
    assert_eq!(
        read_profile_post(&pool, visitor, Uuid::new_v4()).await,
        Err(ProfilePostError::Unavailable)
    );

    // The author sees it in their own list and by direct read.
    let own = list_profile_posts(&pool, author, author, &query(DEFAULT_LIMIT, None)).await?;
    let own_ids: Vec<Uuid> = own.items.iter().map(|item| item.post_id).collect();
    assert!(own_ids.contains(&private), "the author must see their own private post");
    assert!(own_ids.contains(&public));
    let read = read_profile_post(&pool, author, private).await.unwrap();
    assert_eq!(read.visibility, PostVisibility::Private);
    assert_eq!(read.description, "only I can see this");
    Ok(())
}

#[tokio::test]
async fn a_public_post_is_visible_to_a_visitor_in_both_reads() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = Uuid::new_v4();
    let visitor = Uuid::new_v4();
    let public = post(&pool, author, "a photograph", PostVisibility::Public).await?;
    let page = list_profile_posts(&pool, visitor, author, &query(DEFAULT_LIMIT, None)).await?;
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].post_id, public);
    assert_eq!(
        read_profile_post(&pool, visitor, public).await.unwrap().post_id,
        public
    );
    Ok(())
}

#[tokio::test]
async fn a_deleted_post_disappears_from_every_read() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = Uuid::new_v4();
    let visitor = Uuid::new_v4();
    let post_id = post(&pool, author, "gone", PostVisibility::Public).await?;
    concert_service::profile_posts::delete_profile_post(&pool, author, post_id, 1).await?;
    for viewer in [author, visitor] {
        let page = list_profile_posts(&pool, viewer, author, &query(DEFAULT_LIMIT, None)).await?;
        assert!(page.items.is_empty(), "a deleted post must not be listed");
        assert_eq!(
            read_profile_post(&pool, viewer, post_id).await,
            Err(ProfilePostError::Unavailable)
        );
    }
    Ok(())
}

#[tokio::test]
async fn an_expired_post_is_not_visible() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = Uuid::new_v4();
    let visitor = Uuid::new_v4();
    // A post that expires in the past cannot be created through the command
    // (the table requires `expires_at > created_at`), so the fixture writes an
    // already-expired row directly. `created_at` is immutable through the guard,
    // which is why the row is inserted rather than updated.
    let post_id = Uuid::new_v4();
    sqlx::query(
        "insert into public.profile_posts(post_id, author_profile_id, description, visibility, \
         state, created_at, expires_at) \
         values($1,$2,'ephemeral','PUBLIC','PUBLISHED', \
                clock_timestamp() - interval '2 seconds', \
                clock_timestamp() - interval '1 second')",
    )
    .bind(post_id)
    .bind(author)
    .execute(&pool)
    .await?;
    assert_eq!(
        read_profile_post(&pool, visitor, post_id).await,
        Err(ProfilePostError::Unavailable)
    );
    let page = list_profile_posts(&pool, visitor, author, &query(DEFAULT_LIMIT, None)).await?;
    assert!(page.items.is_empty());
    Ok(())
}

// ---------------------------------------------------------------------------
// Acceptance: pagination is inside the query
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_page_is_full_even_when_most_posts_are_private() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = Uuid::new_v4();
    let visitor = Uuid::new_v4();
    // Twenty private posts and five public ones, interleaved so a post-filtered
    // page would come back short.
    let mut public = Vec::new();
    for index in 0..20 {
        post(&pool, author, &format!("private {index}"), PostVisibility::Private).await?;
        if index % 4 == 0 {
            public.push(
                post(&pool, author, &format!("public {index}"), PostVisibility::Public).await?,
            );
        }
    }
    assert_eq!(public.len(), 5);
    // A visitor asking for 5 gets exactly 5 public posts, not a short page.
    let page = list_profile_posts(&pool, visitor, author, &query(5, None)).await?;
    assert_eq!(
        page.items.len(),
        5,
        "the visibility filter must run inside the page query"
    );
    assert!(page.items.iter().all(|item| item.visibility == PostVisibility::Public));
    assert_eq!(page.next_cursor, None, "there is no sixth visible post");
    // The author asking for 5 gets 5 posts of any visibility and a cursor.
    let own = list_profile_posts(&pool, author, author, &query(5, None)).await?;
    assert_eq!(own.items.len(), 5);
    assert!(own.next_cursor.is_some());
    Ok(())
}

#[tokio::test]
async fn the_cursor_walks_the_whole_visible_set_exactly_once() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = Uuid::new_v4();
    let visitor = Uuid::new_v4();
    let mut expected = Vec::new();
    for index in 0..9 {
        let visibility = if index % 3 == 0 {
            PostVisibility::Private
        } else {
            PostVisibility::Public
        };
        let id = post(&pool, author, &format!("post {index}"), visibility).await?;
        if visibility == PostVisibility::Public {
            expected.push(id);
        }
    }
    let mut seen = Vec::new();
    let mut cursor = None;
    for _ in 0..10 {
        let page = list_profile_posts(&pool, visitor, author, &query(2, cursor.clone())).await?;
        assert!(page.items.len() <= 2);
        seen.extend(page.items.iter().map(|item| item.post_id));
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(cursor, None, "the walk must terminate");
    // Every public post appears exactly once, in descending creation order.
    assert_eq!(seen.len(), expected.len());
    let mut sorted: Vec<Uuid> = seen.clone();
    sorted.sort();
    let mut wanted = expected.clone();
    wanted.sort();
    assert_eq!(sorted, wanted);
    assert_eq!(seen, expected.into_iter().rev().collect::<Vec<_>>());
    Ok(())
}

#[tokio::test]
async fn posts_created_in_the_same_microsecond_are_not_skipped() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = Uuid::new_v4();
    let visitor = Uuid::new_v4();
    // `clock_timestamp()` can repeat within one transaction, so the cursor is a
    // `(created_at, post_id)` pair. Five posts sharing one creation instant prove
    // the pair is what makes the walk total. `created_at` is immutable through the
    // guard, so the rows are inserted directly rather than updated.
    let mut ids = Vec::new();
    for index in 0..5 {
        let id = Uuid::new_v4();
        sqlx::query(
            "insert into public.profile_posts(post_id, author_profile_id, description, visibility, \
             state, created_at) values($1,$2,$3,'PUBLIC','PUBLISHED','2026-01-01T00:00:00Z')",
        )
        .bind(id)
        .bind(author)
        .bind(format!("same instant {index}"))
        .execute(&pool)
        .await?;
        ids.push(id);
    }
    let mut seen = Vec::new();
    let mut cursor = None;
    for _ in 0..10 {
        let page = list_profile_posts(&pool, visitor, author, &query(2, cursor.clone())).await?;
        seen.extend(page.items.iter().map(|item| item.post_id));
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    seen.sort();
    let mut wanted = ids.clone();
    wanted.sort();
    assert_eq!(seen, wanted, "every post must be visited exactly once");
    Ok(())
}

#[tokio::test]
async fn the_query_bounds_are_the_frozen_page_query() -> anyhow::Result<()> {
    assert_eq!(DEFAULT_LIMIT, 20);
    assert_eq!(MAX_LIMIT, 100);
    let pool = pool().await?;
    let author = Uuid::new_v4();
    assert_eq!(
        query(0, None).validate(),
        Err(ProfilePostError::Invalid)
    );
    assert_eq!(
        query(101, None).validate(),
        Err(ProfilePostError::Invalid)
    );
    assert_eq!(
        query(1, Some("not a cursor!".into())).validate(),
        Err(ProfilePostError::Invalid)
    );
    // A syntactically valid but unknown cursor is refused when decoded.
    assert_eq!(
        list_profile_posts(&pool, author, author, &query(5, Some("cHB4Onh4".into()))).await,
        Err(ProfilePostError::Invalid)
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Attachments and author projection
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_post_projects_its_attachments_in_position_order() -> anyhow::Result<()> {
    use concert_application::media::{InitUpload, Mime};
    use concert_service::adapters::out::media::codec::BoundedImageSanitizer;
    use concert_service::profile_media::{ProfileMediaId, ProfileMediaStore, UploadPurpose};
    use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
    use platform::cpu::WorkerPool;
    use sha2::{Digest, Sha256};
    use std::sync::Arc;

    let pool = pool().await?;
    let store = ProfileMediaStore::new(
        pool.clone(),
        Arc::new(BoundedImageSanitizer::new(
            WorkerPool::new(1).unwrap().class(0).unwrap(),
        )),
    );
    let author = Uuid::new_v4();
    let visitor = Uuid::new_v4();
    let mut ids = Vec::new();
    for index in 0..3u32 {
        let mut image = RgbImage::new(10 + index, 10 + index);
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            *pixel = Rgb([x as u8, y as u8, index as u8]);
        }
        let mut bytes = Vec::new();
        DynamicImage::ImageRgb8(image)
            .write_to(&mut std::io::Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        let intent = store
            .init(
                author,
                UploadPurpose::ProfilePost,
                InitUpload {
                    purpose: concert_application::media::Purpose::ProfilePost,
                    mime: Mime::Png,
                    byte_length: bytes.len(),
                    sha256: Sha256::digest(&bytes).into(),
                },
            )
            .await
            .unwrap();
        let id = ProfileMediaId::from_uuid(intent.media_id).unwrap();
        let revision = store
            .put(author, UploadPurpose::ProfilePost, id, Mime::Png, bytes)
            .await
            .unwrap();
        store
            .finalize(author, UploadPurpose::ProfilePost, id, revision)
            .await
            .unwrap();
        ids.push(intent.media_id);
    }
    let post_id = create_profile_post(
        &pool,
        author,
        &CreateProfilePost {
            description: "three photographs".into(),
            media_ids: ids.clone(),
            visibility: PostVisibility::Public,
            expires_at: None,
        },
    )
    .await
    .unwrap()
    .post_id;
    let read = read_profile_post(&pool, visitor, post_id).await.unwrap();
    assert_eq!(
        read.media.iter().map(|media| media.media_id).collect::<Vec<_>>(),
        ids
    );
    assert_eq!(read.media[0].width, 10);
    assert_eq!(read.media[2].width, 12);
    let wire = read.to_wire();
    assert_eq!(wire["media"][0]["purpose"], "PROFILE_POST");
    assert_eq!(wire["media"][0]["state"], "READY");
    assert_eq!(
        wire["media"][0]["read_path"],
        format!("/api/media/{}/content", ids[0])
    );
    // The author projection is the profile id; no nickname is invented.
    assert_eq!(wire["author"]["profile_id"], author.to_string());
    Ok(())
}

#[tokio::test]
async fn the_read_module_never_references_an_intro_post() -> anyhow::Result<()> {
    // A profile post's visibility is its own. The read module must not consult
    // an intro post's audience, so it must not name one at all.
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/profile_posts/read.rs"),
    )?;
    let code: String = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in ["intro_posts", "intro_id", "audience", "room"] {
        assert!(
            !code.contains(forbidden),
            "a profile post read must not depend on {forbidden}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn a_nil_viewer_or_author_is_refused() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = Uuid::new_v4();
    assert_eq!(
        list_profile_posts(&pool, Uuid::nil(), author, &query(5, None)).await,
        Err(ProfilePostError::Unavailable)
    );
    assert_eq!(
        list_profile_posts(&pool, author, Uuid::nil(), &query(5, None)).await,
        Err(ProfilePostError::Unavailable)
    );
    assert_eq!(
        read_profile_post(&pool, Uuid::nil(), Uuid::new_v4()).await,
        Err(ProfilePostError::Unavailable)
    );
    Ok(())
}
