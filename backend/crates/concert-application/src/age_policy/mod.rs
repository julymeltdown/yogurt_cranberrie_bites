//! Read-only private claim/status port. No claim persistence/provider/callback.
use crate::Clock;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_domain::age_policy::{assess, AgeAssessment, AgeStatus, OwnerClaim};
use concert_domain::{AuthUserId, EventId};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PolicyError {
    #[error("Resource is unavailable")]
    Denied,
    #[error("Required policy is unavailable")]
    Unavailable,
}
/// These bindings are input, never an authority token. Real adapters must verify
/// the canonical actor/session/current event and preserve the original deadline.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Context {
    pub actor: AuthUserId,
    pub event: EventId,
    pub session: Uuid,
    pub request: Uuid,
    pub expires_at: DateTime<Utc>,
}
impl Context {
    pub fn check(self, now: DateTime<Utc>) -> Result<(), PolicyError> {
        if self.actor.uuid().is_nil()
            || self.event.uuid().is_nil()
            || self.session.is_nil()
            || self.request.is_nil()
            || self.expires_at <= now
        {
            Err(PolicyError::Unavailable)
        } else {
            Ok(())
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ClaimQuery {
    pub context: Context,
    pub subject: AuthUserId,
    pub requested_at: DateTime<Utc>,
}
/// Private source-owner observation. Not Debug/Serialize; not a mutation fence.
pub trait ClaimLease: Send + Sync {
    fn check(&self, query: ClaimQuery, now: DateTime<Utc>) -> Result<(), PolicyError>;
}
pub struct ClaimSnapshot {
    pub lease: Box<dyn ClaimLease>,
    pub query: ClaimQuery,
    pub checked_at: DateTime<Utc>,
    pub valid_until: DateTime<Utc>,
    pub claims: Vec<OwnerClaim>,
    pub verification_available: bool,
}
#[async_trait]
pub trait ClaimReader: Send + Sync {
    /// Resolve current canonical owner rows, verifier availability and actor/
    /// subject/event/session binding from real authority, not request assertions.
    /// Never filter invalid/replaced/expired rows before canonical selection.
    /// The returned lease must preserve claim authority through projection; an
    /// in-memory timestamp alone is not an implementation of that guarantee.
    async fn read(&self, query: ClaimQuery) -> Result<ClaimSnapshot, PolicyError>;
}
pub struct UnavailableClaims;
#[async_trait]
impl ClaimReader for UnavailableClaims {
    async fn read(&self, _: ClaimQuery) -> Result<ClaimSnapshot, PolicyError> {
        Err(PolicyError::Unavailable)
    }
}
/// Opaque C04 own-age status. Exact age/claim/source/private bounds never enter it.
pub struct OwnAgeStatus {
    context: Context,
    status: AgeStatus,
    verified_at: Option<DateTime<Utc>>,
    expires_at: Option<DateTime<Utc>>,
    verification_available: bool,
}
impl OwnAgeStatus {
    pub fn status(&self) -> AgeStatus {
        self.status
    }
    pub fn user_id(&self) -> Uuid {
        self.context.actor.uuid()
    }
    pub fn event_id(&self) -> Uuid {
        self.context.event.uuid()
    }
    pub fn timestamps(&self) -> (Option<DateTime<Utc>>, Option<DateTime<Utc>>) {
        (self.verified_at, self.expires_at)
    }
    pub fn verification_available(&self) -> bool {
        self.verification_available
    }
}
/// Internal evaluated observation used within a fresh read. Not a grant to reuse
/// in a later read or mutation; consumers must call the current ports again.
pub struct CurrentAge {
    pub assessment: AgeAssessment,
    pub valid_until: DateTime<Utc>,
    pub verification_available: bool,
    query: ClaimQuery,
    lease: Box<dyn ClaimLease>,
}
impl CurrentAge {
    pub fn check(&self, now: DateTime<Utc>) -> Result<(), PolicyError> {
        self.query.context.check(now)?;
        self.lease.check(self.query, now)?;
        if self.valid_until <= now {
            Err(PolicyError::Denied)
        } else {
            Ok(())
        }
    }
}
pub async fn current<R: ClaimReader, C: Clock>(
    reader: &R,
    clock: &C,
    context: Context,
    subject: AuthUserId,
) -> Result<CurrentAge, PolicyError> {
    let requested_at = clock.now();
    context.check(requested_at)?;
    if subject.uuid().is_nil() {
        return Err(PolicyError::Unavailable);
    }
    let query = ClaimQuery {
        context,
        subject,
        requested_at,
    };
    let snapshot = reader.read(query).await?;
    let now = clock.now();
    context.check(now)?;
    if snapshot.query != query
        || snapshot.checked_at < requested_at
        || snapshot.checked_at > now
        || snapshot.valid_until <= now
        || snapshot.valid_until > context.expires_at
    {
        return Err(PolicyError::Unavailable);
    }
    snapshot.lease.check(query, now)?;
    let assessment =
        assess(subject, &snapshot.claims, now).map_err(|_| PolicyError::Unavailable)?;
    let expiry = if assessment.status() == AgeStatus::Verified {
        assessment.recheck_at()
    } else {
        None
    };
    Ok(CurrentAge {
        assessment,
        valid_until: expiry.map_or(snapshot.valid_until, |e| e.min(snapshot.valid_until)),
        verification_available: snapshot.verification_available,
        query,
        lease: snapshot.lease,
    })
}
pub async fn own_status<R: ClaimReader, C: Clock>(
    reader: &R,
    clock: &C,
    context: Context,
) -> Result<OwnAgeStatus, PolicyError> {
    let result = current(reader, clock, context, context.actor).await?;
    result.check(clock.now())?;
    let status =
        if result.assessment.status() == AgeStatus::Unknown && !result.verification_available {
            AgeStatus::ProviderUnavailable
        } else {
            result.assessment.status()
        };
    let (verified_at, expires_at) = result.assessment.timestamps();
    Ok(OwnAgeStatus {
        context,
        status,
        verified_at,
        expires_at,
        verification_available: result.verification_available,
    })
}
