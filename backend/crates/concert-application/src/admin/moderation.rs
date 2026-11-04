//! BE39 pure command vocabulary. Event suspension never changes global Auth lifecycle.
use super::{event::reason, Action, Actor};
use crate::commands::{CanonicalValue, CommandError};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UserAction {
    Suspend,
    Unsuspend,
}
impl UserAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Suspend => "SUSPEND",
            Self::Unsuspend => "UNSUSPEND",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentAction {
    Hide,
    Restore,
}
impl ContentAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hide => "HIDE",
            Self::Restore => "RESTORE",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentKind {
    Post,
    Comment,
    Party,
    Giveaway,
    Intro,
}
impl ContentKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Post => "POST",
            Self::Comment => "COMMENT",
            Self::Party => "PARTY",
            Self::Giveaway => "GIVEAWAY",
            Self::Intro => "INTRO",
        }
    }
    pub fn parse(value: &str) -> Result<Self, CommandError> {
        match value {
            "POST" => Ok(Self::Post),
            "COMMENT" => Ok(Self::Comment),
            "PARTY" => Ok(Self::Party),
            "GIVEAWAY" => Ok(Self::Giveaway),
            "INTRO" => Ok(Self::Intro),
            _ => Err(CommandError::InvalidInput),
        }
    }
}
// No Debug: a moderation reason is private operational input, never a log value.
#[derive(Clone)]
pub enum Change {
    User {
        target: Uuid,
        expected_revision: i64,
        action: UserAction,
        expires_at: Option<DateTime<Utc>>,
        reason: String,
    },
    Content {
        kind: ContentKind,
        target: Uuid,
        expected_revision: i64,
        action: ContentAction,
        reason: String,
    },
}
impl Change {
    pub fn target(&self) -> Uuid {
        match self {
            Self::User { target, .. } | Self::Content { target, .. } => *target,
        }
    }
    pub fn revision(&self) -> i64 {
        match self {
            Self::User {
                expected_revision, ..
            }
            | Self::Content {
                expected_revision, ..
            } => *expected_revision,
        }
    }
    pub fn action(&self) -> Action {
        match self {
            Self::User { .. } => Action::ModerateUser,
            Self::Content { .. } => Action::ModerateContent,
        }
    }
    pub fn reason(&self) -> &str {
        match self {
            Self::User { reason, .. } | Self::Content { reason, .. } => reason,
        }
    }
    pub fn validate(&self, actor: Actor, now: DateTime<Utc>) -> Result<(), CommandError> {
        if actor.actor.is_nil()
            || actor.event.is_nil()
            || self.target().is_nil()
            || !(1..=concert_domain::MAX_REVISION).contains(&self.revision())
        {
            return Err(CommandError::InvalidInput);
        }
        reason(self.reason())?;
        if let Self::User {
            action: UserAction::Suspend,
            expires_at: Some(until),
            ..
        } = self
        {
            if *until <= now {
                return Err(CommandError::InvalidInput);
            }
        }
        Ok(())
    }
    pub fn canonical(&self) -> CanonicalValue {
        let mut fields = vec![
            (
                "expected_revision".into(),
                CanonicalValue::Integer(self.revision()),
            ),
            (
                "reason".into(),
                CanonicalValue::String(self.reason().into()),
            ),
        ];
        match self {
            Self::User {
                action, expires_at, ..
            } => {
                fields.push((
                    "action".into(),
                    CanonicalValue::String(action.as_str().into()),
                ));
                fields.push((
                    "expires_at".into(),
                    expires_at
                        .map(|d| {
                            CanonicalValue::String(
                                d.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true),
                            )
                        })
                        .unwrap_or(CanonicalValue::Null),
                ));
            }
            Self::Content {
                kind,
                target,
                action,
                ..
            } => {
                fields.push((
                    "target_type".into(),
                    CanonicalValue::String(kind.as_str().into()),
                ));
                fields.push((
                    "target_id".into(),
                    CanonicalValue::String(target.to_string()),
                ));
                fields.push((
                    "action".into(),
                    CanonicalValue::String(action.as_str().into()),
                ));
            }
        }
        CanonicalValue::Object(fields)
    }
    pub fn steps(&self) -> Vec<OwnerStep> {
        match self {
            Self::User {
                action: UserAction::Suspend,
                ..
            } => vec![
                OwnerStep::EventRestriction,
                OwnerStep::EventWorldTermination,
                OwnerStep::EventPrivateAuthorityRevocation,
            ],
            Self::User { .. } => vec![OwnerStep::EventRestriction],
            Self::Content {
                kind: ContentKind::Post | ContentKind::Comment,
                ..
            } => vec![OwnerStep::PostContent],
            Self::Content {
                kind: ContentKind::Party,
                ..
            } => vec![OwnerStep::PartyContent],
            Self::Content {
                kind: ContentKind::Giveaway,
                ..
            } => vec![OwnerStep::GiveawayContent],
            Self::Content {
                kind: ContentKind::Intro,
                ..
            } => vec![OwnerStep::IntroContent],
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnerStep {
    EventRestriction,
    EventWorldTermination,
    EventPrivateAuthorityRevocation,
    PostContent,
    PartyContent,
    GiveawayContent,
    IntroContent,
}
impl OwnerStep {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EventRestriction => "EVENT_RESTRICTION",
            Self::EventWorldTermination => "EVENT_WORLD_TERMINATION",
            Self::EventPrivateAuthorityRevocation => "EVENT_PRIVATE_AUTHORITY_REVOCATION",
            Self::PostContent => "POST_CONTENT",
            Self::PartyContent => "PARTY_CONTENT",
            Self::GiveawayContent => "GIVEAWAY_CONTENT",
            Self::IntroContent => "INTRO_CONTENT",
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn actor() -> Actor {
        Actor {
            actor: Uuid::new_v4(),
            event: Uuid::new_v4(),
        }
    }
    #[test]
    fn suspension_is_event_local_and_unsuspend_cannot_restore_old_authority() {
        let mut c = Change::User {
            target: Uuid::new_v4(),
            expected_revision: 1,
            action: UserAction::Suspend,
            expires_at: None,
            reason: "Reviewed abuse".into(),
        };
        assert_eq!(
            c.steps(),
            vec![
                OwnerStep::EventRestriction,
                OwnerStep::EventWorldTermination,
                OwnerStep::EventPrivateAuthorityRevocation
            ]
        );
        if let Change::User { action, .. } = &mut c {
            *action = UserAction::Unsuspend;
        }
        assert_eq!(c.steps(), vec![OwnerStep::EventRestriction]);
    }
    #[test]
    fn required_reason_revision_and_future_suspension_expiry() {
        let now = Utc::now();
        let mut c = Change::User {
            target: Uuid::new_v4(),
            expected_revision: 1,
            action: UserAction::Suspend,
            expires_at: Some(now),
            reason: "reason".into(),
        };
        assert_eq!(c.validate(actor(), now), Err(CommandError::InvalidInput));
        if let Change::User {
            expires_at, reason, ..
        } = &mut c
        {
            *expires_at = None;
            *reason = " ".into();
        }
        assert_eq!(c.validate(actor(), now), Err(CommandError::InvalidInput));
        if let Change::User { reason, .. } = &mut c {
            *reason = "r".repeat(500);
        }
        assert!(c.validate(actor(), now).is_ok());
    }
    #[test]
    fn closed_actions_preserve_operator_only_audit() {
        assert!(Action::ModerateUser.permits(super::super::Role::Moderator));
        assert!(!Action::AuditRead.permits(super::super::Role::Moderator));
        assert!(Action::AuditRead.permits(super::super::Role::Operator));
        assert_eq!(
            Action::ModerateContent.operation(),
            "moderateConcertContent"
        );
    }
}
