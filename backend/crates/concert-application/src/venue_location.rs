//! VENU-BE-L01 use cases. Implementations must retain one transaction through the effect.
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_domain::venue_location::*;
use concert_domain::{AuthUserId, EventId, ParticipantKey};
use uuid::Uuid;

pub use concert_domain::venue_location as domain;

#[async_trait]
pub trait VenueLocationPolicyProvider: Send + Sync {
    async fn policy(&self, event: EventId) -> Result<VenueLocationPolicy, VenueLocationError>;
}

#[async_trait]
pub trait VenueLocation: Send + Sync {
    async fn set_sharing(
        &self,
        actor: ParticipantKey,
        command_key: Uuid,
        command: SharingCommand,
    ) -> Result<CommandReceipt, VenueLocationError>;
    async fn publish_fix(
        &self,
        actor: ParticipantKey,
        command_key: Uuid,
        input: PhoneFixInput,
    ) -> Result<CommandReceipt, VenueLocationError>;
    async fn snapshot(&self, actor: ParticipantKey)
        -> Result<LocationSnapshot, VenueLocationError>;
    async fn visible_for(
        &self,
        viewer: ParticipantKey,
        owner: AuthUserId,
    ) -> Result<VisibleLocation, VenueLocationError>;
}

#[derive(Clone, Debug, PartialEq)]
pub enum VenueLocationCommand {
    SetSharing(SharingCommand),
    PublishFix(PhoneFixInput),
}
impl VenueLocationCommand {
    pub fn operation(&self) -> &'static str {
        match self {
            Self::SetSharing(_) => "setVenueLocationSharing",
            Self::PublishFix(_) => "publishVenueLocationFix",
        }
    }
    pub fn validate(
        &self,
        now: DateTime<Utc>,
        policy: &VenueLocationPolicy,
    ) -> Result<(), VenueLocationError> {
        match self {
            Self::SetSharing(command) => command.validate(policy),
            Self::PublishFix(input) => input.validate(now, policy),
        }
    }
}

pub fn command_receipt(
    command: &VenueLocationCommand,
    key: Uuid,
    owner: AuthUserId,
    revision: i64,
    now: DateTime<Utc>,
    policy: &VenueLocationPolicy,
) -> Result<CommandReceipt, VenueLocationError> {
    let kind = match command {
        VenueLocationCommand::SetSharing(_) => "SHARING",
        VenueLocationCommand::PublishFix(_) => "PHONE_FIX",
    };
    receipt(key, command.operation(), kind, owner, revision, now, policy)
}
