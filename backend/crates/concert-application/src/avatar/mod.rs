//! BE07 authority/repository boundary. Default-deny until a current verified
//! server adapter supplies identity, operation, release and mutation capability.
use async_trait::async_trait;
use concert_domain::{
    avatar::{AvatarCatalog, SetAvatar},
    AvatarSettings, Participant, ParticipantKey, Preferences, Revision,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AvatarError {
    InvalidInput,
    Unauthenticated,
    Denied,
    Missing,
    Conflict,
    ReadOnly,
    Unavailable,
    InvalidState,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AvatarOperation {
    Read,
    SetAvatar,
    SetPreferences,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AvatarGrant {
    pub key: ParticipantKey,
    pub operation: AvatarOperation,
    pub release_digest: String,
}
/// Identity captured by the repository from its actual effect transaction.
/// Request/release binding and scope settlement remain the fence owner's duty.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AvatarTransactionIdentity {
    pub system_identifier: String,
    pub database_oid: u32,
    pub full_xid: String,
    pub backend_pid: i32,
    pub backend_started: chrono::DateTime<chrono::Utc>,
}
/// Correlation data from the already-held participant scope, not authority or
/// proof of commit. Only a witness inserted by the repository in its actual
/// effect transaction can establish the durable outcome.
#[derive(Clone, PartialEq, Eq)]
pub struct AvatarEffectWitnessBinding {
    pub producer_instance_id: uuid::Uuid,
    pub scope_id: uuid::Uuid,
    pub operation_id: uuid::Uuid,
    pub session_family_id: uuid::Uuid,
    pub parent_digest: [u8; 32],
    pub guard_binding_digest: [u8; 32],
    pub body_digest: [u8; 32],
    pub release_digest: String,
    pub effect_binding_digest: [u8; 32],
}
impl std::fmt::Debug for AvatarEffectWitnessBinding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AvatarEffectWitnessBinding")
            .finish_non_exhaustive()
    }
}
impl AvatarEffectWitnessBinding {
    pub fn validate_for_release(&self, expected: &str) -> Result<(), AvatarError> {
        if self.producer_instance_id.is_nil()
            || self.scope_id.is_nil()
            || self.operation_id.is_nil()
            || self.session_family_id.is_nil()
            || self.release_digest != expected
            || !valid_release_digest(&self.release_digest)
            || [
                self.parent_digest,
                self.guard_binding_digest,
                self.body_digest,
                self.effect_binding_digest,
            ]
            .contains(&[0; 32])
        {
            return Err(AvatarError::Unavailable);
        }
        Ok(())
    }
}
#[async_trait]
pub trait AvatarEffectFence: Send + Sync {
    async fn register(&self, transaction: AvatarTransactionIdentity) -> Result<(), AvatarError>;
    async fn check(&self) -> Result<(), AvatarError>;
    /// Supplies the original immutable scope/request/effect correlation only.
    /// The default cannot certify a successful production mutation.
    fn witness_binding(&self) -> Result<AvatarEffectWitnessBinding, AvatarError> {
        Err(AvatarError::Unavailable)
    }
}
/// Implementations must check a verified canonical actor, exact event/HTTP
/// target, live bounded authority, current release and server capability.
/// Called after every await and immediately before commit. Client booleans,
/// bootstrap read assertions and fixture metadata alone cannot implement this.
pub trait AvatarAuthority: Send + Sync {
    fn check(&self) -> Result<AvatarGrant, AvatarError>;
    /// Product writes require a real held-scope fence. The default preserves
    /// read-only and explicitly synthetic fixture implementations.
    fn effect_fence(&self) -> Option<&dyn AvatarEffectFence> {
        None
    }
}
pub fn authorize(
    authority: &dyn AvatarAuthority,
    key: ParticipantKey,
    op: AvatarOperation,
    digest: &str,
) -> Result<(), AvatarError> {
    if key.event_id.uuid().is_nil() || key.user_id.uuid().is_nil() {
        return Err(AvatarError::Denied);
    }
    if !valid_release_digest(digest) {
        return Err(AvatarError::Unavailable);
    }
    let grant = authority.check()?;
    if grant.key != key || grant.operation != op {
        return Err(AvatarError::Denied);
    }
    if grant.release_digest != digest {
        return Err(AvatarError::Unavailable);
    }
    Ok(())
}
fn valid_release_digest(digest: &str) -> bool {
    digest.len() == 64
        && digest
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AvatarSnapshot {
    pub avatar: AvatarSettings,
    pub participant: Participant,
}
#[derive(Clone, Debug)]
pub enum AvatarMutation {
    Read,
    Avatar(SetAvatar),
    Preferences {
        expected: Revision,
        value: Preferences,
    },
}
impl AvatarMutation {
    pub fn operation(&self) -> AvatarOperation {
        match self {
            Self::Read => AvatarOperation::Read,
            Self::Avatar(_) => AvatarOperation::SetAvatar,
            Self::Preferences { .. } => AvatarOperation::SetPreferences,
        }
    }
}
#[async_trait]
pub trait AvatarRepository: Send + Sync {
    /// Lock event -> admission -> participant -> avatar. Recheck current
    /// authority/release after waits, reject missing/suspended admissions, and
    /// validate both persisted and resulting selections before committing.
    async fn execute(
        &self,
        key: ParticipantKey,
        mutation: AvatarMutation,
        catalog: &AvatarCatalog,
        digest: &str,
        authority: &dyn AvatarAuthority,
    ) -> Result<AvatarSnapshot, AvatarError>;
}
