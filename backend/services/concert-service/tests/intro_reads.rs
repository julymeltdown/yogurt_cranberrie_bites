//! The two intro reads: `listIntroPosts` and `getIntroPost`.
//!
//! WP-04 built the visibility predicate and the page/detail skeleton; this suite
//! covers what the two *reads* add on top of it:
//!
//! * the `q` filter, applied inside the same `where` clause as the predicate so a
//!   page is filtered before `limit` (never after);
//! * the author projection from `discovery_profiles`;
//! * the viewer's like state, which is a second join beyond the predicate;
//! * the detail's attachments in position order and its comment count;
//! * the cursor being bound to the filter set, so a cursor issued for one search
//!   is refused for another.
//!
//! The database is the runner-owned private cluster reached through
//! `CONCERT_TEST_DATABASE_URL`.
use chrono::{DateTime, Duration, Utc};
use concert_application::identity_claims::Gender;
use concert_application::intro_visibility::{AuthorStanding, BlockState};
use concert_service::intro_posts::repository::{
    get_intro_post, list_intro_posts, IntroCursorCodec, IntroPageError, IntroPostQuery, PageScope,
};
use concert_service::intro_posts::visibility_sql::{ViewerAttributes, VisibilityScope};
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
    Ok(sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await?)
}

/// The reads share one fixture database, so the tests here serialize.
fn read_guard() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|poison| poison.into_inner())
}

fn now() -> DateTime<Utc> {
    Utc::now()
}

fn codec() -> IntroCursorCodec {
    IntroCursorCodec::new([0x7c; 32])
}

fn scope(viewer: Uuid, attributes: ViewerAttributes) -> PageScope {
    PageScope {
        viewer_profile_id: viewer,
        blocks: BlockState::None,
        attributes,
        now: now(),
    }
}

fn unrestricted() -> ViewerAttributes {
    ViewerAttributes::Unavailable
}

fn woman_25() -> ViewerAttributes {
    ViewerAttributes::read(Some(25), Some(Gender::Female), false)
}

/// Insert an author projection, which the reads join for the author fields.
async fn insert_author(pool: &PgPool, profile: Uuid, nickname: &str) -> anyhow::Result<()> {
    sqlx::query(
        "insert into discovery_profiles(profile_id,account_id,nickname,bio,visibility) \
         values($1,$2,$3,$4,'PUBLIC') on conflict (profile_id) do update \
         set nickname = excluded.nickname",
    )
    .bind(profile)
    .bind(Uuid::new_v4())
    .bind(nickname)
    .bind(Some("bio".to_owned()))
    .execute(pool)
    .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn insert_post(
    pool: &PgPool,
    author: Uuid,
    body: &str,
    audience: &str,
    min_age: Option<i16>,
    max_age: Option<i16>,
    published_at: DateTime<Utc>,
    expires_at: Option<DateTime<Utc>>,
) -> anyhow::Result<Uuid> {
    let intro_id = Uuid::new_v4();
    sqlx::query(
        "insert into intro_posts(intro_id,author_profile_id,body,audience_gender,\
         audience_min_age,audience_max_age_exclusive,state,published_at,expires_at) \
         values($1,$2,$3,$4,$5,$6,'PUBLISHED',$7,$8)",
    )
    .bind(intro_id)
    .bind(author)
    .bind(body)
    .bind(audience)
    .bind(min_age)
    .bind(max_age)
    .bind(published_at)
    .bind(expires_at)
    .execute(pool)
    .await?;
    Ok(intro_id)
}

async fn cleanup(pool: &PgPool, ids: &[Uuid]) -> anyhow::Result<()> {
    for id in ids {
        sqlx::query("delete from intro_posts where intro_id = $1")
            .bind(id)
            .execute(pool)
            .await?;
    }
    Ok(())
}

/// `list_intro_posts`, surfacing the driver error so a failure names the actual
/// PostgreSQL error rather than the opaque `Store` mapping.
async fn page(
    pool: &PgPool,
    scope: &PageScope,
    query: &IntroPostQuery,
) -> Result<concert_service::intro_posts::repository::IntroPostPage, IntroPageError> {
    list_intro_posts(pool, &codec(), scope, query).await
}

/// Whether a page contains a given intro id.
fn contains(page: &concert_service::intro_posts::repository::IntroPostPage, id: Uuid) -> bool {
    page.items.iter().any(|row| row.intro_id == id)
}

// ---------------------------------------------------------------------------
// The `q` filter
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_query_filter_selects_matching_posts() -> anyhow::Result<()> {
    let _serial = read_guard();
    let pool = pool().await?;
    let author = Uuid::new_v4();
    insert_author(&pool, author, "filter-author").await?;
    let viewer = Uuid::new_v4();
    let base = now() - Duration::minutes(10);
    let matching = insert_post(
        &pool, author, "I play the bass guitar", "any", None, None, base, None,
    )
    .await?;
    let other = insert_post(
        &pool, author, "I collect old cameras", "any", None, None,
        base + Duration::seconds(1), None,
    )
    .await?;

    let filtered = page(
        &pool,
        &scope(viewer, unrestricted()),
        &IntroPostQuery::with_query(Some(20), None, Some("bass".into()))?,
    )
    .await?;
    assert!(contains(&filtered, matching), "the matching post is present");
    assert!(!contains(&filtered, other), "the non-matching post is absent");

    // A term that matches nothing returns an empty page, not the unfiltered one.
    let none = page(
        &pool,
        &scope(viewer, unrestricted()),
        &IntroPostQuery::with_query(Some(20), None, Some("zzzz-no-such-term".into()))?,
    )
    .await?;
    assert!(none.items.is_empty());
    assert!(none.next_cursor.is_none());

    cleanup(&pool, &[matching, other]).await?;
    Ok(())
}

#[tokio::test]
async fn the_query_filter_also_matches_the_single_line_fields() -> anyhow::Result<()> {
    let _serial = read_guard();
    let pool = pool().await?;
    let author = Uuid::new_v4();
    insert_author(&pool, author, "single-line-author").await?;
    let viewer = Uuid::new_v4();
    let intro_id = Uuid::new_v4();
    sqlx::query(
        "insert into intro_posts(intro_id,author_profile_id,body,currently_seeking,\
         interaction_style,audience_gender,state,published_at) \
         values($1,$2,'plain body','looking for a drummer','quiet', 'any','PUBLISHED',$3)",
    )
    .bind(intro_id)
    .bind(author)
    .bind(now() - Duration::minutes(5))
    .execute(&pool)
    .await?;
    for term in ["drummer", "quiet"] {
        let found = page(
            &pool,
            &scope(viewer, unrestricted()),
            &IntroPostQuery::with_query(Some(20), None, Some(term.into()))?,
        )
        .await?;
        assert!(contains(&found, intro_id), "term {term} must match");
    }
    cleanup(&pool, &[intro_id]).await?;
    Ok(())
}

#[tokio::test]
async fn an_empty_or_oversized_query_term_is_refused() -> anyhow::Result<()> {
    // An empty term is not "no filter": a caller that sent `q=` asked for a
    // search, and an unfiltered page would look like a successful search.
    assert_eq!(
        IntroPostQuery::with_query(Some(20), None, Some(String::new())),
        Err(IntroPageError::InvalidQuery("q"))
    );
    assert_eq!(
        IntroPostQuery::with_query(Some(20), None, Some("   ".into())),
        Err(IntroPageError::InvalidQuery("q"))
    );
    assert_eq!(
        IntroPostQuery::with_query(Some(20), None, Some("x".repeat(101))),
        Err(IntroPageError::InvalidQuery("q"))
    );
    // Exactly at the bound is accepted.
    assert!(IntroPostQuery::with_query(Some(20), None, Some("x".repeat(100))).is_ok());
    Ok(())
}

// ---------------------------------------------------------------------------
// V-03-i: filtered inside the query, never after it
// ---------------------------------------------------------------------------

/// The headline case: a page is full even when most matching rows are filtered.
///
/// Twenty restricted rows and five unrestricted ones, all matching the search
/// term. A page of five must return five rows and a cursor; a post-filter would
/// return five rows scanned, of which one was visible, and report no next page.
#[tokio::test]
async fn a_full_page_is_returned_when_the_filtered_set_has_more_rows() -> anyhow::Result<()> {
    let _serial = read_guard();
    let pool = pool().await?;
    let author = Uuid::new_v4();
    insert_author(&pool, author, "full-page-author").await?;
    let viewer = Uuid::new_v4();
    let base = now() - Duration::hours(1);
    let mut ids = Vec::new();
    // Twenty restricted posts, all matching "pagefull".
    for index in 0..20 {
        ids.push(
            insert_post(
                &pool,
                author,
                "pagefull restricted",
                "female",
                Some(18),
                Some(30),
                base + Duration::seconds(index),
                None,
            )
            .await?,
        );
    }
    // Five unrestricted posts, also matching, interleaved above them.
    for index in 0..5 {
        ids.push(
            insert_post(
                &pool,
                author,
                "pagefull open",
                "any",
                None,
                None,
                base + Duration::seconds(100 + index),
                None,
            )
            .await?,
        );
    }
    // A male viewer cannot see the twenty female-restricted rows.
    let male = ViewerAttributes::read(Some(25), Some(Gender::Male), false);
    let first = page(
        &pool,
        &scope(viewer, male),
        &IntroPostQuery::with_query(Some(5), None, Some("pagefull".into()))?,
    )
    .await?;
    assert_eq!(
        first.items.len(),
        5,
        "a full page is returned even though 20 matching rows were filtered out"
    );
    assert!(
        first.next_cursor.is_some(),
        "a full page reports a next cursor"
    );
    // Every returned row is one the viewer may see.
    for row in &first.items {
        let audience: String = sqlx::query_scalar(
            "select audience_gender from intro_posts where intro_id = $1",
        )
        .bind(row.intro_id)
        .fetch_one(&pool)
        .await?;
        assert_eq!(audience, "any", "a filtered row must never reach the page");
    }
    cleanup(&pool, &ids).await?;
    Ok(())
}

#[tokio::test]
async fn a_restricted_post_is_absent_from_a_page() -> anyhow::Result<()> {
    let _serial = read_guard();
    let pool = pool().await?;
    let author = Uuid::new_v4();
    insert_author(&pool, author, "restricted-author").await?;
    let viewer = Uuid::new_v4();
    let base = now() - Duration::minutes(10);
    let restricted = insert_post(
        &pool, author, "women only", "female", Some(18), Some(30), base, None,
    )
    .await?;
    let open = insert_post(
        &pool, author, "anyone", "any", None, None,
        base + Duration::seconds(1), None,
    )
    .await?;

    // A male viewer: the restricted post is absent, the open one present.
    let male = ViewerAttributes::read(Some(25), Some(Gender::Male), false);
    let male_page = page(&pool, &scope(viewer, male), &IntroPostQuery::new(Some(50), None)?).await?;
    assert!(!contains(&male_page, restricted));
    assert!(contains(&male_page, open));

    // A female viewer sees both.
    let female_page = page(
        &pool,
        &scope(viewer, woman_25()),
        &IntroPostQuery::new(Some(50), None)?,
    )
    .await?;
    assert!(contains(&female_page, restricted));
    assert!(contains(&female_page, open));

    cleanup(&pool, &[restricted, open]).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Cursor discipline
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_cursor_replay_is_rejected_for_a_different_viewer() -> anyhow::Result<()> {
    let _serial = read_guard();
    let pool = pool().await?;
    let author = Uuid::new_v4();
    insert_author(&pool, author, "cursor-author").await?;
    let viewer_a = Uuid::new_v4();
    let viewer_b = Uuid::new_v4();
    let base = now() - Duration::hours(1);
    let mut ids = Vec::new();
    for index in 0..4 {
        ids.push(
            insert_post(
                &pool,
                author,
                "cursor replay",
                "any",
                None,
                None,
                base + Duration::seconds(index),
                None,
            )
            .await?,
        );
    }
    let first = page(
        &pool,
        &scope(viewer_a, unrestricted()),
        &IntroPostQuery::new(Some(2), None)?,
    )
    .await?;
    let cursor = first.next_cursor.expect("a full page has a cursor");

    // Viewer A may continue.
    let continued = page(
        &pool,
        &scope(viewer_a, unrestricted()),
        &IntroPostQuery::new(Some(2), Some(cursor.clone()))?,
    )
    .await;
    assert!(continued.is_ok(), "the issuing viewer may continue");

    // Viewer B may not.
    let refused = page(
        &pool,
        &scope(viewer_b, unrestricted()),
        &IntroPostQuery::new(Some(2), Some(cursor))?,
    )
    .await;
    assert_eq!(
        refused,
        Err(IntroPageError::InvalidCursor),
        "a cursor issued to one viewer must be refused for another"
    );
    cleanup(&pool, &ids).await?;
    Ok(())
}

#[tokio::test]
async fn a_cursor_is_bound_to_its_filter_set() -> anyhow::Result<()> {
    let _serial = read_guard();
    let pool = pool().await?;
    let author = Uuid::new_v4();
    insert_author(&pool, author, "filter-cursor-author").await?;
    let viewer = Uuid::new_v4();
    let base = now() - Duration::hours(1);
    let mut ids = Vec::new();
    for index in 0..4 {
        ids.push(
            insert_post(
                &pool,
                author,
                "filterset",
                "any",
                None,
                None,
                base + Duration::seconds(index),
                None,
            )
            .await?,
        );
    }
    let filtered = page(
        &pool,
        &scope(viewer, unrestricted()),
        &IntroPostQuery::with_query(Some(2), None, Some("filterset".into()))?,
    )
    .await?;
    let cursor = filtered.next_cursor.expect("a full page has a cursor");

    // Replaying the cursor without the filter is a different request.
    let unfiltered = page(
        &pool,
        &scope(viewer, unrestricted()),
        &IntroPostQuery::new(Some(2), Some(cursor.clone()))?,
    )
    .await;
    assert_eq!(
        unfiltered,
        Err(IntroPageError::InvalidCursor),
        "a cursor issued for one filter set must not open another"
    );

    // Replaying it with a *different* filter is refused too.
    let other = page(
        &pool,
        &scope(viewer, unrestricted()),
        &IntroPostQuery::with_query(Some(2), Some(cursor.clone()), Some("different".into()))?,
    )
    .await;
    assert_eq!(other, Err(IntroPageError::InvalidCursor));

    // The original filter still works.
    assert!(page(
        &pool,
        &scope(viewer, unrestricted()),
        &IntroPostQuery::with_query(Some(2), Some(cursor), Some("filterset".into()))?,
    )
    .await
    .is_ok());
    cleanup(&pool, &ids).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// The author projection and the like state
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_page_row_carries_the_author_projection_and_the_like_state() -> anyhow::Result<()> {
    let _serial = read_guard();
    let pool = pool().await?;
    let author = Uuid::new_v4();
    insert_author(&pool, author, "liked-author").await?;
    let viewer = Uuid::new_v4();
    let intro_id = insert_post(
        &pool,
        author,
        "like me",
        "any",
        None,
        None,
        now() - Duration::minutes(5),
        None,
    )
    .await?;
    // Two likes: one from the viewer, one from someone else.
    for liker in [viewer, Uuid::new_v4()] {
        sqlx::query("insert into intro_likes(intro_id,actor_profile_id) values($1,$2)")
            .bind(intro_id)
            .bind(liker)
            .execute(&pool)
            .await?;
    }
    let found = page(
        &pool,
        &scope(viewer, unrestricted()),
        &IntroPostQuery::new(Some(50), None)?,
    )
    .await?;
    let row = found
        .items
        .iter()
        .find(|row| row.intro_id == intro_id)
        .expect("the post is on the page");
    assert_eq!(row.like_count, 2, "the count is every like");
    assert!(row.liked_by_viewer, "the viewer liked it");
    let author_row = row.author.as_ref().expect("the author projection is present");
    assert_eq!(author_row.nickname, "liked-author");
    assert_eq!(author_row.profile_id, author);

    // A different viewer sees the same count but not `liked_by_viewer`.
    let stranger = page(
        &pool,
        &scope(Uuid::new_v4(), unrestricted()),
        &IntroPostQuery::new(Some(50), None)?,
    )
    .await?;
    let stranger_row = stranger
        .items
        .iter()
        .find(|row| row.intro_id == intro_id)
        .expect("the post is on the page");
    assert_eq!(stranger_row.like_count, 2);
    assert!(!stranger_row.liked_by_viewer, "a stranger has not liked it");

    cleanup(&pool, &[intro_id]).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// The detail read
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_detail_carries_attachments_in_order_and_the_comment_count() -> anyhow::Result<()> {
    let _serial = read_guard();
    let pool = pool().await?;
    let author = Uuid::new_v4();
    insert_author(&pool, author, "detail-author").await?;
    let viewer = Uuid::new_v4();
    let intro_id = insert_post(
        &pool,
        author,
        "detail body",
        "any",
        None,
        None,
        now() - Duration::minutes(5),
        None,
    )
    .await?;
    // Two attachments, inserted out of order so the position ordering is proved.
    let mut media = Vec::new();
    for (position, _) in [(1, ()), (0, ())] {
        let media_id = Uuid::new_v4();
        sqlx::query(
            "insert into profile_media(media_id,owner_profile_id,purpose,state,content_type,\
             declared_bytes,input_sha256) values($1,$2,'INTRO_POST','PENDING','image/jpeg',10,$3)",
        )
        .bind(media_id)
        .bind(author)
        .bind(vec![1u8; 32])
        .execute(&pool)
        .await?;
        sqlx::query("insert into intro_attachments(intro_id,media_id,position) values($1,$2,$3)")
            .bind(intro_id)
            .bind(media_id)
            .bind(position)
            .execute(&pool)
            .await?;
        media.push((position, media_id));
    }
    media.sort_by_key(|(position, _)| *position);
    // Two comments.
    for _ in 0..2 {
        sqlx::query(
            "insert into intro_comments(comment_id,intro_id,author_profile_id,body,state) \
             values($1,$2,$3,'a comment','PUBLISHED')",
        )
        .bind(Uuid::new_v4())
        .bind(intro_id)
        .bind(Uuid::new_v4())
        .execute(&pool)
        .await?;
    }
    let detail = get_intro_post(
        &pool,
        &VisibilityScope::new(
            viewer,
            BlockState::None,
            AuthorStanding::Active,
            unrestricted(),
            now(),
        ),
        intro_id,
    )
    .await?;
    assert_eq!(
        detail.attachment_media_ids,
        media.iter().map(|(_, id)| *id).collect::<Vec<_>>(),
        "attachments are in position order"
    );
    assert_eq!(detail.comment_count, 2);
    assert_eq!(detail.post.author.as_ref().expect("author").nickname, "detail-author");

    cleanup(&pool, &[intro_id]).await?;
    Ok(())
}

#[tokio::test]
async fn a_detail_refusal_is_indistinguishable_from_absence() -> anyhow::Result<()> {
    let _serial = read_guard();
    let pool = pool().await?;
    let author = Uuid::new_v4();
    insert_author(&pool, author, "refusal-author").await?;
    let viewer = Uuid::new_v4();
    let restricted = insert_post(
        &pool,
        author,
        "women only",
        "female",
        Some(18),
        Some(30),
        now() - Duration::minutes(5),
        None,
    )
    .await?;
    let missing = Uuid::new_v4();
    let male = VisibilityScope::new(
        viewer,
        BlockState::None,
        AuthorStanding::Active,
        ViewerAttributes::read(Some(25), Some(Gender::Male), false),
        now(),
    );
    let refused = get_intro_post(&pool, &male, restricted).await;
    let absent = get_intro_post(&pool, &male, missing).await;
    assert_eq!(refused, Err(IntroPageError::Unavailable));
    assert_eq!(absent, Err(IntroPageError::Unavailable));
    assert_eq!(
        refused, absent,
        "a restricted post and a missing post are the same answer"
    );
    // The error's rendering carries no distinguishing detail either.
    assert_eq!(
        refused.unwrap_err().to_string(),
        absent.unwrap_err().to_string()
    );
    cleanup(&pool, &[restricted]).await?;
    Ok(())
}

#[tokio::test]
async fn the_detail_carries_the_viewers_like_state() -> anyhow::Result<()> {
    let _serial = read_guard();
    let pool = pool().await?;
    let author = Uuid::new_v4();
    insert_author(&pool, author, "detail-like-author").await?;
    let viewer = Uuid::new_v4();
    let intro_id = insert_post(
        &pool,
        author,
        "detail like",
        "any",
        None,
        None,
        now() - Duration::minutes(5),
        None,
    )
    .await?;
    sqlx::query("insert into intro_likes(intro_id,actor_profile_id) values($1,$2)")
        .bind(intro_id)
        .bind(viewer)
        .execute(&pool)
        .await?;
    let detail = get_intro_post(
        &pool,
        &VisibilityScope::new(
            viewer,
            BlockState::None,
            AuthorStanding::Active,
            unrestricted(),
            now(),
        ),
        intro_id,
    )
    .await?;
    assert!(detail.post.liked_by_viewer);
    assert_eq!(detail.post.like_count, 1);
    cleanup(&pool, &[intro_id]).await?;
    Ok(())
}
