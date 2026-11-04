//! Canonical owner and mutation capability ports. No browser data mints a grant.
use super::{
    context::{VerifiedParticipantRequest, BUDGET},
    Operation,
};
use async_trait::async_trait;
use concert_application::avatar::{
    AvatarAuthority, AvatarEffectFence, AvatarError, AvatarSnapshot,
};
use concert_domain::ParticipantKey;
use uuid::Uuid;

pub struct CurrentAuth {
    pub user_id: Uuid,
    pub session_family_id: Uuid,
    pub observed_at_micros: i64,
    pub expires_micros: i64,
}
pub struct CurrentProfile {
    pub user_id: Uuid,
    pub display_name: String,
    pub bio: Option<String>,
    pub observed_at_micros: i64,
    pub expires_micros: i64,
}
fn freshness(
    observed: i64,
    expires: i64,
    request: &VerifiedParticipantRequest,
) -> Result<(), AvatarError> {
    request.check().map_err(|_| AvatarError::Unauthenticated)?;
    let now = chrono::Utc::now().timestamp_micros();
    if observed <= 0
        || observed > now
        || now.saturating_sub(observed) > BUDGET.as_micros() as i64
        || expires <= now
        || expires <= observed
        || expires.saturating_sub(observed) > BUDGET.as_micros() as i64
    {
        return Err(AvatarError::Unavailable);
    }
    Ok(())
}
impl CurrentAuth {
    pub(crate) fn check(&self, r: &VerifiedParticipantRequest) -> Result<(), AvatarError> {
        if self.user_id != r.key().user_id.uuid() || self.session_family_id != r.session_family_id()
        {
            return Err(AvatarError::Denied);
        }
        freshness(self.observed_at_micros, self.expires_micros, r)
    }
}
impl CurrentProfile {
    pub(crate) fn check(&self, r: &VerifiedParticipantRequest) -> Result<(), AvatarError> {
        if self.user_id != r.key().user_id.uuid() {
            return Err(AvatarError::Denied);
        }
        if self.display_name.trim().is_empty()
            || self.display_name.chars().count() > 40
            || self.display_name.chars().any(char::is_control)
            || self
                .bio
                .as_ref()
                .is_some_and(|v| v.chars().count() > 160 || v.chars().any(char::is_control))
        {
            return Err(AvatarError::InvalidState);
        }
        freshness(self.observed_at_micros, self.expires_micros, r)
    }
}
#[async_trait]
pub trait CanonicalAuthPort: Send + Sync {
    /// Return only after current canonical owner confirms active account AND
    /// live exact session family. A JWT/read signature alone is insufficient.
    async fn read(&self, request: &VerifiedParticipantRequest) -> Result<CurrentAuth, AvatarError>;
}
#[async_trait]
pub trait CanonicalProfilePort: Send + Sync {
    async fn read(
        &self,
        request: &VerifiedParticipantRequest,
    ) -> Result<CurrentProfile, AvatarError>;
}
pub trait MutationPermit: Send + Sync {
    /// Recheck exact actor,event,session,request,method,target,raw body digest,
    /// release, original stop AND current server mutation capability. Never
    /// implement this as a conversion from VerifiedReadAuthority or booleans.
    fn check(&self, request: &VerifiedParticipantRequest) -> Result<(), AvatarError>;
    /// The real permit owns the request/release-bound held-scope adapter. This
    /// remains absent for legacy fixtures; product persistence refuses it.
    fn effect_fence(&self) -> Option<&dyn AvatarEffectFence> {
        None
    }
}
#[async_trait]
pub trait MutationAuthorityPort: Send + Sync {
    async fn authorize(
        &self,
        request: &VerifiedParticipantRequest,
        auth: &CurrentAuth,
    ) -> Result<Box<dyn MutationPermit>, AvatarError>;
}
#[async_trait]
pub trait ParticipantStore: Send + Sync {
    fn release_digest(&self) -> &str;
    async fn execute(
        &self,
        key: ParticipantKey,
        operation: Operation,
        bytes: &[u8],
        authority: &dyn AvatarAuthority,
    ) -> Result<StoreResult, AvatarError>;
}
pub enum StoreResult {
    Snapshot(AvatarSnapshot),
    Avatar(serde_json::Value),
}
#[async_trait]
impl ParticipantStore for crate::adapters::out::repository::avatar::AvatarPersistence {
    fn release_digest(&self) -> &str {
        self.release_digest()
    }
    async fn execute(
        &self,
        key: ParticipantKey,
        operation: Operation,
        bytes: &[u8],
        authority: &dyn AvatarAuthority,
    ) -> Result<StoreResult, AvatarError> {
        match operation {
            Operation::GetParticipant => self.read(key, authority).await.map(StoreResult::Snapshot),
            Operation::SetAvatar => self
                .set_avatar(key, bytes, authority)
                .await
                .map(StoreResult::Avatar),
            Operation::UpdatePreferences => self
                .set_preferences(key, bytes, authority)
                .await
                .map(StoreResult::Snapshot),
        }
    }
}
pub struct Unavailable;
#[async_trait]
impl CanonicalAuthPort for Unavailable {
    async fn read(&self, _: &VerifiedParticipantRequest) -> Result<CurrentAuth, AvatarError> {
        Err(AvatarError::Unavailable)
    }
}
#[async_trait]
impl CanonicalProfilePort for Unavailable {
    async fn read(&self, _: &VerifiedParticipantRequest) -> Result<CurrentProfile, AvatarError> {
        Err(AvatarError::Unavailable)
    }
}
#[async_trait]
impl MutationAuthorityPort for Unavailable {
    async fn authorize(
        &self,
        _: &VerifiedParticipantRequest,
        _: &CurrentAuth,
    ) -> Result<Box<dyn MutationPermit>, AvatarError> {
        Err(AvatarError::Unavailable)
    }
}
#[async_trait]
impl ParticipantStore for Unavailable {
    fn release_digest(&self) -> &str {
        ""
    }
    async fn execute(
        &self,
        _: ParticipantKey,
        _: Operation,
        _: &[u8],
        _: &dyn AvatarAuthority,
    ) -> Result<StoreResult, AvatarError> {
        Err(AvatarError::Unavailable)
    }
}
