//! Use cases depend only on Concert values and explicit ports, never adapters.
pub mod commands;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_domain::{
    AvatarSettings, Event, EventId, Participant, ParticipantKey, Preferences, Revision,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum StoreError {
    #[error("Concert persistence is unavailable")]
    Unavailable,
    #[error("Concert persistence returned invalid data")]
    InvalidData,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PolicyError {
    #[error("Participation is not authorized")]
    Denied,
    #[error("Participation policy is unavailable")]
    Unavailable,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CoreError {
    #[error("Resource is unavailable")]
    ResourceUnavailable,
    #[error("Participation is not authorized")]
    PermissionDenied,
    #[error("Required dependency is unavailable")]
    DependencyUnavailable,
    #[error("Revision changed")]
    VersionConflict,
    #[error("Event does not accept participant writes")]
    EventReadOnly,
}
impl From<StoreError> for CoreError {
    fn from(_: StoreError) -> Self {
        Self::DependencyUnavailable
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    ReadOwnParticipant,
    ReadOwnAvatar,
    WriteOwnPreferences,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PolicyGrant {
    pub key: ParticipantKey,
    pub operation: Operation,
}

pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}
#[async_trait]
pub trait ParticipationPolicy: Send + Sync {
    /// Implementations must resolve canonical identity and fail closed on unavailable authority.
    /// A caller's role/provider assertion is never a grant.
    async fn authorize(
        &self,
        key: ParticipantKey,
        operation: Operation,
        at: DateTime<Utc>,
    ) -> Result<PolicyGrant, PolicyError>;
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreferenceUpdate {
    Updated(Participant),
    Missing,
    Conflict,
    EventReadOnly,
}
#[async_trait]
pub trait CoreRepository: Send + Sync {
    async fn event(&self, id: EventId) -> Result<Option<Event>, StoreError>;
    async fn participant(&self, key: ParticipantKey) -> Result<Option<Participant>, StoreError>;
    async fn avatar(&self, key: ParticipantKey) -> Result<Option<AvatarSettings>, StoreError>;
    /// Implementation must atomically recheck event phase and expected revision.
    async fn update_preferences(
        &self,
        key: ParticipantKey,
        expected: Revision,
        preferences: Preferences,
    ) -> Result<PreferenceUpdate, StoreError>;
}

pub struct ConcertCore<R, C, P> {
    repository: R,
    clock: C,
    policy: P,
}
impl<R: CoreRepository, C: Clock, P: ParticipationPolicy> ConcertCore<R, C, P> {
    pub fn new(repository: R, clock: C, policy: P) -> Self {
        Self {
            repository,
            clock,
            policy,
        }
    }
    async fn authorize(&self, key: ParticipantKey, operation: Operation) -> Result<(), CoreError> {
        let grant = self
            .policy
            .authorize(key, operation, self.clock.now())
            .await
            .map_err(|error| match error {
                PolicyError::Denied => CoreError::PermissionDenied,
                PolicyError::Unavailable => CoreError::DependencyUnavailable,
            })?;
        if grant.key != key || grant.operation != operation {
            return Err(CoreError::PermissionDenied);
        }
        Ok(())
    }
    pub async fn own_participant(&self, key: ParticipantKey) -> Result<Participant, CoreError> {
        self.authorize(key, Operation::ReadOwnParticipant).await?;
        let value = self
            .repository
            .participant(key)
            .await?
            .ok_or(CoreError::ResourceUnavailable)?;
        if value.key != key {
            return Err(CoreError::DependencyUnavailable);
        }
        Ok(value)
    }
    pub async fn own_avatar(&self, key: ParticipantKey) -> Result<AvatarSettings, CoreError> {
        self.authorize(key, Operation::ReadOwnAvatar).await?;
        let value = self
            .repository
            .avatar(key)
            .await?
            .ok_or(CoreError::ResourceUnavailable)?;
        if value.key != key {
            return Err(CoreError::DependencyUnavailable);
        }
        Ok(value)
    }
    pub async fn change_preferences(
        &self,
        key: ParticipantKey,
        expected: Revision,
        preferences: Preferences,
    ) -> Result<Participant, CoreError> {
        self.authorize(key, Operation::WriteOwnPreferences).await?;
        match self
            .repository
            .update_preferences(key, expected, preferences)
            .await?
        {
            PreferenceUpdate::Updated(value)
                if value.key == key && expected.next().ok() == Some(value.revision) =>
            {
                Ok(value)
            }
            PreferenceUpdate::Updated(_) => Err(CoreError::DependencyUnavailable),
            PreferenceUpdate::Missing => Err(CoreError::ResourceUnavailable),
            PreferenceUpdate::Conflict => Err(CoreError::VersionConflict),
            PreferenceUpdate::EventReadOnly => Err(CoreError::EventReadOnly),
        }
    }
}

pub mod identity;
pub mod place;
pub mod bootstrap;
pub mod relationship;
