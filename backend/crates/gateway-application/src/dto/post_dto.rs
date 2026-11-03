use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! redacted_debug {
    ($($type:ident { $($field:ident),+ $(,)? }),+ $(,)?) => {
        $(
            impl std::fmt::Debug for $type {
                fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    let mut debug = formatter.debug_struct(stringify!($type));
                    $(debug.field(stringify!($field), &"[REDACTED]");)+
                    debug.finish()
                }
            }
        )+
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationReceiptStateDto {
    Replayable,
    Expired,
    Incomplete,
}

#[derive(Clone, PartialEq, Eq)]
pub struct MutationReceiptStatusDto {
    pub operation_id: Uuid,
    pub actor_id: Uuid,
    pub scope: String,
    pub committed_at: DateTime<Utc>,
    pub replay_until: DateTime<Utc>,
    pub state: MutationReceiptStateDto,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PostCreateRequest {
    pub content: String,
    pub visibility: String,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommentCreateRequest {
    pub content: String,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PostResponse {
    pub id: String,
    pub author_id: String,
    pub content: String,
    pub visibility: String,
    pub created_at: String,
    pub comment_count: i64,
    pub like_count: i64,
    pub liked_by_me: bool,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommentResponse {
    pub id: String,
    pub post_id: String,
    pub author_id: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FeedResponse {
    pub posts: Vec<PostResponse>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FeedRecommendationMetadataResponse {
    pub post_id: String,
    pub source: String,
    pub reason_code: String,
    pub score_micros: i64,
    pub rank_version: String,
    pub impression_id: String,
    pub position: u32,
    pub propensity_micros: u32,
    pub exploration: bool,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RankedFeedResponse {
    pub posts: Vec<PostResponse>,
    pub next_cursor: Option<String>,
    pub snapshot_id: Option<String>,
    pub rank_version: Option<String>,
    pub request_id: Option<String>,
    pub fallback_used: bool,
    pub recommendation_metadata: Vec<FeedRecommendationMetadataResponse>,
    pub failed_sources: Vec<String>,
}

impl RankedFeedResponse {
    pub fn chronological_fallback(feed: FeedResponse, request_id: String) -> Self {
        Self {
            posts: feed.posts,
            next_cursor: feed.next_cursor,
            snapshot_id: None,
            rank_version: Some("chronological-fallback-v1".into()),
            request_id: Some(request_id),
            fallback_used: true,
            recommendation_metadata: Vec::new(),
            failed_sources: Vec::new(),
        }
    }
}

#[derive(Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FeedRecommendationEventRequest {
    pub post_id: String,
    pub impression_id: String,
    pub action: String,
    pub source: String,
    pub model_version: String,
    pub position: Option<i32>,
    pub propensity_micros: Option<u32>,
    pub dwell_ms: Option<u32>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FeedRecommendationEventResponse {
    pub accepted: bool,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PostDetailResponse {
    pub post: PostResponse,
    pub comments: Vec<CommentResponse>,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeletePostResponse {
    pub post_id: String,
    pub deleted: bool,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PostLikeResponse {
    pub post_id: String,
    pub like_count: i64,
    pub liked: bool,
}

#[derive(Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolveDeadLetterReplayRequest {
    pub resolution_id: Uuid,
    pub reason: String,
    pub evidence_reference: String,
}

impl ResolveDeadLetterReplayRequest {
    pub fn validate(&self) -> Result<(), &'static str> {
        let reason = self.reason.trim();
        if !(20..=1024).contains(&reason.chars().count())
            || reason.len() > 1024
            || reason.chars().any(char::is_control)
        {
            return Err(
                "reason must contain 20 to 1024 non-control characters and at most 1024 UTF-8 bytes",
            );
        }

        let reference = self.evidence_reference.trim();
        if !(3..=256).contains(&reference.len())
            || !reference.is_ascii()
            || !reference
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._:/#-".contains(&byte))
            || !reference
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphanumeric)
        {
            return Err(
                "evidenceReference must be a 3 to 256 character protected-record identifier",
            );
        }
        Ok(())
    }
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

#[derive(Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeadLetterReplayAcceptedResponse {
    pub event_id: String,
    pub resolution_id: String,
    pub accepted: bool,
}

redacted_debug! {
    MutationReceiptStatusDto {
        operation_id,
        actor_id,
        scope,
        committed_at,
        replay_until,
        state,
    },
    PostCreateRequest { content, visibility },
    CommentCreateRequest { content },
    PostResponse {
        id,
        author_id,
        content,
        visibility,
        created_at,
        comment_count,
        like_count,
        liked_by_me,
    },
    CommentResponse { id, post_id, author_id, content, created_at },
    FeedResponse { posts, next_cursor },
    FeedRecommendationMetadataResponse {
        post_id,
        source,
        reason_code,
        score_micros,
        rank_version,
        impression_id,
        position,
        propensity_micros,
        exploration,
    },
    RankedFeedResponse {
        posts,
        next_cursor,
        snapshot_id,
        rank_version,
        request_id,
        fallback_used,
        recommendation_metadata,
        failed_sources,
    },
    FeedRecommendationEventRequest {
        post_id,
        impression_id,
        action,
        source,
        model_version,
        position,
        propensity_micros,
        dwell_ms,
        metadata,
    },
    PostDetailResponse { post, comments },
    DeletePostResponse { post_id, deleted },
    PostLikeResponse { post_id, like_count, liked },
    ResolveDeadLetterReplayRequest { resolution_id, reason, evidence_reference },
    ResolveDeadLetterReplayRequestDto {
        event_id,
        resolution_id,
        actor_id,
        reason,
        evidence_reference,
    },
    ResolveDeadLetterReplayResponseDto { event_id, resolution_id, created },
    DeadLetterReplayAcceptedResponse { event_id, resolution_id, accepted },
}

#[cfg(test)]
mod secret_debug_tests {
    use super::*;

    #[test]
    fn post_dto_debug_does_not_render_content_cursors_or_private_signals() {
        let post = PostResponse {
            id: "private-post-id-01".into(),
            author_id: "private-author-id-02".into(),
            content: "private-post-content-03".into(),
            visibility: "private-visibility-04".into(),
            created_at: "private-post-time-05".into(),
            comment_count: 6,
            like_count: 7,
            liked_by_me: true,
        };
        let comment = CommentResponse {
            id: "private-comment-id-06".into(),
            post_id: "private-comment-post-id-07".into(),
            author_id: "private-comment-author-id-08".into(),
            content: "private-comment-content-09".into(),
            created_at: "private-comment-time-10".into(),
        };
        let recommendation = FeedRecommendationMetadataResponse {
            post_id: "private-ranked-post-id-11".into(),
            source: "private-rank-source-12".into(),
            reason_code: "private-rank-reason-13".into(),
            score_micros: 14,
            rank_version: "private-rank-version-15".into(),
            impression_id: "private-impression-id-16".into(),
            position: 17,
            propensity_micros: 18,
            exploration: true,
        };
        let values = [
            format!(
                "{:?}",
                MutationReceiptStatusDto {
                    operation_id: Uuid::parse_str("11111111-2222-3333-4444-555555555555").unwrap(),
                    actor_id: Uuid::parse_str("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee").unwrap(),
                    scope: "private-receipt-scope-19".into(),
                    committed_at: Utc::now(),
                    replay_until: Utc::now(),
                    state: MutationReceiptStateDto::Replayable,
                }
            ),
            format!(
                "{:?}",
                PostCreateRequest {
                    content: "private-create-content-20".into(),
                    visibility: "private-create-visibility-21".into(),
                }
            ),
            format!(
                "{:?}",
                CommentCreateRequest {
                    content: "private-create-comment-22".into(),
                }
            ),
            format!("{post:?}"),
            format!("{comment:?}"),
            format!(
                "{:?}",
                FeedResponse {
                    posts: vec![post.clone()],
                    next_cursor: Some("private-feed-cursor-23".into()),
                }
            ),
            format!("{recommendation:?}"),
            format!(
                "{:?}",
                RankedFeedResponse {
                    posts: vec![post.clone()],
                    next_cursor: Some("private-ranked-cursor-24".into()),
                    snapshot_id: Some("private-snapshot-25".into()),
                    rank_version: Some("private-page-rank-26".into()),
                    request_id: Some("private-request-id-27".into()),
                    fallback_used: false,
                    recommendation_metadata: vec![recommendation],
                    failed_sources: vec!["private-failed-source-28".into()],
                }
            ),
            format!(
                "{:?}",
                FeedRecommendationEventRequest {
                    post_id: "private-event-post-29".into(),
                    impression_id: "private-event-impression-30".into(),
                    action: "private-event-action-31".into(),
                    source: "private-event-source-32".into(),
                    model_version: "private-event-model-33".into(),
                    position: Some(34),
                    propensity_micros: Some(35),
                    dwell_ms: Some(36),
                    metadata: serde_json::json!({ "private-event-metadata-37": "private-event-value-38" }),
                }
            ),
            format!(
                "{:?}",
                PostDetailResponse {
                    post,
                    comments: vec![comment],
                }
            ),
            format!(
                "{:?}",
                DeletePostResponse {
                    post_id: "private-delete-post-39".into(),
                    deleted: true,
                }
            ),
            format!(
                "{:?}",
                PostLikeResponse {
                    post_id: "private-like-post-40".into(),
                    like_count: 41,
                    liked: true,
                }
            ),
            format!(
                "{:?}",
                ResolveDeadLetterReplayRequest {
                    resolution_id: Uuid::parse_str("99999999-8888-7777-6666-555555555555").unwrap(),
                    reason: "private-operator-reason-42".into(),
                    evidence_reference: "private-evidence-reference-43".into(),
                }
            ),
            format!(
                "{:?}",
                ResolveDeadLetterReplayRequestDto {
                    event_id: "private-event-id-44".into(),
                    resolution_id: "private-resolution-id-45".into(),
                    actor_id: "private-operator-id-46".into(),
                    reason: "private-internal-reason-47".into(),
                    evidence_reference: "private-internal-evidence-48".into(),
                }
            ),
            format!(
                "{:?}",
                ResolveDeadLetterReplayResponseDto {
                    event_id: "private-result-event-49".into(),
                    resolution_id: "private-result-resolution-50".into(),
                    created: true,
                }
            ),
            format!(
                "{:?}",
                DeadLetterReplayAcceptedResponse {
                    event_id: "private-accepted-event-51".into(),
                    resolution_id: "private-accepted-resolution-52".into(),
                    accepted: true,
                }
            ),
        ];
        let rendered = values.join("\n");

        for sentinel in [
            "11111111-2222-3333-4444-555555555555",
            "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
            "private-receipt-scope-19",
            "private-post-id-01",
            "private-author-id-02",
            "private-post-content-03",
            "private-visibility-04",
            "private-post-time-05",
            "private-comment-id-06",
            "private-comment-post-id-07",
            "private-comment-author-id-08",
            "private-comment-content-09",
            "private-comment-time-10",
            "private-ranked-post-id-11",
            "private-rank-source-12",
            "private-rank-reason-13",
            "private-rank-version-15",
            "private-impression-id-16",
            "private-create-content-20",
            "private-create-visibility-21",
            "private-create-comment-22",
            "private-feed-cursor-23",
            "private-ranked-cursor-24",
            "private-snapshot-25",
            "private-page-rank-26",
            "private-request-id-27",
            "private-failed-source-28",
            "private-event-post-29",
            "private-event-impression-30",
            "private-event-action-31",
            "private-event-source-32",
            "private-event-model-33",
            "private-event-metadata-37",
            "private-event-value-38",
            "private-delete-post-39",
            "private-like-post-40",
            "99999999-8888-7777-6666-555555555555",
            "private-operator-reason-42",
            "private-evidence-reference-43",
            "private-event-id-44",
            "private-resolution-id-45",
            "private-operator-id-46",
            "private-internal-reason-47",
            "private-internal-evidence-48",
            "private-result-event-49",
            "private-result-resolution-50",
            "private-accepted-event-51",
            "private-accepted-resolution-52",
        ] {
            assert!(
                !rendered.contains(sentinel),
                "{sentinel} leaked: {rendered}"
            );
        }
    }

    #[test]
    fn post_dto_json_serialization_keeps_existing_wire_fields() {
        assert_eq!(
            serde_json::to_value(PostCreateRequest {
                content: "wire-content".into(),
                visibility: "PUBLIC".into(),
            })
            .unwrap(),
            serde_json::json!({"content": "wire-content", "visibility": "PUBLIC"})
        );

        assert_eq!(
            serde_json::to_value(FeedRecommendationEventRequest {
                post_id: "wire-post".into(),
                impression_id: "wire-impression".into(),
                action: "OPEN".into(),
                source: "wire-source".into(),
                model_version: "wire-model".into(),
                position: Some(3),
                propensity_micros: Some(200_000),
                dwell_ms: Some(700),
                metadata: serde_json::json!({"context": "wire"}),
            })
            .unwrap(),
            serde_json::json!({
                "postId": "wire-post",
                "impressionId": "wire-impression",
                "action": "OPEN",
                "source": "wire-source",
                "modelVersion": "wire-model",
                "position": 3,
                "propensityMicros": 200_000,
                "dwellMs": 700,
                "metadata": {"context": "wire"}
            })
        );
    }
}
