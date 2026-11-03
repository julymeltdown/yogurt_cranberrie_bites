//! Internal operations authority values. None are request-provided role claims.
pub mod event;
pub mod notices;
pub mod places;
use crate::commands::CommandError;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Actor {
    pub actor: Uuid,
    pub event: Uuid,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Member,
    Moderator,
    Operator,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    CreateReport,
    OwnReports,
    ReviewReports,
    ReportDetail,
    SettingsRead,
    SettingsWrite,
    PhaseWrite,
    PlacesRead,
    PlaceWrite,
    NoticeRead,
    NoticeDetail,
    NoticeAdminRead,
    NoticeCreate,
    NoticeUpdate,
}
impl Action {
    pub fn operation(self) -> &'static str {
        match self {
            Self::CreateReport => "createConcertReport",
            Self::OwnReports => "listOwnConcertReports",
            Self::ReviewReports => "listConcertAdminReports",
            Self::ReportDetail => "getConcertAdminReport",
            Self::SettingsRead => "getConcertSettings",
            Self::SettingsWrite => "setConcertSettings",
            Self::PhaseWrite => "setConcertEventPhase",
            Self::PlacesRead => "listConcertAdminPlaces",
            Self::PlaceWrite => "setConcertPlace",
            Self::NoticeRead => "listConcertNotices",
            Self::NoticeDetail => "getConcertNotice",
            Self::NoticeAdminRead => "listConcertAdminNotices",
            Self::NoticeCreate => "createConcertNotice",
            Self::NoticeUpdate => "updateConcertNotice",
        }
    }
    pub fn permits(self, role: Role) -> bool {
        match self {
            Self::CreateReport | Self::OwnReports | Self::NoticeRead | Self::NoticeDetail => true,
            Self::ReviewReports | Self::ReportDetail => {
                matches!(role, Role::Moderator | Role::Operator)
            }
            _ => role == Role::Operator,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuthorityQuery {
    pub actor: Actor,
    pub action: Action,
    pub target: Uuid,
}
/// Owner must retain revocation/revision fences through the protected transaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoleLease {
    pub query: AuthorityQuery,
    pub role: Role,
    pub revision: i64,
    pub checked_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}
impl RoleLease {
    pub fn validate(
        self,
        query: AuthorityQuery,
        started: DateTime<Utc>,
        finished: DateTime<Utc>,
    ) -> Result<(), CommandError> {
        if self.query != query
            || self.revision < 1
            || self.revision > concert_domain::MAX_REVISION
            || self.checked_at < started
            || self.checked_at > finished
            || self.expires_at <= finished
        {
            return Err(CommandError::Unavailable);
        }
        if query.actor.actor.is_nil()
            || query.actor.event.is_nil()
            || !query.action.permits(self.role)
        {
            return Err(CommandError::Denied);
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grants_bind_scope_role_and_time() {
        let now = Utc::now();
        let q = AuthorityQuery {
            actor: Actor {
                actor: Uuid::new_v4(),
                event: Uuid::new_v4(),
            },
            action: Action::ReportDetail,
            target: Uuid::new_v4(),
        };
        let mut lease = RoleLease {
            query: q,
            role: Role::Member,
            revision: 1,
            checked_at: now,
            expires_at: now + chrono::Duration::seconds(1),
        };
        assert_eq!(lease.validate(q, now, now), Err(CommandError::Denied));
        lease.role = Role::Moderator;
        assert!(lease.validate(q, now, now).is_ok());
        let mut other = q;
        other.actor.event = Uuid::new_v4();
        assert_eq!(
            lease.validate(other, now, now),
            Err(CommandError::Unavailable)
        );
        assert_eq!(
            lease.validate(q, now, lease.expires_at),
            Err(CommandError::Unavailable)
        );
    }
}
