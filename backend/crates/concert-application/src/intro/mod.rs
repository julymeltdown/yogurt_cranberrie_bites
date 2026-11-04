//! BE27 pure owner command and viewer projection contracts.
//! BE25 remains the only age/introduction visibility policy.
//!
//! Publication/contact semantics adapt Bigyeting source revision
//! `5eecdcaffd1a9d505b9ab2a974f8d5623217e90a`
//! `application/use_cases/social_intro_policy.rs`. Bigyeting identity, rendering,
//! provider, media and request flows are not copied.
use crate::commands::{
    CanonicalRequest, CanonicalValue as V, CommandError, RegisteredOperation, Scope,
};
use chrono::{DateTime, SecondsFormat, Utc};
use concert_domain::intro_policy::IntroState;
use concert_domain::{AuthUserId, EventId, Revision};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub const INTRO_TEXT_MAX: usize = 1000;
pub const INTRO_QUERY_MAX: usize = 100;
pub const INTRO_PAGE_MAX: u16 = 100;
pub const INTRO_CURSOR_MAX: usize = 1024;
pub const INTRO_PUBLIC_PROFILE_MAX: usize = 16_384;
pub const INTRO_CURSOR_DOMAIN: &[u8] = b"mcr-intro-cursor-v1:";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    pub created_at: DateTime<Utc>,
    pub intro_id: Uuid,
}
impl Position {
    pub fn validate(self) -> Result<Self, IntroError> {
        if self.intro_id.is_nil() {
            Err(IntroError::Invalid)
        } else {
            Ok(self)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum IntroError {
    #[error("Invalid introduction input")]
    Invalid,
    #[error("Introduction not found")]
    NotFound,
    #[error("Introduction authorization denied")]
    Denied,
    #[error("Introduction revision conflict")]
    Conflict,
    #[error("Introduction dependency unavailable")]
    Unavailable,
    #[error("Event is read-only")]
    ReadOnly,
    #[error("Introduction idempotency key expired")]
    Expired,
}

#[derive(Clone)]
pub struct IntroDraft {
    pub text: String,
    /// Closed server representation `null:null` or `min:max`; numeric limits
    /// remain usable by the repository while never entering public DTOs.
    pub age_bounds: String,
    pub expires_at: Option<DateTime<Utc>>,
}
pub struct DraftInput {
    pub text: String,
    pub minimum_age: Option<i16>,
    pub maximum_age_exclusive: Option<i16>,
    pub expires_at: Option<DateTime<Utc>>,
}
impl std::fmt::Debug for DraftInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DraftInput([REDACTED])")
    }
}
impl DraftInput {
    pub fn validate(self) -> Result<IntroDraft, IntroError> {
        self.validate_at(Utc::now())
    }
    pub fn validate_at(self, now: DateTime<Utc>) -> Result<IntroDraft, IntroError> {
        let minimum = self.minimum_age;
        let maximum = self.maximum_age_exclusive;
        let owner_bounds = match (minimum, maximum) {
            (None, None) => "null:null".to_owned(),
            (Some(a), Some(b)) if (14..=100).contains(&a) && (15..=101).contains(&b) && a < b => {
                format!("{a}:{b}")
            }
            _ => return Err(IntroError::Invalid),
        };
        let draft = IntroDraft {
            text: self.text,
            age_bounds: owner_bounds,
            expires_at: self.expires_at,
        };
        draft.validate_at(now)?;
        Ok(draft)
    }
}
impl std::fmt::Debug for IntroDraft {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("IntroDraft([REDACTED])")
    }
}
impl IntroDraft {
    pub fn validate(&self) -> Result<(), IntroError> {
        self.validate_at(Utc::now())
    }
    pub fn validate_at(&self, now: DateTime<Utc>) -> Result<(), IntroError> {
        if self.text.chars().count() > INTRO_TEXT_MAX
            || self.text.contains('\0')
            || self.text.chars().any(|c| c == '\r')
            || !valid_bounds(&self.age_bounds)
            || self.expires_at.is_some_and(|value| value <= now)
        {
            return Err(IntroError::Invalid);
        }
        Ok(())
    }
}
fn valid_bounds(value: &str) -> bool {
    let Some((minimum, maximum)) = value.split_once(':') else {
        return false;
    };
    match (minimum, maximum) {
        ("null", "null") => true,
        (minimum, maximum) => matches!(
            (minimum.parse::<i16>(), maximum.parse::<i16>()),
            (Ok(a), Ok(b)) if (14..=100).contains(&a)
                && (15..=101).contains(&b)
                && a < b
        ),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExpectedRevision {
    Absent,
    Existing(Revision),
}
impl ExpectedRevision {
    pub fn from_input(value: i64) -> Result<Self, IntroError> {
        if value == 0 {
            Ok(Self::Absent)
        } else {
            Revision::new(value)
                .map(Self::Existing)
                .map_err(|_| IntroError::Invalid)
        }
    }
    pub fn stored(self) -> i64 {
        match self {
            Self::Absent => 0,
            Self::Existing(value) => value.get(),
        }
    }
    pub fn revision(self) -> Option<i64> {
        match self {
            Self::Absent => None,
            Self::Existing(value) => Some(value.get()),
        }
    }
}

/// Validated owner command. The actor/event/key/target are correlation, never
/// authority. The adapter must resolve current owner/session/policy in the same
/// database transaction.
#[derive(Clone, Debug)]
pub enum IntroCommand {
    Set {
        actor: AuthUserId,
        event: EventId,
        key: Uuid,
        intro: Uuid,
        expected: ExpectedRevision,
        draft: IntroDraft,
    },
    Publish {
        actor: AuthUserId,
        event: EventId,
        key: Uuid,
        intro: Uuid,
        expected: Revision,
    },
    Close {
        actor: AuthUserId,
        event: EventId,
        key: Uuid,
        intro: Uuid,
        expected: Revision,
    },
}
impl IntroCommand {
    pub fn set(
        actor: AuthUserId,
        event: EventId,
        key: Uuid,
        expected: i64,
        draft: IntroDraft,
    ) -> Result<Self, IntroError> {
        draft.validate_at(Utc::now())?;
        let expected = ExpectedRevision::from_input(expected)?;
        Ok(Self::Set {
            actor,
            event,
            key,
            intro: intro_id(event, actor),
            expected,
            draft,
        })
    }
    pub fn publish(actor: AuthUserId, event: EventId, key: Uuid, expected: Revision) -> Self {
        Self::Publish {
            actor,
            event,
            key,
            intro: intro_id(event, actor),
            expected,
        }
    }
    pub fn close(actor: AuthUserId, event: EventId, key: Uuid, expected: Revision) -> Self {
        Self::Close {
            actor,
            event,
            key,
            intro: intro_id(event, actor),
            expected,
        }
    }
    pub fn actor(&self) -> AuthUserId {
        match self {
            Self::Set { actor, .. } | Self::Publish { actor, .. } | Self::Close { actor, .. } => {
                *actor
            }
        }
    }
    pub fn event(&self) -> EventId {
        match self {
            Self::Set { event, .. } | Self::Publish { event, .. } | Self::Close { event, .. } => {
                *event
            }
        }
    }
    pub fn key(&self) -> Uuid {
        match self {
            Self::Set { key, .. } | Self::Publish { key, .. } | Self::Close { key, .. } => *key,
        }
    }
    pub fn intro(&self) -> Uuid {
        match self {
            Self::Set { intro, .. } | Self::Publish { intro, .. } | Self::Close { intro, .. } => {
                *intro
            }
        }
    }
    pub fn operation(&self) -> &'static str {
        match self {
            Self::Set { .. } => "setConcertIntro",
            Self::Publish { .. } => "publishConcertIntro",
            Self::Close { .. } => "closeConcertIntro",
        }
    }
    pub fn scope(&self) -> Result<Scope, CommandError> {
        Ok(Scope {
            actor: self.actor().uuid(),
            event: self.event().uuid(),
            key: self.key(),
            operation: RegisteredOperation::find(self.operation())?,
        })
    }
    pub fn request(&self) -> Result<CanonicalRequest, CommandError> {
        let event = self.event().uuid().to_string();
        let parameters = [("eventId", event.as_str())];
        let body = match self {
            Self::Set {
                expected, draft, ..
            } => V::Object(vec![
                ("age_bounds".into(), bounds_value(&draft.age_bounds)?),
                (
                    "expires_at".into(),
                    draft
                        .expires_at
                        .map(|v| V::String(stamp(v)))
                        .unwrap_or(V::Null),
                ),
                ("expected_revision".into(), V::Integer(expected.stored())),
                ("text".into(), V::String(draft.text.clone())),
            ]),
            Self::Publish { expected, .. } | Self::Close { expected, .. } => V::Object(vec![(
                "expected_revision".into(),
                V::Integer(expected.get()),
            )]),
        };
        CanonicalRequest::json(self.scope()?, &parameters, &[], Some(&body))
    }
}

fn intro_id(event: EventId, actor: AuthUserId) -> Uuid {
    let mut digest = Sha256::new();
    digest.update(b"mcr-intro-id-v1:");
    digest.update(event.uuid().as_bytes());
    digest.update(actor.uuid().as_bytes());
    let digest = digest.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}
fn stamp(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Micros, true)
}
fn bounds_value(value: &str) -> Result<V, CommandError> {
    let (minimum, maximum) = value.split_once(':').ok_or(CommandError::InvalidInput)?;
    let field = |value: &str| {
        if value == "null" {
            Ok(V::Null)
        } else {
            value
                .parse::<i64>()
                .map(V::Integer)
                .map_err(|_| CommandError::InvalidInput)
        }
    };
    Ok(V::Object(vec![
        ("maximum_age_exclusive".into(), field(maximum)?),
        ("minimum_age".into(), field(minimum)?),
    ]))
}

#[derive(Clone, PartialEq, Eq)]
pub struct PublicAuthor {
    pub user_id: AuthUserId,
    /// Exact validated C03 `PublicProfile` JSON bytes. Never a BE25 fragment.
    pub json: Vec<u8>,
}
impl std::fmt::Debug for PublicAuthor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PublicAuthor([REDACTED])")
    }
}

pub struct PublicForumIntro {
    pub intro_id: Uuid,
    pub event_id: Uuid,
    pub author: AuthUserId,
    pub text: String,
    pub created_at: DateTime<Utc>,
    pub revision: Revision,
}
pub fn validated_public(value: &PublicForumIntro, author: PublicAuthor) -> Result<(), IntroError> {
    if author.user_id != value.author
        || author.json.is_empty()
        || author.json.len() > INTRO_PUBLIC_PROFILE_MAX
        || value.text.trim().is_empty()
        || value.text.chars().count() > INTRO_TEXT_MAX
        || value.intro_id.is_nil()
        || value.event_id.is_nil()
    {
        return Err(IntroError::Unavailable);
    }
    Ok(())
}
pub struct OwnerIntro {
    pub intro_id: Uuid,
    pub event_id: Uuid,
    pub owner_id: Uuid,
    pub text: String,
    pub age_bounds: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub state: IntroState,
    pub revision: Revision,
}
impl OwnerIntro {
    pub fn validate_at(&self, now: DateTime<Utc>) -> Result<(), IntroError> {
        if self.intro_id.is_nil()
            || self.event_id.is_nil()
            || self.owner_id.is_nil()
            || self.text.chars().count() > INTRO_TEXT_MAX
            || self.text.contains('\0')
            || !valid_bounds(&self.age_bounds)
            || self.revision.get() < 1
            || self.created_at > now
            || self.expires_at.is_some_and(|value| value <= now)
            || (self.state != IntroState::Draft && self.text.trim().is_empty())
            || (self.state != IntroState::Draft
                && concert_domain::intro_policy::contact::contact_bypass(&self.text))
        {
            return Err(IntroError::Invalid);
        }
        Ok(())
    }
    pub fn validate(&self) -> Result<(), IntroError> {
        self.validate_at(Utc::now())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IntroReadContext {
    pub actor: AuthUserId,
    pub event: EventId,
    pub session: Uuid,
    pub request: Uuid,
    pub expires_at: DateTime<Utc>,
}
impl IntroReadContext {
    pub fn check(self, at: DateTime<Utc>) -> Result<(), IntroError> {
        if self.actor.uuid().is_nil()
            || self.event.uuid().is_nil()
            || self.session.is_nil()
            || self.request.is_nil()
            || at >= self.expires_at
        {
            Err(IntroError::Denied)
        } else {
            Ok(())
        }
    }
}

pub struct OwnerCommandResult {
    pub receipt_id: Uuid,
    pub state: &'static str,
    pub revision: Option<Revision>,
    pub owner: OwnerIntro,
}

pub mod cursor;
pub mod wire;
