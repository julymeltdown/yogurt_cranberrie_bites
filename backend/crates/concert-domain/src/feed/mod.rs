//! Event feed values, independent of transport and the inherited Post implementation.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum FeedError {
    #[error("Invalid input")]
    InvalidInput,
    #[error("Resource unavailable")]
    ResourceUnavailable,
    #[error("Permission denied")]
    PermissionDenied,
    #[error("Required dependency unavailable")]
    DependencyUnavailable,
    #[error("Version conflict")]
    VersionConflict,
    #[error("Event read only")]
    EventReadOnly,
    #[error("Media not ready")]
    MediaNotReady,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Filter {
    #[default]
    All,
    WithMedia,
    PartyLink,
    GiveawayLink,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TargetType {
    Party,
    Giveaway,
    Post,
    Intro,
    Mural,
    Notice,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceLink {
    pub target_type: TargetType,
    pub target_id: Uuid,
}

/// Text is deliberately not Debug. Never serialize a stored record directly as public DTO.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Content {
    pub text: String,
    pub media_ids: Vec<Uuid>,
    pub links: Vec<ResourceLink>,
}
impl Content {
    pub fn validate(&self) -> Result<(), FeedError> {
        text(&self.text, 4000)?;
        if self.media_ids.len() > 4
            || self.links.len() > 4
            || self.media_ids.iter().any(Uuid::is_nil)
            || self.links.iter().any(|l| l.target_id.is_nil())
            || self
                .media_ids
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
                != self.media_ids.len()
        {
            return Err(FeedError::InvalidInput);
        }
        Ok(())
    }
    pub fn matches(&self, filter: Filter) -> bool {
        match filter {
            Filter::All => true,
            Filter::WithMedia => !self.media_ids.is_empty(),
            Filter::PartyLink => self
                .links
                .iter()
                .any(|l| l.target_type == TargetType::Party),
            Filter::GiveawayLink => self
                .links
                .iter()
                .any(|l| l.target_type == TargetType::Giveaway),
        }
    }
}
pub fn text(value: &str, maximum: usize) -> Result<(), FeedError> {
    if value.trim().is_empty() || value.chars().count() > maximum || value.contains('\0') {
        Err(FeedError::InvalidInput)
    } else {
        Ok(())
    }
}
pub fn revision(value: i64) -> Result<i64, FeedError> {
    if (1..=crate::MAX_REVISION).contains(&value) {
        Ok(value)
    } else {
        Err(FeedError::InvalidInput)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentState {
    Published,
    Hidden,
    Deleted,
}
#[derive(Clone, PartialEq, Eq)]
pub struct Post {
    pub id: Uuid,
    pub event: Uuid,
    pub author: Uuid,
    pub content: Content,
    pub state: ContentState,
    pub public: bool,
    pub revision: i64,
    pub created_at: DateTime<Utc>,
    pub edited_at: Option<DateTime<Utc>>,
    pub comment_count: i32,
    pub reaction_count: i32,
    pub reacted_by_me: bool,
}
impl Post {
    pub fn position(&self) -> Position {
        Position {
            created_at: self.created_at,
            id: self.id,
        }
    }
    pub fn visible_in(&self, event: Uuid) -> bool {
        self.event == event && self.public && self.state == ContentState::Published
    }
    pub fn validate(&self) -> Result<(), FeedError> {
        self.content.validate()?;
        revision(self.revision)?;
        if self.id.is_nil()
            || self.event.is_nil()
            || self.author.is_nil()
            || self.comment_count < 0
            || self.reaction_count < 0
            || self.edited_at.is_some_and(|t| t < self.created_at)
        {
            return Err(FeedError::InvalidInput);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Position {
    pub created_at: DateTime<Utc>,
    pub id: Uuid,
}
/// Snapshot boundary and strict last tuple. Never encode these as a plaintext wire cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Window {
    pub upper: DateTime<Utc>,
    pub after: Option<Position>,
}
impl Window {
    pub fn contains(self, p: Position) -> bool {
        p.created_at <= self.upper && self.after.is_none_or(|last| p < last)
    }
}

/// The owning Post service must bind every comment to its actual parent and event.
#[derive(Clone, PartialEq, Eq)]
pub struct Comment {
    pub id: Uuid,
    pub post: Uuid,
    pub event: Uuid,
    pub author: Uuid,
    pub text: String,
    pub state: ContentState,
    pub revision: i64,
    pub created_at: DateTime<Utc>,
}
impl Comment {
    pub fn position(&self) -> Position {
        Position {
            created_at: self.created_at,
            id: self.id,
        }
    }
    pub fn validate(&self) -> Result<(), FeedError> {
        text(&self.text, 1000)?;
        revision(self.revision)?;
        if [self.id, self.post, self.event, self.author]
            .iter()
            .any(Uuid::is_nil)
        {
            return Err(FeedError::InvalidInput);
        }
        Ok(())
    }
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum FeedCommand {
    CreatePost {
        content: Content,
    },
    EditPost {
        post: Uuid,
        expected_revision: i64,
        content: Content,
    },
    DeletePost {
        post: Uuid,
        expected_revision: i64,
    },
    CreateComment {
        post: Uuid,
        post_revision: i64,
        text: String,
    },
    EditComment {
        comment: Uuid,
        expected_revision: i64,
        text: String,
    },
    DeleteComment {
        comment: Uuid,
        expected_revision: i64,
    },
    SetReaction {
        post: Uuid,
        present: bool,
    },
}
impl FeedCommand {
    pub fn validate(&self) -> Result<(), FeedError> {
        match self {
            Self::CreatePost { content } => content.validate(),
            Self::EditPost {
                post,
                expected_revision,
                content,
            } => {
                non_nil(*post)?;
                revision(*expected_revision)?;
                content.validate()
            }
            Self::DeletePost {
                post,
                expected_revision,
            } => {
                non_nil(*post)?;
                revision(*expected_revision).map(|_| ())
            }
            Self::CreateComment {
                post,
                post_revision,
                text: value,
            } => {
                non_nil(*post)?;
                revision(*post_revision)?;
                text(value, 1000)
            }
            Self::EditComment {
                comment,
                expected_revision,
                text: value,
            } => {
                non_nil(*comment)?;
                revision(*expected_revision)?;
                text(value, 1000)
            }
            Self::DeleteComment {
                comment,
                expected_revision,
            } => {
                non_nil(*comment)?;
                revision(*expected_revision).map(|_| ())
            }
            Self::SetReaction { post, .. } => non_nil(*post),
        }
    }
    pub fn operation(&self) -> &'static str {
        match self {
            Self::CreatePost { .. } => "createConcertPost",
            Self::EditPost { .. } => "editConcertPost",
            Self::DeletePost { .. } => "deleteConcertPost",
            Self::CreateComment { .. } => "createConcertComment",
            Self::EditComment { .. } => "editConcertComment",
            Self::DeleteComment { .. } => "deleteConcertComment",
            Self::SetReaction { present: true, .. } => "likeConcertPost",
            Self::SetReaction { present: false, .. } => "unlikeConcertPost",
        }
    }
    pub fn is_safety(&self) -> bool {
        matches!(self, Self::DeletePost { .. } | Self::DeleteComment { .. })
    }
}
fn non_nil(id: Uuid) -> Result<(), FeedError> {
    if id.is_nil() {
        Err(FeedError::InvalidInput)
    } else {
        Ok(())
    }
}
