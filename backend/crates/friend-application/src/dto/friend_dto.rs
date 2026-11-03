use friend_domain::follow_counts::FollowCounts;
use friend_domain::follow_edge::FollowEdge;
use friend_domain::follow_request::FollowRequest;
use friend_domain::relationship_policy::RelationshipPolicy;
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

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowEdgeDto {
    pub follower_id: String,
    pub followee_id: String,
    pub created_at: String,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowListDto {
    pub edges: Vec<FollowEdgeDto>,
    pub next_cursor: Option<String>,
    pub total_count: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowCountsDto {
    pub follower_count: i64,
    pub following_count: i64,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowActionDto {
    pub edge: Option<FollowEdgeDto>,
    pub following: bool,
    pub status: String,
    pub request_id: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowRequestDto {
    pub id: String,
    pub follower_id: String,
    pub followee_id: String,
    pub status: String,
    pub created_at: String,
    pub decided_at: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowRequestPageDto {
    pub requests: Vec<FollowRequestDto>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipPolicyDto {
    pub blocked_by_viewer: bool,
    pub blocked_viewer: bool,
    pub muted_by_viewer: bool,
    pub following: bool,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowStatsDto {
    pub user_id: String,
    pub follower_count: i64,
    pub following_count: i64,
}

impl From<FollowEdge> for FollowEdgeDto {
    fn from(value: FollowEdge) -> Self {
        Self {
            follower_id: value.follower_id.to_string(),
            followee_id: value.followee_id.to_string(),
            created_at: value.created_at.to_rfc3339(),
        }
    }
}

impl From<FollowCounts> for FollowCountsDto {
    fn from(value: FollowCounts) -> Self {
        Self {
            follower_count: value.follower_count,
            following_count: value.following_count,
        }
    }
}

impl From<FollowRequest> for FollowRequestDto {
    fn from(value: FollowRequest) -> Self {
        Self {
            id: value.id.to_string(),
            follower_id: value.follower_id.to_string(),
            followee_id: value.followee_id.to_string(),
            status: value.status.as_str().to_owned(),
            created_at: value.created_at.to_rfc3339(),
            decided_at: value.decided_at.map(|value| value.to_rfc3339()),
        }
    }
}

impl From<RelationshipPolicy> for RelationshipPolicyDto {
    fn from(value: RelationshipPolicy) -> Self {
        Self {
            blocked_by_viewer: value.blocked_by_viewer,
            blocked_viewer: value.blocked_viewer,
            muted_by_viewer: value.muted_by_viewer,
            following: value.following,
        }
    }
}

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

redacted_debug! {
    FollowEdgeDto { follower_id, followee_id, created_at },
    FollowActionDto { edge, following, status, request_id },
    FollowRequestDto { id, follower_id, followee_id, status, created_at, decided_at },
    RelationshipPolicyDto { blocked_by_viewer, blocked_viewer, muted_by_viewer, following },
    FollowStatsDto { user_id, follower_count, following_count },
    ApplyUserDeletionDto { saga_id, user_id, actor_id, retention_until },
}

impl std::fmt::Debug for FollowListDto {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FollowListDto")
            .field("edges_len", &self.edges.len())
            .field("next_cursor", &"[REDACTED]")
            .field("total_count", &"[REDACTED]")
            .finish()
    }
}

impl std::fmt::Debug for FollowRequestPageDto {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FollowRequestPageDto")
            .field("requests_len", &self.requests.len())
            .field("next_cursor", &"[REDACTED]")
            .finish()
    }
}

#[cfg(test)]
mod secret_debug_tests {
    use super::*;

    #[test]
    fn follow_dtos_redact_social_graph_and_request_state_without_changing_serde() {
        let edge = FollowEdgeDto {
            follower_id: "private-follower-id".into(),
            followee_id: "private-followee-id".into(),
            created_at: "private-follow-created-at".into(),
        };
        let request = FollowRequestDto {
            id: "private-request-id".into(),
            follower_id: "private-request-follower".into(),
            followee_id: "private-request-followee".into(),
            status: "PENDING_PRIVATE".into(),
            created_at: "private-request-created-at".into(),
            decided_at: Some("private-request-decided-at".into()),
        };
        let list = FollowListDto {
            edges: vec![edge.clone()],
            next_cursor: Some("private-follow-cursor".into()),
            total_count: 17,
        };
        let action = FollowActionDto {
            edge: Some(edge.clone()),
            following: false,
            status: "PRIVATE_ACTION_STATUS".into(),
            request_id: Some("private-action-request".into()),
        };
        let request_page = FollowRequestPageDto {
            requests: vec![request.clone()],
            next_cursor: Some("private-request-cursor".into()),
        };
        let policy = RelationshipPolicyDto {
            blocked_by_viewer: true,
            blocked_viewer: false,
            muted_by_viewer: true,
            following: false,
        };
        let stats = FollowStatsDto {
            user_id: "private-stats-user-id".into(),
            follower_count: 18,
            following_count: 19,
        };
        let apply_deletion = ApplyUserDeletionDto {
            saga_id: "private-deletion-saga".into(),
            user_id: "private-deletion-user".into(),
            actor_id: "private-deletion-actor".into(),
            retention_until: "private-retention-date".into(),
        };

        let rendered = [
            format!("{edge:?}"),
            format!("{request:?}"),
            format!("{list:?}"),
            format!("{action:?}"),
            format!("{request_page:?}"),
            format!("{policy:?}"),
            format!("{stats:?}"),
            format!("{apply_deletion:?}"),
        ]
        .join("\n");
        for private_value in [
            "private-follower-id",
            "private-followee-id",
            "private-follow-created-at",
            "private-request-id",
            "private-request-follower",
            "private-request-followee",
            "PENDING_PRIVATE",
            "private-request-created-at",
            "private-request-decided-at",
            "private-follow-cursor",
            "PRIVATE_ACTION_STATUS",
            "private-action-request",
            "private-request-cursor",
            "private-stats-user-id",
            "private-deletion-saga",
            "private-deletion-user",
            "private-deletion-actor",
            "private-retention-date",
        ] {
            assert!(!rendered.contains(private_value), "{rendered}");
        }
        assert!(rendered.contains("edges_len: 1"), "{rendered}");
        assert!(rendered.contains("requests_len: 1"), "{rendered}");
        assert!(rendered.contains("[REDACTED]"), "{rendered}");

        let serialized = serde_json::to_value(&request).expect("serialize FollowRequest DTO");
        assert_eq!(serialized["followerId"], "private-request-follower");
        assert_eq!(serialized["status"], "PENDING_PRIVATE");
    }
}
