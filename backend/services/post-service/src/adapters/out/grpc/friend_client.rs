use super::authority_exchange_client::GrpcAuthorizationExchangeClient;
use crate::application::ports::FriendClientPort;
use crate::domain::post::RelationshipPolicySnapshot;
use anyhow::{anyhow, ensure, Context, Result};
use async_trait::async_trait;
use platform::edge::Edge;
use platform::edge_metrics::EdgeDeadlineMetrics;
use platform::grpc_auth::{ServiceIdentityClientInterceptor, Workload};
use platform::grpc_client_trace::GrpcClientTraceService;
use platform::grpc_deadline::EdgeDeadlineCallExt;
use post_application::query_witness::PublishedWitnessStageDto;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tonic::service::interceptor::InterceptedService;
use tonic::transport::Channel;
use uuid::Uuid;

pub mod proto {
    tonic::include_proto!("friend.v1");
}

const LIST_FOLLOWING: &str = "/friend.v1.FriendService/ListFollowing";
const LIST_FOLLOWERS: &str = "/friend.v1.FriendService/ListFollowers";
const GET_RELATIONSHIP: &str = "/friend.v1.FriendService/GetRelationship";
const BATCH_GET_RELATIONSHIPS: &str = "/friend.v1.FriendService/BatchGetRelationships";

#[derive(Clone)]
pub struct GrpcFriendClient {
    client: proto::friend_service_client::FriendServiceClient<
        InterceptedService<GrpcClientTraceService<Channel>, ServiceIdentityClientInterceptor>,
    >,
    edge_deadlines: EdgeDeadlineMetrics,
    exchange: Option<Arc<GrpcAuthorizationExchangeClient>>,
}

impl GrpcFriendClient {
    pub fn with_exchange(mut self, exchange: Arc<GrpcAuthorizationExchangeClient>) -> Self {
        self.exchange = Some(exchange);
        self
    }
    async fn interactive<T: prost::Message>(
        &self,
        method: &str,
        message: T,
        stage: Option<&PublishedWitnessStageDto>,
    ) -> Result<tonic::Request<T>> {
        match &self.exchange {
            Some(exchange) => Ok(exchange
                .authorize(method, Workload::Friend, message, stage)
                .await?),
            None if stage.is_some() => Err(tonic::Status::unavailable(
                "Forwarded relationship exchange is not configured",
            )
            .into()),
            None => Ok(tonic::Request::new(message)),
        }
    }

    /// One ListFollowing page, retried as a replay-safe read with a fresh
    /// child authority per attempt. An exchange failure is returned at once.
    async fn following_page(
        &self,
        request: proto::ListFollowingRequest,
    ) -> Result<proto::ListFollowingResponse> {
        let (this, request) = (self, &request);
        Ok(platform::grpc_retry::read(
            Edge::PostFriend,
            LIST_FOLLOWING,
            &self.edge_deadlines,
            move || async move {
                let request = this
                    .interactive(LIST_FOLLOWING, request.clone(), None)
                    .await
                    .map_err(platform::grpc_retry::not_retried)?;
                anyhow::Ok(
                    this.client
                        .clone()
                        .list_following(request)
                        .with_edge_deadline(Edge::PostFriend, this.edge_deadlines.clone())
                        .await?,
                )
            },
        )
        .await?
        .into_inner())
    }

    async fn followers_page(
        &self,
        request: proto::ListFollowersRequest,
    ) -> Result<proto::ListFollowersResponse> {
        let (this, request) = (self, &request);
        Ok(platform::grpc_retry::read(
            Edge::PostFriend,
            LIST_FOLLOWERS,
            &self.edge_deadlines,
            move || async move {
                this.client
                    .clone()
                    .list_followers(request.clone())
                    .with_edge_deadline(Edge::PostFriend, this.edge_deadlines.clone())
                    .await
            },
        )
        .await?
        .into_inner())
    }

    /// A relationship read. With a witness stage the exchange binds published
    /// query evidence, so it runs once; without one it is a replay-safe read.
    async fn relationship_read<T, R, F, Fut>(
        &self,
        method: &'static str,
        message: T,
        stage: Option<&PublishedWitnessStageDto>,
        call: F,
    ) -> Result<R>
    where
        T: prost::Message + Clone,
        F: Fn(tonic::Request<T>) -> Fut,
        Fut: std::future::Future<Output = std::result::Result<tonic::Response<R>, tonic::Status>>
            + Send,
        R: Send,
    {
        if stage.is_some() {
            let request = self.interactive(method, message, stage).await?;
            return Ok(call(request)
                .with_edge_deadline(Edge::PostFriend, self.edge_deadlines.clone())
                .await?
                .into_inner());
        }
        let (this, message, call) = (self, &message, &call);
        Ok(platform::grpc_retry::read(
            Edge::PostFriend,
            method,
            &self.edge_deadlines,
            move || async move {
                let request = this
                    .interactive(method, message.clone(), None)
                    .await
                    .map_err(platform::grpc_retry::not_retried)?;
                anyhow::Ok(
                    call(request)
                        .with_edge_deadline(Edge::PostFriend, this.edge_deadlines.clone())
                        .await?,
                )
            },
        )
        .await?
        .into_inner())
    }
    /// Build the client without waiting for the peer to answer.
    ///
    /// The channel connects on first use, so a dependency that is down
    /// does not stop this service from starting. Configuration is still
    /// validated here: an invalid endpoint or TLS material fails now.
    pub async fn connect(
        endpoint: String,
        caller: Workload,
        service_token: &str,
        tls_material: Option<&platform::grpc_tls::TlsMaterial>,
    ) -> Result<Self> {
        Self::connect_with_metrics(
            endpoint,
            caller,
            service_token,
            tls_material,
            EdgeDeadlineMetrics::default(),
        )
        .await
    }

    pub async fn connect_with_metrics(
        endpoint: String,
        caller: Workload,
        service_token: &str,
        tls_material: Option<&platform::grpc_tls::TlsMaterial>,
        edge_deadlines: EdgeDeadlineMetrics,
    ) -> Result<Self> {
        // The peer is the friend service; `caller` is this workload's own identity.
        let channel = platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
            tls_material,
            &endpoint,
            Workload::Friend,
            platform::edge::Edge::PostFriend.budget(),
        )?;
        Ok(Self {
            client: proto::friend_service_client::FriendServiceClient::with_interceptor(
                GrpcClientTraceService::new(channel),
                ServiceIdentityClientInterceptor::new(caller, service_token)?,
            ),
            edge_deadlines,
            exchange: None,
        })
    }
}

#[async_trait]
impl FriendClientPort for GrpcFriendClient {
    async fn fetch_feed_authors(
        &self,
        viewer_id: Uuid,
    ) -> Result<post_application::feed_authors::FeedAuthorsDto> {
        ensure!(!viewer_id.is_nil(), "Feed viewer must be non-nil");
        // Friend's ListFollowing query owns both membership and block/mute
        // filtering. Its response is query data, not evidence for issuing an
        // arbitrary-target relationship child. That read follows the Post query.
        let authors = self.fetch_following_ids(viewer_id).await?;
        post_application::feed_authors::FeedAuthorsDto::new(viewer_id, authors)
    }

    async fn fetch_following_ids(&self, user_id: Uuid) -> Result<Vec<Uuid>> {
        let mut cursor = String::new();
        let mut cursors = HashSet::from([String::new()]);
        let mut user_ids = Vec::new();
        loop {
            let response = self
                .following_page(proto::ListFollowingRequest {
                    user_id: user_id.to_string(),
                    cursor: cursor.clone(),
                    limit: 100,
                })
                .await?;
            user_ids.extend(checked_following_authors(user_id, &response.edges, 100)?);
            let next_cursor = response.next_cursor;
            if next_cursor.is_empty() {
                break;
            }
            ensure!(
                cursors.insert(next_cursor.clone()),
                "Friend service returned a cyclic following cursor"
            );
            cursor = next_cursor;
        }
        user_ids.sort_unstable();
        user_ids.dedup();
        Ok(user_ids)
    }

    async fn fetch_following_ids_bounded(
        &self,
        user_id: Uuid,
        max_user_ids: usize,
    ) -> Result<Vec<Uuid>> {
        let max_user_ids = max_user_ids.clamp(1, 10_000);
        let mut cursor = String::new();
        let mut cursors = HashSet::from([String::new()]);
        let mut user_ids = Vec::with_capacity(max_user_ids.min(500));
        while user_ids.len() < max_user_ids {
            let remaining = max_user_ids - user_ids.len();
            let response = self
                .following_page(proto::ListFollowingRequest {
                    user_id: user_id.to_string(),
                    cursor: cursor.clone(),
                    limit: i32::try_from(remaining.min(100))?,
                })
                .await?;
            user_ids.extend(checked_following_authors(
                user_id,
                &response.edges,
                remaining.min(100),
            )?);
            if response.next_cursor.is_empty() || user_ids.len() >= max_user_ids {
                break;
            }
            ensure!(
                cursors.insert(response.next_cursor.clone()),
                "Friend service returned a cyclic following cursor"
            );
            cursor = response.next_cursor;
        }
        user_ids.sort_unstable();
        user_ids.dedup();
        user_ids.truncate(max_user_ids);
        Ok(user_ids)
    }

    async fn fetch_follower_ids(&self, user_id: Uuid) -> Result<Vec<Uuid>> {
        let mut cursor = String::new();
        let mut user_ids = Vec::new();
        loop {
            let response = self
                .followers_page(proto::ListFollowersRequest {
                    user_id: user_id.to_string(),
                    cursor: cursor.clone(),
                    limit: 100,
                })
                .await?;
            user_ids.extend(
                response
                    .edges
                    .into_iter()
                    .map(|edge| edge.follower_id.parse().map_err(anyhow::Error::from))
                    .collect::<Result<Vec<Uuid>>>()?,
            );
            let next_cursor = response.next_cursor;
            if next_cursor.is_empty() {
                break;
            }
            if next_cursor == cursor {
                anyhow::bail!("Friend service returned a non-advancing follower cursor");
            }
            cursor = next_cursor;
        }
        user_ids.sort_unstable();
        user_ids.dedup();
        Ok(user_ids)
    }

    async fn fetch_follower_page(
        &self,
        user_id: Uuid,
        cursor: Option<&str>,
        limit: i32,
    ) -> Result<crate::domain::recommendation::FollowerPage> {
        let response = self
            .followers_page(proto::ListFollowersRequest {
                user_id: user_id.to_string(),
                cursor: cursor.unwrap_or_default().to_owned(),
                limit: limit.clamp(1, 100),
            })
            .await?;
        Ok(crate::domain::recommendation::FollowerPage {
            follower_ids: response
                .edges
                .into_iter()
                .map(|edge| Uuid::parse_str(&edge.follower_id).map_err(anyhow::Error::from))
                .collect::<Result<Vec<_>>>()?,
            next_cursor: (!response.next_cursor.is_empty()).then_some(response.next_cursor),
            total_count: response.total_count,
        })
    }

    async fn follower_count(&self, user_id: Uuid) -> Result<i64> {
        let this = self;
        Ok(platform::grpc_retry::read(
            Edge::PostFriend,
            "/friend.v1.FriendService/GetFollowCounts",
            &self.edge_deadlines,
            move || async move {
                this.client
                    .clone()
                    .get_follow_counts(proto::GetFollowCountsRequest {
                        user_id: user_id.to_string(),
                    })
                    .with_edge_deadline(Edge::PostFriend, this.edge_deadlines.clone())
                    .await
            },
        )
        .await?
        .into_inner()
        .follower_count)
    }

    async fn is_following(&self, follower_id: Uuid, followee_id: Uuid) -> Result<bool> {
        Ok(self
            .relationship_policy(follower_id, followee_id, None)
            .await?
            .following)
    }

    async fn relationship_policy(
        &self,
        viewer_id: Uuid,
        target_id: Uuid,
        _stage: Option<&PublishedWitnessStageDto>,
    ) -> Result<RelationshipPolicySnapshot> {
        let response = self
            .relationship_read(
                GET_RELATIONSHIP,
                proto::RelationshipRequest {
                    viewer_id: viewer_id.to_string(),
                    target_id: target_id.to_string(),
                },
                _stage,
                |request| {
                    let mut client = self.client.clone();
                    async move { client.get_relationship(request).await }
                },
            )
            .await?;
        Ok(map_policy(response))
    }

    async fn batch_relationship_policies(
        &self,
        viewer_id: Uuid,
        target_ids: &[Uuid],
        _stage: Option<&PublishedWitnessStageDto>,
    ) -> Result<std::collections::HashMap<Uuid, RelationshipPolicySnapshot>> {
        if target_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }
        if target_ids.len() > 100 {
            anyhow::bail!("Relationship batch supports at most 100 target IDs");
        }
        let mut seen = HashSet::with_capacity(target_ids.len());
        let unique_target_ids = target_ids
            .iter()
            .copied()
            .filter(|target_id| seen.insert(*target_id))
            .collect::<Vec<_>>();
        let response = self
            .relationship_read(
                BATCH_GET_RELATIONSHIPS,
                proto::BatchRelationshipRequest {
                    viewer_id: viewer_id.to_string(),
                    target_ids: unique_target_ids.iter().map(ToString::to_string).collect(),
                },
                _stage,
                |request| {
                    let mut client = self.client.clone();
                    async move { client.batch_get_relationships(request).await }
                },
            )
            .await?;
        map_batch_policies(&unique_target_ids, response.policies)
    }
}

fn map_batch_policies(
    target_ids: &[Uuid],
    entries: Vec<proto::RelationshipPolicyEntry>,
) -> Result<HashMap<Uuid, RelationshipPolicySnapshot>> {
    let expected = target_ids.iter().copied().collect::<HashSet<_>>();
    ensure!(
        expected.len() == target_ids.len(),
        "requested relationship targets must be unique"
    );
    ensure!(
        entries.len() == expected.len(),
        "Friend service returned an incomplete relationship-policy batch"
    );

    let mut policies = HashMap::with_capacity(entries.len());
    for entry in entries {
        let target_id = Uuid::parse_str(&entry.target_id)
            .context("Friend service returned an invalid relationship target ID")?;
        ensure!(
            expected.contains(&target_id),
            "Friend service returned an unrequested relationship target"
        );
        let policy = entry
            .policy
            .ok_or_else(|| anyhow!("Friend service omitted a relationship policy"))?;
        ensure!(
            policies.insert(target_id, map_policy(policy)).is_none(),
            "Friend service returned a duplicate relationship policy"
        );
    }
    ensure!(
        policies.len() == expected.len(),
        "Friend service omitted a requested relationship policy"
    );
    Ok(policies)
}

fn map_policy(value: proto::RelationshipPolicy) -> RelationshipPolicySnapshot {
    RelationshipPolicySnapshot {
        blocked_by_viewer: value.blocked_by_viewer,
        blocked_viewer: value.blocked_viewer,
        muted_by_viewer: value.muted_by_viewer,
        following: value.following,
    }
}

// Every returned edge must belong to the authenticated request's viewer.
// Validate the whole page before exposing any author data to the application.
fn checked_following_authors(
    viewer: Uuid,
    edges: &[proto::FollowEdge],
    limit: usize,
) -> Result<Vec<Uuid>> {
    ensure!(
        edges.len() <= limit,
        "Friend service exceeded the requested following page"
    );
    edges
        .iter()
        .map(|edge| {
            ensure!(
                edge.follower_id.parse::<Uuid>().ok() == Some(viewer),
                "Friend service returned another follower"
            );
            let author = edge
                .followee_id
                .parse::<Uuid>()
                .context("Invalid following author")?;
            ensure!(!author.is_nil(), "Invalid following author");
            Ok(author)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn following_page_requires_exact_viewer_non_nil_author_and_requested_bound() {
        let viewer = uuid::Uuid::from_u128(1);
        let author = uuid::Uuid::from_u128(2);
        let edge = super::proto::FollowEdge {
            follower_id: viewer.to_string(),
            followee_id: author.to_string(),
            created_at: String::new(),
        };
        assert_eq!(
            super::checked_following_authors(viewer, &[edge.clone()], 1).unwrap(),
            vec![author]
        );
        assert!(
            super::checked_following_authors(uuid::Uuid::from_u128(3), &[edge.clone()], 1).is_err()
        );
        assert!(super::checked_following_authors(viewer, &[edge.clone()], 0).is_err());
        let mut invalid = edge;
        invalid.followee_id = uuid::Uuid::nil().to_string();
        assert!(super::checked_following_authors(viewer, &[invalid.clone()], 1).is_err());
        invalid.followee_id = "invalid".into();
        assert!(super::checked_following_authors(viewer, &[invalid], 1).is_err());
    }

    use super::*;

    fn entry(
        target_id: Uuid,
        policy: Option<proto::RelationshipPolicy>,
    ) -> proto::RelationshipPolicyEntry {
        proto::RelationshipPolicyEntry {
            target_id: target_id.to_string(),
            policy,
        }
    }

    #[test]
    fn batch_relationship_mapping_requires_one_policy_for_each_requested_target() {
        let first = Uuid::from_u128(1);
        let second = Uuid::from_u128(2);

        let policies = map_batch_policies(
            &[first, second],
            vec![
                entry(
                    first,
                    Some(proto::RelationshipPolicy {
                        blocked_by_viewer: true,
                        ..Default::default()
                    }),
                ),
                entry(second, Some(proto::RelationshipPolicy::default())),
            ],
        )
        .expect("complete response");

        assert!(policies[&first].blocked_by_viewer);
        assert_eq!(policies[&second], RelationshipPolicySnapshot::default());
    }

    #[test]
    fn batch_relationship_mapping_rejects_missing_and_unexpected_targets() {
        let requested = Uuid::from_u128(3);
        let other = Uuid::from_u128(4);

        assert!(map_batch_policies(&[requested], Vec::new()).is_err());
        assert!(
            map_batch_policies(&[requested], vec![entry(other, Some(Default::default()))]).is_err()
        );
    }

    #[test]
    fn batch_relationship_mapping_rejects_missing_policy_and_duplicate_targets() {
        let first = Uuid::from_u128(5);
        let second = Uuid::from_u128(6);

        assert!(map_batch_policies(&[first], vec![entry(first, None)]).is_err());
        assert!(map_batch_policies(
            &[first, second],
            vec![
                entry(first, Some(Default::default())),
                entry(first, Some(Default::default())),
            ],
        )
        .is_err());
    }
}
