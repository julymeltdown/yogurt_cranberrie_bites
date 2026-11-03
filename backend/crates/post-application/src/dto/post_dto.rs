use post_domain::comment::Comment;
use post_domain::feed::{FeedView, PostView};
use post_domain::post::{Post, PostDetail, PostLikeStatus, ReplayTarget};
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostDto {
    pub id: String,
    pub author_id: String,
    pub content: String,
    pub visibility: String,
    pub created_at: String,
    pub comment_count: i64,
    pub like_count: i64,
    pub liked_by_me: bool,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentDto {
    pub id: String,
    pub post_id: String,
    pub author_id: String,
    pub content: String,
    pub created_at: String,
}

/// A post count for one author, as observed at read time.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorPostCountDto {
    pub author_id: String,
    /// Total active posts authored by this user: it excludes soft-deleted and
    /// moderation-hidden posts, but does not vary by the viewer or post visibility.
    /// This is an authoritative read-time aggregate, not a stored counter.
    pub post_count: i64,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedItemMetadataDto {
    pub post_id: String,
    pub source: String,
    pub reason_code: String,
    pub score_micros: i64,
    pub position: u32,
    pub impression_id: String,
    pub propensity_micros: u32,
    pub exploration: bool,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedDto {
    pub posts: Vec<PostDto>,
    pub next_cursor: Option<String>,
    pub request_id: Option<String>,
    pub snapshot_id: Option<String>,
    pub rank_version: Option<String>,
    pub fallback_used: bool,
    pub items: Vec<FeedItemMetadataDto>,
    pub failed_sources: Vec<String>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostDetailDto {
    pub post: PostDto,
    pub comments: Vec<CommentDto>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostLikeDto {
    pub post_id: String,
    pub like_count: i64,
    pub liked: bool,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePostDto {
    pub author_id: String,
    pub content: String,
    pub visibility: String,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCommentDto {
    pub post_id: String,
    pub author_id: String,
    pub content: String,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeletePostDto {
    pub post_id: String,
    pub actor_id: String,
    pub request_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeletePostResponseDto {
    pub post_id: String,
    pub deleted: bool,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteCommentDto {
    pub comment_id: String,
    pub actor_id: String,
    pub request_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteCommentResponseDto {
    pub comment_id: String,
    pub post_id: String,
    pub deleted: bool,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PurgeExpiredPostsDto {
    pub actor_id: String,
    pub request_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub dry_run: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PurgeExpiredPostsResponseDto {
    pub purged_count: i64,
    pub purged_post_count: i64,
    pub purged_comment_count: i64,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayRequestDto {
    pub actor_id: Option<String>,
    pub event_type: Option<String>,
    pub attribute: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub published: Option<bool>,
    pub target: Option<ReplayTarget>,
    pub limit: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayResponseDto {
    pub replayed_count: i64,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ResolveDeadLetterReplayRequestDto {
    pub event_id: String,
    pub resolution_id: String,
    pub actor_id: String,
    pub reason: String,
    pub evidence_reference: String,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ResolveDeadLetterReplayResponseDto {
    pub event_id: String,
    pub resolution_id: String,
    pub created: bool,
}

impl From<PostView> for PostDto {
    fn from(value: PostView) -> Self {
        Self {
            id: value.id.to_string(),
            author_id: value.author_id.to_string(),
            content: value.content,
            visibility: value.visibility,
            created_at: value.created_at.to_rfc3339(),
            comment_count: value.comment_count,
            like_count: value.like_count,
            liked_by_me: value.liked_by_viewer,
        }
    }
}

impl From<Post> for PostDto {
    fn from(value: Post) -> Self {
        Self {
            id: value.id.to_string(),
            author_id: value.author_id.to_string(),
            content: value.content,
            visibility: value.visibility,
            created_at: value.created_at.to_rfc3339(),
            comment_count: value.comment_count,
            like_count: value.like_count,
            liked_by_me: false,
        }
    }
}

impl From<Comment> for CommentDto {
    fn from(value: Comment) -> Self {
        Self {
            id: value.id.to_string(),
            post_id: value.post_id.to_string(),
            author_id: value.author_id.to_string(),
            content: value.content,
            created_at: value.created_at.to_rfc3339(),
        }
    }
}

impl From<FeedView> for FeedDto {
    fn from(value: FeedView) -> Self {
        Self {
            posts: value.posts.into_iter().map(Into::into).collect(),
            next_cursor: value.next_cursor,
            request_id: None,
            snapshot_id: None,
            rank_version: None,
            fallback_used: false,
            items: Vec::new(),
            failed_sources: Vec::new(),
        }
    }
}

impl From<PostDetail> for PostDetailDto {
    fn from(value: PostDetail) -> Self {
        Self {
            post: value.post.into(),
            comments: value.comments.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<PostLikeStatus> for PostLikeDto {
    fn from(value: PostLikeStatus) -> Self {
        Self {
            post_id: value.post_id.to_string(),
            like_count: value.like_count,
            liked: value.liked,
        }
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
    PostDto,
    CommentDto,
    AuthorPostCountDto,
    FeedItemMetadataDto,
    FeedDto,
    PostDetailDto,
    PostLikeDto,
    CreatePostDto,
    CreateCommentDto,
    DeletePostDto,
    DeletePostResponseDto,
    DeleteCommentDto,
    DeleteCommentResponseDto,
    PurgeExpiredPostsDto,
    ReplayRequestDto,
    ResolveDeadLetterReplayRequestDto,
    ResolveDeadLetterReplayResponseDto,
    ApplyUserDeletionDto,
    ApplyContentModerationDto,
    RecordFeedRecommendationEventDto,
);

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyUserDeletionDto {
    pub saga_id: String,
    pub user_id: String,
    pub actor_id: String,
    pub retention_until: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyUserDeletionResponseDto {
    pub applied: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplyContentModerationDto {
    pub report_id: String,
    pub target_type: String,
    pub target_id: String,
    pub actor_id: String,
    pub action: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyContentModerationResponseDto {
    pub applied: bool,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordFeedRecommendationEventDto {
    pub event_id: String,
    pub schema_version: u16,
    pub occurred_at: String,
    pub viewer_id: String,
    pub post_id: String,
    pub actor_id: Option<String>,
    pub position: Option<i32>,
    pub request_id: Option<String>,
    pub session_id: Option<String>,
    pub action: String,
    pub source: Option<String>,
    pub model_version: Option<String>,
    pub impression_id: Option<String>,
    pub propensity_micros: Option<u32>,
    pub dwell_ms: Option<u32>,
    pub metadata_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordFeedRecommendationEventResponseDto {
    pub accepted: bool,
}

#[cfg(test)]
mod debug_redaction_tests {
    use super::*;

    #[test]
    fn post_and_operational_dto_debug_output_hides_private_values() {
        let values = [
            format!(
                "{:?}",
                PostDto {
                    id: "private-post-id-3cb8".into(),
                    author_id: "private-author-id-5a02".into(),
                    content: "private-post-body-d807".into(),
                    visibility: "PRIVATE".into(),
                    created_at: "private-timestamp".into(),
                    comment_count: 0,
                    like_count: 0,
                    liked_by_me: false,
                }
            ),
            format!(
                "{:?}",
                CreatePostDto {
                    author_id: "private-create-author-459b".into(),
                    content: "private-create-body-8ddc".into(),
                    visibility: "PRIVATE".into(),
                }
            ),
            format!(
                "{:?}",
                DeletePostDto {
                    post_id: "private-delete-post-id".into(),
                    actor_id: "private-delete-actor-id".into(),
                    request_id: Some("private-request-id".into()),
                    ip_address: Some("203.0.113.17".into()),
                    user_agent: Some("private-user-agent".into()),
                }
            ),
            format!(
                "{:?}",
                RecordFeedRecommendationEventDto {
                    event_id: "private-event-id".into(),
                    schema_version: 1,
                    occurred_at: "private-event-time".into(),
                    viewer_id: "private-viewer-id".into(),
                    post_id: "private-event-post-id".into(),
                    actor_id: Some("private-event-actor-id".into()),
                    position: Some(1),
                    request_id: Some("private-feed-request".into()),
                    session_id: Some("private-session-id".into()),
                    action: "IMPRESSION".into(),
                    source: Some("private-source".into()),
                    model_version: Some("private-model".into()),
                    impression_id: Some("private-impression-id".into()),
                    propensity_micros: Some(1),
                    dwell_ms: Some(2),
                    metadata_json: r#"{"private":"private-recommendation-meta"}"#.into(),
                }
            ),
        ];

        for rendered in values {
            for secret in ["private-", "203.0.113.17", "PRIVATE", "private-user-agent"] {
                assert!(
                    !rendered.contains(secret),
                    "debug leaked {secret}: {rendered}"
                );
            }
            assert!(rendered.contains("[REDACTED]"));
        }
    }

    #[test]
    fn debug_redaction_does_not_change_post_dto_json() {
        let dto = PostDto {
            id: "post-id".into(),
            author_id: "author-id".into(),
            content: "wire-content".into(),
            visibility: "PUBLIC".into(),
            created_at: "2026-10-04T00:00:00Z".into(),
            comment_count: 2,
            like_count: 3,
            liked_by_me: true,
        };
        let value = serde_json::to_value(dto).unwrap();
        assert_eq!(value["content"], "wire-content");
        assert_eq!(value["authorId"], "author-id");
        assert_eq!(value["likedByMe"], true);
    }
}
#[derive(Clone)]
pub struct AuthorizeNotificationPostsDto {
    pub viewer_id: uuid::Uuid,
    pub post_ids: Vec<uuid::Uuid>,
}

#[derive(Clone)]
pub struct AuthorizedNotificationPostsDto {
    pub visible_post_ids: Vec<uuid::Uuid>,
}
