use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use friend_service::application::ports::{
    FollowRepositoryPort, FollowRequestRepositoryPort, ProfilePrivacyPort,
    RelationshipPolicyRepositoryPort,
};
use friend_service::application::use_cases::accept_follow_request::AcceptFollowRequestUseCase;
use friend_service::application::use_cases::block_user::BlockUserUseCase;
use friend_service::application::use_cases::follow::FollowUseCase;
use friend_service::application::use_cases::mute_user::MuteUserUseCase;
use friend_service::domain::follow_counts::FollowCounts;
use friend_service::domain::follow_edge::{FollowCursor, FollowEdge};
use friend_service::domain::follow_request::{
    AcceptFollowRequestOutcome, AccountVisibility, FollowRequest, FollowRequestStatus,
};
use friend_service::domain::relationship_policy::RelationshipPolicy;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[tokio::test]
async fn private_account_follow_creates_pending_request_without_edge() {
    let repository = Arc::new(FakeSocialRepository::default());
    let privacy = FakePrivacy {
        visibility: AccountVisibility::Private,
    };
    let use_case = FollowUseCase::new(Arc::clone(&repository), privacy);
    let follower = uuid("10000000-0000-0000-0000-000000000001");
    let followee = uuid("10000000-0000-0000-0000-000000000002");

    let response = use_case
        .execute(follower, followee)
        .await
        .expect("pending follow");

    assert_eq!(response.status, "PENDING");
    assert!(!response.following);
    assert!(response.edge.is_none());
    assert!(response.request_id.is_some());
    assert!(repository.followed_edges.lock().unwrap().is_empty());
    assert_eq!(repository.requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn accepting_pending_request_creates_active_edge() {
    let repository = Arc::new(FakeSocialRepository::default());
    let follower = uuid("10000000-0000-0000-0000-000000000003");
    let followee = uuid("10000000-0000-0000-0000-000000000004");
    let request = repository
        .create_request(follower, followee, Utc::now())
        .await
        .expect("request");
    let use_case = AcceptFollowRequestUseCase::new(Arc::clone(&repository));

    let response = use_case
        .execute(request.id, followee)
        .await
        .expect("accept request");

    assert_eq!(response.status, "ACTIVE");
    assert!(response.following);
    assert_eq!(response.edge.unwrap().follower_id, follower.to_string());
    assert_eq!(repository.followed_edges.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn blocking_relationship_removes_edges_and_prevents_follow() {
    let repository = Arc::new(FakeSocialRepository::default());
    let viewer = uuid("10000000-0000-0000-0000-000000000005");
    let target = uuid("10000000-0000-0000-0000-000000000006");
    repository
        .followed_edges
        .lock()
        .unwrap()
        .push(FollowEdge::new(viewer, target, Utc::now()).unwrap());
    let block = BlockUserUseCase::new(Arc::clone(&repository));

    let policy = block.execute(viewer, target).await.expect("block");

    assert!(policy.blocked_by_viewer);
    assert!(repository.followed_edges.lock().unwrap().is_empty());
    let follow = FollowUseCase::new(
        Arc::clone(&repository),
        crate::public_profile_privacy::PublicProfilePrivacy,
    );
    let error = follow
        .execute(viewer, target)
        .await
        .expect_err("blocked follow");
    assert_eq!(error.to_string(), "Blocked relationship");
}

#[tokio::test]
async fn muting_preserves_follow_edge_and_marks_policy() {
    let repository = Arc::new(FakeSocialRepository::default());
    let viewer = uuid("10000000-0000-0000-0000-000000000007");
    let target = uuid("10000000-0000-0000-0000-000000000008");
    repository
        .followed_edges
        .lock()
        .unwrap()
        .push(FollowEdge::new(viewer, target, Utc::now()).unwrap());
    let mute = MuteUserUseCase::new(Arc::clone(&repository));

    let policy = mute.execute(viewer, target).await.expect("mute");

    assert!(policy.muted_by_viewer);
    assert!(policy.following);
    assert_eq!(repository.followed_edges.lock().unwrap().len(), 1);
}

#[derive(Clone, Copy)]
struct FakePrivacy {
    visibility: AccountVisibility,
}

#[async_trait]
impl ProfilePrivacyPort for FakePrivacy {
    async fn account_visibility(&self, _user_id: Uuid) -> Result<AccountVisibility> {
        Ok(self.visibility)
    }
}

#[derive(Default)]
struct FakeSocialRepository {
    followed_edges: Mutex<Vec<FollowEdge>>,
    requests: Mutex<Vec<FollowRequest>>,
    blocks: Mutex<Vec<(Uuid, Uuid)>>,
    mutes: Mutex<Vec<(Uuid, Uuid)>>,
}

#[async_trait]
impl FollowRepositoryPort for FakeSocialRepository {
    async fn follow(&self, edge: &FollowEdge) -> Result<FollowEdge> {
        if !self.followed_edges.lock().unwrap().iter().any(|value| {
            value.follower_id == edge.follower_id && value.followee_id == edge.followee_id
        }) {
            self.followed_edges.lock().unwrap().push(edge.clone());
        }
        Ok(edge.clone())
    }
    async fn unfollow(&self, follower_id: Uuid, followee_id: Uuid) -> Result<bool> {
        let mut edges = self.followed_edges.lock().unwrap();
        let before = edges.len();
        edges.retain(|edge| edge.follower_id != follower_id || edge.followee_id != followee_id);
        Ok(before != edges.len())
    }
    async fn list_followers(
        &self,
        _user_id: Uuid,
        _cursor: Option<FollowCursor>,
        _limit: i32,
    ) -> Result<(Vec<FollowEdge>, Option<String>, i64)> {
        Ok((Vec::new(), None, 0))
    }
    async fn list_following(
        &self,
        _user_id: Uuid,
        _cursor: Option<FollowCursor>,
        _limit: i32,
    ) -> Result<(Vec<FollowEdge>, Option<String>, i64)> {
        Ok((Vec::new(), None, 0))
    }
    async fn is_following(&self, follower_id: Uuid, followee_id: Uuid) -> Result<bool> {
        Ok(self
            .followed_edges
            .lock()
            .unwrap()
            .iter()
            .any(|edge| edge.follower_id == follower_id && edge.followee_id == followee_id))
    }
    async fn counts(&self, _user_id: Uuid) -> Result<FollowCounts> {
        Ok(FollowCounts::default())
    }
    async fn batch_counts(&self, _user_ids: &[Uuid]) -> Result<HashMap<Uuid, FollowCounts>> {
        Ok(HashMap::new())
    }
}

#[async_trait]
impl FollowRequestRepositoryPort for FakeSocialRepository {
    async fn create_request(
        &self,
        follower_id: Uuid,
        followee_id: Uuid,
        created_at: DateTime<Utc>,
    ) -> Result<FollowRequest> {
        if let Some(existing) = self
            .requests
            .lock()
            .unwrap()
            .iter()
            .find(|value| {
                value.follower_id == follower_id
                    && value.followee_id == followee_id
                    && value.status == FollowRequestStatus::Pending
            })
            .cloned()
        {
            return Ok(existing);
        }
        let request = FollowRequest {
            id: Uuid::new_v4(),
            follower_id,
            followee_id,
            status: FollowRequestStatus::Pending,
            created_at,
            decided_at: None,
        };
        self.requests.lock().unwrap().push(request.clone());
        Ok(request)
    }
    async fn accept_request(
        &self,
        request_id: Uuid,
        actor_id: Uuid,
        decided_at: DateTime<Utc>,
    ) -> Result<AcceptFollowRequestOutcome> {
        let mut requests = self.requests.lock().unwrap();
        let Some(request) = requests
            .iter_mut()
            .find(|value| value.id == request_id && value.followee_id == actor_id)
        else {
            return Ok(AcceptFollowRequestOutcome::NotFound);
        };
        request.status = FollowRequestStatus::Accepted;
        request.decided_at = Some(decided_at);
        let edge = FollowEdge::new(request.follower_id, request.followee_id, decided_at)?;
        self.followed_edges.lock().unwrap().push(edge.clone());
        Ok(AcceptFollowRequestOutcome::Accepted {
            request: request.clone(),
            edge,
        })
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
impl RelationshipPolicyRepositoryPort for FakeSocialRepository {
    async fn block(
        &self,
        blocker_id: Uuid,
        blocked_id: Uuid,
        _created_at: DateTime<Utc>,
    ) -> Result<RelationshipPolicy> {
        self.blocks.lock().unwrap().push((blocker_id, blocked_id));
        self.followed_edges.lock().unwrap().retain(|edge| {
            !((edge.follower_id == blocker_id && edge.followee_id == blocked_id)
                || (edge.follower_id == blocked_id && edge.followee_id == blocker_id))
        });
        self.policy(blocker_id, blocked_id).await
    }
    async fn unblock(&self, blocker_id: Uuid, blocked_id: Uuid) -> Result<RelationshipPolicy> {
        self.blocks
            .lock()
            .unwrap()
            .retain(|value| *value != (blocker_id, blocked_id));
        self.policy(blocker_id, blocked_id).await
    }
    async fn mute(
        &self,
        muter_id: Uuid,
        muted_id: Uuid,
        _created_at: DateTime<Utc>,
    ) -> Result<RelationshipPolicy> {
        self.mutes.lock().unwrap().push((muter_id, muted_id));
        self.policy(muter_id, muted_id).await
    }
    async fn unmute(&self, muter_id: Uuid, muted_id: Uuid) -> Result<RelationshipPolicy> {
        self.mutes
            .lock()
            .unwrap()
            .retain(|value| *value != (muter_id, muted_id));
        self.policy(muter_id, muted_id).await
    }
    async fn policy(&self, viewer_id: Uuid, target_id: Uuid) -> Result<RelationshipPolicy> {
        let (blocked_by_viewer, blocked_viewer) = {
            let blocks = self.blocks.lock().unwrap();
            (
                blocks.contains(&(viewer_id, target_id)),
                blocks.contains(&(target_id, viewer_id)),
            )
        };
        let muted_by_viewer = self.mutes.lock().unwrap().contains(&(viewer_id, target_id));
        let following = self
            .followed_edges
            .lock()
            .unwrap()
            .iter()
            .any(|edge| edge.follower_id == viewer_id && edge.followee_id == target_id);
        Ok(RelationshipPolicy {
            blocked_by_viewer,
            blocked_viewer,
            muted_by_viewer,
            following,
        })
    }
    async fn batch_policy(
        &self,
        viewer_id: Uuid,
        target_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, RelationshipPolicy>> {
        let mut output = HashMap::new();
        for target in target_ids {
            output.insert(*target, self.policy(viewer_id, *target).await?);
        }
        Ok(output)
    }
}

fn uuid(value: &str) -> Uuid {
    Uuid::parse_str(value).expect("uuid")
}
