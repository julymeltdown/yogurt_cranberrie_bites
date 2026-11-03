use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Comment {
    pub id: Uuid,
    pub post_id: Uuid,
    pub author_id: Uuid,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

impl std::fmt::Debug for Comment {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Comment")
            .field("details", &"[REDACTED]")
            .finish()
    }
}

#[cfg(test)]
mod debug_redaction_tests {
    use super::*;

    #[test]
    fn comment_debug_hides_content_and_identity() {
        let comment = Comment {
            id: Uuid::new_v4(),
            post_id: Uuid::new_v4(),
            author_id: Uuid::new_v4(),
            content: "private-comment-7ac3".into(),
            created_at: chrono::Utc::now(),
        };

        let rendered = format!("{comment:?}");
        assert!(!rendered.contains("private-comment-7ac3"));
        assert!(!rendered.contains(&comment.author_id.to_string()));
        assert!(rendered.contains("[REDACTED]"));
    }
}
