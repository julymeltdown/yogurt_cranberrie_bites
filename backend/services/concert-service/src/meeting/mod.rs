//! BE09/BE19 internal adapters. Public routing and production authority composition are separate.
pub mod beacon;
pub mod account_deletion;
pub mod commands;

use async_trait::async_trait;
use concert_application::beacon::{
    AudienceGrant, AudienceQuery, MeetingActor, MeetingError, MeetingOperation, PartyFence,
};
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
    /// Lock all referenced party roots in a stable order before any pair or
    /// command-admission lock. No audience/private permission is granted here.
    async fn lock_audiences_in(
        &self,
        c: &mut PgConnection,
        actor: MeetingActor,
        _: &[concert_domain::beacon::Audience],
    ) -> Result<(), MeetingError> {
        self.actor_in(c, actor).await
    }
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
) -> Result<Option<PartyFence>, MeetingError> {
    match a.authorize_in(c, q.clone()).await {
        Ok(g) if g.query == q => {
            let valid = match (&q.audience, g.party) {
                (_, None) if q.operation == MeetingOperation::Revoke => true,
                (concert_domain::beacon::Audience::ExplicitUsers { .. }, None) => {
                    q.expected_party.is_none()
                }
                (concert_domain::beacon::Audience::PartyMembers { party_id }, Some(f)) => {
                    f.party == *party_id
                        && (1..=concert_domain::MAX_REVISION).contains(&f.meeting_revision)
                        && (1..=concert_domain::MAX_REVISION).contains(&f.audience_revision)
                        && q.expected_party.is_none_or(|expected| expected == f)
                }
                _ => false,
            };
            if valid {
                Ok(g.party)
            } else {
                Err(MeetingError::ResourceUnavailable)
            }
        }
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

pub mod rally;

pub mod authority;

/// Check the concrete owning policy's pinned earliest deadline after all blocking
/// read work. Missing settings are not a grant: custom authority remains obliged
/// to provide its own qualified through-commit fence.
pub(crate) async fn ensure_authority_current(c: &mut PgConnection) -> Result<(), MeetingError> {
    let fresh:bool=sqlx::query_scalar("select coalesce(nullif(current_setting('mcr.relationship_authority_until',true),'')::timestamptz>clock_timestamp(),true)")
        .fetch_one(c).await.map_err(storage)?;
    if fresh {
        Ok(())
    } else {
        Err(MeetingError::PolicyUnavailable)
    }
}
