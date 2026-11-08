//! T-206: verification status read.
//!
//! Every case runs against the real `identity_claims` table. The interesting
//! assertions are the ones about *ordering* and *time*, because those are where
//! a claim read goes wrong: an older usable row must never be selected over a
//! newer expired one, and the current age must move on the Seoul calendar date
//! without any stored mutation.
use chrono::{Duration, TimeZone, Utc};
use profile_service::identity::status::{project, ReadVerification, VerificationState};
use profile_service::identity::{CallbackCommand, CallbackResult, CompleteVerification, StartVerification};

#[path = "wp04b_support.rs"]
mod support;
use support::{claims, clear_user, pool, repository, FakeProfile, FakeProvider};

fn at(value: &str) -> chrono::DateTime<Utc> {
    chrono::DateTime::parse_from_rfc3339(value)
        .expect("timestamp")
        .with_timezone(&Utc)
}

/// Verify a user with a given profile at a given instant.
async fn verify(
    pool: &sqlx::PgPool,
    provider: &FakeProvider,
    user: uuid::Uuid,
    now: chrono::DateTime<Utc>,
) -> anyhow::Result<CallbackResult> {
    let started = StartVerification::new(repository(pool.clone()), provider.config())
        .begin(user, Some("/me/identity"), now)
        .await
        .map_err(|error| anyhow::anyhow!("start: {error:?}"))?;
    let state = started
        .authorization_url
        .split("state=")
        .nth(1)
        .expect("state")
        .split('&')
        .next()
        .expect("state value")
        .to_owned();
    let outcome = CompleteVerification::new(repository(pool.clone()), provider.client()?)
        .complete(CallbackCommand {
            state,
            code: Some("fake-authorization-code".into()),
            provider_error: None,
            now,
        })
        .await
        .map_err(|error| anyhow::anyhow!("callback: {error:?}"))?;
    Ok(outcome.result)
}

/// V-02-b: no row reads as `NOT_VERIFIED` with `claims: null`.
#[tokio::test]
async fn no_row_reads_as_not_verified() -> anyhow::Result<()> {
    let pool = pool().await?;
    let user = support::user();
    clear_user(&pool, user).await?;
    let verification = ReadVerification::new(repository(pool.clone()))
        .read(user, Utc::now())
        .await;
    assert_eq!(verification.status, VerificationState::NotVerified);
    assert!(verification.claims.is_none());
    assert!(verification.verified_at.is_none());
    assert!(verification.expires_at.is_none());
    let json = verification.to_json();
    assert_eq!(json["status"], "NOT_VERIFIED");
    assert_eq!(json["claims"], serde_json::Value::Null);
    assert_eq!(json["provider"], "naver");
    assert_eq!(json["display_on_profile"], false);
    Ok(())
}

/// V-02-j: a live row reads as `VERIFIED` with the derived age and bucket.
#[tokio::test]
async fn a_live_row_reads_as_verified_with_the_derived_age() -> anyhow::Result<()> {
    let pool = pool().await?;
    let provider = FakeProvider::start(FakeProfile::woman().with_subject(&format!(
        "naver-subject-{}",
        uuid::Uuid::new_v4()
    )))
    .await?;
    let user = support::user();
    clear_user(&pool, user).await?;
    // 2026-07-12 in Seoul: a 2000-07-12 birth date is exactly 26.
    let verified_at = at("2026-07-12T03:00:00Z");
    assert_eq!(
        verify(&pool, &provider, user, verified_at).await?,
        CallbackResult::Success
    );
    let read = ReadVerification::new(repository(pool.clone()))
        .read(user, verified_at + Duration::days(1))
        .await;
    assert_eq!(read.status, VerificationState::Verified);
    let claims = read.claims.as_ref().expect("a live claim carries its attributes");
    assert_eq!(claims.age, 26);
    assert_eq!(claims.gender_bucket.as_str(), "female");
    assert_eq!(read.verified_at.as_deref(), Some("2026-07-12T03:00:00Z"));
    assert_eq!(read.expires_at.as_deref(), Some("2026-10-10T03:00:00Z"));
    clear_user(&pool, user).await?;
    Ok(())
}

/// V-12-b: the current age advances on the Seoul birthday with no stored write.
#[tokio::test]
async fn the_current_age_advances_on_the_seoul_birthday_without_a_stored_mutation() -> anyhow::Result<()> {
    let pool = pool().await?;
    // A 1999-07-12 birth date: 26 on 2026-07-11, 27 on 2026-07-12.
    let provider = FakeProvider::start(
        FakeProfile::woman()
            .with_birth(Some("1999"), Some("07-12"))
            .with_subject(&format!("naver-subject-{}", uuid::Uuid::new_v4())),
    )
    .await?;
    let user = support::user();
    clear_user(&pool, user).await?;
    // Verified the day before the 27th birthday.
    let verified_at = at("2026-07-11T03:00:00Z");
    assert_eq!(
        verify(&pool, &provider, user, verified_at).await?,
        CallbackResult::Success
    );
    let reader = ReadVerification::new(repository(pool.clone()));
    let before = reader.read(user, at("2026-07-11T12:00:00Z")).await;
    assert_eq!(before.claims.as_ref().expect("live").age, 26);
    // On the birthday itself, in Seoul, the age is 27.
    let on = reader.read(user, at("2026-07-12T00:00:00Z")).await;
    assert_eq!(on.claims.as_ref().expect("live").age, 27);
    // One second before Seoul midnight it is still 26: the rule is the Seoul
    // calendar date, not the UTC instant.
    let just_before = reader.read(user, at("2026-07-11T14:59:59Z")).await;
    assert_eq!(just_before.claims.as_ref().expect("live").age, 26);
    // The stored row is unchanged: no read mutated it.
    let rows = claims(&pool, user).await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].age_at_verification, Some(26));
    assert_eq!(rows[0].verified_at, verified_at);
    clear_user(&pool, user).await?;
    Ok(())
}

/// An expired row reads as `EXPIRED` with `claims: null`.
#[tokio::test]
async fn an_expired_row_reads_as_expired_with_no_claims() -> anyhow::Result<()> {
    let pool = pool().await?;
    let provider = FakeProvider::start(FakeProfile::woman().with_subject(&format!(
        "naver-subject-{}",
        uuid::Uuid::new_v4()
    )))
    .await?;
    let user = support::user();
    clear_user(&pool, user).await?;
    let verified_at = at("2026-07-12T03:00:00Z");
    assert_eq!(
        verify(&pool, &provider, user, verified_at).await?,
        CallbackResult::Success
    );
    // 91 days later the 90-day claim has expired.
    let read = ReadVerification::new(repository(pool.clone()))
        .read(user, verified_at + Duration::days(91))
        .await;
    assert_eq!(read.status, VerificationState::Expired);
    assert!(read.claims.is_none(), "an expired claim yields no attributes");
    assert_eq!(read.to_json()["claims"], serde_json::Value::Null);
    clear_user(&pool, user).await?;
    Ok(())
}

/// V-02-j: an older usable row is never selected over a newer expired one.
///
/// This is the donor's canonical ordering, and the case that proves it: a newer
/// expired claim must win over an older claim that is still inside its lifetime.
#[tokio::test]
async fn an_older_usable_row_is_never_selected_over_a_newer_expired_one() -> anyhow::Result<()> {
    let pool = pool().await?;
    let user = support::user();
    clear_user(&pool, user).await?;
    let older = at("2026-01-01T00:00:00Z");
    let newer = at("2026-06-01T00:00:00Z");
    // Two rows written directly: an older one that is still inside its lifetime
    // at the read instant, and a newer one that has expired. `REPLACED` is the
    // only status a second row may hold (the one-VERIFIED-per-user index), and
    // it is exactly the status a superseded claim carries.
    insert_claim(&pool, user, older, older + Duration::days(90), "female", "REPLACED").await?;
    insert_claim(&pool, user, newer, newer + Duration::days(1), "male", "VERIFIED").await?;
    let read_at = at("2026-06-15T00:00:00Z");
    let read = ReadVerification::new(repository(pool.clone()))
        .read(user, read_at)
        .await;
    assert_eq!(
        read.status,
        VerificationState::Expired,
        "the newer expired row must be canonical, not the older usable one"
    );
    assert!(read.claims.is_none());
    assert_eq!(read.verified_at.as_deref(), Some("2026-06-01T00:00:00Z"));
    clear_user(&pool, user).await?;
    Ok(())
}

/// The canonical ordering breaks a `verified_at` tie on `expires_at`.
#[tokio::test]
async fn a_verified_at_tie_is_broken_by_the_later_expiry() -> anyhow::Result<()> {
    let pool = pool().await?;
    let user = support::user();
    clear_user(&pool, user).await?;
    let verified_at = at("2026-03-01T00:00:00Z");
    // A superseded row and its replacement share `verified_at`; the surviving
    // claim holds the larger expiry, so it must win. The superseded row's expiry
    // is one microsecond after its own verification, which is the smallest value
    // `identity_claims_lifetime` admits.
    insert_claim(
        &pool,
        user,
        verified_at,
        verified_at + Duration::microseconds(1),
        "female",
        "REPLACED",
    )
    .await?;
    insert_claim(
        &pool,
        user,
        verified_at,
        verified_at + Duration::days(90),
        "male",
        "VERIFIED",
    )
    .await?;
    let read = ReadVerification::new(repository(pool.clone()))
        .read(user, verified_at + Duration::days(1))
        .await;
    assert_eq!(read.status, VerificationState::Verified);
    assert_eq!(read.claims.as_ref().expect("live").gender_bucket.as_str(), "male");
    clear_user(&pool, user).await?;
    Ok(())
}

/// A revoked row reads as `REVOKED` with no claims.
#[tokio::test]
async fn a_revoked_row_reads_as_revoked_with_no_claims() -> anyhow::Result<()> {
    let pool = pool().await?;
    let user = support::user();
    clear_user(&pool, user).await?;
    let verified_at = at("2026-03-01T00:00:00Z");
    insert_claim(
        &pool,
        user,
        verified_at,
        verified_at + Duration::microseconds(1),
        "female",
        "REVOKED",
    )
    .await?;
    let read = ReadVerification::new(repository(pool.clone()))
        .read(user, verified_at + Duration::days(1))
        .await;
    assert_eq!(read.status, VerificationState::Revoked);
    assert!(read.claims.is_none());
    clear_user(&pool, user).await?;
    Ok(())
}

/// A VERIFIED row whose current age left the supported range reads as EXPIRED.
///
/// The donor's `current_supported_claim_sql_at` evaluates the arithmetic before
/// validity for exactly this reason: a claim cannot become usable again.
#[tokio::test]
async fn a_claim_whose_current_age_left_the_range_reads_as_expired() -> anyhow::Result<()> {
    let pool = pool().await?;
    let user = support::user();
    clear_user(&pool, user).await?;
    // A 100-year-old whose birthday falls inside the claim's lifetime would be
    // 101 after it, which is outside the supported range.
    let verified_at = at("2026-03-01T00:00:00Z");
    let next_birthday = chrono::NaiveDate::from_ymd_opt(2026, 3, 20).expect("date");
    sqlx::query(
        "insert into identity_claims \
         (claim_id,user_id,verifier,source_revision,subject_hash,gender_bucket,age_at_verification,\
          next_birthday_at,verified_at,expires_at,status) \
         values ($1,$2,'naver','naver.nid.me.v1',$3,'female',100,$4,$5,$6,'VERIFIED')",
    )
    .bind(uuid::Uuid::new_v4())
    .bind(user)
    .bind(sha2_hash(&format!("subject-{}", uuid::Uuid::new_v4())))
    .bind(next_birthday)
    .bind(verified_at)
    .bind(verified_at + Duration::days(90))
    .execute(&pool)
    .await?;
    let reader = ReadVerification::new(repository(pool.clone()));
    // Before the birthday the claim is usable.
    let before = reader.read(user, at("2026-03-10T00:00:00Z")).await;
    assert_eq!(before.status, VerificationState::Verified);
    // After it the current age is 101, outside 14..=100, so it is not usable.
    let after = reader.read(user, at("2026-03-21T00:00:00Z")).await;
    assert_eq!(after.status, VerificationState::Expired);
    assert!(after.claims.is_none());
    clear_user(&pool, user).await?;
    Ok(())
}

/// The projection is pure: a row and an instant decide the DTO.
#[tokio::test]
async fn the_projection_is_pure_over_a_row_and_an_instant() -> anyhow::Result<()> {
    let pool = pool().await?;
    let user = support::user();
    clear_user(&pool, user).await?;
    let verified_at = at("2026-03-01T00:00:00Z");
    insert_claim(
        &pool,
        user,
        verified_at,
        verified_at + Duration::days(90),
        "unknown",
        "VERIFIED",
    )
    .await?;
    let row = repository(pool.clone())
        .canonical_claim(user)
        .await?
        .expect("a row");
    // The same row projects identically at the same instant.
    let first = project(Some(row), verified_at + Duration::days(1));
    let second = project(
        repository(pool.clone()).canonical_claim(user).await?,
        verified_at + Duration::days(1),
    );
    assert_eq!(first.status, second.status);
    assert_eq!(first.to_json(), second.to_json());
    // And the `unknown` bucket is reported as-is, not widened.
    assert_eq!(
        first.claims.as_ref().expect("live").gender_bucket.as_str(),
        "unknown"
    );
    clear_user(&pool, user).await?;
    Ok(())
}

/// The DTO's `Debug` renders no age.
#[tokio::test]
async fn the_claims_debug_renders_no_age() -> anyhow::Result<()> {
    let pool = pool().await?;
    let user = support::user();
    clear_user(&pool, user).await?;
    let verified_at = at("2026-03-01T00:00:00Z");
    insert_claim(
        &pool,
        user,
        verified_at,
        verified_at + Duration::days(90),
        "female",
        "VERIFIED",
    )
    .await?;
    let read = ReadVerification::new(repository(pool.clone()))
        .read(user, verified_at + Duration::days(1))
        .await;
    let rendered = format!("{:?}", read.claims.as_ref().expect("live"));
    assert!(!rendered.contains("26"), "{rendered}");
    assert!(rendered.contains("[REDACTED]"), "{rendered}");
    clear_user(&pool, user).await?;
    Ok(())
}

/// Insert one claim row directly, for the ordering cases.
async fn insert_claim(
    pool: &sqlx::PgPool,
    user: uuid::Uuid,
    verified_at: chrono::DateTime<Utc>,
    expires_at: chrono::DateTime<Utc>,
    gender_bucket: &str,
    status: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "insert into identity_claims \
         (claim_id,user_id,verifier,source_revision,subject_hash,gender_bucket,age_at_verification,\
          next_birthday_at,verified_at,expires_at,status) \
         values ($1,$2,'naver','naver.nid.me.v1',$3,$4,26,$5,$6,$7,$8)",
    )
    .bind(uuid::Uuid::new_v4())
    .bind(user)
    .bind(sha2_hash(&format!("subject-{}", uuid::Uuid::new_v4())))
    .bind(gender_bucket)
    .bind(chrono::NaiveDate::from_ymd_opt(2027, 1, 1).expect("date"))
    .bind(verified_at)
    .bind(expires_at)
    .bind(status)
    .execute(pool)
    .await?;
    Ok(())
}

fn sha2_hash(value: &str) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    hasher.finalize().to_vec()
}

/// The Seoul date conversion is a fixed nine-hour offset with no DST.
#[test]
fn the_seoul_date_is_a_fixed_offset() {
    let midnight_utc = Utc.with_ymd_and_hms(2026, 7, 11, 15, 0, 0).unwrap();
    // 15:00 UTC is 00:00 the next day in Seoul.
    assert_eq!(
        profile_service::identity::repository::seoul_date(midnight_utc),
        chrono::NaiveDate::from_ymd_opt(2026, 7, 12).expect("date")
    );
    assert_eq!(
        profile_service::identity::repository::seoul_date(midnight_utc - Duration::seconds(1)),
        chrono::NaiveDate::from_ymd_opt(2026, 7, 11).expect("date")
    );
}
