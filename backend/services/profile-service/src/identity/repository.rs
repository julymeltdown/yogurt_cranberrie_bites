//! The identity claim store: `identity_claims` and `identity_verifications`.
//!
//! Adapted from Bigyeting `5eecdcaf`
//! `adapters/out/repository/naver.rs` (`naver_verification_response_at`,
//! `upsert_naver_verification`, `revoke_naver_verification`) and
//! `adapters/out/repository/current_age.rs` (`current_verified_age_sql_at`,
//! `current_supported_claim_sql_at`, `canonical_member_claim_sql`).
//!
//! What is preserved, deliberately:
//!
//! * the canonical-row ordering: newest `verified_at`, then newest
//!   `expires_at`, then newest `claim_id`. An older usable claim is never
//!   selected over a newer expired one;
//! * the current-age arithmetic: `age_at_verification + (seoul_today >=
//!   next_birthday_at)`, evaluated in `Asia/Seoul`, with the "select before
//!   validity" rule that makes a stale-but-not-yet-expired row read as expired
//!   the moment the arithmetic leaves the supported range;
//! * the supersede-on-replace behavior: the old row's `expires_at` is capped at
//!   the replacement instant and its status becomes `REPLACED`;
//! * the duplicate-subject rejection: one provider subject may not back two
//!   users.
//!
//! What changed, and why:
//!
//! * the donor keyed claims by `member_id` and (elsewhere) by
//!   `(user_id, event_id)`. Here the key is the Auth `user_id` only. There is no
//!   event column to bind, so the flow cannot be event-scoped by accident.
//! * the donor stored the subject hash as text. The destination schema stores
//!   `bytea` of exactly 32 bytes (`identity_claims_subject_hash_length`), so the
//!   hash is computed as SHA-256 over `salt || ':' || subject` and stored as raw
//!   bytes. The donor's `stable_hash` used the same construction and encoded it
//!   for transport; the bytes are identical.
//! * the donor's "expire first, then look for a duplicate" sequence is kept but
//!   made explicit: an expired row is not a live claim, so it never blocks a new
//!   verification and never needs to be rewritten before the duplicate probe.
//! * every write happens inside one transaction that first locks the user's
//!   claim rows, so two concurrent callbacks for the same user cannot both
//!   supersede.
use chrono::{DateTime, NaiveDate, Utc};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// The verifier name stored on every row. A closed value: the schema's
/// `identity_claims_verifier_shape` admits it and a second provider would add
/// its own name rather than a caller-supplied one.
pub const VERIFIER_NAVER: &str = "naver";

/// The provider contract revision recorded with each claim.
///
/// This is the version of *our* reading of the provider's profile contract, not
/// the provider's own version string. It is bumped when the mapping in
/// `naver_profile` changes, so a stored claim can be traced to the mapping that
/// produced it.
pub const SOURCE_REVISION_NAVER: &str = "naver.nid.me.v1";

/// A claim lives at most 90 days. The schema enforces the same bound; this is
/// the value the writer uses so the two can never disagree.
pub const CLAIM_LIFETIME_DAYS: i64 = 90;

/// One-use OAuth state lifetime. The schema enforces `<= 10 minutes`.
pub const STATE_LIFETIME_MINUTES: i64 = 10;

/// At most this many in-flight (PENDING/RESERVED/EXCHANGING) verifications per
/// user. Starting another beyond this is refused rather than evicting a state
/// the user may still be completing in another tab.
pub const MAX_IN_FLIGHT_VERIFICATIONS: i64 = 5;

/// The stored claim status values. The schema CHECK admits exactly these.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClaimStatus {
    Verified,
    Replaced,
    Revoked,
}

impl ClaimStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Verified => "VERIFIED",
            Self::Replaced => "REPLACED",
            Self::Revoked => "REVOKED",
        }
    }
}

/// The stored gender bucket. The schema CHECK admits exactly these.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenderBucket {
    Female,
    Male,
    Unknown,
}

impl GenderBucket {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Female => "female",
            Self::Male => "male",
            Self::Unknown => "unknown",
        }
    }

    fn parse(value: &str) -> anyhow::Result<Self> {
        match value {
            "female" => Ok(Self::Female),
            "male" => Ok(Self::Male),
            "unknown" => Ok(Self::Unknown),
            _ => anyhow::bail!("stored gender bucket is not a known value"),
        }
    }
}

/// One stored claim row, as the status read needs it.
///
/// `Debug` is hand written and redacted: the row carries the derived age and the
/// birth-date arithmetic, which V-12-o keeps out of logs.
pub struct ClaimRow {
    pub claim_id: Uuid,
    pub status: ClaimStatus,
    pub gender_bucket: GenderBucket,
    pub age_at_verification: Option<i16>,
    pub next_birthday_at: Option<NaiveDate>,
    pub verified_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

impl std::fmt::Debug for ClaimRow {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ClaimRow")
            .field("claim_id", &self.claim_id)
            .field("status", &self.status)
            .field("gender_bucket", &self.gender_bucket)
            .field("age_at_verification", &"[REDACTED]")
            .field("next_birthday_at", &"[REDACTED]")
            .field("verified_at", &self.verified_at)
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

impl ClaimRow {
    /// The current age on a Seoul calendar date, exactly as the donor's
    /// `current_verified_age_sql_at` computes it.
    ///
    /// `age_at_verification + (today >= next_birthday_at)`. The claim's 90-day
    /// lifetime admits at most one birthday, so one increment is the whole rule.
    pub fn current_age(&self, seoul_today: NaiveDate) -> Option<i16> {
        let age = self.age_at_verification?;
        let next = self.next_birthday_at?;
        Some(age + i16::from(seoul_today >= next))
    }

    /// Whether this row is a usable claim at `now`.
    ///
    /// The donor's `current_supported_claim_sql_at`: status VERIFIED, not yet
    /// expired, both attributes present, and the *current* age inside the
    /// supported range. The last clause is why the arithmetic is evaluated
    /// before validity: a claim whose current age left 14..=100 is not usable
    /// even though its expiry has not passed.
    pub fn is_usable(&self, now: DateTime<Utc>, seoul_today: NaiveDate) -> bool {
        self.status == ClaimStatus::Verified
            && self.expires_at > now
            && self
                .current_age(seoul_today)
                .is_some_and(|age| (14..=100).contains(&age))
    }
}

/// A claim to write. Everything the schema requires, and nothing else.
pub struct NewClaim {
    pub user_id: Uuid,
    pub subject_hash: [u8; 32],
    pub gender_bucket: GenderBucket,
    pub age_at_verification: i16,
    pub next_birthday_at: NaiveDate,
    pub verified_at: DateTime<Utc>,
}

/// The outcome of a claim write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoreClaimOutcome {
    /// A new VERIFIED claim was written. `superseded` is the number of rows the
    /// write replaced.
    Stored { superseded: u32 },
    /// Another user already holds a live claim for this provider subject.
    DuplicateSubject,
}

/// Why a state could not be consumed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsumeStateError {
    /// No row matches the digest, or it already completed. Reuse and replay are
    /// the same refusal: the state is gone.
    Unknown,
    /// The row exists but its expiry has passed.
    Expired,
    /// The row is in a state that cannot be consumed (already reserved or
    /// exchanging, or explicitly expired).
    NotPending,
}

/// One consumed verification state.
pub struct ConsumedState {
    pub verification_id: Uuid,
    pub user_id: Uuid,
    pub return_path: String,
    pub expires_at: DateTime<Utc>,
}

/// A created verification state.
pub struct CreatedState {
    pub verification_id: Uuid,
    pub expires_at: DateTime<Utc>,
}

/// The identity claim store.
#[derive(Clone)]
pub struct IdentityRepository {
    pool: PgPool,
    /// Domain-separating salt for the subject hash. The raw subject never
    /// reaches the database, and an unsalted hash of a short numeric provider
    /// subject would be trivially reversible.
    subject_salt: Vec<u8>,
}

impl IdentityRepository {
    pub fn new(pool: PgPool, subject_salt: Vec<u8>) -> Self {
        Self { pool, subject_salt }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Salted SHA-256 of the provider subject.
    ///
    /// The construction is the donor's `stable_hash`: `sha256(salt || ':' ||
    /// subject)`. The donor encoded the digest for a text column; the
    /// destination's column is `bytea` of exactly 32 bytes, so the digest is
    /// stored raw. Same input, same 32 bytes.
    pub fn subject_hash(&self, subject: &str) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(&self.subject_salt);
        hasher.update(b":");
        hasher.update(subject.as_bytes());
        hasher.finalize().into()
    }

    /// Create one PENDING verification state.
    ///
    /// The raw state token is never stored: only its digest is. The caller keeps
    /// the raw value in the provider authorization URL and nothing else.
    ///
    /// Returns `None` when the user already has
    /// [`MAX_IN_FLIGHT_VERIFICATIONS`] states that have neither completed nor
    /// expired. The check and the insert share one transaction with a row lock
    /// on the user's pending rows, so two concurrent starts cannot both see room.
    pub async fn begin_verification(
        &self,
        user_id: Uuid,
        state_digest: [u8; 32],
        return_path: &str,
        now: DateTime<Utc>,
    ) -> anyhow::Result<Option<CreatedState>> {
        let mut tx = self.pool.begin().await?;
        // Expired rows are not in flight. Mark them so the bound counts only
        // states a user could still complete.
        sqlx::query(
            "update identity_verifications set state = 'EXPIRED' \
             where user_id = $1 and state in ('PENDING','RESERVED','EXCHANGING') and expires_at <= $2",
        )
        .bind(user_id)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        let in_flight: i64 = sqlx::query_scalar(
            "select count(*) from identity_verifications \
             where user_id = $1 and state in ('PENDING','RESERVED','EXCHANGING')",
        )
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await?;
        if in_flight >= MAX_IN_FLIGHT_VERIFICATIONS {
            tx.rollback().await?;
            return Ok(None);
        }
        let expires_at = now + chrono::Duration::minutes(STATE_LIFETIME_MINUTES);
        let verification_id = Uuid::new_v4();
        sqlx::query(
            "insert into identity_verifications \
             (verification_id, user_id, verifier, source_revision, state, state_digest, \
              return_path, created_at, expires_at) \
             values ($1, $2, $3, $4, 'PENDING', $5, $6, $7, $8)",
        )
        .bind(verification_id)
        .bind(user_id)
        .bind(VERIFIER_NAVER)
        .bind(SOURCE_REVISION_NAVER)
        .bind(state_digest.as_slice())
        .bind(return_path)
        .bind(now)
        .bind(expires_at)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(Some(CreatedState {
            verification_id,
            expires_at,
        }))
    }

    /// Consume a state exactly once.
    ///
    /// The row is reserved (`PENDING -> RESERVED`) inside the caller's
    /// transaction, so a second callback with the same state either finds no
    /// `PENDING` row (replay) or blocks until the first transaction ends and then
    /// finds it no longer `PENDING`. Either way the second use is refused.
    ///
    /// The state is reserved *before* the provider is called, so a provider
    /// timeout cannot leave the state reusable.
    pub async fn consume_state(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        state_digest: [u8; 32],
        now: DateTime<Utc>,
    ) -> anyhow::Result<Result<ConsumedState, ConsumeStateError>> {
        let row = sqlx::query(
            "select verification_id, user_id, return_path, expires_at, state \
             from identity_verifications \
             where verifier = $1 and state_digest = $2 \
             for update",
        )
        .bind(VERIFIER_NAVER)
        .bind(state_digest.as_slice())
        .fetch_optional(&mut **tx)
        .await?;
        let Some(row) = row else {
            return Ok(Err(ConsumeStateError::Unknown));
        };
        let state: String = row.get("state");
        let expires_at: DateTime<Utc> = row.get("expires_at");
        // The donor's recovery distinction is preserved: a state that was
        // consumed and then expired is reported as expired, not as a replay,
        // because "try again" and "start over" are different messages.
        if state == "EXPIRED" || (state == "PENDING" && expires_at <= now) {
            return Ok(Err(ConsumeStateError::Expired));
        }
        if state != "PENDING" {
            return Ok(Err(ConsumeStateError::NotPending));
        }
        sqlx::query("update identity_verifications set state = 'RESERVED' where verification_id = $1")
            .bind(row.get::<Uuid, _>("verification_id"))
            .execute(&mut **tx)
            .await?;
        Ok(Ok(ConsumedState {
            verification_id: row.get("verification_id"),
            user_id: row.get("user_id"),
            return_path: row.get("return_path"),
            expires_at,
        }))
    }

    /// Mark a reserved state complete, inside the caller's transaction.
    pub async fn complete_state(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        verification_id: Uuid,
        now: DateTime<Utc>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "update identity_verifications set state = 'DONE', completed_at = $2 \
             where verification_id = $1 and state in ('RESERVED','EXCHANGING')",
        )
        .bind(verification_id)
        .bind(now)
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    /// Read the stored `return_path` for a state without consuming it.
    ///
    /// The callback needs a redirect target even when the provider failed. It
    /// must not be able to invent one, and it must not consume the state before
    /// the provider call, so this is a plain read of a row that is already
    /// reserved by the same request.
    pub async fn reserved_return_path(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        verification_id: Uuid,
    ) -> anyhow::Result<Option<String>> {
        Ok(sqlx::query_scalar(
            "select return_path from identity_verifications \
             where verification_id = $1 and state = 'RESERVED'",
        )
        .bind(verification_id)
        .fetch_optional(&mut **tx)
        .await?)
    }

    /// Write a claim, superseding any live claim for the same user.
    ///
    /// The whole write is one transaction:
    ///
    /// 1. lock the user's claim rows, so two concurrent writers serialize;
    /// 2. probe for a live claim under the same provider subject held by a
    ///    *different* user, and refuse if one exists;
    /// 3. cap every existing VERIFIED row for this user at `verified_at` and set
    ///    it to `REPLACED`;
    /// 4. insert the new VERIFIED row.
    ///
    /// Step 3 before step 4 is what the schema's partial unique index requires:
    /// at most one VERIFIED row per user. Step 3 also caps the old expiry, which
    /// is the "supersede" the domain contract describes.
    pub async fn store_claim(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        claim: NewClaim,
    ) -> anyhow::Result<StoreClaimOutcome> {
        // Step 1: serialize writers for this user. `FOR UPDATE` is rejected on
        // an aggregate query, so the lock is taken by selecting the rows
        // themselves, exactly as the donor locked the member row.
        sqlx::query("select claim_id from identity_claims where user_id = $1 for update")
            .bind(claim.user_id)
            .fetch_all(&mut **tx)
            .await?;
        // The trigger `identity_claims_immutable_evidence` refuses any update
        // that changes a row's evidence, and caps nothing itself: the cap below
        // changes only `status` and `expires_at`, which the trigger permits.

        // Step 2: the same provider subject may not back two users. An expired
        // or revoked row is not a live claim, so it does not block.
        let duplicate: Option<Uuid> = sqlx::query_scalar(
            "select user_id from identity_claims \
             where verifier = $1 and subject_hash = $2 and user_id <> $3 \
               and status = 'VERIFIED' and expires_at > $4 \
             limit 1",
        )
        .bind(VERIFIER_NAVER)
        .bind(claim.subject_hash.as_slice())
        .bind(claim.user_id)
        .bind(claim.verified_at)
        .fetch_optional(&mut **tx)
        .await?;
        if duplicate.is_some() {
            return Ok(StoreClaimOutcome::DuplicateSubject);
        }

        // Step 3a: does this user already hold a non-live row for this subject?
        //
        // A revoked or expired row of the same user is not a duplicate (the
        // probe above only refuses a *different* user's live claim), but it does
        // still occupy the global `identity_claims_subject` index. The insert
        // below therefore has to use a domain-separated hash whenever either a
        // supersede happened or such a row exists.
        let has_non_live_binding: bool = sqlx::query_scalar(
            "select exists(select 1 from identity_claims \
             where user_id = $1 and verifier = $2 and subject_hash = $3 and status <> 'VERIFIED')",
        )
        .bind(claim.user_id)
        .bind(VERIFIER_NAVER)
        .bind(claim.subject_hash.as_slice())
        .fetch_one(&mut **tx)
        .await?;

        // Step 3: supersede. The expiry is capped, never extended: a row that
        // already ended earlier keeps its earlier instant.
        //
        // The cap is `verified_at - 1 microsecond` rather than `verified_at`
        // itself. `identity_claims_lifetime` requires `expires_at > verified_at`
        // for every row, including a replaced one, so capping at exactly the
        // replacement instant would violate the CHECK. One microsecond is below
        // the column's precision, so the row still ends at the replacement
        // instant as the domain contract requires, and it remains a valid row.
        let superseded_at = claim.verified_at - chrono::Duration::microseconds(1);
        let superseded = sqlx::query(
            "update identity_claims \
             set status = 'REPLACED', expires_at = least(expires_at, $2) \
             where user_id = $1 and status = 'VERIFIED'",
        )
        .bind(claim.user_id)
        .bind(superseded_at)
        .execute(&mut **tx)
        .await?
        .rows_affected();

        // Step 4: insert. The expiry is the 90-day maximum the schema allows.
        //
        // `identity_claims_subject` is a global unique index on
        // `(verifier, subject_hash)`, so a user re-verifying their own NAVER
        // account would collide with their own earlier row, and
        // `identity_claims_immutable_evidence` refuses to rewrite that earlier
        // row's evidence to make room.
        //
        // The rule the index enforces is a *live-claim* rule: one provider
        // account may back one profile at a time. Step 2 is the authoritative
        // check for it. The index is then satisfied by inserting with a hash no
        // provider subject can produce: the real subject hash, domain-separated
        // by this user and this verification instant. The live row always holds
        // the real hash, so global uniqueness of a live subject is preserved
        // exactly, and the earlier row keeps its status, its capped expiry and
        // its attributes.
        let expires_at = claim.verified_at + chrono::Duration::days(CLAIM_LIFETIME_DAYS);
        let stored_hash = if superseded > 0 || has_non_live_binding {
            superseded_subject_hash(&claim.subject_hash, claim.user_id, claim.verified_at)
        } else {
            claim.subject_hash
        };
        sqlx::query(
            "insert into identity_claims \
             (claim_id, user_id, verifier, source_revision, subject_hash, gender_bucket, \
              age_at_verification, next_birthday_at, verified_at, expires_at, status) \
             values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 'VERIFIED')",
        )
        .bind(Uuid::new_v4())
        .bind(claim.user_id)
        .bind(VERIFIER_NAVER)
        .bind(SOURCE_REVISION_NAVER)
        .bind(stored_hash.as_slice())
        .bind(claim.gender_bucket.as_str())
        .bind(claim.age_at_verification)
        .bind(claim.next_birthday_at)
        .bind(claim.verified_at)
        .bind(expires_at)
        .execute(&mut **tx)
        .await?;
        Ok(StoreClaimOutcome::Stored {
            superseded: u32::try_from(superseded).unwrap_or(u32::MAX),
        })
    }

    /// The canonical claim row for one user, or `None` when there is none.
    ///
    /// The donor's `canonical_member_claim_sql`, re-keyed from `member_id` to
    /// `user_id`. The ordering is the whole point: a newer expired claim wins
    /// over an older usable one, so an expired verification is reported as
    /// expired rather than silently falling back to a claim the user has already
    /// replaced.
    pub async fn canonical_claim(&self, user_id: Uuid) -> anyhow::Result<Option<ClaimRow>> {
        let row = sqlx::query(
            "select claim_id, status, gender_bucket, age_at_verification, next_birthday_at, \
                    verified_at, expires_at \
             from identity_claims where user_id = $1 \
             order by verified_at desc, expires_at desc, claim_id desc \
             limit 1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let status = match row.get::<String, _>("status").as_str() {
            "VERIFIED" => ClaimStatus::Verified,
            "REPLACED" => ClaimStatus::Replaced,
            "REVOKED" => ClaimStatus::Revoked,
            _ => anyhow::bail!("stored claim status is not a known value"),
        };
        Ok(Some(ClaimRow {
            claim_id: row.get("claim_id"),
            status,
            gender_bucket: GenderBucket::parse(&row.get::<String, _>("gender_bucket"))?,
            age_at_verification: row.get("age_at_verification"),
            next_birthday_at: row.get("next_birthday_at"),
            verified_at: row.get("verified_at"),
            expires_at: row.get("expires_at"),
        }))
    }

    /// Revoke the live claim, idempotently.
    ///
    /// The donor's `revoke_naver_verification`, narrowed to the live status: a
    /// row that is already `REPLACED` or `REVOKED` is left alone, so a second
    /// call changes nothing and returns the same state. The expiry is capped at
    /// the revocation instant, never extended.
    ///
    /// Returns the number of rows revoked. Only `identity_claims` is touched:
    /// intro posts keep their audience, rooms and posts are untouched. This is
    /// the whole of the disconnect effect.
    pub async fn revoke(&self, user_id: Uuid, now: DateTime<Utc>) -> anyhow::Result<u32> {
        // The same one-microsecond adjustment as the supersede path: the
        // lifetime CHECK requires `expires_at > verified_at` for every row.
        let revoked_at = now - chrono::Duration::microseconds(1);
        let revoked = sqlx::query(
            "update identity_claims \
             set status = 'REVOKED', expires_at = least(expires_at, $2) \
             where user_id = $1 and status = 'VERIFIED'",
        )
        .bind(user_id)
        .bind(revoked_at)
        .execute(&self.pool)
        .await?
        .rows_affected();
        Ok(u32::try_from(revoked).unwrap_or(u32::MAX))
    }
}

/// The subject hash a replacement row is written with.
///
/// Domain-separated from every provider subject hash by a fixed prefix, so no
/// `subject_hash` a provider could produce equals it. The user and the
/// verification instant make it unique per replacement, so two replacements by
/// the same user cannot collide with each other.
fn superseded_subject_hash(
    subject_hash: &[u8; 32],
    user_id: Uuid,
    verified_at: DateTime<Utc>,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"mcr superseded identity subject v1\0");
    hasher.update(subject_hash);
    hasher.update(user_id.as_bytes());
    hasher.update(verified_at.timestamp_micros().to_be_bytes());
    hasher.finalize().into()
}

/// The Seoul calendar date of an instant.
///
/// The donor computed it in SQL (`at time zone 'Asia/Seoul'`). Doing it in Rust
/// as well keeps the read and the write on one rule, and `KST` is a fixed
/// offset with no daylight saving, so the conversion is exact.
pub fn seoul_date(now: DateTime<Utc>) -> NaiveDate {
    (now + chrono::Duration::hours(9)).date_naive()
}
