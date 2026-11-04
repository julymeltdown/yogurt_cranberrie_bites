pub mod repository;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::{Clock, Operation, ParticipationPolicy, PolicyError, PolicyGrant};
use concert_domain::ParticipantKey;

pub struct SystemClock;
impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}
/// BE04 has not wired authenticated Auth/Profile authority. Never substitute demo approval.
pub struct UnavailablePolicy;
#[async_trait]
impl ParticipationPolicy for UnavailablePolicy {
    async fn authorize(
        &self,
        _: ParticipantKey,
        _: Operation,
        _: DateTime<Utc>,
    ) -> Result<PolicyGrant, PolicyError> {
        Err(PolicyError::Unavailable)
    }
}

pub mod identity;

pub mod media;

pub mod relationship_authority;
pub(crate) mod world_profile;

pub mod public_profile;

pub mod age_provider;
