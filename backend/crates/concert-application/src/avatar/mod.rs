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
#[async_trait]
pub trait AvatarEffectFence: Send + Sync {
    async fn register(&self, transaction: AvatarTransactionIdentity) -> Result<(), AvatarError>;
    async fn check(&self) -> Result<(), AvatarError>;
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
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
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
