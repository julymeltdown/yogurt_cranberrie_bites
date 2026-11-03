use serde::{Deserialize, Serialize};

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

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FollowRequest {
    pub target_user_id: String,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FollowActionResponse {
    pub following: bool,
    pub followed_at: Option<String>,
    pub status: String,
    pub request_id: Option<String>,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FollowEdgeResponse {
    pub user_id: String,
    pub followed_at: String,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FollowListResponse {
    pub edges: Vec<FollowEdgeResponse>,
    pub next_cursor: Option<String>,
    pub total_count: i64,
}

#[derive(Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FollowCountsResponse {
    pub follower_count: i64,
    pub following_count: i64,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FollowStatusResponse {
    pub following: bool,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FollowRequestResponse {
    pub id: String,
    pub follower_id: String,
    pub followee_id: String,
    pub status: String,
    pub created_at: String,
    pub decided_at: Option<String>,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FollowRequestPageResponse {
    pub requests: Vec<FollowRequestResponse>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipPolicyResponse {
    pub blocked_by_viewer: bool,
    pub blocked_viewer: bool,
    pub muted_by_viewer: bool,
    pub following: bool,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PeopleRecommendationResponse {
    pub user_id: String,
    pub username: String,
    pub nickname: String,
    pub bio: String,
    pub avatar_url: String,
    pub source: String,
    pub reason_code: String,
    pub score_micros: i64,
    pub rank_version: String,
    pub impression_id: String,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PeopleRecommendationPageResponse {
    pub items: Vec<PeopleRecommendationResponse>,
    pub next_cursor: Option<String>,
    pub snapshot_id: String,
    pub rank_version: String,
    pub request_id: String,
    pub fallback_used: bool,
}

#[derive(Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PeopleRecommendationEventRequest {
    pub candidate_user_id: String,
    pub impression_id: String,
    pub action: String,
    pub source: String,
    pub model_version: String,
    pub position: Option<i32>,
    pub propensity_micros: Option<i32>,
    pub dwell_ms: Option<i32>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

redacted_debug! {
    FollowRequest { target_user_id },
    FollowActionResponse { following, followed_at, status, request_id },
    FollowEdgeResponse { user_id, followed_at },
    FollowListResponse { edges, next_cursor, total_count },
    FollowCountsResponse { follower_count, following_count },
    FollowStatusResponse { following },
    FollowRequestResponse {
        id,
        follower_id,
        followee_id,
        status,
        created_at,
        decided_at,
    },
    FollowRequestPageResponse { requests, next_cursor },
    RelationshipPolicyResponse { blocked_by_viewer, blocked_viewer, muted_by_viewer, following },
    PeopleRecommendationResponse {
        user_id,
        username,
        nickname,
        bio,
        avatar_url,
        source,
        reason_code,
        score_micros,
        rank_version,
        impression_id,
    },
    PeopleRecommendationPageResponse {
        items,
        next_cursor,
        snapshot_id,
        rank_version,
        request_id,
        fallback_used,
    },
    PeopleRecommendationEventRequest {
        candidate_user_id,
        impression_id,
        action,
        source,
        model_version,
        position,
        propensity_micros,
        dwell_ms,
        metadata,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationEventResponse {
    pub accepted: bool,
}

#[cfg(test)]
mod secret_debug_tests {
    use super::*;

    #[test]
    fn friend_dto_debug_does_not_render_social_graph_or_recommendation_data() {
        let edge = FollowEdgeResponse {
            user_id: "private-follow-user-1".into(),
            followed_at: "private-follow-time-2".into(),
        };
        let follow_request = FollowRequestResponse {
            id: "private-request-id-3".into(),
            follower_id: "private-follower-id-4".into(),
            followee_id: "private-followee-id-5".into(),
            status: "private-request-status-6".into(),
            created_at: "private-request-created-7".into(),
            decided_at: Some("private-request-decided-8".into()),
        };
        let recommendation = PeopleRecommendationResponse {
            user_id: "private-recommended-user-9".into(),
            username: "private-recommended-handle-10".into(),
            nickname: "private-recommended-name-11".into(),
            bio: "private-recommendation-bio-12".into(),
            avatar_url: "https://private.example/recommendation-avatar-13".into(),
            source: "private-recommendation-source-14".into(),
            reason_code: "private-recommendation-reason-15".into(),
            score_micros: 123_456,
            rank_version: "private-rank-version-16".into(),
            impression_id: "private-impression-id-17".into(),
        };
        let values = [
            format!(
                "{:?}",
                FollowRequest {
                    target_user_id: "private-follow-target-18".into(),
                }
            ),
            format!(
                "{:?}",
                FollowActionResponse {
                    following: true,
                    followed_at: Some("private-action-time-19".into()),
                    status: "private-action-status-20".into(),
                    request_id: Some("private-action-request-id-21".into()),
                }
            ),
            format!("{edge:?}"),
            format!(
                "{:?}",
                FollowListResponse {
                    edges: vec![edge],
                    next_cursor: Some("private-follow-cursor-22".into()),
                    total_count: 31,
                }
            ),
            format!(
                "{:?}",
                FollowCountsResponse {
                    follower_count: 32,
                    following_count: 33
                }
            ),
            format!("{:?}", FollowStatusResponse { following: true }),
            format!("{follow_request:?}"),
            format!(
                "{:?}",
                FollowRequestPageResponse {
                    requests: vec![follow_request],
                    next_cursor: Some("private-request-cursor-23".into()),
                }
            ),
            format!(
                "{:?}",
                RelationshipPolicyResponse {
                    blocked_by_viewer: true,
                    blocked_viewer: false,
                    muted_by_viewer: true,
                    following: false,
                }
            ),
            format!("{recommendation:?}"),
            format!(
                "{:?}",
                PeopleRecommendationPageResponse {
                    items: vec![recommendation],
                    next_cursor: Some("private-recommendation-cursor-24".into()),
                    snapshot_id: "private-snapshot-id-25".into(),
                    rank_version: "private-page-rank-version-26".into(),
                    request_id: "private-recommendation-request-id-27".into(),
                    fallback_used: false,
                }
            ),
            format!(
                "{:?}",
                PeopleRecommendationEventRequest {
                    candidate_user_id: "private-event-candidate-28".into(),
                    impression_id: "private-event-impression-29".into(),
                    action: "private-event-action-30".into(),
                    source: "private-event-source-31".into(),
                    model_version: "private-event-model-32".into(),
                    position: Some(4),
                    propensity_micros: Some(123),
                    dwell_ms: Some(456),
                    metadata: serde_json::json!({ "private-metadata-key-33": "private-metadata-value-34" }),
                }
            ),
        ];
        let rendered = values.join("\n");

        for sentinel in [
            "private-follow-user-1",
            "private-follow-time-2",
            "private-request-id-3",
            "private-follower-id-4",
            "private-followee-id-5",
            "private-request-status-6",
            "private-request-created-7",
            "private-request-decided-8",
            "private-recommended-user-9",
            "private-recommended-handle-10",
            "private-recommended-name-11",
            "private-recommendation-bio-12",
            "https://private.example/recommendation-avatar-13",
            "private-recommendation-source-14",
            "private-recommendation-reason-15",
            "private-rank-version-16",
            "private-impression-id-17",
            "private-follow-target-18",
            "private-action-time-19",
            "private-action-status-20",
            "private-action-request-id-21",
            "private-follow-cursor-22",
            "private-request-cursor-23",
            "private-recommendation-cursor-24",
            "private-snapshot-id-25",
            "private-page-rank-version-26",
            "private-recommendation-request-id-27",
            "private-event-candidate-28",
            "private-event-impression-29",
            "private-event-action-30",
            "private-event-source-31",
            "private-event-model-32",
            "private-metadata-key-33",
            "private-metadata-value-34",
        ] {
            assert!(
                !rendered.contains(sentinel),
                "{sentinel} leaked: {rendered}"
            );
        }
    }

    #[test]
    fn friend_dto_json_serialization_keeps_existing_wire_fields() {
        assert_eq!(
            serde_json::to_value(FollowRequest {
                target_user_id: "wire-target".into(),
            })
            .unwrap(),
            serde_json::json!({"targetUserId": "wire-target"})
        );

        assert_eq!(
            serde_json::to_value(PeopleRecommendationEventRequest {
                candidate_user_id: "wire-candidate".into(),
                impression_id: "wire-impression".into(),
                action: "CLICK".into(),
                source: "wire-source".into(),
                model_version: "wire-model".into(),
                position: Some(2),
                propensity_micros: Some(250_000),
                dwell_ms: Some(900),
                metadata: serde_json::json!({"context": "wire"}),
            })
            .unwrap(),
            serde_json::json!({
                "candidateUserId": "wire-candidate",
                "impressionId": "wire-impression",
                "action": "CLICK",
                "source": "wire-source",
                "modelVersion": "wire-model",
                "position": 2,
                "propensityMicros": 250_000,
                "dwellMs": 900,
                "metadata": {"context": "wire"}
            })
        );
    }
}
