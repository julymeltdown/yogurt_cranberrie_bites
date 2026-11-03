//! Request-bound owner observations for projections. This module intentionally
//! does not implement FencedRelationshipAuthority or produce FreshAuthority.
use chrono::{DateTime, Utc};
use concert_domain::relationship::{Field, PolicyQuery, RelationshipError};
use platform::actor_delegation::authority_deadline::AuthorityRpcDeadline;
use std::{fmt, time::Duration};
use uuid::Uuid;
pub mod grpc;
pub mod invalidation;
pub(crate) mod world_auth;
pub(crate) mod world_projection;
pub const BUDGET: Duration = Duration::from_secs(2);
pub const MAX_REVISION: i64 = 9_007_199_254_740_991;
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct OwnerRevision {
    pub epoch: Uuid,
    pub revision: i64,
}
impl OwnerRevision {
    pub fn validate(self) -> Result<Self, RelationshipError> {
        if self.epoch.is_nil() || !(1..=MAX_REVISION).contains(&self.revision) {
            Err(RelationshipError::Unavailable)
        } else {
            Ok(self)
        }
    }
}
impl fmt::Debug for OwnerRevision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("OwnerRevision([REDACTED])")
    }
}
/// Preserve both independent owner generations. No max, hash or scalar packing.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct RevisionVector {
    pub auth: OwnerRevision,
    pub friend: OwnerRevision,
}
impl fmt::Debug for RevisionVector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RevisionVector([REDACTED])")
    }
}
/// Only constructed from an already sealed Gateway bootstrap authority. This
/// boundary does not authenticate arbitrary caller-provided actor/family strings.
pub struct RequestBinding {
    query: PolicyQuery,
    family: Uuid,
    request: Uuid,
    deadline: AuthorityRpcDeadline,
}
impl RequestBinding {
    /// The caller must reserve and run both owner attempts around I/O. This
    /// conversion preserves wire scope; the legacy binding alone has no cancel gate.
    pub(crate) fn from_world(
        context: &world_projection::WorldPresenceProjection,
    ) -> Result<Self, RelationshipError> {
        context
            .check()
            .map_err(|_| RelationshipError::Unavailable)?;
        Ok(Self {
            query: context.query(),
            family: context.session_family_id(),
            request: context.request_id(),
            deadline: context.deadline(),
        })
    }
    pub fn from_verified(
        authority: &crate::rpc::VerifiedBootstrap,
        query: PolicyQuery,
    ) -> Result<Self, RelationshipError> {
        authority
            .check()
            .map_err(|_| RelationshipError::Unavailable)?;
        if authority.user_id() != query.actor.uuid()
            || authority.event_id() != query.event.uuid()
            || query.subject.uuid().is_nil()
            || (query.field == Field::SafetySelf && query.actor != query.subject)
        {
            return Err(RelationshipError::Denied);
        }
        Ok(Self {
            query,
            family: authority.session_family_id(),
            request: authority.request_id(),
            deadline: authority.deadline(),
        })
    }
    pub fn query(&self) -> PolicyQuery {
        self.query
    }
    pub fn session_family(&self) -> Uuid {
        self.family
    }
    pub fn request_id(&self) -> Uuid {
        self.request
    }
    pub fn deadline(&self) -> AuthorityRpcDeadline {
        self.deadline
    }
    pub fn check(&self) -> Result<(), RelationshipError> {
        self.deadline
            .remaining(BUDGET, self.deadline.expires_micros())
            .map(|_| ())
            .map_err(|_| RelationshipError::Unavailable)
    }
}
impl fmt::Debug for RequestBinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RequestBinding([REDACTED])")
    }
}
#[derive(Clone)]
pub struct BoundedObservation {
    query: PolicyQuery,
    family: Uuid,
    request: Uuid,
    revision: RevisionVector,
    auth_observed_at: DateTime<Utc>,
    friend_observed_at: DateTime<Utc>,
    valid_until: DateTime<Utc>,
    actor_allowed: bool,
    subject_allowed: bool,
    inherited_block: bool,
}
impl fmt::Debug for BoundedObservation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BoundedObservation([REDACTED])")
    }
}
impl BoundedObservation {
    pub fn query(&self) -> PolicyQuery {
        self.query
    }
    pub fn revision(&self) -> RevisionVector {
        self.revision
    }
    pub fn auth_observed_at(&self) -> DateTime<Utc> {
        self.auth_observed_at
    }
    pub fn friend_observed_at(&self) -> DateTime<Utc> {
        self.friend_observed_at
    }
    pub fn valid_until(&self) -> DateTime<Utc> {
        self.valid_until
    }
    pub fn actor_allowed(&self) -> bool {
        self.actor_allowed
    }
    pub fn subject_allowed(&self) -> bool {
        self.subject_allowed
    }
    pub fn inherited_block(&self) -> bool {
        self.inherited_block
    }
    /// A fresh read must supply `current`; comparing an old vector with itself
    /// does not observe revocation. This result is never mutation permission.
    pub fn ensure_current(
        &self,
        binding: &RequestBinding,
        current: RevisionVector,
        now: DateTime<Utc>,
    ) -> Result<(), RelationshipError> {
        binding.check()?;
        if self.query != binding.query
            || self.family != binding.family
            || self.request != binding.request
            || self.revision != current
            || now < self.auth_observed_at
            || now < self.friend_observed_at
            || now >= self.valid_until
        {
            return Err(RelationshipError::Stale);
        }
        Ok(())
    }
    /// Only the public/presence projection side can inspect this allow. Local
    /// event membership, pair restrictions and field privacy are checked elsewhere.
    pub fn permits_public_projection(
        &self,
        binding: &RequestBinding,
        current: RevisionVector,
        now: DateTime<Utc>,
    ) -> Result<(), RelationshipError> {
        self.ensure_current(binding, current, now)?;
        if !matches!(self.query.field, Field::PublicProfile | Field::Presence)
            || !self.actor_allowed
            || !self.subject_allowed
            || self.inherited_block
        {
            return Err(RelationshipError::Denied);
        }
        Ok(())
    }
}
#[async_trait::async_trait]
pub trait BoundedRelationshipAuthority: Send + Sync {
    async fn observe(
        &self,
        binding: &RequestBinding,
    ) -> Result<BoundedObservation, RelationshipError>;
}
