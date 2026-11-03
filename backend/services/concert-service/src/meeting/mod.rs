//! BE09/BE19 internal adapters. Public routing and production authority composition are separate.
pub mod beacon;
pub mod commands;

use async_trait::async_trait;
use concert_application::beacon::{AudienceGrant, AudienceQuery, MeetingActor, MeetingError};
use sqlx::PgConnection;

#[async_trait]
pub trait MeetingAuthority: Send + Sync {
    /// Trusted request-bound adapter: verify canonical Auth/session/event eligibility,
    /// relationship restrictions, current audience membership and owner eligibility.
    /// All local policy rows (including absence/pair fences) remain locked through
    /// caller commit. External authority must be fenced, not a cached boolean.
    /// PARTY_MEMBERS reads require BOTH owner and actor still belong to that party.
    /// Create/update verify every selected recipient; reads verify only the viewer.
    /// Revoke verifies actor eligibility+ownership without requiring remaining audience.
    async fn authorize_in(
        &self,
        connection: &mut PgConnection,
        query: AudienceQuery,
    ) -> Result<AudienceGrant, MeetingError>;
    /// Receipt lookup/replay is freshly gated even when private guidance expired.
    async fn actor_in(
        &self,
        connection: &mut PgConnection,
        actor: MeetingActor,
    ) -> Result<(), MeetingError>;
}

pub(crate) async fn grant<A: MeetingAuthority>(
    a: &A,
    c: &mut PgConnection,
    q: AudienceQuery,
) -> Result<(), MeetingError> {
    match a.authorize_in(c, q.clone()).await {
        Ok(g) if g.query == q => Ok(()),
        Ok(_) => Err(MeetingError::ResourceUnavailable),
        Err(MeetingError::ResourceUnavailable) => Err(MeetingError::ResourceUnavailable),
        Err(_) => Err(MeetingError::PolicyUnavailable),
    }
}
pub(crate) fn storage(_: sqlx::Error) -> MeetingError {
    MeetingError::StorageUnavailable
}
pub(crate) fn invalid(_: impl std::fmt::Debug) -> MeetingError {
    MeetingError::InvalidState
}

/// Safe composition default. No caller can substitute cached session/membership claims.
pub struct UnavailableMeetingAuthority;
#[async_trait]
impl MeetingAuthority for UnavailableMeetingAuthority {
    async fn actor_in(&self, _: &mut PgConnection, _: MeetingActor) -> Result<(), MeetingError> {
        Err(MeetingError::PolicyUnavailable)
    }
    async fn authorize_in(
        &self,
        _: &mut PgConnection,
        _: AudienceQuery,
    ) -> Result<AudienceGrant, MeetingError> {
        Err(MeetingError::PolicyUnavailable)
    }
}
