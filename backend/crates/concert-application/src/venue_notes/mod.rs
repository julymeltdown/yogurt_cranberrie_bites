//! VENU-BE-N01 commands and fail-closed external owner seams.
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_domain::venue_notes::{
    LandmarkSelection, NoteAudience, NoteCursor, NoteExpiry, NotePage, NoteText, PlaceNote,
    PlaceNoteError,
};
use uuid::Uuid;

pub const DEFAULT_LIMIT: usize = 20;
pub const MAX_LIMIT: usize = 50;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreatePlaceNote {
    pub text: NoteText,
    pub audience: NoteAudience,
    pub expiry: NoteExpiry,
    pub landmark_selection: LandmarkSelection,
}
impl CreatePlaceNote {
    pub fn new(
        text: &str,
        audience: NoteAudience,
        expiry: NoteExpiry,
        landmark_selection: LandmarkSelection,
    ) -> Result<Self, PlaceNoteError> {
        Ok(Self {
            text: NoteText::new(text)?,
            audience,
            expiry,
            landmark_selection,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreatePlaceReply {
    pub text: NoteText,
    pub expected_parent_revision: i64,
}
impl CreatePlaceReply {
    pub fn new(text: &str, expected_parent_revision: i64) -> Result<Self, PlaceNoteError> {
        if !(1..=concert_domain::MAX_REVISION).contains(&expected_parent_revision) {
            return Err(PlaceNoteError::InvalidInput);
        }
        Ok(Self {
            text: NoteText::new(text)?,
            expected_parent_revision,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteListQuery {
    pub landmark_id: String,
    pub limit: usize,
    pub before: Option<NoteCursor>,
}
impl NoteListQuery {
    pub fn new(
        landmark_id: &str,
        limit: Option<usize>,
        before: Option<&str>,
    ) -> Result<Self, PlaceNoteError> {
        let limit = limit.unwrap_or(DEFAULT_LIMIT);
        if !concert_domain::venue_notes::valid_landmark_id(landmark_id)
            || !(1..=MAX_LIMIT).contains(&limit)
        {
            return Err(PlaceNoteError::InvalidInput);
        }
        Ok(Self {
            landmark_id: landmark_id.to_owned(),
            limit,
            before: before.map(NoteCursor::parse).transpose()?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteThread {
    pub note: PlaceNote,
    pub replies: Vec<PlaceNote>,
    pub next_before: Option<String>,
    pub server_now: DateTime<Utc>,
    pub access_until: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoteMutation {
    pub resource_id: Uuid,
    pub revision: i64,
}

#[async_trait]
pub trait PlaceNoteUseCases: Send + Sync {
    async fn create_note(
        &self,
        actor: Uuid,
        event: Uuid,
        command: CreatePlaceNote,
        idempotency_key: Uuid,
    ) -> Result<PlaceNote, PlaceNoteError>;
    async fn list_notes(
        &self,
        viewer: Uuid,
        event: Uuid,
        query: NoteListQuery,
    ) -> Result<NotePage, PlaceNoteError>;
    async fn note_thread(
        &self,
        viewer: Uuid,
        event: Uuid,
        landmark_id: &str,
        note_id: Uuid,
        limit: Option<usize>,
        before: Option<&str>,
    ) -> Result<NoteThread, PlaceNoteError>;
    async fn list_replies(
        &self,
        viewer: Uuid,
        event: Uuid,
        landmark_id: &str,
        note_id: Uuid,
        query: NoteListQuery,
    ) -> Result<NotePage, PlaceNoteError>;
    async fn create_reply(
        &self,
        actor: Uuid,
        event: Uuid,
        landmark_id: &str,
        parent_id: Uuid,
        command: CreatePlaceReply,
        idempotency_key: Uuid,
    ) -> Result<PlaceNote, PlaceNoteError>;
    async fn withdraw_note(
        &self,
        actor: Uuid,
        event: Uuid,
        landmark_id: &str,
        note_id: Uuid,
        expected_revision: i64,
        idempotency_key: Uuid,
    ) -> Result<NoteMutation, PlaceNoteError>;
    async fn hide_note(
        &self,
        moderator: Uuid,
        event: Uuid,
        note_id: Uuid,
    ) -> Result<(), PlaceNoteError>;
}
