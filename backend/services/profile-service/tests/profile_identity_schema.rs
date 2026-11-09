//! WP-02 T-103, T-104, T-105, T-111: profile-service schema invariants.
//!
//! Every negative case asserts the specific constraint name that fires, not
//! merely that some error occurred. That is deliberate: a migration that drops a
//! CHECK to "make the migration pass" would still satisfy a generic error
//! assertion, but it cannot satisfy a named one.
//!
//! The database is the runner-owned private cluster described by
//! `backend/scripts/mcr/migration_smoke.py`; the URL arrives through
//! `PROFILE_TEST_DATABASE_URL`.
use sqlx::{PgPool, Row};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Fixture
// ---------------------------------------------------------------------------

async fn pool() -> anyhow::Result<PgPool> {
    let url = std::env::var("PROFILE_TEST_DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("PROFILE_TEST_DATABASE_URL is required"))?;
    let options: sqlx::postgres::PgConnectOptions = url.parse()?;
    anyhow::ensure!(
        options.get_username() == "mcr_owner",
        "schema fixture must use the owned cluster"
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
    sqlx::query("select pg_advisory_xact_lock(hashtextextended('wp02-profile-migrations',0))")
        .execute(&mut *tx)
        .await?;
    let applied: bool =
        sqlx::query_scalar("select to_regclass('public.profile_product_scopes') is not null")
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

/// The constraint name a failed statement reported, if any.
fn constraint(error: &sqlx::Error) -> Option<String> {
    match error {
        sqlx::Error::Database(database) => database.constraint().map(str::to_owned),
        _ => None,
    }
}

/// Assert that `statement` fails with exactly `expected`.
async fn rejects(pool: &PgPool, expected: &str, statement: &str) {
    let error = sqlx::query(statement)
        .execute(pool)
        .await
        .expect_err(&format!("statement must violate {expected}: {statement}"));
    assert_eq!(
        constraint(&error).as_deref(),
        Some(expected),
        "expected constraint {expected}, got {error:?}"
    );
}

async fn account(pool: &PgPool) -> anyhow::Result<Uuid> {
    let account = Uuid::new_v4();
    sqlx::query(
        "insert into profile_identities(account_id, nickname) values($1,'fixture')",
    )
    .bind(account)
    .execute(pool)
    .await?;
    Ok(account)
}

async fn profile_of(pool: &PgPool, account: Uuid) -> anyhow::Result<Uuid> {
    Ok(
        sqlx::query_scalar("select profile_id from profile_identities where account_id=$1")
            .bind(account)
            .fetch_one(pool)
            .await?,
    )
}

// ---------------------------------------------------------------------------
// T-103 profiles
// ---------------------------------------------------------------------------

#[tokio::test]
async fn profile_identity_is_unique_per_live_account() -> anyhow::Result<()> {
    let pool = pool().await?;
    let account = account(&pool).await?;
    // A second live identity for the same account is refused by the partial
    // unique index.
    let error = sqlx::query("insert into profile_identities(account_id, nickname) values($1,'again')")
        .bind(account)
        .execute(&pool)
        .await
        .expect_err("a second live profile must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("profile_identities_live_account")
    );
    // A soft-deleted row does not block a new live identity.
    sqlx::query("update profile_identities set deleted_at=clock_timestamp(), revision=revision+1 where account_id=$1")
        .bind(account)
        .execute(&pool)
        .await?;
    sqlx::query("insert into profile_identities(account_id, nickname) values($1,'replacement')")
        .bind(account)
        .execute(&pool)
        .await?;
    Ok(())
}

#[tokio::test]
async fn profile_identity_id_is_immutable() -> anyhow::Result<()> {
    let pool = pool().await?;
    let account = account(&pool).await?;
    let profile = profile_of(&pool, account).await?;
    let error = sqlx::query("update profile_identities set profile_id=$2, revision=revision+1 where profile_id=$1")
        .bind(profile)
        .bind(Uuid::new_v4())
        .execute(&pool)
        .await
        .expect_err("profile id must be immutable");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("profile_identities_immutable_identity")
    );
    // The account binding is immutable too.
    let error = sqlx::query("update profile_identities set account_id=$2, revision=revision+1 where profile_id=$1")
        .bind(profile)
        .bind(Uuid::new_v4())
        .execute(&pool)
        .await
        .expect_err("account id must be immutable");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("profile_identities_immutable_identity")
    );
    Ok(())
}

#[tokio::test]
async fn profile_nickname_and_bio_bounds_are_enforced() -> anyhow::Result<()> {
    let pool = pool().await?;
    // Empty nickname.
    let error = sqlx::query("insert into profile_identities(account_id, nickname) values($1,'')")
        .bind(Uuid::new_v4())
        .execute(&pool)
        .await
        .expect_err("empty nickname must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("profile_identities_nickname_length")
    );
    // 41 characters.
    let error = sqlx::query("insert into profile_identities(account_id, nickname) values($1,$2)")
        .bind(Uuid::new_v4())
        .bind("x".repeat(41))
        .execute(&pool)
        .await
        .expect_err("41-character nickname must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("profile_identities_nickname_length")
    );
    // Bio above 160 characters.
    let error = sqlx::query("insert into profile_identities(account_id, nickname, bio) values($1,'ok',$2)")
        .bind(Uuid::new_v4())
        .bind("y".repeat(161))
        .execute(&pool)
        .await
        .expect_err("161-character bio must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("profile_identities_bio_length")
    );
    // Visibility is a closed set.
    let error = sqlx::query("insert into profile_identities(account_id, nickname, visibility) values($1,'ok','FRIENDS')")
        .bind(Uuid::new_v4())
        .execute(&pool)
        .await
        .expect_err("unknown visibility must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("profile_identities_visibility")
    );
    Ok(())
}

#[tokio::test]
async fn the_backfill_assigns_exactly_one_identity_per_profile() -> anyhow::Result<()> {
    let pool = pool().await?;
    let account = Uuid::new_v4();
    sqlx::query(
        "insert into profiles(user_id, username, nickname, avatar_url, bio) \
         values($1,$2,'backfilled','','a backfilled bio')",
    )
    .bind(account)
    .bind(format!("backfill-{}", account.simple()))
    .execute(&pool)
    .await?;
    // Re-running the identity migration's backfill statement must be a no-op and
    // must produce the same deterministic id.
    let sql = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("migrations/0011_profile_identity.sql"),
    )?;
    // The migration is already applied, so a second full application would fail
    // on the table; apply only the backfill statement, which is idempotent.
    let backfill = sql
        .split_once("insert into profile_identities (profile_id, account_id")
        .map(|(_, rest)| format!("insert into profile_identities (profile_id, account_id{rest}"))
        .and_then(|statement| statement.split_once("on conflict (profile_id) do nothing;").map(|(head, _)| format!("{head}on conflict (profile_id) do nothing;")))
        .expect("backfill statement");
    sqlx::raw_sql(&backfill).execute(&pool).await?;
    let ids: Vec<Uuid> =
        sqlx::query_scalar("select profile_id from profile_identities where account_id=$1")
            .bind(account)
            .fetch_all(&pool)
            .await?;
    assert_eq!(ids.len(), 1, "backfill must be idempotent");
    let expected: Uuid = sqlx::query_scalar("select md5('mcr profile identity v1:' || $1::text)::uuid")
        .bind(account)
        .fetch_one(&pool)
        .await?;
    assert_eq!(ids[0], expected, "the backfilled id must be deterministic");
    Ok(())
}

// ---------------------------------------------------------------------------
// T-104 favorites
// ---------------------------------------------------------------------------

#[tokio::test]
async fn favorites_are_capped_at_five_per_category() -> anyhow::Result<()> {
    let pool = pool().await?;
    let account = account(&pool).await?;
    let profile = profile_of(&pool, account).await?;
    for position in 0..5 {
        sqlx::query(
            "insert into profile_favorites(profile_id, category, catalog_item_id, position) \
             values($1,'music',$2,$3)",
        )
        .bind(profile)
        .bind(Uuid::new_v4())
        .bind(i16::try_from(position).expect("position"))
        .execute(&pool)
        .await?;
    }
    let error = sqlx::query(
        "insert into profile_favorites(profile_id, category, catalog_item_id, position) \
         values($1,'music',$2,4)",
    )
    .bind(profile)
    .bind(Uuid::new_v4())
    .execute(&pool)
    .await
    .expect_err("a sixth favorite must be refused");
    let fired = constraint(&error);
    assert!(
        matches!(
            fired.as_deref(),
            Some("profile_favorites_capacity") | Some("profile_favorites_slot")
        ),
        "a sixth favorite must hit the capacity or slot constraint, got {fired:?}"
    );
    // A different category still has room.
    sqlx::query(
        "insert into profile_favorites(profile_id, category, catalog_item_id, position) \
         values($1,'anime',$2,0)",
    )
    .bind(profile)
    .bind(Uuid::new_v4())
    .execute(&pool)
    .await?;
    Ok(())
}

#[tokio::test]
async fn a_catalog_item_appears_once_per_category() -> anyhow::Result<()> {
    let pool = pool().await?;
    let account = account(&pool).await?;
    let profile = profile_of(&pool, account).await?;
    let item = Uuid::new_v4();
    sqlx::query(
        "insert into profile_favorites(profile_id, category, catalog_item_id, position) \
         values($1,'game',$2,0)",
    )
    .bind(profile)
    .bind(item)
    .execute(&pool)
    .await?;
    let error = sqlx::query(
        "insert into profile_favorites(profile_id, category, catalog_item_id, position) \
         values($1,'game',$2,1)",
    )
    .bind(profile)
    .bind(item)
    .execute(&pool)
    .await
    .expect_err("a duplicate favorite must be refused");
    assert_eq!(constraint(&error).as_deref(), Some("profile_favorites_pkey"));
    Ok(())
}

#[tokio::test]
async fn favorites_reject_unknown_categories_and_positions() -> anyhow::Result<()> {
    let pool = pool().await?;
    let account = account(&pool).await?;
    let profile = profile_of(&pool, account).await?;
    let error = sqlx::query(
        "insert into profile_favorites(profile_id, category, catalog_item_id, position) \
         values($1,'kpop',$2,0)",
    )
    .bind(profile)
    .bind(Uuid::new_v4())
    .execute(&pool)
    .await
    .expect_err("an unknown category must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("profile_favorites_category")
    );
    let error = sqlx::query(
        "insert into profile_favorites(profile_id, category, catalog_item_id, position) \
         values($1,'music',$2,5)",
    )
    .bind(profile)
    .bind(Uuid::new_v4())
    .execute(&pool)
    .await
    .expect_err("position 5 must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("profile_favorites_position")
    );
    Ok(())
}

#[tokio::test]
async fn deleting_a_profile_cascades_its_favorites() -> anyhow::Result<()> {
    let pool = pool().await?;
    let account = account(&pool).await?;
    let profile = profile_of(&pool, account).await?;
    sqlx::query(
        "insert into profile_favorites(profile_id, category, catalog_item_id, position) \
         values($1,'music',$2,0)",
    )
    .bind(profile)
    .bind(Uuid::new_v4())
    .execute(&pool)
    .await?;
    sqlx::query("delete from profile_identities where profile_id=$1")
        .bind(profile)
        .execute(&pool)
        .await?;
    let remaining: i64 =
        sqlx::query_scalar("select count(*) from profile_favorites where profile_id=$1")
            .bind(profile)
            .fetch_one(&pool)
            .await?;
    assert_eq!(remaining, 0, "favorites must cascade with the profile");
    Ok(())
}

// ---------------------------------------------------------------------------
// T-105 identity claims
// ---------------------------------------------------------------------------

/// A syntactically valid claim row, parameterised by the fields under test.
/// Each call uses a fresh subject hash, because the same provider subject may
/// not back two users.
fn claim_sql(
    claim: Uuid,
    user: Uuid,
    status: &str,
    age: Option<i16>,
    next_birthday: Option<&str>,
    verified_at: &str,
    expires_at: &str,
) -> String {
    let subject = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    format!(
        "insert into identity_claims(claim_id,user_id,verifier,source_revision,subject_hash,\
         gender_bucket,age_at_verification,next_birthday_at,verified_at,expires_at,status) \
         values('{claim}','{user}','naver','rev-1',decode('{subject}','hex'),'female',{},{},'{}','{}','{}')",
        age.map(|value| value.to_string()).unwrap_or_else(|| "null".into()),
        next_birthday
            .map(|date| format!("'{date}'"))
            .unwrap_or_else(|| "null".into()),
        verified_at,
        expires_at,
        status,
    )
}

#[tokio::test]
async fn a_second_verified_claim_for_one_user_is_refused() -> anyhow::Result<()> {
    let pool = pool().await?;
    let user = Uuid::new_v4();
    // 2026-01-01T00:00:00Z is 2026-01-01 09:00 Seoul, so a birthday on
    // 2026-01-02 is strictly in the future.
    let first = claim_sql(
        Uuid::new_v4(),
        user,
        "VERIFIED",
        Some(20),
        Some("2026-01-02"),
        "2026-01-01T00:00:00Z",
        "2026-02-01T00:00:00Z",
    );
    sqlx::query(&first).execute(&pool).await?;
    let second = claim_sql(
        Uuid::new_v4(),
        user,
        "VERIFIED",
        Some(20),
        Some("2026-01-02"),
        "2026-01-01T00:00:00Z",
        "2026-02-01T00:00:00Z",
    );
    let error = sqlx::query(&second)
        .execute(&pool)
        .await
        .expect_err("a second VERIFIED claim must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("identity_claims_one_verified")
    );
    Ok(())
}

#[tokio::test]
async fn a_verified_claim_requires_its_attributes() -> anyhow::Result<()> {
    let pool = pool().await?;
    // VERIFIED without age_at_verification.
    let error = sqlx::query(&claim_sql(
        Uuid::new_v4(),
        Uuid::new_v4(),
        "VERIFIED",
        None,
        Some("2026-01-02"),
        "2026-01-01T00:00:00Z",
        "2026-02-01T00:00:00Z",
    ))
    .execute(&pool)
    .await
    .expect_err("VERIFIED without an age must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("identity_claims_verified_requires_attributes")
    );
    // VERIFIED without next_birthday_at.
    let error = sqlx::query(&claim_sql(
        Uuid::new_v4(),
        Uuid::new_v4(),
        "VERIFIED",
        Some(20),
        None,
        "2026-01-01T00:00:00Z",
        "2026-02-01T00:00:00Z",
    ))
    .execute(&pool)
    .await
    .expect_err("VERIFIED without a next birthday must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("identity_claims_verified_requires_attributes")
    );
    // The same two shapes are accepted when the attributes are present, so the
    // negative cases above are not passing because of an unrelated refusal.
    sqlx::query(&claim_sql(
        Uuid::new_v4(),
        Uuid::new_v4(),
        "VERIFIED",
        Some(20),
        Some("2026-01-02"),
        "2026-01-01T00:00:00Z",
        "2026-02-01T00:00:00Z",
    ))
    .execute(&pool)
    .await?;
    Ok(())
}

#[tokio::test]
async fn claim_lifetime_is_capped_at_ninety_days() -> anyhow::Result<()> {
    let pool = pool().await?;
    // 91 days.
    let error = sqlx::query(&claim_sql(
        Uuid::new_v4(),
        Uuid::new_v4(),
        "VERIFIED",
        Some(20),
        Some("2026-01-02"),
        "2026-01-01T00:00:00Z",
        "2026-04-02T00:00:01Z",
    ))
    .execute(&pool)
    .await
    .expect_err("a 91-day claim must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("identity_claims_lifetime")
    );
    // An expiry at or before verification.
    let error = sqlx::query(&claim_sql(
        Uuid::new_v4(),
        Uuid::new_v4(),
        "VERIFIED",
        Some(20),
        Some("2026-01-02"),
        "2026-01-01T00:00:00Z",
        "2026-01-01T00:00:00Z",
    ))
    .execute(&pool)
    .await
    .expect_err("a non-positive lifetime must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("identity_claims_lifetime")
    );
    // Exactly 90 days is allowed.
    sqlx::query(&claim_sql(
        Uuid::new_v4(),
        Uuid::new_v4(),
        "VERIFIED",
        Some(20),
        Some("2026-01-02"),
        "2026-01-01T00:00:00Z",
        "2026-04-01T00:00:00Z",
    ))
    .execute(&pool)
    .await?;
    Ok(())
}

#[tokio::test]
async fn next_birthday_must_be_after_the_seoul_verification_date() -> anyhow::Result<()> {
    let pool = pool().await?;
    // 2026-01-01T00:00:00Z is 2026-01-01 09:00 in Seoul. A birthday on that same
    // Seoul date is not in the future.
    let error = sqlx::query(&claim_sql(
        Uuid::new_v4(),
        Uuid::new_v4(),
        "VERIFIED",
        Some(20),
        Some("2026-01-01"),
        "2026-01-01T00:00:00Z",
        "2026-02-01T00:00:00Z",
    ))
    .execute(&pool)
    .await
    .expect_err("a birthday on the verification date must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("identity_claims_next_birthday_future")
    );
    // The Seoul date can be a day ahead of the UTC date. 2026-01-01T16:00:00Z is
    // 2026-01-02 01:00 Seoul, so a birthday on 2026-01-02 is not in the future
    // even though it is after the UTC date.
    let error = sqlx::query(&claim_sql(
        Uuid::new_v4(),
        Uuid::new_v4(),
        "VERIFIED",
        Some(20),
        Some("2026-01-02"),
        "2026-01-01T16:00:00Z",
        "2026-02-01T16:00:00Z",
    ))
    .execute(&pool)
    .await
    .expect_err("the Seoul date, not the UTC date, decides the future check");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("identity_claims_next_birthday_future")
    );
    Ok(())
}

#[tokio::test]
async fn claim_enums_and_shapes_are_closed() -> anyhow::Result<()> {
    let pool = pool().await?;
    // Unknown gender bucket.
    let error = sqlx::query(
        "insert into identity_claims(claim_id,user_id,verifier,source_revision,subject_hash,\
         gender_bucket,verified_at,expires_at,status) \
         values($1,$2,'naver','rev-1',decode(repeat('ab',32),'hex'),'other',\
         '2026-01-01T00:00:00Z','2026-02-01T00:00:00Z','REVOKED')",
    )
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .execute(&pool)
    .await
    .expect_err("an unknown gender bucket must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("identity_claims_gender_bucket")
    );
    // Unknown status.
    let error = sqlx::query(
        "insert into identity_claims(claim_id,user_id,verifier,source_revision,subject_hash,\
         gender_bucket,verified_at,expires_at,status) \
         values($1,$2,'naver','rev-1',decode(repeat('ab',32),'hex'),'female',\
         '2026-01-01T00:00:00Z','2026-02-01T00:00:00Z','EXPIRED')",
    )
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .execute(&pool)
    .await
    .expect_err("an unknown status must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("identity_claims_status")
    );
    // A subject hash of the wrong length.
    let error = sqlx::query(
        "insert into identity_claims(claim_id,user_id,verifier,source_revision,subject_hash,\
         gender_bucket,verified_at,expires_at,status) \
         values($1,$2,'naver','rev-1',decode(repeat('ab',16),'hex'),'female',\
         '2026-01-01T00:00:00Z','2026-02-01T00:00:00Z','REVOKED')",
    )
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .execute(&pool)
    .await
    .expect_err("a short subject hash must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("identity_claims_subject_hash_length")
    );
    Ok(())
}

#[tokio::test]
async fn identity_tables_have_no_event_column() -> anyhow::Result<()> {
    let pool = pool().await?;
    // The donor claim store was event-scoped. A personal profile must not
    // inherit that, so no identity table may carry an event column.
    let columns: Vec<String> = sqlx::query_scalar(
        "select table_name || '.' || column_name from information_schema.columns \
         where table_schema='public' \
           and table_name in ('identity_claims','identity_verifications') \
           and column_name like '%event%'",
    )
    .fetch_all(&pool)
    .await?;
    assert!(
        columns.is_empty(),
        "identity tables must not carry an event column: {columns:?}"
    );
    Ok(())
}

#[tokio::test]
async fn identity_verification_expiry_is_ten_minutes() -> anyhow::Result<()> {
    let pool = pool().await?;
    let error = sqlx::query(
        "insert into identity_verifications(verification_id,user_id,verifier,source_revision,\
         state,state_digest,return_path,expires_at) \
         values($1,$2,'naver','rev-1','PENDING',decode(replace($3::text,'-','') || replace($4::text,'-',''),'hex'),'/me',\
         clock_timestamp() + interval '11 minutes')",
    )
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .execute(&pool)
    .await
    .expect_err("an 11-minute verification must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("identity_verifications_expiry")
    );
    sqlx::query(
        "insert into identity_verifications(verification_id,user_id,verifier,source_revision,\
         state,state_digest,return_path,expires_at) \
         values($1,$2,'naver','rev-1','PENDING',decode(replace($3::text,'-','') || replace($4::text,'-',''),'hex'),'/me',\
         clock_timestamp() + interval '10 minutes')",
    )
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .execute(&pool)
    .await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// T-111 account deletion coverage
// ---------------------------------------------------------------------------

#[tokio::test]
async fn account_deletion_erases_every_new_profile_row() -> anyhow::Result<()> {
    let pool = pool().await?;
    let account = account(&pool).await?;
    let profile = profile_of(&pool, account).await?;
    sqlx::query(
        "insert into profile_favorites(profile_id, category, catalog_item_id, position) \
         values($1,'music',$2,0)",
    )
    .bind(profile)
    .bind(Uuid::new_v4())
    .execute(&pool)
    .await?;
    sqlx::query(&claim_sql(
        Uuid::new_v4(),
        account,
        "VERIFIED",
        Some(20),
        Some("2026-01-02"),
        "2026-01-01T00:00:00Z",
        "2026-02-01T00:00:00Z",
    ))
    .execute(&pool)
    .await?;
    sqlx::query(
        "insert into identity_verifications(verification_id,user_id,verifier,source_revision,\
         state,state_digest,return_path,expires_at) \
         values($1,$2,'naver','rev-1','PENDING',decode(replace($3::text,'-','') || replace($4::text,'-',''),'hex'),'/me',\
         clock_timestamp() + interval '5 minutes')",
    )
    .bind(Uuid::new_v4())
    .bind(account)
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .execute(&pool)
    .await?;
    // The saga writes the tombstone first. The erase functions refuse a user
    // without one, so this ordering is enforced, not assumed.
    let untombstoned = sqlx::query("select profile_erase_identity_claims($1)")
        .bind(account)
        .execute(&pool)
        .await
        .expect_err("erasing without a tombstone must be refused");
    assert_eq!(
        constraint(&untombstoned).as_deref(),
        Some("profile_erase_requires_tombstone")
    );
    sqlx::query(
        "insert into profile_user_deletion_tombstones(user_id,saga_id,actor_id,retention_until) \
         values($1,$2,$3,clock_timestamp() + interval '30 days')",
    )
    .bind(account)
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .execute(&pool)
    .await?;
    // Children before parents.
    sqlx::query("select profile_erase_identity_claims($1)")
        .bind(account)
        .execute(&pool)
        .await?;
    sqlx::query("select profile_erase_favorites($1)")
        .bind(profile)
        .execute(&pool)
        .await?;
    sqlx::query("select profile_erase_identity($1)")
        .bind(profile)
        .execute(&pool)
        .await?;
    let remaining: i64 = sqlx::query_scalar(
        "select (select count(*) from identity_claims where user_id=$1) \
              + (select count(*) from identity_verifications where user_id=$1) \
              + (select count(*) from profile_identities where account_id=$1)",
    )
    .bind(account)
    .fetch_one(&pool)
    .await?;
    assert_eq!(remaining, 0, "every owned row must be erased");
    Ok(())
}

#[tokio::test]
async fn identity_erase_refuses_to_run_out_of_order() -> anyhow::Result<()> {
    let pool = pool().await?;
    let account = account(&pool).await?;
    let profile = profile_of(&pool, account).await?;
    sqlx::query(
        "insert into profile_favorites(profile_id, category, catalog_item_id, position) \
         values($1,'music',$2,0)",
    )
    .bind(profile)
    .bind(Uuid::new_v4())
    .execute(&pool)
    .await?;
    sqlx::query(
        "insert into profile_user_deletion_tombstones(user_id,saga_id,actor_id,retention_until) \
         values($1,$2,$3,clock_timestamp() + interval '30 days')",
    )
    .bind(account)
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .execute(&pool)
    .await?;
    // Deleting the profile before its favorites is refused, so the saga cannot
    // silently skip a step.
    let error = sqlx::query("select profile_erase_identity($1)")
        .bind(profile)
        .execute(&pool)
        .await
        .expect_err("erasing a profile that still owns favorites must be refused");
    assert_eq!(constraint(&error).as_deref(), Some("profile_erase_order"));
    Ok(())
}

#[tokio::test]
async fn deletion_progress_is_monotonic_and_terminal_only_when_clean() -> anyhow::Result<()> {
    let pool = pool().await?;
    let account = account(&pool).await?;
    sqlx::query(
        "insert into profile_user_deletion_tombstones(user_id,saga_id,actor_id,retention_until) \
         values($1,$2,$3,clock_timestamp() + interval '30 days')",
    )
    .bind(account)
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .execute(&pool)
    .await?;
    sqlx::query("insert into profile_account_deletion_progress(user_id,saga_id) values($1,$2)")
        .bind(account)
        .bind(Uuid::new_v4())
        .execute(&pool)
        .await?;
    // Completion while the account still owns a profile is refused.
    let error = sqlx::query(
        "update profile_account_deletion_progress set progress_revision=1, \
         completed_at=clock_timestamp() where user_id=$1",
    )
    .bind(account)
    .execute(&pool)
    .await
    .expect_err("completion with live rows must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("profile_account_deletion_progress_terminal")
    );
    // A non-consecutive revision is refused.
    let error = sqlx::query(
        "update profile_account_deletion_progress set progress_revision=5 where user_id=$1",
    )
    .bind(account)
    .execute(&pool)
    .await
    .expect_err("a skipped revision must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("profile_account_deletion_progress_monotonic")
    );
    Ok(())
}

/// A helper used by the profile-service migration ledger test.
pub async fn migration_versions(pool: &PgPool) -> anyhow::Result<Vec<String>> {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let mut names: Vec<String> = std::fs::read_dir(directory)?
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".sql") && !name.ends_with(".down.sql"))
        .collect();
    names.sort();
    Ok(names)
}

#[tokio::test]
async fn every_profile_migration_is_ledgered_in_order() -> anyhow::Result<()> {
    let pool = pool().await?;
    let names = migration_versions(&pool).await?;
    let numbers: Vec<u32> = names
        .iter()
        .filter_map(|name| name.split('_').next())
        .filter_map(|number| number.parse().ok())
        .collect();
    assert_eq!(numbers.len(), names.len(), "every migration has a number");
    let mut sorted = numbers.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(numbers, sorted, "migration numbers are unique and ordered");
    // The new migrations are strictly greater than the previous head, 0010.
    let head = *numbers.last().expect("at least one migration");
    assert!(head > 10, "new migrations must follow the previous head");
    let row = sqlx::query(
        "select count(*) as total from information_schema.tables \
         where table_schema='public' and table_name in \
         ('profile_identities','profile_favorites','identity_claims',\
          'identity_verifications','profile_product_scopes',\
          'profile_account_deletion_progress')",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(row.get::<i64, _>("total"), 6, "every new table exists");
    Ok(())
}

// ---------------------------------------------------------------------------
// 0018: the runtime identity and the gap backfill
// ---------------------------------------------------------------------------

/// The profile id the backfill derives for an account.
///
/// This is the expression migration 0011 and 0018 both use. A runtime insert
/// that derived the id any other way would give one account two profiles, so the
/// tests below compare against this rather than against whatever the code did.
async fn derived_profile_id(pool: &PgPool, account: Uuid) -> anyhow::Result<Uuid> {
    Ok(sqlx::query_scalar(
        "select md5('mcr profile identity v1:' || $1::text)::uuid",
    )
    .bind(account)
    .fetch_one(pool)
    .await?)
}

/// Saving a new profile creates its identity in the same transaction.
///
/// This is the defect 0018 repairs: before the runtime insert existed, an
/// account registered after migration 0011 got a `profiles` row and no
/// `profile_identities` row, so it could own no profile-owned table and the intro
/// audience decision denied every restricted post to it.
#[tokio::test]
async fn saving_a_new_profile_creates_its_identity_with_the_backfill_id() -> anyhow::Result<()> {
    use profile_application::dto::repository_dto::{ProfileRecordDto, ProfileVisibilityDto};
    use profile_application::ports::ProfileRepositoryPort;
    use profile_service::adapters::out::persistence::profile_repository::PgProfileRepository;

    let pool = pool().await?;
    let account = Uuid::new_v4();
    let repository = PgProfileRepository::new(pool.clone());
    repository
        .save(&ProfileRecordDto {
            user_id: account,
            username: format!("newcomer{}", account.simple()),
            nickname: "Newcomer".to_owned(),
            avatar_url: String::new(),
            bio: "Fresh account".to_owned(),
            follower_count: 0,
            post_count: 0,
            following_count: 0,
            account_visibility: ProfileVisibilityDto::Public,
            lifecycle_status: "ACTIVE".to_owned(),
        })
        .await?;
    // The identity exists...
    let created: Option<Uuid> =
        sqlx::query_scalar("select profile_id from profile_identities where account_id=$1")
            .bind(account)
            .fetch_optional(&pool)
            .await?;
    let created = created.expect("a new profile must have an identity");
    // ...and it is the id the backfill would have derived.
    assert_eq!(created, derived_profile_id(&pool, account).await?);
    Ok(())
}

/// A second save reuses the identity and never rewrites its id.
///
/// A profile edit must not move the id that posts, media, rooms and avatars
/// already reference.
#[tokio::test]
async fn a_second_save_keeps_the_same_identity() -> anyhow::Result<()> {
    use profile_application::dto::repository_dto::{ProfileRecordDto, ProfileVisibilityDto};
    use profile_application::ports::ProfileRepositoryPort;
    use profile_service::adapters::out::persistence::profile_repository::PgProfileRepository;

    let pool = pool().await?;
    let account = Uuid::new_v4();
    let repository = PgProfileRepository::new(pool.clone());
    let record = |nickname: &str| ProfileRecordDto {
        user_id: account,
        username: format!("editor{}", account.simple()),
        nickname: nickname.to_owned(),
        avatar_url: String::new(),
        bio: "Bio".to_owned(),
        follower_count: 0,
        post_count: 0,
        following_count: 0,
        account_visibility: ProfileVisibilityDto::Public,
        lifecycle_status: "ACTIVE".to_owned(),
    };
    repository.save(&record("First")).await?;
    let first: Uuid =
        sqlx::query_scalar("select profile_id from profile_identities where account_id=$1")
            .bind(account)
            .fetch_one(&pool)
            .await?;
    repository.save(&record("Second")).await?;
    let rows: Vec<Uuid> =
        sqlx::query_scalar("select profile_id from profile_identities where account_id=$1")
            .bind(account)
            .fetch_all(&pool)
            .await?;
    assert_eq!(rows, vec![first], "an edit must not create or move an identity");
    Ok(())
}

/// The 0018 backfill repairs accounts that have a profile and no identity.
#[tokio::test]
async fn the_gap_backfill_creates_only_the_missing_identities() -> anyhow::Result<()> {
    let pool = pool().await?;
    // One account that already has an identity, and one that is missing it.
    let with_identity = account(&pool).await?;
    let existing = profile_of(&pool, with_identity).await?;
    let missing = Uuid::new_v4();
    sqlx::query("insert into profiles(user_id,username,nickname,avatar_url,bio) values($1,$2,'Gap','a','b')")
        .bind(missing)
        .bind(format!("gap{}", missing.simple()))
        .execute(&pool)
        .await?;
    // Apply the migration's own SQL.
    let sql = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("migrations/0018_profile_identity_gap_backfill.sql"),
    )?;
    sqlx::raw_sql(&sql).execute(&pool).await?;
    // The gap is closed, with the derived id.
    let repaired: Option<Uuid> =
        sqlx::query_scalar("select profile_id from profile_identities where account_id=$1")
            .bind(missing)
            .fetch_optional(&pool)
            .await?;
    assert_eq!(repaired, Some(derived_profile_id(&pool, missing).await?));
    // The account that already had one kept exactly the id it had.
    assert_eq!(profile_of(&pool, with_identity).await?, existing);
    // Every profile row now has an identity.
    let orphans: i64 = sqlx::query_scalar(
        "select count(*) from profiles p where not exists ( \
             select 1 from profile_identities i where i.account_id = p.user_id)",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(orphans, 0, "the backfill must be total");
    Ok(())
}
