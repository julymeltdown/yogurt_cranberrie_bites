//! Sealed, read-only Presence observations. Never a distributed publication fence.
//! Runtime composition retains the parent operation and separately invalidates/fences
//! publication. No browser actor, generated scalar stamp, or permissive default exists.
mod local;
use crate::adapters::out::relationship_authority::{
    grpc::GrpcRelationshipAuthority, world_projection::WorldPresenceProjection,
    BoundedRelationshipAuthority, RequestBinding, RevisionVector,
};
use chrono::{DateTime, Utc};
use concert_application::world_ticket::WorldError;
use concert_domain::relationship::{PolicyQuery, RelationshipError};
pub(crate) use local::LocalRevision;
#[cfg(test)]
pub(crate) use local::probe_visible_in as probe_presence_visible_in;
use sqlx::PgPool;
use uuid::Uuid;

/// Actual signed Auth/Friend client plus Concert-owned local policy.
/// Only server composition can supply the sealed WorldPresenceProjection.
pub(crate) struct WorldRecipientReader {
    pool: PgPool,
    owners: GrpcRelationshipAuthority,
    local_slots: std::sync::Arc<tokio::sync::Semaphore>,
}
impl WorldRecipientReader {
    pub(crate) fn new(pool: PgPool, owners: GrpcRelationshipAuthority) -> Self {
        // Each local read reserves a data and cancellation-control connection.
        let slots = (pool.options().get_max_connections() / 2).max(1) as usize;
        Self {
            pool,
            owners,
            local_slots: std::sync::Arc::new(tokio::sync::Semaphore::new(slots)),
        }
    }
    /// One candidate per call. Distinct room candidates share the parent's original
    /// stop/cancellation; repeated queries cannot reset the owner-attempt gates.
    pub(crate) async fn read(
        &self,
        context: &WorldPresenceProjection,
    ) -> Result<RecipientObservation, WorldError> {
        context.check()?;
        let binding = RequestBinding::from_world(context).map_err(owner_error)?;
        let auth = context.begin_auth_read()?;
        let friend = context.begin_friend_read()?;
        // Reserve BOTH before doing any I/O, including when either gate was burned.
        auth.check()?;
        friend.check()?;
        let stop = context.deadline().stop();
        let read = auth.run(friend.run(self.read_inner(context, &binding)));
        tokio::time::timeout_at(stop, read)
            .await
            .map_err(|_| WorldError::Unavailable)?
    }
    async fn read_inner(
        &self,
        context: &WorldPresenceProjection,
        binding: &RequestBinding,
    ) -> Result<RecipientObservation, WorldError> {
        // No Concert lock is held during remote owner I/O.
        let remote = self.owners.observe(binding).await.map_err(owner_error)?;
        context.check()?;
        let actor = context
            .parent()
            .current_membership()
            .ok_or(WorldError::Denied)?;
        let subject = context.candidate();
        let slot = self
            .local_slots
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| WorldError::Unavailable)?;
        let local = local::read(
            &self.pool,
            binding.query(),
            actor,
            subject,
            context.deadline(),
            slot,
        )
        .await
        .map_err(owner_error)?;
        context.check()?;
        let now = Utc::now();
        let until = remote
            .valid_until()
            .min(local.valid_until)
            .min(context.credential_expires_at())
            .min(context.source_expires_at())
            .min(
                DateTime::from_timestamp_micros(context.deadline().expires_micros())
                    .ok_or(WorldError::Unavailable)?,
            );
        if remote.query() != binding.query()
            || now < remote.auth_observed_at()
            || now < remote.friend_observed_at()
            || now < local.checked_at
            || until <= now
        {
            return Err(WorldError::Unavailable);
        }
        remote.revision().auth.validate().map_err(owner_error)?;
        remote.revision().friend.validate().map_err(owner_error)?;
        if !remote.actor_allowed() {
            return Err(WorldError::Denied);
        }
        Ok(RecipientObservation {
            query: binding.query(),
            family: binding.session_family(),
            request: binding.request_id(),
            actor: actor.clone(),
            candidate: subject.clone(),
            owners: remote.revision(),
            local: local.revision,
            checked_at: now,
            valid_until: until,
            visible: local.visible && remote.subject_allowed() && !remote.inherited_block(),
        })
    }
}
fn owner_error(error: RelationshipError) -> WorldError {
    match error {
        RelationshipError::Denied => WorldError::Denied,
        _ => WorldError::Unavailable,
    }
}
/// Independent owner and local revisions describe observed state, not a permission
/// to publish after revocation. Deliberately no scalar conversion or public builder.
pub(crate) struct RecipientObservation {
    query: PolicyQuery,
    family: Uuid,
    request: Uuid,
    actor: super::CurrentOwnedMembership,
    candidate: super::CurrentOwnedMembership,
    owners: RevisionVector,
    local: LocalRevision,
    checked_at: DateTime<Utc>,
    valid_until: DateTime<Utc>,
    visible: bool,
}
impl std::fmt::Debug for RecipientObservation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RecipientObservation([REDACTED])")
    }
}
impl RecipientObservation {
    /// Scope/expiry check ONLY. This does not re-observe owner state or certify
    /// freshness against a later block, session revoke, owner restart or local edit.
    pub(crate) fn check_scope(&self, context: &WorldPresenceProjection) -> Result<(), WorldError> {
        context.check()?;
        let now = Utc::now();
        if self.query != context.query()
            || self.family != context.session_family_id()
            || self.request != context.request_id()
            || Some(&self.actor) != context.parent().current_membership()
            || &self.candidate != context.candidate()
            || now < self.checked_at
            || now >= self.valid_until
        {
            return Err(WorldError::Denied);
        }
        Ok(())
    }
    pub(crate) fn visible(&self) -> bool {
        self.visible
    }
    pub(crate) fn owner_revisions(&self) -> RevisionVector {
        self.owners
    }
    pub(crate) fn local_revisions(&self) -> LocalRevision {
        self.local
    }
    pub(crate) fn checked_at(&self) -> DateTime<Utc> {
        self.checked_at
    }
    pub(crate) fn valid_until(&self) -> DateTime<Utc> {
        self.valid_until
    }
}
#[cfg(test)]
#[path = "../../tests/world_recipient_owners/mod.rs"]
mod tests;
