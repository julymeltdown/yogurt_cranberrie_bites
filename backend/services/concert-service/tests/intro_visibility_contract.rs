//! T-305: the SQL visibility predicate against the shared case table.
//!
//! The case table is `crates/concert-application/tests/visibility_cases/mod.rs`,
//! included here with `#[path]`. Every case is inserted as a real `intro_posts`
//! row and evaluated by the SQL predicate against a real PostgreSQL 16
//! database; the expected decision is the one the pure function returns for the
//! same case. A divergence between the two implementations fails this binary.
//!
//! The database is the runner-owned private cluster reached through
//! `CONCERT_TEST_DATABASE_URL`; the URL must name the `mcr_owner` role. The
//! service's own migration files are applied, so the predicate is evaluated
//! against the exact schema a deployment has, including the audience CHECK
//! constraints.
#[path = "../../../crates/concert-application/tests/visibility_cases/mod.rs"]
mod visibility_cases;

use concert_application::intro_visibility::{PostState, Row};
use concert_domain::intro_audience::IntroAudience;
use concert_service::intro_posts::visibility_sql::{
    expected_decision, expected_row, predicate, VisibilityParameters, VisibilityScope,
};
use sqlx::PgPool;
use uuid::Uuid;
use visibility_cases::*;

// ---------------------------------------------------------------------------
// Fixture
// ---------------------------------------------------------------------------

/// The fixture database is shared by every test **and every test binary**, and the
/// block rows are global state: two tests writing `discovery_blocks` at once would
/// each see the other's rows. A process-local mutex cannot coordinate across
/// binaries, so the guard is a PostgreSQL advisory lock held for the test body —
/// the same mechanism the migration helper uses.
async fn fixture_guard(pool: &PgPool) -> sqlx::pool::PoolConnection<sqlx::Postgres> {
    let mut connection = pool.acquire().await.expect("a fixture connection");
    sqlx::query("select pg_advisory_lock(hashtextextended('wp04-shared-fixture', 0))")
        .execute(&mut *connection)
        .await
        .expect("the fixture lock");
    connection
}

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

/// Apply every concert-service migration once, under an advisory lock so the
/// test binaries of one run cannot race.
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
        // The mirror rows are idempotent and must exist even when the schema was
        // migrated by an earlier run or another test binary.
        seed_profiles(pool).await?;
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
    seed_profiles(pool).await?;
    Ok(())
}

/// Mirror the fixture profiles into `discovery_profiles`.
///
/// The predicate's block term resolves a viewer/author pair through this mirror.
/// A pair it cannot resolve is denied, which is the fail-closed direction, so the
/// fixture must contain the profiles the cases use or every case would be denied
/// for the wrong reason. The rows are inserted idempotently, so a second test
/// binary in the same run does not fail on the primary key.
async fn seed_profiles(pool: &PgPool) -> anyhow::Result<()> {
    for (index, profile_id) in [AUTHOR, VIEWER, OTHER_VIEWER].into_iter().enumerate() {
        sqlx::query(
            "insert into discovery_profiles(profile_id, account_id, nickname, bio, visibility) \
             values($1,$2,$3,'fixture','PUBLIC') on conflict (profile_id) do nothing",
        )
        .bind(profile_id)
        // The account mapping is the shared table's, so a block case's declared
        // rows name accounts this fixture actually created.
        .bind(visibility_cases::account_of(profile_id))
        .bind(format!("wp04-fixture-{index}"))
        .execute(pool)
        .await?;
    }
    Ok(())
}

/// Create the block rows a case declares, and remove any left by another case.
///
/// The block cases run in one binary against one database, so a row written for
/// one case would deny every later case. The table is cleared for the two fixture
/// accounts before each insert, which is safe because these accounts exist only
/// for this fixture.
async fn apply_block_rows(
    pool: &PgPool,
    rows: &[(Uuid, Uuid)],
) -> anyhow::Result<()> {
    for account in [
        visibility_cases::account_of(AUTHOR),
        visibility_cases::account_of(VIEWER),
        visibility_cases::account_of(OTHER_VIEWER),
    ] {
        sqlx::query(
            "delete from discovery_blocks \
             where blocker_account_id = $1 or blocked_account_id = $1",
        )
        .bind(account)
        .execute(pool)
        .await?;
    }
    for (blocker, blocked) in rows {
        sqlx::query(
            "insert into discovery_blocks(blocker_account_id, blocked_account_id) \
             values($1,$2) on conflict do nothing",
        )
        .bind(blocker)
        .bind(blocked)
        .execute(pool)
        .await?;
    }
    Ok(())
}

/// Insert one `intro_posts` row exactly as the case describes it.
///
/// The row's `author_profile_id` is the case's author; the state, audience and
/// expiry come from the case. Nothing else is invented: the columns the
/// predicate reads are the columns the case sets.
async fn insert_case(pool: &PgPool, case: &visibility_cases::Case) -> anyhow::Result<Uuid> {
    let intro_id = Uuid::new_v4();
    let audience = case.post.audience;
    let published_at = if case.post.state == PostState::Published {
        Some(now() - chrono::Duration::days(1))
    } else {
        None
    };
    sqlx::query(
        "insert into intro_posts(intro_id,author_profile_id,body,audience_gender,\
         audience_min_age,audience_max_age_exclusive,state,published_at,expires_at) \
         values($1,$2,'a case body',$3,$4,$5,$6,$7,$8)",
    )
    .bind(intro_id)
    .bind(case.post.author_profile_id)
    .bind(audience.gender().as_str())
    .bind(audience.minimum_age().map(|age| age.get()))
    .bind(audience.maximum_age_exclusive().map(|max| max.get()))
    .bind(case.post.state.as_str())
    .bind(published_at)
    .bind(case.post.expires_at)
    .execute(pool)
    .await?;
    Ok(intro_id)
}

/// Run the predicate for one row and scope.
async fn sql_visible(
    pool: &PgPool,
    scope: &VisibilityScope,
    intro_id: Uuid,
) -> anyhow::Result<bool> {
    let sql = format!(
        "select {} as visible from intro_posts p where p.intro_id = ${}",
        predicate(1),
        VisibilityParameters::PLACEHOLDERS + 1
    );
    let query = sqlx::query_scalar::<_, bool>(&sql);
    let query = scope.parameters().bind_scalar(query);
    Ok(query.bind(intro_id).fetch_one(pool).await?)
}

/// Delete a case row and everything that references it.
async fn cleanup(pool: &PgPool, intro_id: Uuid) -> anyhow::Result<()> {
    sqlx::query("delete from intro_posts where intro_id = $1")
        .bind(intro_id)
        .execute(pool)
        .await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// The parity test
// ---------------------------------------------------------------------------

#[tokio::test]
async fn every_shared_case_agrees_between_the_pure_function_and_sql() -> anyhow::Result<()> {
    let pool = pool().await?;
    let _serial = fixture_guard(&pool).await;
    // A previous test (or the parity loop) may have left block rows; every test
    // here asserts against a known fixture, so the blocks start empty.
    apply_block_rows(&pool, &[]).await?;
    let cases = all_cases();
    assert!(cases.len() >= 40, "the shared table must be exhaustive");
    let mut disagreements = Vec::new();
    for case in &cases {
        // The block rows are per case: a row left by an earlier case would deny
        // this one, so the fixture is reset to exactly what this case declares.
        apply_block_rows(&pool, case.block_rows).await?;
        let intro_id = insert_case(&pool, case).await?;
        let scope = VisibilityScope::new(
            case.viewer.profile_id,
            case.viewer.blocks,
            case.post.author_standing,
            scope_attributes(case),
            case.viewer.now,
        );
        let sql = sql_visible(&pool, &scope, intro_id).await?;
        let pure = expected_decision(
            &scope,
            intro_id,
            case.post.author_profile_id,
            case.post.state,
            case.post.audience,
            case.post.expires_at,
        );
        // The case's own expectation is the third opinion: the table says what
        // the answer must be, the pure function computes it, and SQL must agree
        // with both.
        let (case_decision, case_row) = expected_row(
            &scope,
            intro_id,
            case.post.author_profile_id,
            case.post.state,
            case.post.audience,
            case.post.expires_at,
        );
        if sql != pure.is_allowed()
            || case_decision != case.expected
            || case_row != case.expected_row
        {
            disagreements.push(format!(
                "{} ({}): sql={sql} pure={:?} case={:?} row={:?}/{:?}",
                case.id,
                case.what,
                pure,
                case.expected,
                case_row,
                case.expected_row
            ));
        }
        cleanup(&pool, intro_id).await?;
    }
    assert!(
        disagreements.is_empty(),
        "pure and SQL decisions diverged:\n{}",
        disagreements.join("\n")
    );
    Ok(())
}

/// The SQL scope's attributes for a case.
///
/// The case carries an `IdentityClaims` snapshot; the SQL predicate takes the
/// current age and the gender bucket. The conversion is the adapter's job in
/// production and is done here the same way: the current age comes from the
/// claim's Seoul-date arithmetic, and `Unspecified` becomes
/// `gender: None, gender_specified: true`.
fn scope_attributes(case: &visibility_cases::Case) -> concert_service::intro_posts::visibility_sql::ViewerAttributes {
    use concert_application::identity_claims::Attribute;
    use concert_service::intro_posts::visibility_sql::ViewerAttributes;
    let Some(claims) = case.viewer.claims.as_ref() else {
        return ViewerAttributes::NotRead;
    };
    let age = claims
        .age
        .known()
        .and_then(|claim| concert_application::intro_visibility::current_age(claim, case.viewer.now));
    // The provider "answered" for a live gender and for the explicit
    // "not specified" answer; it did not answer for an unavailable claim.
    match claims.gender {
        // A live gender means the provider answered, but row 11 asks whether it
        // answered "not specified", which it did not.
        Attribute::Known(gender) => ViewerAttributes::read(age, Some(gender), false),
        Attribute::Unspecified => ViewerAttributes::read(age, None, true),
        Attribute::Unavailable => {
            // An unavailable gender with a known age is still a live age
            // snapshot: the age rows need it, and row 11 must still deny.
            if age.is_some() {
                ViewerAttributes::read(age, None, false)
            } else {
                ViewerAttributes::Unavailable
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The predicate itself
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_predicate_is_a_boolean_expression_over_the_post_alias() -> anyhow::Result<()> {
    let pool = pool().await?;
    let _serial = fixture_guard(&pool).await;
    // A previous test (or the parity loop) may have left block rows; every test
    // here asserts against a known fixture, so the blocks start empty.
    apply_block_rows(&pool, &[]).await?;
    // The fragment must be usable as a `where` clause with no subquery: this
    // query has no `join` and no `exists`, so a row is admitted by the predicate
    // alone.
    let sql = format!(
        "select count(*) from intro_posts p where {}",
        predicate(1)
    );
    let scope = VisibilityScope::new(
        VIEWER,
        concert_application::intro_visibility::BlockState::None,
        concert_application::intro_visibility::AuthorStanding::Active,
        concert_service::intro_posts::visibility_sql::ViewerAttributes::Unavailable,
        now(),
    );
    let count: i64 = scope
        .parameters()
        .bind_scalar(sqlx::query_scalar::<_, i64>(&sql))
        .fetch_one(&pool)
        .await?;
    // The count is whatever the fixture holds; the assertion is that the query
    // planned and ran without a parameter-count or type error.
    assert!(count >= 0);
    Ok(())
}

#[tokio::test]
async fn an_unrestricted_row_is_admitted_without_any_claim_parameter() -> anyhow::Result<()> {
    let pool = pool().await?;
    let _serial = fixture_guard(&pool).await;
    // A previous test (or the parity loop) may have left block rows; every test
    // here asserts against a known fixture, so the blocks start empty.
    apply_block_rows(&pool, &[]).await?;
    apply_block_rows(&pool, &[]).await?;
    // A viewer with no claim at all sees an unrestricted post: the SQL predicate
    // must not require the age or gender parameters to be present, only that the
    // row's audience is unrestricted.
    let intro_id = Uuid::new_v4();
    sqlx::query(
        "insert into intro_posts(intro_id,author_profile_id,body,audience_gender,state,published_at) \
         values($1,$2,'unrestricted',$3,'PUBLISHED',$4)",
    )
    .bind(intro_id)
    .bind(AUTHOR)
    .bind("any")
    .bind(now() - chrono::Duration::days(1))
    .execute(&pool)
    .await?;
    let scope = VisibilityScope::without_claims(
        VIEWER,
        concert_application::intro_visibility::BlockState::None,
        concert_application::intro_visibility::AuthorStanding::Active,
        now(),
    );
    assert!(sql_visible(&pool, &scope, intro_id).await?);
    // The same row with an age bound and no claim is denied: row 7.
    sqlx::query(
        "update intro_posts set audience_min_age = 18, audience_max_age_exclusive = 30, \
         revision = revision + 1 where intro_id = $1",
    )
    .bind(intro_id)
    .execute(&pool)
    .await?;
    assert!(!sql_visible(&pool, &scope, intro_id).await?);
    cleanup(&pool, intro_id).await?;
    Ok(())
}

#[tokio::test]
async fn the_author_sees_their_own_row_whatever_its_state() -> anyhow::Result<()> {
    let pool = pool().await?;
    let _serial = fixture_guard(&pool).await;
    // A previous test (or the parity loop) may have left block rows; every test
    // here asserts against a known fixture, so the blocks start empty.
    apply_block_rows(&pool, &[]).await?;
    apply_block_rows(&pool, &[]).await?;
    let intro_id = Uuid::new_v4();
    // A draft, expired, restricted by a female audience the author cannot
    // satisfy. Row 1 still admits the author.
    sqlx::query(
        "insert into intro_posts(intro_id,author_profile_id,body,audience_gender,\
         audience_min_age,audience_max_age_exclusive,state,expires_at) \
         values($1,$2,'own draft',$3,18,30,'DRAFT',$4)",
    )
    .bind(intro_id)
    .bind(AUTHOR)
    .bind("female")
    .bind(now() - chrono::Duration::days(1))
    .execute(&pool)
    .await?;
    let author = VisibilityScope::without_claims(
        AUTHOR,
        concert_application::intro_visibility::BlockState::Mutual,
        concert_application::intro_visibility::AuthorStanding::Sanctioned,
        now(),
    );
    assert!(sql_visible(&pool, &author, intro_id).await?);
    // A stranger with a live female claim in range is still denied: the post is
    // not published.
    let stranger = VisibilityScope::new(
        VIEWER,
        concert_application::intro_visibility::BlockState::None,
        concert_application::intro_visibility::AuthorStanding::Active,
        concert_service::intro_posts::visibility_sql::ViewerAttributes::read(
            Some(25),
            Some(concert_application::identity_claims::Gender::Female),
            true,
        ),
        now(),
    );
    assert!(!sql_visible(&pool, &stranger, intro_id).await?);
    cleanup(&pool, intro_id).await?;
    Ok(())
}

#[tokio::test]
async fn a_block_denies_even_an_unrestricted_row() -> anyhow::Result<()> {
    let pool = pool().await?;
    let _serial = fixture_guard(&pool).await;
    // A previous test (or the parity loop) may have left block rows; every test
    // here asserts against a known fixture, so the blocks start empty.
    apply_block_rows(&pool, &[]).await?;
    let intro_id = Uuid::new_v4();
    sqlx::query(
        "insert into intro_posts(intro_id,author_profile_id,body,audience_gender,state,published_at) \
         values($1,$2,'blocked',$3,'PUBLISHED',$4)",
    )
    .bind(intro_id)
    .bind(AUTHOR)
    .bind("any")
    .bind(now() - chrono::Duration::days(1))
    .execute(&pool)
    .await?;
    // The predicate resolves the block per row from the actual pair, so the test
    // writes the block rows rather than setting a scope flag. Each direction is
    // its own case: a block denies in both.
    let scope = VisibilityScope::without_claims(
        VIEWER,
        concert_application::intro_visibility::BlockState::None,
        concert_application::intro_visibility::AuthorStanding::Active,
        now(),
    );
    for (label, rows) in [
        (
            "viewer blocked author",
            vec![(visibility_cases::account_of(VIEWER), visibility_cases::account_of(AUTHOR))],
        ),
        (
            "author blocked viewer",
            vec![(visibility_cases::account_of(AUTHOR), visibility_cases::account_of(VIEWER))],
        ),
        (
            "mutual",
            vec![
                (visibility_cases::account_of(VIEWER), visibility_cases::account_of(AUTHOR)),
                (visibility_cases::account_of(AUTHOR), visibility_cases::account_of(VIEWER)),
            ],
        ),
    ] {
        apply_block_rows(&pool, &rows).await?;
        assert!(
            !sql_visible(&pool, &scope, intro_id).await?,
            "a block denies an unrestricted row: {label}"
        );
    }
    // With no block rows, the same row is visible: the denial above came from the
    // block, not from the fixture.
    apply_block_rows(&pool, &[]).await?;
    assert!(
        sql_visible(&pool, &scope, intro_id).await?,
        "the same row is visible once the block is lifted"
    );
    cleanup(&pool, intro_id).await?;
    Ok(())
}

#[tokio::test]
async fn the_predicate_can_be_composed_after_other_parameters() -> anyhow::Result<()> {
    let pool = pool().await?;
    let _serial = fixture_guard(&pool).await;
    // A previous test (or the parity loop) may have left block rows; every test
    // here asserts against a known fixture, so the blocks start empty.
    apply_block_rows(&pool, &[]).await?;
    // Another test may have left a block row; this test asserts admission.
    apply_block_rows(&pool, &[]).await?;
    // A caller with its own parameter takes the fragment at an offset. This is
    // what the page query does: `$1` is the cursor, the predicate starts at `$2`.
    let intro_id = Uuid::new_v4();
    sqlx::query(
        "insert into intro_posts(intro_id,author_profile_id,body,audience_gender,state,published_at) \
         values($1,$2,'composed',$3,'PUBLISHED',$4)",
    )
    .bind(intro_id)
    .bind(AUTHOR)
    .bind("any")
    .bind(now() - chrono::Duration::days(1))
    .execute(&pool)
    .await?;
    let scope = VisibilityScope::without_claims(
        VIEWER,
        concert_application::intro_visibility::BlockState::None,
        concert_application::intro_visibility::AuthorStanding::Active,
        now(),
    );
    let sql = format!(
        "select count(*) from intro_posts p where p.intro_id = $1 and {}",
        predicate(2)
    );
    // `$1` is the caller's own parameter; the predicate's placeholders start at
    // `$2`, so the caller's value must be bound first.
    let count: i64 = sqlx::query_scalar::<_, i64>(&sql)
        .bind(intro_id)
        .bind(scope.parameters().viewer)
        .bind(scope.parameters().author_active)
        .bind(scope.parameters().age)
        .bind(scope.parameters().gender)
        .bind(scope.parameters().provider_answered)
        .bind(scope.parameters().now)
        .fetch_one(&pool)
        .await?;
    assert_eq!(count, 1);
    cleanup(&pool, intro_id).await?;
    Ok(())
}

#[tokio::test]
async fn the_fragment_interpolates_no_value() -> anyhow::Result<()> {
    // The fragment is built from placeholder numbers only and no database is
    // touched, so this test needs no fixture lock.
    // The fragment is built from placeholder numbers only. A value that reached
    // the SQL text would be a SQL injection path and would also make the
    // predicate unplannable. This asserts the text contains nothing but
    // placeholders.
    let text = predicate(1);
    assert!(text.contains("$1"));
    // Six placeholders: the block term is a correlated subquery that reuses the
    // viewer's `$1`, so it adds none of its own.
    assert_eq!(VisibilityParameters::PLACEHOLDERS, 6);
    assert!(text.contains("$6"));
    assert!(!text.contains("$7"));
    for forbidden in ["'female'", "'male'", "'unknown'", "18", "30"] {
        // The audience *column values* `'any'`, `'female'`, `'male'` and
        // `'unknown'` are literals by design (they are the closed column
        // domain), but no numeric bound and no viewer value may appear.
        if forbidden.starts_with('\'') {
            continue;
        }
        assert!(!text.contains(forbidden), "the fragment contains {forbidden}");
    }
    // The closed audience domain is compared as literals, never as a bind.
    assert!(text.contains("p.audience_gender = 'any'"));
    assert!(text.contains("p.audience_gender in ('female','male')"));
    assert!(text.contains("p.audience_gender = 'unknown'"));
    Ok(())
}

#[tokio::test]
async fn the_row_table_is_reproducible_from_sql_columns() -> anyhow::Result<()> {
    // The predicate reads exactly five post columns and four scope facts. This
    // test pins the column set so a later change to the table cannot silently
    // read a column the fragment does not select.
    let pool = pool().await?;
    let _serial = fixture_guard(&pool).await;
    // A previous test (or the parity loop) may have left block rows; every test
    // here asserts against a known fixture, so the blocks start empty.
    apply_block_rows(&pool, &[]).await?;
    let columns: Vec<String> = sqlx::query_scalar(
        "select column_name from information_schema.columns \
         where table_name = 'intro_posts' order by column_name",
    )
    .fetch_all(&pool)
    .await?;
    for required in [
        "audience_gender",
        "audience_max_age_exclusive",
        "audience_min_age",
        "author_profile_id",
        "expires_at",
        "intro_id",
        "published_at",
        "state",
    ] {
        assert!(
            columns.iter().any(|column| column == required),
            "intro_posts must carry {required}"
        );
    }
    Ok(())
}

/// A row of the evidence table: the case, the SQL answer and the pure answer.
#[derive(Debug)]
pub struct EvidenceRow {
    pub id: &'static str,
    pub row: Row,
    pub sql: bool,
    pub pure: bool,
}

/// Run the whole table and return the evidence rows.
///
/// `every_shared_case_agrees_between_the_pure_function_and_sql` asserts on this;
/// the evidence generator for T-312 calls it to record the table result.
pub async fn evidence_rows(pool: &PgPool) -> anyhow::Result<Vec<EvidenceRow>> {
    let mut rows = Vec::new();
    for case in all_cases() {
        let intro_id = insert_case(pool, &case).await?;
        let scope = VisibilityScope::new(
            case.viewer.profile_id,
            case.viewer.blocks,
            case.post.author_standing,
            scope_attributes(&case),
            case.viewer.now,
        );
        let sql = sql_visible(pool, &scope, intro_id).await?;
        let (pure, row) = expected_row(
            &scope,
            intro_id,
            case.post.author_profile_id,
            case.post.state,
            case.post.audience,
            case.post.expires_at,
        );
        rows.push(EvidenceRow {
            id: case.id,
            row,
            sql,
            pure: pure.is_allowed(),
        });
        cleanup(pool, intro_id).await?;
    }
    Ok(rows)
}

/// The audience each case used, for the evidence record.
pub fn case_audience(case: &visibility_cases::Case) -> IntroAudience {
    case.post.audience
}

/// A viewer and author with **no** discovery projection row are not "blocked".
///
/// The block term resolves the pair's account ids through `discovery_profiles`,
/// which is a projection written by a background worker. When either side has
/// never been mirrored the subquery yields nothing, and an earlier version
/// wrapped that in `coalesce(..., true)` — so an unprojected pair evaluated to
/// "blocked" and every restricted post was denied to every viewer on a database
/// whose projection had not run yet. That is the state a freshly seeded stack is
/// in, which is why the product appeared to work only after a worker caught up.
///
/// A missing projection row means "no block is recorded", which is what an
/// absent block is. This test removes both mirror rows and asserts the pair is
/// still admitted.
#[tokio::test]
async fn an_unprojected_pair_is_not_treated_as_blocked() -> anyhow::Result<()> {
    let pool = pool().await?;
    let _serial = fixture_guard(&pool).await;
    apply_block_rows(&pool, &[]).await?;
    // Remove the mirror rows for exactly this pair, leaving the fixture's third
    // profile so a later test still finds its own.
    sqlx::query("delete from discovery_profiles where profile_id = any($1)")
        .bind(vec![AUTHOR, VIEWER])
        .execute(&pool)
        .await?;
    let scope = VisibilityScope::new(
        VIEWER,
        concert_application::intro_visibility::BlockState::None,
        concert_application::intro_visibility::AuthorStanding::Active,
        // A live, matching claim: the audience is satisfied, so the only thing
        // that could deny the row is the block term.
        concert_service::intro_posts::visibility_sql::ViewerAttributes::read(
            Some(25),
            Some(concert_application::identity_claims::Gender::Female),
            true,
        ),
        now(),
    );
    // An unrestricted published row owned by the unprojected author.
    let intro_id = Uuid::new_v4();
    sqlx::query(
        "insert into intro_posts(intro_id,author_profile_id,body,audience_gender,\
         audience_min_age,audience_max_age_exclusive,state,published_at,favorites_snapshot) \
         values($1,$2,'unprojected pair','any',null,null,'PUBLISHED',now(),'[]'::jsonb)",
    )
    .bind(intro_id)
    .bind(AUTHOR)
    .execute(&pool)
    .await?;
    let visible = sql_visible(&pool, &scope, intro_id).await?;
    cleanup(&pool, intro_id).await?;
    // Restore the mirror so the other cases in this binary keep their fixture.
    seed_profiles(&pool).await?;
    assert!(
        visible,
        "an unprojected viewer/author pair must not be denied as blocked"
    );
    Ok(())
}
