//! Trusted owner interfaces. Absent adapters do not imply success or authority.
use crate::adapters::out::{
    relationship_authority::guarded::{OwnerGuardScope, ProducerTransaction},
    repository::event_operations::FencedOperationsAuthority,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::{
    admin::{
        moderation::{Change, OwnerStep},
        Actor, AuthorityQuery, Role, RoleLease,
    },
    commands::{CommandError, SafeFailure},
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::PgConnection;
use std::sync::Arc;
use uuid::Uuid;

pub struct TargetRoleLease {
    pub event: Uuid,
    pub target: Uuid,
    pub role: Role,
    pub revision: i64,
    pub checked_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}
/// Implementations must be acquired from the real event-role owner, retain its
/// revocation/disclosure fence in Root through HTTP terminal, and bind this exact
/// canonical scope. A short transaction snapshot, JWT/header or synthetic role
/// cannot implement production authority. No default provider exists.
#[async_trait]
pub trait EventRoleFence: FencedOperationsAuthority {
    fn root_binding_digest(&self) -> [u8; 32];
    async fn target_role_in(
        &self,
        c: &mut PgConnection,
        actor: Actor,
        target: Uuid,
        now: DateTime<Utc>,
    ) -> Result<TargetRoleLease, CommandError>;
    async fn check_disclosure(&self) -> Result<(), CommandError>;
}
#[async_trait]
pub trait EventRoleProvider: Send + Sync {
    /// MUST retain the returned owner fence under Root's terminal lifecycle,
    /// including cancellation/lost responses; dispatch return is not terminal.
    async fn acquire_for_root_terminal(
        &self,
        scope: Arc<OwnerGuardScope>,
    ) -> Result<Arc<dyn EventRoleFence>, CommandError>;
}
#[async_trait]
pub trait ModerationAuthority: Send + Sync {
    /// Production adapters expose the actual acquired scope. None is for internal
    /// fixture authorities only; real owner adapters must reject its absence.
    fn canonical_scope(&self) -> Option<Arc<OwnerGuardScope>> {
        None
    }
    async fn check_in(
        &self,
        c: &mut PgConnection,
        q: AuthorityQuery,
        now: DateTime<Utc>,
    ) -> Result<RoleLease, CommandError>;
    async fn target_role_in(
        &self,
        c: &mut PgConnection,
        actor: Actor,
        target: Uuid,
        now: DateTime<Utc>,
    ) -> Result<TargetRoleLease, CommandError>;
    async fn check_disclosure(&self) -> Result<(), CommandError>;
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnerIntent {
    pub receipt: Uuid,
    pub actor: Actor,
    pub target: Uuid,
    pub operation: String,
    pub step: OwnerStep,
    pub expected_revision: i64,
}
impl OwnerIntent {
    pub fn digest(&self) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(b"MCR-moderation-owner-intent-v1\0");
        for value in [
            self.receipt.as_bytes().as_slice(),
            self.actor.actor.as_bytes(),
            self.actor.event.as_bytes(),
            self.target.as_bytes(),
            self.operation.as_bytes(),
            self.step.as_str().as_bytes(),
            &self.expected_revision.to_be_bytes(),
        ] {
            h.update((value.len() as u64).to_be_bytes());
            h.update(value);
        }
        h.finalize().into()
    }
}
/// APPLIED must carry the actual owning transaction identity and durable owner
/// receipt for exactly this intent. No fabricated tx IDs or peer-store writes.
pub enum OwnerOutcome {
    Applied {
        intent: OwnerIntent,
        owner_receipt: Uuid,
        revision: i64,
        transaction: ProducerTransaction,
    },
    Failed {
        intent: OwnerIntent,
        reason: SafeFailure,
    },
    Pending {
        intent: OwnerIntent,
    },
}
#[async_trait]
pub trait ModerationOwner: Send + Sync {
    /// Owner enforces current event/target/CAS/eligibility and role fence. Event
    /// suspension advances Concert admission epoch; World/private authority is
    /// event scoped. Never mutate Auth refresh families/global account state.
    async fn apply(
        &self,
        authority: &dyn ModerationAuthority,
        intent: &OwnerIntent,
        change: &Change,
    ) -> Result<OwnerOutcome, CommandError>;
    /// Resolve durable native identity/status first. Resume only under the
    /// owner's reviewed identical-intent idempotency policy; uncertainty remains
    /// PENDING. An HTTP error/absence is never proof of nonexecution.
    async fn recover(
        &self,
        authority: &dyn ModerationAuthority,
        intent: &OwnerIntent,
        change: &Change,
    ) -> Result<OwnerOutcome, CommandError>;
    /// Current authorized public projection only, never a saved response body.
    async fn review(
        &self,
        authority: &dyn ModerationAuthority,
        actor: Actor,
        target: Uuid,
    ) -> Result<Value, CommandError>;
}
#[derive(Clone, Default)]
pub struct ModerationOwners {
    pub event_restriction: Option<Arc<dyn ModerationOwner>>,
    pub event_world_termination: Option<Arc<dyn ModerationOwner>>,
    pub event_private_authority_revocation: Option<Arc<dyn ModerationOwner>>,
    pub post_content: Option<Arc<dyn ModerationOwner>>,
    pub party_content: Option<Arc<dyn ModerationOwner>>,
    pub giveaway_content: Option<Arc<dyn ModerationOwner>>,
    pub intro_content: Option<Arc<dyn ModerationOwner>>,
    pub directory: Option<Arc<dyn ModerationDirectory>>,
}
impl ModerationOwners {
    pub fn port(&self, step: OwnerStep) -> Result<&Arc<dyn ModerationOwner>, CommandError> {
        match step {
            OwnerStep::EventRestriction => &self.event_restriction,
            OwnerStep::EventWorldTermination => &self.event_world_termination,
            OwnerStep::EventPrivateAuthorityRevocation => &self.event_private_authority_revocation,
            OwnerStep::PostContent => &self.post_content,
            OwnerStep::PartyContent => &self.party_content,
            OwnerStep::GiveawayContent => &self.giveaway_content,
            OwnerStep::IntroContent => &self.intro_content,
        }
        .as_ref()
        .ok_or(CommandError::Unavailable)
    }
}
#[async_trait]
pub trait ModerationDirectory: Send + Sync {
    /// Safe C04 UserReview/ContentReview rows, each reauthorized. No evidence,
    /// reporter identities, handles, private contents or secrets. Enforce the
    /// supplied immutable snapshot, authenticated filters and order bounds.
    async fn page_in(
        &self,
        authority: &dyn ModerationAuthority,
        c: &mut PgConnection,
        actor: Actor,
        operation: &str,
        query: &super::wire::Query,
        after: Option<&str>,
        snapshot: DateTime<Utc>,
    ) -> Result<Vec<Value>, CommandError>;
}
