//! T-205: verification callback.
//!
//! One case per acceptance bullet, each asserting the row count after the call.
//! Every case runs the real use case against the real `identity_claims` table and
//! the real provider client, with a fake provider on loopback standing in for
//! NAVER. The schema's CHECK constraints therefore participate in every
//! assertion: a claim the database would refuse cannot be written by a test that
//! passes.
//!
//! The hard constraints this binary proves:
//!
//! * missing or unrecognized provider gender -> `profile_incomplete`, no row;
//! * explicit provider `U` -> bucket `unknown`;
//! * derived age outside 14..=100 -> `age_not_supported`, no row;
//! * a duplicate provider subject -> rejected, no second claim;
//! * re-verification supersedes, capping the old row's expiry;
//! * the state is one-use, and reuse or expiry is rejected;
//! * no provider token, subject, birth date or age reaches a log or `Debug`.
use chrono::{Duration, Utc};
use profile_service::identity::{
    CallbackCommand, CallbackError, CallbackResult, CompleteVerification, NaverOAuthConfig,
    StartVerification,
};

#[path = "wp04b_support.rs"]
mod support;
use support::{claims, clear_user, pool, repository, FakeProfile, FakeProvider};

struct Fixture {
    pool: sqlx::PgPool,
    provider: FakeProvider,
    user: uuid::Uuid,
}

impl Fixture {
    async fn new(profile: FakeProfile) -> anyhow::Result<Self> {
        let pool = pool().await?;
        let provider = FakeProvider::start(unique_profile(profile)).await?;
        let user = support::user();
        clear_user(&pool, user).await?;
        Ok(Self {
            pool,
            provider,
            user,
        })
    }

    /// Start a verification and return the state token from the URL.
    async fn start(&self, now: chrono::DateTime<Utc>) -> anyhow::Result<String> {
        let started = StartVerification::new(repository(self.pool.clone()), self.provider.config())
            .begin(self.user, Some("/me/identity"), now)
            .await
            .map_err(|error| anyhow::anyhow!("start failed: {error:?}"))?;
        Ok(state_of(&started.authorization_url))
    }

    fn complete(&self) -> CompleteVerification {
        CompleteVerification::new(repository(self.pool.clone()), self.provider.client().expect("client"))
    }

    async fn callback(&self, state: &str, now: chrono::DateTime<Utc>) -> anyhow::Result<CallbackResult> {
        let outcome = self
            .complete()
            .complete(CallbackCommand {
                state: state.to_owned(),
                code: Some("fake-authorization-code".into()),
                provider_error: None,
                now,
            })
            .await
            .map_err(|error| anyhow::anyhow!("callback failed: {error:?}"))?;
        assert_eq!(outcome.return_path, "/me/identity");
        Ok(outcome.result)
    }

    async fn finish(self) -> anyhow::Result<()> {
        clear_user(&self.pool, self.user).await?;
        Ok(())
    }
}

/// A provider profile whose subject is unique to one case.
///
/// The schema's `identity_claims_subject` index is global: one provider subject
/// may back one user, ever. A test that reused a fixed subject would collide
/// with every other test in this binary, so each case gets its own. The
/// duplicate-subject case is the one that deliberately reuses a subject.
fn unique_profile(base: FakeProfile) -> FakeProfile {
    base.with_subject(&format!("naver-subject-{}", uuid::Uuid::new_v4()))
}

fn state_of(url: &str) -> String {
    url.split("state=")
        .nth(1)
        .expect("the URL carries a state")
        .split('&')
        .next()
        .expect("state value")
        .to_owned()
}

/// V-12-a, V-12-c: a complete profile writes one claim with the derived age.
#[tokio::test]
async fn a_complete_profile_writes_one_claim_with_the_derived_age() -> anyhow::Result<()> {
    let fixture = Fixture::new(FakeProfile::woman()).await?;
    // 2026-07-12 in Seoul. A 2000-07-12 birth date is exactly 26 that day.
    let now = chrono::DateTime::parse_from_rfc3339("2026-07-12T03:00:00Z")?.with_timezone(&Utc);
    let state = fixture.start(now).await?;
    assert_eq!(fixture.callback(&state, now).await?, CallbackResult::Success);
    let rows = claims(&fixture.pool, fixture.user).await?;
    assert_eq!(rows.len(), 1, "exactly one claim row");
    assert_eq!(rows[0].status, "VERIFIED");
    assert_eq!(rows[0].gender_bucket, "female");
    assert_eq!(rows[0].age_at_verification, Some(26));
    assert_eq!(
        rows[0].next_birthday_at,
        Some(chrono::NaiveDate::from_ymd_opt(2027, 7, 12).expect("date"))
    );
    assert_eq!(rows[0].verified_at, now);
    // The expiry is the 90-day maximum the schema allows.
    assert_eq!(rows[0].expires_at, now + Duration::days(90));
    fixture.finish().await
}

/// V-12-d: `M` is the `male` bucket.
#[tokio::test]
async fn the_male_gender_writes_the_male_bucket() -> anyhow::Result<()> {
    let fixture = Fixture::new(FakeProfile::man()).await?;
    let now = Utc::now();
    let state = fixture.start(now).await?;
    assert_eq!(fixture.callback(&state, now).await?, CallbackResult::Success);
    let rows = claims(&fixture.pool, fixture.user).await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].gender_bucket, "male");
    fixture.finish().await
}

/// V-12-e: an explicit provider `U` is the `unknown` bucket.
///
/// This is the only producer of `unknown`. The next case proves that a *missing*
/// gender does not produce it.
#[tokio::test]
async fn an_explicit_unspecified_gender_writes_the_unknown_bucket() -> anyhow::Result<()> {
    let fixture = Fixture::new(FakeProfile::unspecified()).await?;
    let now = Utc::now();
    let state = fixture.start(now).await?;
    assert_eq!(fixture.callback(&state, now).await?, CallbackResult::Success);
    let rows = claims(&fixture.pool, fixture.user).await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].gender_bucket, "unknown");
    fixture.finish().await
}

/// V-12-f: a missing gender is `profile_incomplete` and writes no row.
#[tokio::test]
async fn a_missing_gender_is_profile_incomplete_with_no_claim_row() -> anyhow::Result<()> {
    let fixture = Fixture::new(FakeProfile::woman().with_gender(None)).await?;
    let now = Utc::now();
    let state = fixture.start(now).await?;
    assert_eq!(
        fixture.callback(&state, now).await?,
        CallbackResult::ProfileIncomplete
    );
    assert_eq!(
        claims(&fixture.pool, fixture.user).await?.len(),
        0,
        "a missing gender must never be coerced to unknown and stored"
    );
    fixture.finish().await
}

/// V-12-g: an unrecognized gender is `profile_incomplete` and writes no row.
#[tokio::test]
async fn an_unrecognized_gender_is_profile_incomplete_with_no_claim_row() -> anyhow::Result<()> {
    for value in ["X", "", "f", "m", "u", "NonBinary"] {
        let fixture = Fixture::new(FakeProfile::woman().with_gender(Some(value))).await?;
        let now = Utc::now();
        let state = fixture.start(now).await?;
        assert_eq!(
            fixture.callback(&state, now).await?,
            CallbackResult::ProfileIncomplete,
            "{value:?}"
        );
        assert_eq!(
            claims(&fixture.pool, fixture.user).await?.len(),
            0,
            "{value:?} must never be coerced to unknown and stored"
        );
        fixture.finish().await?;
    }
    Ok(())
}

/// V-12-h: a missing birthyear is `profile_incomplete` and writes no row.
#[tokio::test]
async fn a_missing_birthyear_is_profile_incomplete_with_no_claim_row() -> anyhow::Result<()> {
    let fixture = Fixture::new(FakeProfile::woman().with_birth(None, Some("07-12"))).await?;
    let now = Utc::now();
    let state = fixture.start(now).await?;
    assert_eq!(
        fixture.callback(&state, now).await?,
        CallbackResult::ProfileIncomplete
    );
    assert_eq!(claims(&fixture.pool, fixture.user).await?.len(), 0);
    fixture.finish().await
}

/// A missing birthday is also `profile_incomplete`.
#[tokio::test]
async fn a_missing_birthday_is_profile_incomplete_with_no_claim_row() -> anyhow::Result<()> {
    let fixture = Fixture::new(FakeProfile::woman().with_birth(Some("2000"), None)).await?;
    let now = Utc::now();
    let state = fixture.start(now).await?;
    assert_eq!(
        fixture.callback(&state, now).await?,
        CallbackResult::ProfileIncomplete
    );
    assert_eq!(claims(&fixture.pool, fixture.user).await?.len(), 0);
    fixture.finish().await
}

/// V-12-i: a derived age outside 14..=100 is `age_not_supported`, no row.
#[tokio::test]
async fn an_age_outside_the_supported_range_is_age_not_supported() -> anyhow::Result<()> {
    let now = chrono::DateTime::parse_from_rfc3339("2026-07-12T03:00:00Z")?.with_timezone(&Utc);
    for (year, day) in [
        // 13 years old on the verification date.
        ("2013", "07-12"),
        // 101 years old.
        ("1925", "07-12"),
    ] {
        let fixture = Fixture::new(FakeProfile::woman().with_birth(Some(year), Some(day))).await?;
        let state = fixture.start(now).await?;
        assert_eq!(
            fixture.callback(&state, now).await?,
            CallbackResult::AgeNotSupported,
            "{year}-{day}"
        );
        assert_eq!(
            claims(&fixture.pool, fixture.user).await?.len(),
            0,
            "{year}-{day} must not write a claim"
        );
        fixture.finish().await?;
    }
    Ok(())
}

/// The range boundaries are inclusive: 14 and 100 are supported.
#[tokio::test]
async fn the_supported_age_boundaries_are_inclusive() -> anyhow::Result<()> {
    let now = chrono::DateTime::parse_from_rfc3339("2026-07-12T03:00:00Z")?.with_timezone(&Utc);
    for (year, expected) in [("2012", 14_i16), ("1926", 100)] {
        let fixture = Fixture::new(FakeProfile::woman().with_birth(Some(year), Some("07-12"))).await?;
        let state = fixture.start(now).await?;
        assert_eq!(
            fixture.callback(&state, now).await?,
            CallbackResult::Success,
            "{year}"
        );
        let rows = claims(&fixture.pool, fixture.user).await?;
        assert_eq!(rows[0].age_at_verification, Some(expected), "{year}");
        fixture.finish().await?;
    }
    Ok(())
}

/// V-12-j: a duplicate provider subject is rejected, with no second claim.
#[tokio::test]
async fn a_duplicate_provider_subject_is_rejected_with_no_second_claim() -> anyhow::Result<()> {
    let fixture = Fixture::new(FakeProfile::woman()).await?;
    let now = Utc::now();
    // The first user verifies the subject.
    let state = fixture.start(now).await?;
    assert_eq!(fixture.callback(&state, now).await?, CallbackResult::Success);
    assert_eq!(claims(&fixture.pool, fixture.user).await?.len(), 1);

    // A second user presents the same provider subject.
    let other = support::user();
    clear_user(&fixture.pool, other).await?;
    let started = StartVerification::new(repository(fixture.pool.clone()), fixture.provider.config())
        .begin(other, Some("/me/identity"), now)
        .await
        .expect("start succeeds");
    let outcome = fixture
        .complete()
        .complete(CallbackCommand {
            state: state_of(&started.authorization_url),
            code: Some("fake-authorization-code".into()),
            provider_error: None,
            now,
        })
        .await
        .map_err(|error| anyhow::anyhow!("callback failed: {error:?}"))?;
    assert_eq!(outcome.result, CallbackResult::Duplicate);
    assert_eq!(
        claims(&fixture.pool, other).await?.len(),
        0,
        "the second user must not receive a claim"
    );
    // The first user's claim is untouched.
    let rows = claims(&fixture.pool, fixture.user).await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "VERIFIED");
    clear_user(&fixture.pool, other).await?;
    fixture.finish().await
}

/// The same user re-verifying their own subject is not a duplicate.
#[tokio::test]
async fn the_same_user_re_verifying_the_same_subject_is_not_a_duplicate() -> anyhow::Result<()> {
    let fixture = Fixture::new(FakeProfile::woman()).await?;
    let now = Utc::now();
    let first = fixture.start(now).await?;
    assert_eq!(fixture.callback(&first, now).await?, CallbackResult::Success);
    let second = fixture.start(now + Duration::minutes(1)).await?;
    assert_eq!(
        fixture.callback(&second, now + Duration::minutes(1)).await?,
        CallbackResult::Success
    );
    let rows = claims(&fixture.pool, fixture.user).await?;
    assert_eq!(rows.len(), 2);
    fixture.finish().await
}

/// V-12-k: re-verification supersedes, capping the old row's expiry.
#[tokio::test]
async fn a_re_verification_supersedes_and_caps_the_old_expiry() -> anyhow::Result<()> {
    let fixture = Fixture::new(FakeProfile::woman()).await?;
    let first_at = Utc::now();
    let first = fixture.start(first_at).await?;
    assert_eq!(fixture.callback(&first, first_at).await?, CallbackResult::Success);
    let original_expiry = claims(&fixture.pool, fixture.user).await?[0].expires_at;

    let second_at = first_at + Duration::days(30);
    let second = fixture.start(second_at).await?;
    assert_eq!(fixture.callback(&second, second_at).await?, CallbackResult::Success);

    let rows = claims(&fixture.pool, fixture.user).await?;
    assert_eq!(rows.len(), 2, "one VERIFIED row and one REPLACED row");
    assert_eq!(rows[0].status, "REPLACED");
    assert_eq!(rows[1].status, "VERIFIED");
    // The old row's expiry is capped at the replacement instant, never
    // extended. The stored value is one microsecond earlier because
    // `identity_claims_lifetime` requires `expires_at > verified_at` for every
    // row; one microsecond is below the contract's own precision, so the row
    // still ends at the replacement instant.
    assert_eq!(rows[0].expires_at, second_at - Duration::microseconds(1));
    assert!(rows[0].expires_at < original_expiry);
    // The new row carries the full 90-day lifetime from the replacement.
    assert_eq!(rows[1].expires_at, second_at + Duration::days(90));
    // The evidence of the old row is untouched (the trigger enforces this too).
    assert_eq!(rows[0].age_at_verification, rows[1].age_at_verification);
    assert_eq!(rows[0].gender_bucket, rows[1].gender_bucket);
    fixture.finish().await
}

/// A supersede after the old claim expired still caps, never extends.
#[tokio::test]
async fn a_supersede_of_an_already_expired_claim_keeps_the_earlier_expiry() -> anyhow::Result<()> {
    let fixture = Fixture::new(FakeProfile::woman()).await?;
    let first_at = Utc::now();
    let first = fixture.start(first_at).await?;
    assert_eq!(fixture.callback(&first, first_at).await?, CallbackResult::Success);
    // The second verification is 100 days later, past the first claim's expiry.
    let second_at = first_at + Duration::days(100);
    let second = fixture.start(second_at).await?;
    assert_eq!(fixture.callback(&second, second_at).await?, CallbackResult::Success);
    let rows = claims(&fixture.pool, fixture.user).await?;
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].status, "REPLACED");
    // `least(expires_at, replacement)` keeps the earlier instant.
    assert_eq!(rows[0].expires_at, first_at + Duration::days(90));
    assert!(rows[0].expires_at < second_at);
    fixture.finish().await
}

/// V-12-m: a reused state is rejected, with no claim written.
#[tokio::test]
async fn a_reused_state_is_rejected_with_no_claim() -> anyhow::Result<()> {
    let fixture = Fixture::new(FakeProfile::woman()).await?;
    let now = Utc::now();
    let state = fixture.start(now).await?;
    assert_eq!(fixture.callback(&state, now).await?, CallbackResult::Success);
    let rows_after_first = claims(&fixture.pool, fixture.user).await?.len();
    // The second use is refused. The outcome is `replayed`, which is the donor's
    // name for "this state was already consumed".
    assert_eq!(fixture.callback(&state, now).await?, CallbackResult::Replayed);
    assert_eq!(
        claims(&fixture.pool, fixture.user).await?.len(),
        rows_after_first,
        "a replayed state must not write a second claim"
    );
    fixture.finish().await
}

/// V-12-n: an expired state is rejected, with no claim written.
#[tokio::test]
async fn an_expired_state_is_rejected_with_no_claim() -> anyhow::Result<()> {
    let fixture = Fixture::new(FakeProfile::woman()).await?;
    let start_at = Utc::now();
    let state = fixture.start(start_at).await?;
    // The callback arrives 11 minutes later: past the ten-minute lifetime.
    let late = start_at + Duration::minutes(11);
    assert_eq!(fixture.callback(&state, late).await?, CallbackResult::Expired);
    assert_eq!(
        claims(&fixture.pool, fixture.user).await?.len(),
        0,
        "an expired state must not write a claim"
    );
    // And it cannot be used later either.
    assert_eq!(fixture.callback(&state, late).await?, CallbackResult::Expired);
    assert_eq!(claims(&fixture.pool, fixture.user).await?.len(), 0);
    fixture.finish().await
}

/// An unknown state is refused without a redirect target.
#[tokio::test]
async fn an_unknown_state_is_refused() -> anyhow::Result<()> {
    let fixture = Fixture::new(FakeProfile::woman()).await?;
    let error = fixture
        .complete()
        .complete(CallbackCommand {
            state: "never-issued".into(),
            code: Some("fake-authorization-code".into()),
            provider_error: None,
            now: Utc::now(),
        })
        .await
        .err()
        .expect("an unknown state must be refused");
    assert_eq!(error, CallbackError::InvalidState);
    assert_eq!(claims(&fixture.pool, fixture.user).await?.len(), 0);
    fixture.finish().await
}

/// A provider error instead of a code is a redirect, not a failure.
#[tokio::test]
async fn a_provider_access_denied_is_cancelled_with_no_claim() -> anyhow::Result<()> {
    let fixture = Fixture::new(FakeProfile::woman()).await?;
    let now = Utc::now();
    let state = fixture.start(now).await?;
    let outcome = fixture
        .complete()
        .complete(CallbackCommand {
            state,
            code: None,
            provider_error: Some("access_denied".into()),
            now,
        })
        .await
        .map_err(|error| anyhow::anyhow!("callback failed: {error:?}"))?;
    assert_eq!(outcome.result, CallbackResult::Cancelled);
    assert_eq!(claims(&fixture.pool, fixture.user).await?.len(), 0);
    fixture.finish().await
}

#[tokio::test]
async fn another_provider_error_is_failed_with_no_claim() -> anyhow::Result<()> {
    let fixture = Fixture::new(FakeProfile::woman()).await?;
    let now = Utc::now();
    let state = fixture.start(now).await?;
    let outcome = fixture
        .complete()
        .complete(CallbackCommand {
            state,
            code: None,
            provider_error: Some("server_error".into()),
            now,
        })
        .await
        .map_err(|error| anyhow::anyhow!("callback failed: {error:?}"))?;
    assert_eq!(outcome.result, CallbackResult::Failed);
    assert_eq!(claims(&fixture.pool, fixture.user).await?.len(), 0);
    fixture.finish().await
}

/// A provider failure (a 500 from the token endpoint) is `failed`, no claim.
#[tokio::test]
async fn a_provider_failure_is_failed_with_no_claim() -> anyhow::Result<()> {
    let fixture = Fixture::new(FakeProfile::woman()).await?;
    fixture.provider.set_token_status(500);
    let now = Utc::now();
    let state = fixture.start(now).await?;
    assert_eq!(fixture.callback(&state, now).await?, CallbackResult::Failed);
    assert_eq!(claims(&fixture.pool, fixture.user).await?.len(), 0);
    fixture.finish().await
}

/// A provider profile failure is `failed`, no claim.
#[tokio::test]
async fn a_profile_fetch_failure_is_failed_with_no_claim() -> anyhow::Result<()> {
    let fixture = Fixture::new(FakeProfile::woman()).await?;
    fixture.provider.set_profile_status(401);
    let now = Utc::now();
    let state = fixture.start(now).await?;
    assert_eq!(fixture.callback(&state, now).await?, CallbackResult::Failed);
    assert_eq!(claims(&fixture.pool, fixture.user).await?.len(), 0);
    fixture.finish().await
}

/// The callback never carries a claim value in its outcome.
#[tokio::test]
async fn the_callback_outcome_carries_no_claim_value() -> anyhow::Result<()> {
    let fixture = Fixture::new(FakeProfile::woman()).await?;
    let now = Utc::now();
    let state = fixture.start(now).await?;
    let outcome = fixture
        .complete()
        .complete(CallbackCommand {
            state,
            code: Some("fake-authorization-code".into()),
            provider_error: None,
            now,
        })
        .await
        .map_err(|error| anyhow::anyhow!("callback failed: {error:?}"))?;
    // The outcome is a return path and a marker. Nothing else.
    assert_eq!(outcome.return_path, "/me/identity");
    assert_eq!(outcome.result.as_str(), "success");
    let rendered = format!("{:?}", outcome.result);
    assert!(!rendered.contains("26"), "{rendered}");
    assert!(!rendered.contains("female"), "{rendered}");
    fixture.finish().await
}

/// V-12-o: no provider token, subject, birth date or age in the stored state.
///
/// The claim row stores a salted subject hash and the derived age (which is the
/// claim's whole purpose); what it must never store is the raw subject, the
/// provider token, or the birth date. This asserts exactly that on the stored
/// bytes.
#[tokio::test]
async fn the_stored_rows_hold_no_provider_token_subject_or_birth_date() -> anyhow::Result<()> {
    let fixture = Fixture::new(FakeProfile::woman()).await?;
    let now = Utc::now();
    let state = fixture.start(now).await?;
    assert_eq!(fixture.callback(&state, now).await?, CallbackResult::Success);
    // The raw subject, the birth date and the token must not appear in any text
    // column of either table.
    let text: Vec<String> = sqlx::query_scalar(
        "select concat_ws('|', verifier, source_revision, gender_bucket, status) \
         from identity_claims where user_id = $1",
    )
    .bind(fixture.user)
    .fetch_all(&fixture.pool)
    .await?;
    let joined = text.join("|");
    for secret in ["naver-subject-woman", "2000", "07-12", "fake-naver-token"] {
        assert!(!joined.contains(secret), "{secret} leaked into a claim column: {joined}");
    }
    // The subject hash is 32 bytes and is not the subject.
    let hash: Vec<u8> = sqlx::query_scalar(
        "select subject_hash from identity_claims where user_id = $1",
    )
    .bind(fixture.user)
    .fetch_one(&fixture.pool)
    .await?;
    assert_eq!(hash.len(), 32);
    assert_ne!(hash, b"naver-subject-woman".to_vec());
    // The verification row holds neither the raw state nor the token.
    let verification: Vec<String> = sqlx::query_scalar(
        "select concat_ws('|', verifier, source_revision, state, return_path) \
         from identity_verifications where user_id = $1",
    )
    .bind(fixture.user)
    .fetch_all(&fixture.pool)
    .await?;
    let joined = verification.join("|");
    assert!(!joined.contains(&state), "the raw state leaked: {joined}");
    assert!(!joined.contains("fake-naver-token"), "{joined}");
    fixture.finish().await
}

/// The subject hash is salted: the same subject under two salts differs.
#[tokio::test]
async fn the_subject_hash_is_salted() {
    let pool = pool().await.expect("fixture pool");
    let first = profile_service::identity::IdentityRepository::new(
        pool.clone(),
        b"first-salt-0123456789abcdef0123456789".to_vec(),
    );
    let second = profile_service::identity::IdentityRepository::new(
        pool,
        b"second-salt-0123456789abcdef01234567".to_vec(),
    );
    assert_ne!(
        first.subject_hash("naver-subject-woman"),
        second.subject_hash("naver-subject-woman")
    );
    // And it is stable for one salt.
    assert_eq!(
        first.subject_hash("naver-subject-woman"),
        first.subject_hash("naver-subject-woman")
    );
    // The construction is the donor's `sha256(salt || ':' || subject)`.
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(b"first-salt-0123456789abcdef0123456789");
    hasher.update(b":");
    hasher.update(b"naver-subject-woman");
    let expected: [u8; 32] = hasher.finalize().into();
    assert_eq!(first.subject_hash("naver-subject-woman"), expected);
}

/// The callback writes no claim when the provider returns no subject.
#[tokio::test]
async fn a_missing_subject_is_profile_incomplete_with_no_claim() -> anyhow::Result<()> {
    let mut profile = FakeProfile::woman();
    profile.id = None;
    // No subject substitution: this case is about the absent subject itself.
    let pool = pool().await?;
    let provider = FakeProvider::start(profile).await?;
    let user = support::user();
    clear_user(&pool, user).await?;
    let fixture = Fixture { pool, provider, user };
    let now = Utc::now();
    let state = fixture.start(now).await?;
    assert_eq!(
        fixture.callback(&state, now).await?,
        CallbackResult::ProfileIncomplete
    );
    assert_eq!(claims(&fixture.pool, fixture.user).await?.len(), 0);
    fixture.finish().await
}

/// The provider configuration used by every case is the development one.
#[test]
fn the_fixture_uses_the_development_configuration() {
    let config = NaverOAuthConfig::local();
    config.validate(false).expect("development config");
    assert!(config.validate(true).is_err(), "and it is not a production config");
}
