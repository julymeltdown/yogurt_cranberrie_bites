//! VENU-BE-N01 place-note values. Audience and expiry are server-evaluated.
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::str::FromStr;
use uuid::Uuid;

pub const MAX_TEXT_SCALARS: usize = 1000;
pub const MAX_TEXT_BYTES: usize = 4000;
pub const ONE_HOUR_SECONDS: i64 = 3_600;
pub const MAX_NOTE_COUNT: usize = 1000;
pub const MIN_LANDMARK_ID: usize = 1;
pub const MAX_LANDMARK_ID: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PlaceNoteError {
    #[error("place note input is invalid")]
    InvalidInput,
    #[error("the current viewer is not authorized")]
    Denied,
    #[error("place note is unavailable")]
    NotFound,
    #[error("place note changed")]
    Conflict,
    #[error("the event does not accept note writes")]
    EventReadOnly,
    #[error("content moderation denied the action")]
    ModerationDenied,
    #[error("a required owner is unavailable")]
    DependencyUnavailable,
    #[error("place note storage returned invalid data")]
    InvalidState,
}

macro_rules! wire_enum {
    ($name:ident, $error:ident, $( $variant:ident = $value:literal ),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum $name { $( $variant ),+ }
        impl $name {
            pub fn as_str(self) -> &'static str { match self { $( Self::$variant => $value ),+ } }
        }
        impl FromStr for $name {
            type Err = PlaceNoteError;
            fn from_str(value: &str) -> Result<Self, PlaceNoteError> {
                match value { $( $value => Ok(Self::$variant), )+ _ => Err(PlaceNoteError::$error) }
            }
        }
    };
}

wire_enum!(
    NoteAudience,
    InvalidInput,
    Friends = "FRIENDS",
    Everyone = "EVERYONE"
);
wire_enum!(
    NoteExpiry,
    InvalidInput,
    OneHour = "ONE_HOUR",
    VenueEndDay = "VENUE_END_DAY"
);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteText(String);
impl NoteText {
    pub fn new(value: &str) -> Result<Self, PlaceNoteError> {
        if value.trim().is_empty()
            || value.chars().count() > MAX_TEXT_SCALARS
            || value.len() > MAX_TEXT_BYTES
            || value.chars().any(|c| c.is_control() && c != '\n')
        {
            return Err(PlaceNoteError::InvalidInput);
        }
        Ok(Self(value.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LandmarkSelection {
    pub landmark_id: String,
    pub catalog_revision: i64,
    pub landmark_revision: i64,
}
impl LandmarkSelection {
    pub fn new(
        landmark_id: &str,
        catalog_revision: i64,
        landmark_revision: i64,
    ) -> Result<Self, PlaceNoteError> {
        if !valid_landmark_id(landmark_id)
            || !(1..=super::MAX_REVISION).contains(&catalog_revision)
            || !(1..=super::MAX_REVISION).contains(&landmark_revision)
        {
            return Err(PlaceNoteError::InvalidInput);
        }
        Ok(Self {
            landmark_id: landmark_id.to_owned(),
            catalog_revision,
            landmark_revision,
        })
    }
}
pub fn valid_landmark_id(value: &str) -> bool {
    (MIN_LANDMARK_ID..=MAX_LANDMARK_ID).contains(&value.len())
        && value != "."
        && value != ".."
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'~' | b'-'))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteAuthor {
    pub user_id: Uuid,
    pub display_name: String,
    pub bio: Option<String>,
    pub avatar: Value,
}
impl NoteAuthor {
    pub fn new(
        user_id: Uuid,
        display_name: &str,
        bio: Option<&str>,
        avatar: Value,
    ) -> Result<Self, PlaceNoteError> {
        if user_id.is_nil()
            || display_name.trim().is_empty()
            || display_name.chars().count() > 40
            || display_name.chars().any(char::is_control)
            || bio.is_some_and(|v| v.chars().count() > 160 || v.chars().any(char::is_control))
            || !avatar.is_object()
        {
            return Err(PlaceNoteError::InvalidState);
        }
        Ok(Self {
            user_id,
            display_name: display_name.to_owned(),
            bio: bio.map(str::to_owned),
            avatar,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteMeetingEntry {
    pub landmark_id: String,
    pub recipient_user_id: Uuid,
    pub landmark_revision: i64,
    pub access_until: DateTime<Utc>,
}
impl NoteMeetingEntry {
    pub fn validate(&self) -> Result<(), PlaceNoteError> {
        if !valid_landmark_id(&self.landmark_id)
            || self.recipient_user_id.is_nil()
            || !(1..=super::MAX_REVISION).contains(&self.landmark_revision)
        {
            return Err(PlaceNoteError::InvalidState);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaceNote {
    pub id: Uuid,
    pub landmark_id: String,
    pub landmark_revision: i64,
    pub author: NoteAuthor,
    pub text: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub revision: i64,
    pub audience: NoteAudience,
    pub expiry: NoteExpiry,
    pub parent_id: Option<Uuid>,
    pub reply_count: usize,
    pub can_reply: bool,
    pub can_withdraw: bool,
    pub meet_here: Option<NoteMeetingEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotePage {
    pub items: Vec<PlaceNote>,
    pub visible_count: usize,
    pub parent: Option<PlaceNote>,
    pub next_before: Option<String>,
    pub server_now: DateTime<Utc>,
    pub access_until: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoteCursor {
    pub created_at: DateTime<Utc>,
    pub id: Uuid,
}
impl NoteCursor {
    pub fn parse(value: &str) -> Result<Self, PlaceNoteError> {
        let (micros, id_text) = value.split_once('.').ok_or(PlaceNoteError::InvalidInput)?;
        if micros.is_empty()
            || micros.len() > 19
            || micros.starts_with('0')
            || !micros.bytes().all(|b| b.is_ascii_digit())
            || value.len() > 56
        {
            return Err(PlaceNoteError::InvalidInput);
        }
        let micros: i64 = micros.parse().map_err(|_| PlaceNoteError::InvalidInput)?;
        let id = Uuid::parse_str(id_text).map_err(|_| PlaceNoteError::InvalidInput)?;
        if id.is_nil() || id.to_string() != id_text {
            return Err(PlaceNoteError::InvalidInput);
        }
        let created_at =
            DateTime::from_timestamp_micros(micros).ok_or(PlaceNoteError::InvalidInput)?;
        Ok(Self { created_at, id })
    }
    pub fn encode(self) -> String {
        format!("{}.{}", self.created_at.timestamp_micros(), self.id)
    }
    pub fn is_before(self, other: NoteCursor) -> bool {
        (self.created_at, self.id) < (other.created_at, other.id)
    }
}

pub fn reply_expiry(parent: DateTime<Utc>) -> DateTime<Utc> {
    parent
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_rejects_controls_and_accepts_newlines() {
        assert!(NoteText::new("hello\nworld").is_ok());
        assert!(NoteText::new(" \t ").is_err());
        assert!(NoteText::new("bell\u{7}").is_err());
        assert!(NoteText::new(&"a".repeat(MAX_TEXT_SCALARS + 1)).is_err());
    }

    #[test]
    fn landmark_ids_are_opaque_unreserved_ascii() {
        assert!(LandmarkSelection::new("lm.place_1~x", 1, 1).is_ok());
        assert!(LandmarkSelection::new(".", 1, 1).is_err());
        assert!(LandmarkSelection::new("..", 1, 1).is_err());
        assert!(LandmarkSelection::new("a/b", 1, 1).is_err());
        assert!(LandmarkSelection::new("x", 0, 1).is_err());
    }

    #[test]
    fn cursor_requires_canonical_epoch_and_uuid() {
        let id = Uuid::parse_str("22222222-2222-4222-8222-222222222222").unwrap();
        let encoded = format!("1700000000000000.{id}");
        assert_eq!(NoteCursor::parse(&encoded).unwrap().id, id);
        assert!(NoteCursor::parse(&format!("01700000000000000.{id}")).is_err());
        assert!(NoteCursor::parse("1700000000000000.not-a-uuid").is_err());
    }
}
