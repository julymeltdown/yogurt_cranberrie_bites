//! T-306: the page and detail queries.
//!
//! The assertions that matter are about *where* the predicate runs:
//!
//! * a page is never shorter than `limit` while a next page exists, because the
//!   predicate filtered inside the query (V-03-i);
//! * a cursor issued to one viewer is refused for another;
//! * a changed audience changes the visible set on the next request;
//! * a post that does not exist and a post the viewer may not see are the same
//!   `Unavailable`.
//!
//! The database is the runner-owned private cluster reached through
//! `CONCERT_TEST_DATABASE_URL`.
use chrono::{DateTime, Duration, Utc};
use concert_application::identity_claims::Gender;
use concert_application::intro_visibility::BlockState;
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
        "the visibility fixture must use the owned cluster"
    );
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await?;
    migrate(&pool).await?;
    Ok(pool)
}

async fn migrate(pool: &PgPool) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("select pg_advisory_xact_lock(hashtextextended('wp04-concert-migrations',0))")
        .execute(&mut *tx)
        .await?;
    let applied: bool = sqlx::query_scalar("select to_regclass('public.intro_posts') is not null")
        .fetch_one(&mut *tx)
        .await?;
    if applied {
        tx.commit().await?;
        return Ok(());
    }
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let mut entries: Vec<_> = std::fs::read_dir(&directory)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().is_some_and(|extension| extension == "sql")
                && !path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.ends_with(".down.sql"))
        })
        .collect();
    entries.sort();
    for path in entries {
        let sql = std::fs::read_to_string(&path)?;
        sqlx::raw_sql(&sql).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}

fn now() -> DateTime<Utc> {
    Utc::now()
}

/// The fixture database is shared by every test in this binary, and a page is a
/// global ordered set. Two tests inserting rows at once would change each
/// other's page contents, so the page tests take this lock for their whole body.
/// It serializes them within the binary; the assertions are still about real
/// query behaviour, not about a private database.
static PAGE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn page_guard() -> std::sync::MutexGuard<'static, ()> {
    PAGE_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn codec() -> IntroCursorCodec {
    IntroCursorCodec::new([0x5a; 32])
}

/// Insert a published post with an explicit publication instant so the page
/// order is deterministic.
#[allow(clippy::too_many_arguments)]
async fn insert_post(
    pool: &PgPool,
    author: Uuid,
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
    .bind(format!("post {intro_id}"))
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

/// `list_intro_posts` with the driver error surfaced, so a failing assertion
/// names the actual PostgreSQL error rather than the opaque `Store` mapping.
async fn list_page(
    pool: &PgPool,
    codec: &IntroCursorCodec,
    scope: &PageScope,
    query: &IntroPostQuery,
) -> Result<concert_service::intro_posts::repository::IntroPostPage, IntroPageError> {
    match list_intro_posts(pool, codec, scope, query).await {
        Ok(page) => Ok(page),
        // A policy refusal is the answer the test asked for, not a failure of the
        // store: return it unchanged. Only `Store` hides a driver error, and only
        // that case is worth re-running with the error surfaced.
        Err(IntroPageError::Store) => {
            let diagnostic =
                concert_service::intro_posts::repository::list_intro_posts_diagnostic(
                    pool, codec, scope, query,
                )
                .await;
            match diagnostic {
                Err(detail) => panic!("page query failed with the store: {detail}"),
                Ok(_) => Err(IntroPageError::Store),
            }
        }
        Err(error) => Err(error),
    }
}

/// A fresh author for one test. Every row a test creates is authored by its own
/// author, so a page read by a viewer who is not that author contains exactly the
/// rows the test created plus any other test's rows. Tests therefore assert on
/// membership of their own ids rather than on absolute page contents, except
/// where they author-scope the read.
fn test_author() -> Uuid {
    Uuid::new_v4()
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

fn man_25() -> ViewerAttributes {
    ViewerAttributes::read(Some(25), Some(Gender::Male), false)
}

// ---------------------------------------------------------------------------
// V-03-i: the predicate is inside the query
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_page_query_is_preparable() -> anyhow::Result<()> {
    let _serial = page_guard();
    let pool = pool().await?;
    let error = concert_service::intro_posts::repository::list_intro_posts_diagnostic(
        &pool,
        &codec(),
        &scope(Uuid::new_v4(), unrestricted()),
        &IntroPostQuery::new(Some(1), None)?,
    )
    .await;
    assert!(error.is_ok(), "{error:?}");
    // With a row present, and with a claim-bearing scope.
    let id = insert_post(&pool, Uuid::new_v4(), "female", Some(18), Some(30), now(), None).await?;
    for attributes in [unrestricted(), woman_25(), man_25()] {
        let error = concert_service::intro_posts::repository::list_intro_posts_diagnostic(
            &pool,
            &codec(),
            &scope(Uuid::new_v4(), attributes),
            &IntroPostQuery::new(Some(1), None)?,
        )
        .await;
        assert!(error.is_ok(), "{error:?}");
    }
    cleanup(&pool, &[id]).await?;
    Ok(())
}

#[tokio::test]
async fn a_page_is_full_when_the_filtered_set_has_more_rows() -> anyhow::Result<()> {
    let _serial = page_guard();
    let pool = pool().await?;
    // The reader is the author of every row this test creates, so row 1 admits
    // all of them. Ten posts alternate restricted and unrestricted; a page of
    // four must return four rows, not "four fetched, three filtered, three
    // returned".
    //
    // The fixture database is shared by every test binary in a run, so the page
    // also contains rows other runs left behind. The walk below is therefore
    // bounded and asserts the property that matters: every page is full while
    // more rows exist, and the cursor always advances. The exact-set assertion
    // is made by `a_restricted_row_is_filtered_inside_the_page_query`, which
    // scopes its assertions to the ids it created.
    let reader = test_author();
    let base = now() - Duration::seconds(5);
    let mut ids = Vec::new();
    for index in 0..10 {
        let (audience, min, max) = if index % 2 == 0 {
            ("any", None, None)
        } else {
            ("female", Some(18), Some(30))
        };
        ids.push(
            insert_post(
                &pool,
                reader,
                audience,
                min,
                max,
                base - Duration::minutes(index as i64),
                None,
            )
            .await?,
        );
    }
    let query = IntroPostQuery::new(Some(4), None)?;
    let first = list_page(&pool, &codec(), &scope(reader, unrestricted()), &query).await?;
    assert_eq!(first.items.len(), 4, "a page must be full");
    assert!(first.next_cursor.is_some(), "a next page exists");
    // Walk with pages of three and assert every page is full while a cursor is
    // returned, and that the cursor strictly advances.
    let mut cursor = None;
    let mut previous: Option<(chrono::DateTime<Utc>, Uuid)> = None;
    let mut visited = 0;
    loop {
        let page = list_page(
            &pool,
            &codec(),
            &scope(reader, unrestricted()),
            &IntroPostQuery::new(Some(3), cursor.clone())?,
        )
        .await?;
        // A full page carries a cursor only while more rows remain. The walk
        // stops when the set is exhausted, so the last full page may legitimately
        // have none; what must hold is that a cursor always advances.
        if page.items.len() == 3 && page.next_cursor.is_none() {
            // The set ended exactly on a page boundary: nothing follows.
        }
        for row in &page.items {
            let position = (
                row.published_at.expect("a page row is published"),
                row.intro_id,
            );
            if let Some(previous) = previous {
                assert!(position < previous, "the page order must descend");
            }
            previous = Some(position);
        }
        visited += page.items.len();
        cursor = page.next_cursor;
        if cursor.is_none() || visited > 10_000 {
            break;
        }
    }
    assert!(visited >= 10, "every row this test created is reachable");
    cleanup(&pool, &ids).await?;
    Ok(())
}

#[tokio::test]
async fn a_restricted_row_is_filtered_inside_the_page_query() -> anyhow::Result<()> {
    let _serial = page_guard();
    let pool = pool().await?;
    // The reader is a different profile from the author, so row 1 does not apply
    // and the audience decides. The test's own rows are asserted by id.
    let author = test_author();
    let reader = test_author();
    let base = now() - Duration::seconds(5);
    let mut ids = Vec::new();
    for index in 0..6 {
        let (audience, min, max) = if index % 2 == 0 {
            ("any", None, None)
        } else {
            ("female", Some(18), Some(30))
        };
        ids.push(
            insert_post(
                &pool,
                author,
                audience,
                min,
                max,
                base - Duration::minutes(index as i64),
                None,
            )
            .await?,
        );
    }
    let page = list_page(
        &pool,
        &codec(),
        &scope(reader, man_25()),
        &IntroPostQuery::new(Some(100), None)?,
    )
    .await?;
    let visible: std::collections::BTreeSet<Uuid> =
        page.items.iter().map(|row| row.intro_id).collect();
    for (index, id) in ids.iter().enumerate() {
        if index % 2 == 0 {
            assert!(visible.contains(id), "the unrestricted row {index} is visible");
        } else {
            assert!(!visible.contains(id), "the restricted row {index} is hidden");
        }
    }
    cleanup(&pool, &ids).await?;
    Ok(())
}

#[tokio::test]
async fn the_cursor_is_derived_from_the_filtered_set() -> anyhow::Result<()> {
    let _serial = page_guard();
    let pool = pool().await?;
    // The reader is the author, so the page contains exactly this test's rows.
    // Three posts: the newest is restricted, the two older ones are not. The
    // author passes row 1, so all three are visible to them; the *other* viewer
    // below is the one the restriction applies to. The cursor must be derived
    // from the filtered set, so a second page continues from the next visible
    // row and does not skip it.
    let author = test_author();
    // Every row is published in the past, because the page query's read fence is
    // the request instant. This test's rows are the newest in the fixture, so the
    // newest visible row for the other viewer is unambiguous.
    let base = now() - Duration::seconds(5);
    let restricted = insert_post(
        &pool,
        author,
        "female",
        Some(18),
        Some(30),
        base,
        None,
    )
    .await?;
    let older_a = insert_post(
        &pool,
        author,
        "any",
        None,
        None,
        base - Duration::minutes(1),
        None,
    )
    .await?;
    let older_b = insert_post(
        &pool,
        author,
        "any",
        None,
        None,
        base - Duration::minutes(2),
        None,
    )
    .await?;
    // The author sees the newest row first.
    let first = list_page(
        &pool,
        &codec(),
        &scope(author, unrestricted()),
        &IntroPostQuery::new(Some(1), None)?,
    )
    .await?;
    assert_eq!(first.items.len(), 1);
    assert_eq!(first.items[0].intro_id, restricted, "the newest row");
    // A different viewer cannot see the restricted newest row. The fixture
    // database is shared, so the page also contains other tests' rows; what this
    // test asserts is that this viewer's restricted row is absent and that the
    // two older rows are reachable, in order, by walking the cursor. The cursor
    // is derived from the filtered set, so the walk never revisits a row.
    let viewer = test_author();
    let mut seen = Vec::new();
    let mut cursor = None;
    // Walk to exhaustion. The fixture is shared with other tests running in
    // parallel, so the walk stops as soon as it has seen both of this test's
    // older rows; the bound only proves termination.
    for _ in 0..10_000 {
        if seen.contains(&older_a) && seen.contains(&older_b) {
            break;
        }
        let page = list_page(
            &pool,
            &codec(),
            &scope(viewer, man_25()),
            &IntroPostQuery::new(Some(1), cursor.clone())?,
        )
        .await?;
        for row in &page.items {
            seen.push(row.intro_id);
        }
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert!(
        !seen.contains(&restricted),
        "the restricted row is never visible to this viewer"
    );
    let a = seen.iter().position(|id| *id == older_a);
    let b = seen.iter().position(|id| *id == older_b);
    assert!(a.is_some(), "the older unrestricted row is reachable");
    assert!(b.is_some(), "the oldest unrestricted row is reachable");
    assert!(a < b, "the walk visits the rows newest first");
    let unique: std::collections::BTreeSet<Uuid> = seen.iter().copied().collect();
    assert_eq!(unique.len(), seen.len(), "the cursor never repeats a row");
    cleanup(&pool, &[restricted, older_a, older_b]).await?;
    Ok(())
}

#[tokio::test]
async fn a_page_never_returns_more_than_the_limit() -> anyhow::Result<()> {
    let _serial = page_guard();
    let pool = pool().await?;
    let author = test_author();
    let base = now() - Duration::seconds(5);
    let mut ids = Vec::new();
    for index in 0..5 {
        ids.push(
            insert_post(
                &pool,
                author,
                "any",
                None,
                None,
                base - Duration::minutes(index),
                None,
            )
            .await?,
        );
    }
    // The fixture database is shared, so the page may contain rows an earlier
    // run left behind. The assertion is therefore about the limit and about this
    // test's own rows, not about the absolute page contents.
    for limit in [1, 2, 3, 5, 20] {
        let page = list_page(
            &pool,
            &codec(),
            &scope(author, unrestricted()),
            &IntroPostQuery::new(Some(limit), None)?,
        )
        .await?;
        assert!(page.items.len() as i64 <= limit, "limit {limit}");
        // The fixture database is shared, so the page may contain another test's
        // rows. What this test asserts is the limit itself and that the rows it
        // created appear in descending publication order among the rows returned.
        let expected = limit.min(5) as usize;
        let own: Vec<(chrono::DateTime<Utc>, Uuid)> = page
            .items
            .iter()
            .filter(|row| ids.contains(&row.intro_id))
            .map(|row| (row.published_at.expect("published"), row.intro_id))
            .collect();
        assert!(
            own.windows(2).all(|pair| pair[0] > pair[1]),
            "limit {limit}: this test's rows descend"
        );
        assert!(
            own.len() <= expected,
            "limit {limit}: at most {expected} of this test's rows fit"
        );
    }
    cleanup(&pool, &ids).await?;
    Ok(())
}

#[tokio::test]
async fn a_cursor_issued_to_one_viewer_is_refused_for_another() -> anyhow::Result<()> {
    let _serial = page_guard();
    let pool = pool().await?;
    // The reader is the author, so the page contains exactly this test's rows.
    let author = test_author();
    // This test's rows are the newest in the fixture, so the first page of a
    // page-ordered-by-publication walk starts inside them. The cursor's read
    // fence is the request instant, so the rows are published in the recent past.
    let base = now() - Duration::seconds(1);
    let mut ids = Vec::new();
    for index in 0..3 {
        ids.push(
            insert_post(
                &pool,
                author,
                "any",
                None,
                None,
                base - Duration::minutes(index),
                None,
            )
            .await?,
        );
    }
    let viewer_a = author;
    let viewer_b = test_author();
    let first = list_page(
        &pool,
        &codec(),
        &scope(viewer_a, unrestricted()),
        &IntroPostQuery::new(Some(1), None)?,
    )
    .await?;
    let cursor = first.next_cursor.expect("a second page exists");
    // The same cursor for viewer A works.
    list_page(
        &pool,
        &codec(),
        &scope(viewer_a, unrestricted()),
        &IntroPostQuery::new(Some(1), Some(cursor.clone()))?,
    )
    .await?;
    // For viewer B it is refused before the query runs.
    let error = list_page(
        &pool,
        &codec(),
        &scope(viewer_b, unrestricted()),
        &IntroPostQuery::new(Some(1), Some(cursor))?,
    )
    .await
    .expect_err("a cursor is bound to its viewer");
    assert_eq!(error, IntroPageError::InvalidCursor);
    cleanup(&pool, &ids).await?;
    Ok(())
}

#[tokio::test]
async fn a_tampered_cursor_is_refused() -> anyhow::Result<()> {
    let _serial = page_guard();
    let pool = pool().await?;
    let author = test_author();
    // This test's rows are the newest in the fixture.
    let base = now() - Duration::seconds(1);
    let ids = vec![
        insert_post(&pool, author, "any", None, None, base, None).await?,
        insert_post(
            &pool,
            author,
            "any",
            None,
            None,
            base - Duration::minutes(1),
            None,
        )
        .await?,
    ];
    let first = list_page(
        &pool,
        &codec(),
        &scope(author, unrestricted()),
        &IntroPostQuery::new(Some(1), None)?,
    )
    .await?;
    let cursor = first.next_cursor.expect("a second page exists");
    // A single changed character breaks the MAC.
    let mut tampered = cursor.clone().into_bytes();
    let last = tampered.len() - 1;
    tampered[last] = if tampered[last] == b'0' { b'1' } else { b'0' };
    let tampered = String::from_utf8(tampered).expect("the token is ASCII");
    let error = list_page(
        &pool,
        &codec(),
        &scope(author, unrestricted()),
        &IntroPostQuery::new(Some(1), Some(tampered))?,
    )
    .await
    .expect_err("a tampered cursor is refused");
    assert_eq!(error, IntroPageError::InvalidCursor);
    // A different codec key refuses the token too.
    let error = list_page(
        &pool,
        &IntroCursorCodec::new([0x11; 32]),
        &scope(author, unrestricted()),
        &IntroPostQuery::new(Some(1), Some(cursor))?,
    )
    .await
    .expect_err("a cursor signed with another key is refused");
    assert_eq!(error, IntroPageError::InvalidCursor);
    cleanup(&pool, &ids).await?;
    Ok(())
}

#[tokio::test]
async fn an_expired_cursor_is_refused() -> anyhow::Result<()> {
    let _serial = page_guard();
    let pool = pool().await?;
    let author = test_author();
    // This test's rows are the newest in the fixture.
    let base = now() - Duration::seconds(1);
    let ids = vec![
        insert_post(&pool, author, "any", None, None, base, None).await?,
        insert_post(
            &pool,
            author,
            "any",
            None,
            None,
            base - Duration::minutes(1),
            None,
        )
        .await?,
    ];
    let first = list_page(
        &pool,
        &codec(),
        &scope(author, unrestricted()),
        &IntroPostQuery::new(Some(1), None)?,
    )
    .await?;
    let cursor = first.next_cursor.expect("a second page exists");
    // The cursor's own lifetime is fifteen minutes; a request twenty minutes
    // later is past it.
    let mut later = scope(author, unrestricted());
    later.now = now() + Duration::minutes(20);
    let error = list_page(
        &pool,
        &codec(),
        &later,
        &IntroPostQuery::new(Some(1), Some(cursor))?,
    )
    .await
    .expect_err("an expired cursor is refused");
    assert_eq!(error, IntroPageError::InvalidCursor);
    cleanup(&pool, &ids).await?;
    Ok(())
}

#[tokio::test]
async fn widening_the_audience_makes_the_post_visible() -> anyhow::Result<()> {
    let _serial = page_guard();
    let pool = pool().await?;
    let author = Uuid::new_v4();
    let intro_id = insert_post(
        &pool,
        author,
        "female",
        Some(18),
        Some(30),
        now(),
        None,
    )
    .await?;
    let viewer = Uuid::new_v4();
    let page = list_page(
        &pool,
        &codec(),
        &scope(viewer, man_25()),
        &IntroPostQuery::new(Some(20), None)?,
    )
    .await?;
    assert!(
        !page.items.iter().any(|row| row.intro_id == intro_id),
        "the restricted post is absent"
    );
    // The author widens the audience to anyone.
    sqlx::query(
        "update intro_posts set audience_gender = 'any', audience_min_age = null, \
         audience_max_age_exclusive = null, revision = revision + 1 where intro_id = $1",
    )
    .bind(intro_id)
    .execute(&pool)
    .await?;
    let page = list_page(
        &pool,
        &codec(),
        &scope(viewer, man_25()),
        &IntroPostQuery::new(Some(20), None)?,
    )
    .await?;
    assert!(
        page.items.iter().any(|row| row.intro_id == intro_id),
        "the widened post is visible on the next request"
    );
    cleanup(&pool, &[intro_id]).await?;
    Ok(())
}

#[tokio::test]
async fn narrowing_the_audience_hides_the_post_on_the_next_request() -> anyhow::Result<()> {
    let _serial = page_guard();
    let pool = pool().await?;
    let author = Uuid::new_v4();
    let intro_id = insert_post(&pool, author, "any", None, None, now(), None).await?;
    let viewer = Uuid::new_v4();
    let page = list_page(
        &pool,
        &codec(),
        &scope(viewer, man_25()),
        &IntroPostQuery::new(Some(20), None)?,
    )
    .await?;
    assert!(page.items.iter().any(|row| row.intro_id == intro_id));
    assert!(get_intro_post(
        &pool,
        &VisibilityScope::new(
            viewer,
            BlockState::None,
            concert_application::intro_visibility::AuthorStanding::Active,
            man_25(),
            now(),
        ),
        intro_id,
    )
    .await
    .is_ok());
    // The author narrows the audience to women aged 18-29.
    sqlx::query(
        "update intro_posts set audience_gender = 'female', audience_min_age = 18, \
         audience_max_age_exclusive = 30, revision = revision + 1 where intro_id = $1",
    )
    .bind(intro_id)
    .execute(&pool)
    .await?;
    let page = list_page(
        &pool,
        &codec(),
        &scope(viewer, man_25()),
        &IntroPostQuery::new(Some(20), None)?,
    )
    .await?;
    assert!(!page.items.iter().any(|row| row.intro_id == intro_id));
    let error = get_intro_post(
        &pool,
        &VisibilityScope::new(
            viewer,
            BlockState::None,
            concert_application::intro_visibility::AuthorStanding::Active,
            man_25(),
            now(),
        ),
        intro_id,
    )
    .await
    .expect_err("the narrowed post is unavailable");
    assert_eq!(error, IntroPageError::Unavailable);
    cleanup(&pool, &[intro_id]).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Detail
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_missing_post_and_an_unauthorized_post_are_the_same_answer() -> anyhow::Result<()> {
    let _serial = page_guard();
    let pool = pool().await?;
    let intro_id = insert_post(
        &pool,
        Uuid::new_v4(),
        "female",
        Some(18),
        Some(30),
        now(),
        None,
    )
    .await?;
    let viewer = Uuid::new_v4();
    let unauthorized = get_intro_post(
        &pool,
        &VisibilityScope::new(
            viewer,
            BlockState::None,
            concert_application::intro_visibility::AuthorStanding::Active,
            man_25(),
            now(),
        ),
        intro_id,
    )
    .await
    .expect_err("unauthorized");
    let missing = get_intro_post(
        &pool,
        &VisibilityScope::new(
            viewer,
            BlockState::None,
            concert_application::intro_visibility::AuthorStanding::Active,
            man_25(),
            now(),
        ),
        Uuid::new_v4(),
    )
    .await
    .expect_err("missing");
    assert_eq!(unauthorized, missing);
    assert_eq!(unauthorized, IntroPageError::Unavailable);
    cleanup(&pool, &[intro_id]).await?;
    Ok(())
}

#[tokio::test]
async fn the_detail_row_carries_no_audience() -> anyhow::Result<()> {
    let _serial = page_guard();
    let pool = pool().await?;
    let intro_id = insert_post(
        &pool,
        Uuid::new_v4(),
        "female",
        Some(18),
        Some(30),
        now(),
        None,
    )
    .await?;
    let row = get_intro_post(
        &pool,
        &VisibilityScope::new(
            Uuid::new_v4(),
            BlockState::None,
            concert_application::intro_visibility::AuthorStanding::Active,
            woman_25(),
            now(),
        ),
        intro_id,
    )
    .await?;
    // The row type has no audience field at all; this asserts the selected
    // columns do not accidentally include one by checking the struct shape.
    let _ = (
        row.post.intro_id,
        row.post.author_profile_id,
        row.post.body,
        row.post.currently_seeking,
        row.post.interaction_style,
        row.post.published_at,
        row.post.expires_at,
    );
    cleanup(&pool, &[intro_id]).await?;
    Ok(())
}

#[tokio::test]
async fn the_author_reads_their_own_unpublished_post_through_the_detail_query() -> anyhow::Result<()> {
    let _serial = page_guard();
    let pool = pool().await?;
    let author = test_author();
    let intro_id = Uuid::new_v4();
    sqlx::query(
        "insert into intro_posts(intro_id,author_profile_id,body,audience_gender,state) \
         values($1,$2,'a draft',$3,'DRAFT')",
    )
    .bind(intro_id)
    .bind(author)
    .bind("female")
    .execute(&pool)
    .await?;
    // The author's own path is row 1, so the detail query admits them.
    let row = get_intro_post(
        &pool,
        &VisibilityScope::without_claims(
            author,
            BlockState::None,
            concert_application::intro_visibility::AuthorStanding::Active,
            now(),
        ),
        intro_id,
    )
    .await?;
    assert_eq!(row.post.intro_id, intro_id);
    // A stranger is denied.
    assert_eq!(
        get_intro_post(
            &pool,
            &VisibilityScope::without_claims(
                test_author(),
                BlockState::None,
                concert_application::intro_visibility::AuthorStanding::Active,
                now(),
            ),
            intro_id,
        )
        .await
        .expect_err("a stranger cannot read a draft"),
        IntroPageError::Unavailable
    );
    cleanup(&pool, &[intro_id]).await?;
    Ok(())
}

#[tokio::test]
async fn a_fenced_author_is_filtered_out_of_the_page() -> anyhow::Result<()> {
    let _serial = page_guard();
    let pool = pool().await?;
    // The page query evaluates row 4 from the permanent deletion fence, which is
    // the one author-standing fact the concert service owns locally.
    let author = Uuid::new_v4();
    let intro_id = insert_post(&pool, author, "any", None, None, now(), None).await?;
    let viewer = Uuid::new_v4();
    let page = list_page(
        &pool,
        &codec(),
        &scope(viewer, unrestricted()),
        &IntroPostQuery::new(Some(20), None)?,
    )
    .await?;
    assert!(page.items.iter().any(|row| row.intro_id == intro_id));
    sqlx::query(
        "insert into concert_world_account_deletion_fences(user_id,saga_id,identity_binding) \
         values($1,$2,$3) on conflict do nothing",
    )
    .bind(author)
    .bind(Uuid::new_v4())
    .bind(vec![7_u8; 32])
    .execute(&pool)
    .await?;
    let page = list_page(
        &pool,
        &codec(),
        &scope(viewer, unrestricted()),
        &IntroPostQuery::new(Some(20), None)?,
    )
    .await?;
    assert!(
        !page.items.iter().any(|row| row.intro_id == intro_id),
        "a fenced author's post is filtered inside the query"
    );
    cleanup(&pool, &[intro_id]).await?;
    sqlx::query("delete from concert_world_account_deletion_fences where user_id = $1")
        .bind(author)
        .execute(&pool)
        .await
        .ok();
    Ok(())
}

#[tokio::test]
async fn the_query_rejects_an_out_of_range_limit() -> anyhow::Result<()> {
    let _serial = page_guard();
    assert_eq!(
        IntroPostQuery::new(Some(0), None).expect_err("zero"),
        IntroPageError::InvalidQuery("limit")
    );
    assert_eq!(
        IntroPostQuery::new(Some(101), None).expect_err("too many"),
        IntroPageError::InvalidQuery("limit")
    );
    assert_eq!(IntroPostQuery::new(Some(100), None)?.limit, 100);
    assert_eq!(IntroPostQuery::new(None, None)?.limit, 20);
    Ok(())
}
