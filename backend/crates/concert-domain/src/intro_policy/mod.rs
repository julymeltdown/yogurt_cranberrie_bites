//! Pure introduction rules. Canonical Auth UUIDs replace old member/X identities.
pub mod contact;
use crate::age_policy::{AgeAssessment, AgeBounds};
use crate::{AuthUserId, EventId, Revision};
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum IntroError {
    #[error("Introduction input is invalid")]
    Invalid,
    #[error("Resource is unavailable")]
    Unavailable,
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IntroState {
    Draft,
    Published,
    Closed,
}
/// Internal owner data, not a public DTO or authorization proof.
#[derive(Clone)]
pub struct IntroRecord {
    pub intro_id: Uuid,
    pub event_id: EventId,
    pub owner_id: AuthUserId,
    pub text: String,
    pub age_bounds: AgeBounds,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub state: IntroState,
    pub revision: Revision,
}
impl IntroRecord {
    pub fn validate(
        &self,
        now: DateTime<Utc>,
        event_cutoff: DateTime<Utc>,
        publishing: bool,
    ) -> Result<(), IntroError> {
        if self.intro_id.is_nil()
            || self.event_id.uuid().is_nil()
            || self.owner_id.uuid().is_nil()
            || self.created_at > now
            || event_cutoff <= now
            || self.text.chars().count() > 1000
            || self.text.contains('\0')
            || self
                .expires_at
                .is_some_and(|e| e <= now || e > event_cutoff)
            || (publishing || self.state != IntroState::Draft) && self.text.trim().is_empty()
            || (publishing || self.state == IntroState::Published)
                && contact::contact_bypass(&self.text)
        {
            return Err(IntroError::Invalid);
        }
        Ok(())
    }
    pub fn visible(&self, event: EventId, now: DateTime<Utc>, age: Option<&AgeAssessment>) -> bool {
        self.event_id == event
            && self.state == IntroState::Published
            && self.created_at <= now
            && self.expires_at.is_none_or(|e| e > now)
            && !self.text.trim().is_empty()
            && self.text.chars().count() <= 1000
            && !self.text.contains('\0')
            && !contact::contact_bypass(&self.text)
            && (!self.age_bounds.restricted()
                || age.is_some_and(|a| self.age_bounds.permits(a, false)))
    }
    pub fn owner_projection(&self) -> OwnerIntro {
        OwnerIntro {
            intro_id: self.intro_id,
            event_id: self.event_id.uuid(),
            owner_id: self.owner_id.uuid(),
            text: self.text.clone(),
            age_bounds: self.age_bounds,
            expires_at: self.expires_at,
            created_at: self.created_at,
            state: self.state,
            revision: self.revision.get(),
        }
    }
    /// C04 public content fragment. BE27's composer must resolve the canonical
    /// author's C03 PublicProfile through its owner. No profile/art is fabricated.
    pub fn public_content(&self) -> PublicIntroContent {
        PublicIntroContent {
            intro_id: self.intro_id,
            event_id: self.event_id.uuid(),
            author: self.owner_id,
            text: self.text.clone(),
            created_at: self.created_at,
            state: IntroState::Published,
            revision: self.revision.get(),
        }
    }
}
#[derive(Serialize)]
pub struct PublicIntroContent {
    intro_id: Uuid,
    event_id: Uuid,
    #[serde(skip)]
    author: AuthUserId,
    text: String,
    created_at: DateTime<Utc>,
    state: IntroState,
    revision: i64,
}
impl PublicIntroContent {
    pub fn author(&self) -> AuthUserId {
        self.author
    }
    pub fn id(&self) -> Uuid {
        self.intro_id
    }
}
#[derive(Serialize)]
pub struct OwnerIntro {
    intro_id: Uuid,
    event_id: Uuid,
    owner_id: Uuid,
    text: String,
    age_bounds: AgeBounds,
    expires_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    state: IntroState,
    revision: i64,
}
