use crate::post::Post;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::{Display, Formatter};
use uuid::Uuid;

const MAX_CURSOR_LENGTH: usize = 256;

/// How many rows one search may examine while looking for a full page.
///
/// Post search filters visibility in the application rather than in SQL, so a
/// database batch is not a response page: a batch can be entirely filtered out
/// while rows still remain behind it. Re-fetching until the page is full is what
/// makes a short page mean "no more visible rows" rather than "the first batch was
/// mostly hidden". The bound keeps that loop finite when a viewer can see almost
/// nothing, because otherwise a search would walk the whole table on every page.
pub const SEARCH_SCAN_ROWS: usize = 500;

/// Smallest batch a search asks for while scanning.
///
/// A page's remaining space is a poor batch size on its own: asking for two rows
/// at a time against a mostly-hidden result set would spend the whole row budget
/// on round trips. Asking for at least this many rows keeps the number of queries
/// bounded by the row budget rather than by the page size.
pub const MIN_SEARCH_BATCH: usize = 25;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedPage {
    pub posts: Vec<Post>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedCachePage {
    pub post_ids: Vec<Uuid>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostView {
    pub id: Uuid,
    pub author_id: Uuid,
    pub content: String,
    pub visibility: String,
    pub created_at: DateTime<Utc>,
    pub comment_count: i64,
    pub like_count: i64,
    pub liked_by_viewer: bool,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedView {
    pub posts: Vec<PostView>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidCursor;

impl Display for InvalidCursor {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Invalid cursor format")
    }
}

impl Error for InvalidCursor {}

#[derive(Clone, PartialEq, Eq)]
pub struct PostCursor {
    pub created_at: DateTime<Utc>,
    pub post_id: Uuid,
}

impl PostCursor {
    pub fn from(post: &Post) -> Self {
        Self {
            created_at: post.created_at,
            post_id: post.id,
        }
    }

    pub fn parse(value: &str) -> Result<Option<Self>, InvalidCursor> {
        if value.len() > MAX_CURSOR_LENGTH {
            return Err(InvalidCursor);
        }
        let value = value.trim();
        if value.is_empty() {
            return Ok(None);
        }
        let mut parts = value.split('|');
        let created_at = parts.next().ok_or(InvalidCursor)?;
        let post_id = parts.next().ok_or(InvalidCursor)?;
        if parts.next().is_some() {
            return Err(InvalidCursor);
        }
        Ok(Some(Self {
            created_at: DateTime::parse_from_rfc3339(created_at)
                .map_err(|_| InvalidCursor)?
                .with_timezone(&Utc),
            post_id: Uuid::parse_str(post_id).map_err(|_| InvalidCursor)?,
        }))
    }

    pub fn encode(&self) -> String {
        format!("{}|{}", self.created_at.to_rfc3339(), self.post_id)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct TrendingCursor {
    pub like_count: i64,
    pub created_at: DateTime<Utc>,
    pub post_id: Uuid,
}

impl TrendingCursor {
    pub fn from(post: &Post) -> Self {
        Self {
            like_count: post.like_count,
            created_at: post.created_at,
            post_id: post.id,
        }
    }

    pub fn parse(value: &str) -> Result<Option<Self>, InvalidCursor> {
        if value.len() > MAX_CURSOR_LENGTH {
            return Err(InvalidCursor);
        }
        let value = value.trim();
        if value.is_empty() {
            return Ok(None);
        }
        let mut parts = value.split('|');
        let like_count = parts.next().ok_or(InvalidCursor)?;
        let created_at = parts.next().ok_or(InvalidCursor)?;
        let post_id = parts.next().ok_or(InvalidCursor)?;
        if parts.next().is_some() {
            return Err(InvalidCursor);
        }
        Ok(Some(Self {
            like_count: like_count.parse().map_err(|_| InvalidCursor)?,
            created_at: DateTime::parse_from_rfc3339(created_at)
                .map_err(|_| InvalidCursor)?
                .with_timezone(&Utc),
            post_id: Uuid::parse_str(post_id).map_err(|_| InvalidCursor)?,
        }))
    }

    pub fn encode(&self) -> String {
        format!(
            "{}|{}|{}",
            self.like_count,
            self.created_at.to_rfc3339(),
            self.post_id
        )
    }
}

macro_rules! redact_debug {
    ($($type:ty),+ $(,)?) => {
        $(
            impl std::fmt::Debug for $type {
                fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    formatter
                        .debug_struct(stringify!($type))
                        .field("details", &"[REDACTED]")
                        .finish()
                }
            }
        )+
    };
}

redact_debug!(
    FeedPage,
    FeedCachePage,
    PostView,
    FeedView,
    PostCursor,
    TrendingCursor
);

#[cfg(test)]
mod debug_redaction_tests {
    use super::*;

    #[test]
    fn feed_debug_hides_content_cursors_and_post_ids() {
        let post_id = Uuid::new_v4();
        let view = PostView {
            id: post_id,
            author_id: Uuid::new_v4(),
            content: "private-feed-content-45bd".into(),
            visibility: "PRIVATE".into(),
            created_at: Utc::now(),
            comment_count: 1,
            like_count: 2,
            liked_by_viewer: false,
        };
        let feed = FeedView {
            posts: vec![view],
            next_cursor: Some("private-cursor-299e".into()),
        };
        let rendered = format!("{feed:?}");

        assert!(!rendered.contains("private-feed-content-45bd"));
        assert!(!rendered.contains("private-cursor-299e"));
        assert!(!rendered.contains(&post_id.to_string()));
        assert!(rendered.contains("[REDACTED]"));
    }

    #[test]
    fn feed_debug_redaction_preserves_serialized_content() {
        let feed = FeedView {
            posts: vec![PostView {
                id: Uuid::nil(),
                author_id: Uuid::nil(),
                content: "serialized-feed-content".into(),
                visibility: "PUBLIC".into(),
                created_at: Utc::now(),
                comment_count: 0,
                like_count: 0,
                liked_by_viewer: false,
            }],
            next_cursor: None,
        };
        let value = serde_json::to_value(feed).unwrap();
        assert_eq!(value["posts"][0]["content"], "serialized-feed-content");
        assert_eq!(value["posts"][0]["visibility"], "PUBLIC");
    }
}
