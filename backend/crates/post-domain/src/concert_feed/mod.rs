//! Canonical event content owned by Post, independent of Concert rendering/policy.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use uuid::Uuid;

pub const MAX_REVISION: i64 = 9_007_199_254_740_991;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("invalid concert content command")]
    Invalid,
    #[error("concert content is unavailable")]
    NotFound,
    #[error("concert content owner authorization denied")]
    Forbidden,
    #[error("concert content revision conflict")]
    Conflict,
    #[error("concert content authority missing, stale or expired")]
    Authority,
    #[error("concert content idempotency identity conflict")]
    Idempotency,
    #[error("concert content storage unavailable")]
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LinkKind {
    Party,
    Giveaway,
    Post,
    Intro,
    Mural,
    Notice,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Link {
    pub target_type: LinkKind,
    pub target_id: Uuid,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Content {
    pub text: String,
    pub media_ids: Vec<Uuid>,
    pub links: Vec<Link>,
}
impl Content {
    pub fn validate(&self) -> Result<(), Error> {
        valid_text(&self.text, 4000)?;
        if self.media_ids.len() > 4
            || self.links.len() > 4
            || self.media_ids.iter().any(Uuid::is_nil)
            || self.links.iter().any(|l| l.target_id.is_nil())
            || self.media_ids.iter().collect::<BTreeSet<_>>().len() != self.media_ids.len()
        {
            return Err(Error::Invalid);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum State {
    Published,
    Hidden,
    Deleted,
}

/// Event and author never change after creation. No client-supplied URLs.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Post {
    pub post_id: Uuid,
    pub event_id: Uuid,
    pub author_id: Uuid,
    pub content: Content,
    pub revision: i64,
    pub state: State,
    pub created_at: DateTime<Utc>,
    pub edited_at: Option<DateTime<Utc>>,
    pub comment_count: i64,
    pub reaction_count: i64,
}
impl Post {
    pub fn visible_in(&self, event_id: Uuid) -> Result<(), Error> {
        if self.event_id != event_id || self.state != State::Published {
            Err(Error::NotFound)
        } else {
            Ok(())
        }
    }
    pub fn owner(&self, actor_id: Uuid, event_id: Uuid, revision: i64) -> Result<(), Error> {
        self.visible_in(event_id)?;
        if self.author_id != actor_id {
            return Err(Error::Forbidden);
        }
        self.expect(revision)
    }
    pub fn expect(&self, revision: i64) -> Result<(), Error> {
        if self.revision != revision {
            Err(Error::Conflict)
        } else {
            Ok(())
        }
    }
    pub fn advance(&mut self) -> Result<(), Error> {
        self.revision = next_revision(self.revision)?;
        Ok(())
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Comment {
    pub comment_id: Uuid,
    pub event_id: Uuid,
    pub post_id: Uuid,
    pub author_id: Uuid,
    pub text: String,
    pub revision: i64,
    pub state: State,
    pub created_at: DateTime<Utc>,
    pub edited_at: Option<DateTime<Utc>>,
}
impl Comment {
    pub fn owner(
        &self,
        actor_id: Uuid,
        event_id: Uuid,
        post_id: Uuid,
        revision: i64,
    ) -> Result<(), Error> {
        if self.event_id != event_id || self.post_id != post_id || self.state != State::Published {
            return Err(Error::NotFound);
        }
        if self.author_id != actor_id {
            return Err(Error::Forbidden);
        }
        if self.revision != revision {
            return Err(Error::Conflict);
        }
        Ok(())
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum Mutation {
    CreatePost {
        content: Content,
    },
    EditPost {
        expected_revision: i64,
        content: Content,
    },
    DeletePost {
        expected_revision: i64,
    },
    CreateComment {
        comment_id: Uuid,
        post_revision: i64,
        text: String,
    },
    EditComment {
        comment_id: Uuid,
        expected_revision: i64,
        text: String,
    },
    DeleteComment {
        comment_id: Uuid,
        expected_revision: i64,
    },
    SetReaction {
        expected_revision: i64,
        active: bool,
    },
}
impl Mutation {
    pub fn name(&self) -> &'static str {
        match self {
            Self::CreatePost { .. } => "CREATE_POST",
            Self::EditPost { .. } => "EDIT_POST",
            Self::DeletePost { .. } => "DELETE_POST",
            Self::CreateComment { .. } => "CREATE_COMMENT",
            Self::EditComment { .. } => "EDIT_COMMENT",
            Self::DeleteComment { .. } => "DELETE_COMMENT",
            Self::SetReaction { .. } => "SET_REACTION",
        }
    }
    pub fn content(&self) -> Option<&Content> {
        match self {
            Self::CreatePost { content } | Self::EditPost { content, .. } => Some(content),
            _ => None,
        }
    }
    pub fn comment_id(&self) -> Option<Uuid> {
        match self {
            Self::CreateComment { comment_id, .. }
            | Self::EditComment { comment_id, .. }
            | Self::DeleteComment { comment_id, .. } => Some(*comment_id),
            _ => None,
        }
    }
    pub fn validate(&self) -> Result<(), Error> {
        if self.comment_id().is_some_and(|id| id.is_nil()) {
            return Err(Error::Invalid);
        }
        if let Some(c) = self.content() {
            c.validate()?;
        }
        match self {
            Self::EditPost {
                expected_revision, ..
            }
            | Self::DeletePost { expected_revision }
            | Self::EditComment {
                expected_revision, ..
            }
            | Self::DeleteComment {
                expected_revision, ..
            }
            | Self::SetReaction {
                expected_revision, ..
            } => valid_revision(*expected_revision)?,
            Self::CreateComment { post_revision, .. } => valid_revision(*post_revision)?,
            _ => (),
        }
        match self {
            Self::CreateComment { text, .. } | Self::EditComment { text, .. } => {
                valid_text(text, 1000)
            }
            _ => Ok(()),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    pub operation_id: Uuid,
    pub actor_id: Uuid,
    pub event_id: Uuid,
    pub post_id: Uuid,
    /// Scoped to actor + event, retained as a digest by persistence.
    pub idempotency_key: String,
    pub mutation: Mutation,
}
impl Command {
    pub fn validate(&self) -> Result<(), Error> {
        if [
            self.operation_id,
            self.actor_id,
            self.event_id,
            self.post_id,
        ]
        .iter()
        .any(Uuid::is_nil)
            || self.idempotency_key.is_empty()
            || self.idempotency_key.len() > 128
            || !self.idempotency_key.bytes().all(|b| b.is_ascii_graphic())
        {
            return Err(Error::Invalid);
        }
        self.mutation.validate()
    }
}

/// Receipt/outbox data contains identifiers and revisions only, never user text.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outcome {
    pub actor_id: Uuid,
    pub operation_id: Uuid,
    pub event_id: Uuid,
    pub post_id: Uuid,
    pub comment_id: Option<Uuid>,
    pub revision: i64,
    pub post_revision: i64,
    pub deleted: bool,
    pub reacted_by_me: bool,
    pub reaction_count: i64,
    pub committed_at: DateTime<Utc>,
}

pub fn valid_text(text: &str, max: usize) -> Result<(), Error> {
    if text.trim().is_empty() || text.chars().count() > max || text.contains('\0') {
        Err(Error::Invalid)
    } else {
        Ok(())
    }
}
pub fn valid_revision(revision: i64) -> Result<(), Error> {
    if !(1..=MAX_REVISION).contains(&revision) {
        Err(Error::Invalid)
    } else {
        Ok(())
    }
}
pub fn next_revision(revision: i64) -> Result<i64, Error> {
    valid_revision(revision)?;
    revision
        .checked_add(1)
        .filter(|r| *r <= MAX_REVISION)
        .ok_or(Error::Conflict)
}
