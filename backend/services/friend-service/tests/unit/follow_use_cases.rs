use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use friend_service::application::ports::{
    FollowRepositoryPort, FollowRequestRepositoryPort, RelationshipPolicyRepositoryPort,
};
use friend_service::application::use_cases::check_follow::CheckFollowUseCase;
use friend_service::application::use_cases::follow::FollowUseCase;
use friend_service::application::use_cases::get_counts::GetCountsUseCase;
use friend_service::application::use_cases::get_stats_batch::GetStatsBatchUseCase;
use friend_service::application::use_cases::list_followers::ListFollowersUseCase;
use friend_service::application::use_cases::list_following::ListFollowingUseCase;
use friend_service::application::use_cases::unfollow::UnfollowUseCase;
use friend_service::domain::follow_counts::FollowCounts;
use friend_service::domain::follow_edge::{FollowCursor, FollowEdge};
use friend_service::domain::follow_request::{
    AcceptFollowRequestOutcome, FollowRequest, FollowRequestStatus,
};
use friend_service::domain::relationship_policy::RelationshipPolicy;
use proptest::prelude::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[tokio::test]
async fn follow_persists_valid_domain_edge_and_returns_following_true() {
    let repository = Arc::new(FakeFollowRepository::default());
    let use_case = FollowUseCase::new(
        Arc::clone(&repository),
        crate::public_profile_privacy::PublicProfilePrivacy,
    );
    let follower_id = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
    let followee_id = Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap();

    let response = use_case.execute(follower_id, followee_id).await.unwrap();

    assert!(response.following);
    let response_edge = response.edge.expect("follow response should include edge");
    assert_eq!(response_edge.follower_id, follower_id.to_string());
    assert_eq!(response_edge.followee_id, followee_id.to_string());

    let followed_edges = repository.followed_edges.lock().unwrap();
    assert_eq!(followed_edges.len(), 1);
    assert_eq!(followed_edges[0].follower_id, follower_id);
    assert_eq!(followed_edges[0].followee_id, followee_id);
}

#[tokio::test]
async fn self_follow_is_rejected_before_repository_mutation() {
    let repository = Arc::new(FakeFollowRepository::default());
    let use_case = FollowUseCase::new(
        Arc::clone(&repository),
        crate::public_profile_privacy::PublicProfilePrivacy,
    );
    let user_id = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();

    let error = use_case.execute(user_id, user_id).await.unwrap_err();

    assert_eq!(error.to_string(), "Cannot follow yourself");
    assert!(repository.followed_edges.lock().unwrap().is_empty());
}

#[tokio::test]
async fn list_followers_caps_huge_limit_and_defaults_non_positive_limit() {
    let repository = Arc::new(FakeFollowRepository::default());
    let use_case = ListFollowersUseCase::new(Arc::clone(&repository));
    let user_id = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();

    use_case.execute(user_id, None, 10_000).await.unwrap();
    use_case.execute(user_id, None, 0).await.unwrap();

    assert_eq!(*repository.followers_limits.lock().unwrap(), vec![100, 20]);
}

#[tokio::test]
async fn list_following_caps_huge_limit_and_defaults_non_positive_limit() {
    let repository = Arc::new(FakeFollowRepository::default());
    let use_case = ListFollowingUseCase::new(Arc::clone(&repository));
    let user_id = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();

    use_case.execute(user_id, None, 10_000).await.unwrap();
    use_case.execute(user_id, None, -1).await.unwrap();

    assert_eq!(*repository.following_limits.lock().unwrap(), vec![100, 20]);
}

#[tokio::test]
async fn unfollow_persists_relationship_removal_and_returns_following_false() {
    let repository = Arc::new(FakeFollowRepository::default());
    let use_case = UnfollowUseCase::new(Arc::clone(&repository));
    let follower_id = uuid("00000000-0000-0000-0000-000000000011");
    let followee_id = uuid("00000000-0000-0000-0000-000000000012");

    let response = use_case
        .execute(follower_id, followee_id)
        .await
        .expect("unfollow");

    assert!(!response.following);
    assert!(response.edge.is_none());
    assert_eq!(
        *repository.unfollow_calls.lock().unwrap(),
        vec![UnfollowCall {
            follower_id,
            followee_id,
        }]
    );
}

#[tokio::test]
async fn self_unfollow_is_rejected_before_repository_mutation() {
    let repository = Arc::new(FakeFollowRepository::default());
    let use_case = UnfollowUseCase::new(Arc::clone(&repository));
    let user_id = uuid("00000000-0000-0000-0000-000000000013");

    let error = use_case.execute(user_id, user_id).await.unwrap_err();

    assert_eq!(error.to_string(), "Cannot unfollow yourself");
    assert!(repository.unfollow_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn check_follow_returns_repository_answer_and_records_lookup() {
    let repository = Arc::new(FakeFollowRepository::default());
    *repository.is_following_result.lock().unwrap() = true;
    let use_case = CheckFollowUseCase::new(Arc::clone(&repository));
    let follower_id = uuid("00000000-0000-0000-0000-000000000014");
    let followee_id = uuid("00000000-0000-0000-0000-000000000015");

    let following = use_case
        .execute(follower_id, followee_id)
        .await
        .expect("check follow");

    assert!(following);
    assert_eq!(
        *repository.check_follow_calls.lock().unwrap(),
        vec![CheckFollowCall {
            follower_id,
            followee_id,
        }]
    );
}

#[tokio::test]
async fn get_counts_returns_repository_counts() {
    let repository = Arc::new(FakeFollowRepository::default());
    let user_id = uuid("00000000-0000-0000-0000-000000000016");
    repository.counts_result.lock().unwrap().insert(
        user_id,
        FollowCounts {
            follower_count: 7,
            following_count: 3,
        },
    );
    let use_case = GetCountsUseCase::new(Arc::clone(&repository));

    let response = use_case.execute(user_id).await.expect("get counts");

    assert_eq!(response.follower_count, 7);
    assert_eq!(response.following_count, 3);
    assert_eq!(*repository.counts_calls.lock().unwrap(), vec![user_id]);
}

#[tokio::test]
async fn batch_stats_preserves_requested_order_and_explicit_zero_counts() {
    let repository = Arc::new(FakeFollowRepository::default());
    let first = uuid("00000000-0000-0000-0000-000000000017");
    let missing = uuid("00000000-0000-0000-0000-000000000018");
    let second = uuid("00000000-0000-0000-0000-000000000019");
    repository.counts_result.lock().unwrap().extend([
        (
            second,
            FollowCounts {
                follower_count: 20,
                following_count: 2,
            },
        ),
        (
            first,
            FollowCounts {
                follower_count: 10,
                following_count: 1,
            },
        ),
        (missing, FollowCounts::default()),
    ]);
    let use_case = GetStatsBatchUseCase::new(Arc::clone(&repository));

    let response = use_case
        .execute(&[first, missing, second])
        .await
        .expect("batch stats");

    assert_eq!(response.len(), 3);
    assert_eq!(response[0].user_id, first.to_string());
    assert_eq!(response[0].follower_count, 10);
    assert_eq!(response[1].user_id, missing.to_string());
    assert_eq!(response[1].follower_count, 0);
    assert_eq!(response[2].user_id, second.to_string());
    assert_eq!(response[2].follower_count, 20);
    assert_eq!(
        *repository.batch_counts_calls.lock().unwrap(),
        vec![vec![first, missing, second]]
    );
}

#[tokio::test]
async fn batch_stats_rejects_repository_response_missing_a_requested_user() {
    let repository = Arc::new(FakeFollowRepository::default());
    let present = uuid("00000000-0000-0000-0000-000000000020");
    let missing = uuid("00000000-0000-0000-0000-000000000021");
    repository
        .counts_result
        .lock()
        .unwrap()
        .insert(present, FollowCounts::default());
    let use_case = GetStatsBatchUseCase::new(Arc::clone(&repository));

    let error = use_case
        .execute(&[present, missing])
        .await
        .expect_err("an omitted count must not be reported as a real zero");

    assert!(matches!(
        error,
        friend_service::domain::error::FriendError::Unexpected(_)
    ));
}

#[tokio::test]
async fn batch_stats_rejects_repository_response_with_an_unrequested_user() {
    let repository = Arc::new(FakeFollowRepository::default());
    let requested = uuid("00000000-0000-0000-0000-000000000022");
    let unexpected = uuid("00000000-0000-0000-0000-000000000023");
    repository.counts_result.lock().unwrap().extend([
        (requested, FollowCounts::default()),
        (unexpected, FollowCounts::default()),
    ]);
    let use_case = GetStatsBatchUseCase::new(Arc::clone(&repository));

    let error = use_case
        .execute(&[requested])
        .await
        .expect_err("an unrequested count must invalidate the batch response");

    assert!(matches!(
        error,
        friend_service::domain::error::FriendError::Unexpected(_)
    ));
}

#[test]
fn follow_cursor_round_trips_timestamp_and_tie_breaker() {
    let created_at = Utc
        .with_ymd_and_hms(2026, 8, 3, 1, 2, 3)
        .single()
        .expect("timestamp");
    let tie_breaker_id = uuid("00000000-0000-0000-0000-000000000099");
    let encoded = FollowCursor::new(created_at, tie_breaker_id).encode();

    let parsed = FollowCursor::parse(&encoded)
        .expect("valid cursor")
        .expect("cursor");

    assert_eq!(parsed.created_at, created_at);
    assert_eq!(parsed.tie_breaker_id, tie_breaker_id);
}

#[test]
fn follow_cursor_rejects_timestamp_only_cursor() {
    let error = FollowCursor::parse("2026-08-03T01:02:03Z")
        .expect_err("timestamp-only cursor must be rejected");
    assert_eq!(error.to_string(), "Invalid cursor");
}

#[test]
fn follow_cursor_rejects_oversized_whitespace_before_normalization() {
    let error = FollowCursor::parse(&" ".repeat(129))
        .expect_err("an oversized cursor must be rejected before trimming");
    assert_eq!(error.to_string(), "Invalid cursor");
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(proptest::test_runner::FileFailurePersistence::WithSource("proptest-regressions"))),
        ..ProptestConfig::default()
    })]

    #[test]
    fn follow_cursor_parser_is_bounded_and_round_trips_accepted_values(input in prop::collection::vec(any::<u8>(), 0..=512)) {
        let candidate = String::from_utf8_lossy(&input);
        let parsed = FollowCursor::parse(&candidate);
        if candidate.len() > 128 {
            prop_assert!(parsed.is_err());
        }
        if let Ok(Some(cursor)) = parsed {
            let replayed = FollowCursor::parse(&cursor.encode()).expect("encoded cursor remains valid");
            prop_assert_eq!(replayed, Some(cursor));
        }
    }
}

#[tokio::test]
async fn batch_stats_rejects_more_than_one_hundred_ids() {
    let repository = Arc::new(FakeFollowRepository::default());
    let use_case = GetStatsBatchUseCase::new(Arc::clone(&repository));
    let user_ids = (0..101).map(|_| Uuid::new_v4()).collect::<Vec<_>>();

    let error = use_case
        .execute(&user_ids)
        .await
        .expect_err("oversized follow batch must fail");

    assert_eq!(
        error.to_string(),
        "Follow stats support at most 100 user IDs"
    );
    assert!(repository.batch_counts_calls.lock().unwrap().is_empty());
}

#[derive(Default)]
struct FakeFollowRepository {
    followed_edges: Mutex<Vec<FollowEdge>>,
    followers_limits: Mutex<Vec<i32>>,
    following_limits: Mutex<Vec<i32>>,
    unfollow_calls: Mutex<Vec<UnfollowCall>>,
    check_follow_calls: Mutex<Vec<CheckFollowCall>>,
    counts_calls: Mutex<Vec<Uuid>>,
    batch_counts_calls: Mutex<Vec<Vec<Uuid>>>,
    is_following_result: Mutex<bool>,
    counts_result: Mutex<HashMap<Uuid, FollowCounts>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct UnfollowCall {
    follower_id: Uuid,
    followee_id: Uuid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CheckFollowCall {
    follower_id: Uuid,
    followee_id: Uuid,
}

#[async_trait]
impl FollowRepositoryPort for FakeFollowRepository {
    async fn follow(&self, edge: &FollowEdge) -> Result<FollowEdge> {
        self.followed_edges.lock().unwrap().push(edge.clone());
        Ok(edge.clone())
    }

    async fn unfollow(&self, follower_id: Uuid, followee_id: Uuid) -> Result<bool> {
        self.unfollow_calls.lock().unwrap().push(UnfollowCall {
            follower_id,
            followee_id,
        });
        Ok(true)
    }

    async fn list_followers(
        &self,
        _user_id: Uuid,
        _cursor: Option<FollowCursor>,
        limit: i32,
    ) -> Result<(Vec<FollowEdge>, Option<String>, i64)> {
        self.followers_limits.lock().unwrap().push(limit);
        Ok((Vec::new(), None, 0))
    }

    async fn list_following(
        &self,
        _user_id: Uuid,
        _cursor: Option<FollowCursor>,
        limit: i32,
    ) -> Result<(Vec<FollowEdge>, Option<String>, i64)> {
        self.following_limits.lock().unwrap().push(limit);
        Ok((Vec::new(), None, 0))
    }

    async fn is_following(&self, follower_id: Uuid, followee_id: Uuid) -> Result<bool> {
        self.check_follow_calls
            .lock()
            .unwrap()
            .push(CheckFollowCall {
                follower_id,
                followee_id,
            });
        Ok(*self.is_following_result.lock().unwrap())
    }

    async fn counts(&self, user_id: Uuid) -> Result<FollowCounts> {
        self.counts_calls.lock().unwrap().push(user_id);
        Ok(self
            .counts_result
            .lock()
            .unwrap()
            .get(&user_id)
            .copied()
            .unwrap_or(FollowCounts {
                follower_count: 0,
                following_count: 0,
            }))
    }

    async fn batch_counts(&self, user_ids: &[Uuid]) -> Result<HashMap<Uuid, FollowCounts>> {
        self.batch_counts_calls
            .lock()
            .unwrap()
            .push(user_ids.to_vec());
        Ok(self.counts_result.lock().unwrap().clone())
    }
}

#[async_trait]
impl FollowRequestRepositoryPort for FakeFollowRepository {
    async fn create_request(
        &self,
        follower_id: Uuid,
        followee_id: Uuid,
        created_at: DateTime<Utc>,
    ) -> Result<FollowRequest> {
        Ok(FollowRequest {
            id: Uuid::new_v4(),
            follower_id,
            followee_id,
            status: FollowRequestStatus::Pending,
            created_at,
            decided_at: None,
        })
    }

    async fn accept_request(
        &self,
        _request_id: Uuid,
        _actor_id: Uuid,
        _decided_at: DateTime<Utc>,
    ) -> Result<AcceptFollowRequestOutcome> {
        Ok(AcceptFollowRequestOutcome::NotFound)
    }

    async fn reject_request(
        &self,
        _request_id: Uuid,
        _actor_id: Uuid,
        _decided_at: DateTime<Utc>,
    ) -> Result<Option<FollowRequest>> {
        Ok(None)
    }

    async fn cancel_request(
        &self,
        _request_id: Uuid,
        _actor_id: Uuid,
        _decided_at: DateTime<Utc>,
    ) -> Result<Option<FollowRequest>> {
        Ok(None)
    }

    async fn list_incoming_requests(
        &self,
        _user_id: Uuid,
        _cursor: Option<FollowCursor>,
        _limit: i32,
    ) -> Result<(Vec<FollowRequest>, Option<String>)> {
        Ok((Vec::new(), None))
    }

    async fn list_outgoing_requests(
        &self,
        _user_id: Uuid,
        _cursor: Option<FollowCursor>,
        _limit: i32,
    ) -> Result<(Vec<FollowRequest>, Option<String>)> {
        Ok((Vec::new(), None))
    }
}

#[async_trait]
impl RelationshipPolicyRepositoryPort for FakeFollowRepository {
    async fn block(
        &self,
        _blocker_id: Uuid,
        _blocked_id: Uuid,
        _created_at: DateTime<Utc>,
    ) -> Result<RelationshipPolicy> {
        Ok(RelationshipPolicy::default())
    }

    async fn unblock(&self, _blocker_id: Uuid, _blocked_id: Uuid) -> Result<RelationshipPolicy> {
        Ok(RelationshipPolicy::default())
    }

    async fn mute(
        &self,
        _muter_id: Uuid,
        _muted_id: Uuid,
        _created_at: DateTime<Utc>,
    ) -> Result<RelationshipPolicy> {
        Ok(RelationshipPolicy::default())
    }

    async fn unmute(&self, _muter_id: Uuid, _muted_id: Uuid) -> Result<RelationshipPolicy> {
        Ok(RelationshipPolicy::default())
    }

    async fn policy(&self, viewer_id: Uuid, target_id: Uuid) -> Result<RelationshipPolicy> {
        Ok(RelationshipPolicy {
            following: self
                .followed_edges
                .lock()
                .unwrap()
                .iter()
                .any(|edge| edge.follower_id == viewer_id && edge.followee_id == target_id),
            ..RelationshipPolicy::default()
        })
    }

    async fn batch_policy(
        &self,
        viewer_id: Uuid,
        target_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, RelationshipPolicy>> {
        Ok(target_ids
            .iter()
            .copied()
            .map(|target_id| {
                (
                    target_id,
                    RelationshipPolicy {
                        following: self.followed_edges.lock().unwrap().iter().any(|edge| {
                            edge.follower_id == viewer_id && edge.followee_id == target_id
                        }),
                        ..RelationshipPolicy::default()
                    },
                )
            })
            .collect())
    }
}

fn uuid(value: &str) -> Uuid {
    Uuid::parse_str(value).expect("valid uuid")
}
