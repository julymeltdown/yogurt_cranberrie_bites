//! BE26-CLAIM-READER safe independent projection scope, NOT a successful legacy lease.
//! Root registration and the application projection-scope port remain coordinated work.
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use concert_application::{
    age_policy::{ClaimQuery, ClaimReader, ClaimSnapshot, PolicyError},
    Clock,
};
use concert_domain::age_policy::{
    assess, AgeAssessment, AgeBounds, AgeStatus, OwnerClaim, StoredStatus,
};
use sqlx::{PgConnection, PgPool, Row};
use std::{future::Future, pin::Pin, sync::Arc};

/// Decision data, not a lease. The implementation MUST retain real source fences
/// in this transaction through finalization. Copying a remote observation here
/// does not meet that contract. Availability must be qualified at the source.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct AuthorityDecision {
    pub query: ClaimQuery,
    pub verification_available: bool,
}
#[async_trait]
pub trait ProjectionAuthority: Send + Sync {
    /// Called before claim locks and again AFTER private projection. Resolve exact
    /// canonical actor/subject/session/event/request/deadline and current policy;
    /// hold participating owner/session/scope fences through the final COMMIT.
    /// Cross-service authority needs a qualified source protocol, not a cached
    /// token, process mutex, receipt, timestamp, or successful RPC observation.
    /// This adapter provides no production Auth implementation.
    async fn authorize_in(
        &self,
        connection: &mut PgConnection,
        query: ClaimQuery,
    ) -> Result<AuthorityDecision, PolicyError>;
}
pub struct UnavailableProjectionAuthority;
#[async_trait]
impl ProjectionAuthority for UnavailableProjectionAuthority {
    async fn authorize_in(
        &self,
        _: &mut PgConnection,
        _: ClaimQuery,
    ) -> Result<AuthorityDecision, PolicyError> {
        Err(PolicyError::Unavailable)
    }
}

/// Borrowed private policy input. No claim ID, raw provenance, DOB or exact-age
/// getter; no Clone/Debug/Serialize. It cannot outlive its projection closure.
pub struct ProjectionAge<'a> {
    query: ClaimQuery,
    assessment: &'a AgeAssessment,
    verification_available: bool,
}
impl ProjectionAge<'_> {
    pub fn query(&self) -> ClaimQuery {
        self.query
    }
    pub fn status(&self) -> AgeStatus {
        match self.assessment.status() {
            AgeStatus::Unknown if !self.verification_available => AgeStatus::ProviderUnavailable,
            other => other,
        }
    }
    pub fn timestamps(&self) -> (Option<DateTime<Utc>>, Option<DateTime<Utc>>) {
        self.assessment.timestamps()
    }
    pub fn verification_available(&self) -> bool {
        self.verification_available
    }
    /// Only a decision within this projection, never a grant for a later action.
    pub fn permits(&self, bounds: AgeBounds, sharing: bool) -> bool {
        bounds.permits(self.assessment, sharing)
    }
}
pub type ProjectionFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, PolicyError>> + Send + 'a>>;

#[derive(Clone)]
pub struct PgClaimProjection {
    pool: PgPool,
    authority: Arc<dyn ProjectionAuthority>,
    clock: Arc<dyn Clock>,
}
impl PgClaimProjection {
    pub fn new(
        pool: PgPool,
        authority: Arc<dyn ProjectionAuthority>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            pool,
            authority,
            clock,
        }
    }
    pub fn unavailable(pool: PgPool, clock: Arc<dyn Clock>) -> Self {
        Self::new(pool, Arc::new(UnavailableProjectionAuthority), clock)
    }
    /// Execute trusted, side-effect-free projection inside a private SQL scope.
    /// The closure must not publish/log/cache its intermediate values or return
    /// an authorization capability. Its value is discarded on any final failure.
    /// All read-policy/collection/row decisions must share this scope in a future
    /// BE25/BE27 composition; this is not a drop-in replacement for ClaimReader.
    ///
    /// Successful COMMIT is the linearization point. Later revocation invalidates
    /// later reads; it cannot retroactively retract a completed response. No
    /// successful result is exposed on cancellation, rollback or lost source ACK.
    pub async fn project<T: Send, F>(
        &self,
        query: ClaimQuery,
        projection: F,
    ) -> Result<T, PolicyError>
    where
        F: for<'a> FnOnce(ProjectionAge<'a>) -> ProjectionFuture<'a, T> + Send,
    {
        validate(query, self.clock.now())?;
        // Bound resource occupancy without extending the original request deadline.
        let until = query
            .context
            .expires_at
            .min(Utc::now() + Duration::seconds(5));
        let remaining = (until - Utc::now())
            .to_std()
            .map_err(|_| PolicyError::Unavailable)?;
        tokio::time::timeout(remaining, self.project_inner(query, until, projection))
            .await
            .map_err(|_| PolicyError::Unavailable)?
    }
    async fn project_inner<T: Send, F>(
        &self,
        query: ClaimQuery,
        until: DateTime<Utc>,
        projection: F,
    ) -> Result<T, PolicyError>
    where
        F: for<'a> FnOnce(ProjectionAge<'a>) -> ProjectionFuture<'a, T> + Send,
    {
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        // Same clamp as the BE26 producer; never widen shorter SQL timeouts.
        sqlx::query("select set_config('lock_timeout',(extract(epoch from least(coalesce(nullif(current_setting('lock_timeout')::interval,interval '0'),interval '1500 milliseconds'),interval '1500 milliseconds'))*1000)::bigint::text||'ms',true),set_config('statement_timeout',(extract(epoch from least(coalesce(nullif(current_setting('statement_timeout')::interval,interval '0'),interval '5 seconds'),interval '5 seconds'))*1000)::bigint::text||'ms',true)")
            .execute(&mut *tx).await.map_err(unavailable)?;
        let decision = self.authority.authorize_in(&mut tx, query).await?;
        if decision.query != query {
            return Err(PolicyError::Denied);
        }
        // Same order as BE26: authority -> event -> owner guard -> claim.
        let phase: Option<String> =
            sqlx::query_scalar("select phase from concert_events where event_id=$1 for share")
                .bind(query.context.event.uuid())
                .fetch_optional(&mut *tx)
                .await
                .map_err(unavailable)?;
        if phase.is_none() {
            return Err(PolicyError::Denied);
        }
        sqlx::query("insert into concert_age_owners(user_id) values($1) on conflict do nothing")
            .bind(query.subject.uuid())
            .execute(&mut *tx)
            .await
            .map_err(unavailable)?;
        sqlx::query("select user_id from concert_age_owners where user_id=$1 for share")
            .bind(query.subject.uuid())
            .fetch_one(&mut *tx)
            .await
            .map_err(unavailable)?;
        // Crucially no status/expiry/event predicate before the canonical LIMIT.
        let row = sqlx::query("select * from concert_age_claims where user_id=$1 order by verified_at desc,expires_at desc,claim_id desc limit 1 for share")
            .bind(query.subject.uuid()).fetch_optional(&mut *tx).await.map_err(unavailable)?;
        let claims = row
            .map(owner_claim)
            .transpose()?
            .into_iter()
            .collect::<Vec<_>>();
        let now = self.clock.now();
        validate(query, now)?;
        source_deadline(&mut tx, until).await?;
        let assessment =
            assess(query.subject, &claims, now).map_err(|_| PolicyError::Unavailable)?;
        let valid_until = if assessment.status() == AgeStatus::Verified {
            assessment
                .recheck_at()
                .unwrap_or(query.context.expires_at)
                .min(query.context.expires_at)
        } else {
            query.context.expires_at
        };
        let value = projection(ProjectionAge {
            query,
            assessment: &assessment,
            verification_available: decision.verification_available,
        })
        .await?;
        // Revalidate AFTER projection; no detached synchronous connection lease.
        let final_decision = self.authority.authorize_in(&mut tx, query).await?;
        if final_decision.query != query {
            return Err(PolicyError::Denied);
        }
        if final_decision != decision {
            return Err(PolicyError::Unavailable);
        }
        let now = self.clock.now();
        validate(query, now)?;
        if now >= valid_until {
            return Err(PolicyError::Denied);
        }
        source_deadline(&mut tx, until.min(valid_until)).await?;
        tx.commit().await.map_err(unavailable)?;
        // SQL/COMMIT may itself cross a claim/birthday/request boundary. A known
        // commit establishes source ordering but never extends temporal validity.
        validate(query, self.clock.now())?;
        if self.clock.now() >= valid_until || Utc::now() >= until.min(valid_until) {
            return Err(PolicyError::Denied);
        }
        // The private projection is released only after a known source commit.
        Ok(value)
    }
}
/// Fail closed under the unchanged BE25 synchronous lease API. In particular,
/// configuration of a SQL authority must not turn this into an insufficient lease.
#[async_trait]
impl ClaimReader for PgClaimProjection {
    async fn read(&self, _: ClaimQuery) -> Result<ClaimSnapshot, PolicyError> {
        Err(PolicyError::Unavailable)
    }
}
fn validate(query: ClaimQuery, now: DateTime<Utc>) -> Result<(), PolicyError> {
    query.context.check(now)?;
    if query.subject.uuid().is_nil() || query.requested_at > now {
        Err(PolicyError::Denied)
    } else {
        Ok(())
    }
}
async fn source_deadline(
    connection: &mut PgConnection,
    until: DateTime<Utc>,
) -> Result<(), PolicyError> {
    let live: bool = sqlx::query_scalar("select clock_timestamp()<$1")
        .bind(until)
        .fetch_one(connection)
        .await
        .map_err(unavailable)?;
    if live {
        Ok(())
    } else {
        Err(PolicyError::Unavailable)
    }
}
// Exact BE26 owner_claim mapping, including erasure tombstones. New SQL scope;
// no schema change or exposure of these private inputs to the projection closure.
fn owner_claim(r: sqlx::postgres::PgRow) -> Result<OwnerClaim, PolicyError> {
    let status = match r
        .try_get::<String, _>("status")
        .map_err(unavailable)?
        .as_str()
    {
        "VERIFIED" | "EXPIRED" => StoredStatus::Verified,
        "REPLACED" => StoredStatus::Replaced,
        "REVOKED" => StoredStatus::Revoked,
        "UNSUPPORTED" => StoredStatus::Unsupported,
        _ => return Err(PolicyError::Unavailable),
    };
    Ok(OwnerClaim {
        claim_id: r.try_get("claim_id").map_err(unavailable)?,
        user_id: concert_domain::AuthUserId::from_uuid(r.try_get("user_id").map_err(unavailable)?),
        verifier_id: r.try_get("verifier_id").map_err(unavailable)?,
        source_revision: r.try_get("source_revision").map_err(unavailable)?,
        status,
        verified_at: r.try_get("verified_at").map_err(unavailable)?,
        expires_at: r.try_get("expires_at").map_err(unavailable)?,
        age_at_verification: r.try_get("age_at_verification").map_err(unavailable)?,
        next_birthday_at: r.try_get("next_birthday_at").map_err(unavailable)?,
    })
}
fn unavailable(_: sqlx::Error) -> PolicyError {
    PolicyError::Unavailable
}
