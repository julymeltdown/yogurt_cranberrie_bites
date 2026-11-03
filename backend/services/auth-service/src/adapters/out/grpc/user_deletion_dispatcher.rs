use crate::application::ports::UserDeletionDispatcherPort;
use crate::domain::user_deletion::UserDeletionTarget;
use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use platform::edge::Edge;
use platform::edge_metrics::EdgeDeadlineMetrics;
use platform::grpc_auth::{ServiceIdentityClientInterceptor, Workload};
use platform::grpc_client_trace::GrpcClientTraceService;
use platform::grpc_deadline::EdgeDeadlineCallExt;
use std::sync::Arc;
use tonic::service::interceptor::InterceptedService;
use tonic::transport::Channel;
use uuid::Uuid;

pub mod profile_proto {
    tonic::include_proto!("profile.v1");
}
pub mod friend_proto {
    tonic::include_proto!("friend.v1");
}
pub mod post_proto {
    tonic::include_proto!("post.v1");
}
pub mod notification_proto {
    tonic::include_proto!("notification.v1");
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserDeletionDispatchRequest {
    pub saga_id: Uuid,
    pub user_id: Uuid,
    pub actor_id: Uuid,
    pub retention_until: DateTime<Utc>,
}

impl UserDeletionDispatchRequest {
    fn strings(&self) -> (String, String, String, String) {
        (
            self.saga_id.to_string(),
            self.user_id.to_string(),
            self.actor_id.to_string(),
            self.retention_until.to_rfc3339(),
        )
    }
}

#[async_trait]
pub trait UserDeletionTargetClientPort: Send + Sync {
    async fn apply(&self, request: UserDeletionDispatchRequest) -> Result<()>;
}

type ServiceChannel =
    InterceptedService<GrpcClientTraceService<Channel>, ServiceIdentityClientInterceptor>;

#[derive(Clone)]
struct ProfileDeletionClient {
    client: profile_proto::profile_service_client::ProfileServiceClient<ServiceChannel>,
    edge_deadlines: EdgeDeadlineMetrics,
}

#[async_trait]
impl UserDeletionTargetClientPort for ProfileDeletionClient {
    async fn apply(&self, request: UserDeletionDispatchRequest) -> Result<()> {
        let (saga_id, user_id, actor_id, retention_until) = request.strings();
        let mut client = self.client.clone();
        client
            .apply_user_deletion(profile_proto::ApplyUserDeletionRequest {
                saga_id,
                user_id,
                actor_id,
                retention_until,
            })
            .with_edge_deadline(Edge::AuthProfile, self.edge_deadlines.clone())
            .await?;
        Ok(())
    }
}

#[derive(Clone)]
struct FriendDeletionClient {
    client: friend_proto::friend_service_client::FriendServiceClient<ServiceChannel>,
    edge_deadlines: EdgeDeadlineMetrics,
}

#[async_trait]
impl UserDeletionTargetClientPort for FriendDeletionClient {
    async fn apply(&self, request: UserDeletionDispatchRequest) -> Result<()> {
        let (saga_id, user_id, actor_id, retention_until) = request.strings();
        let mut client = self.client.clone();
        client
            .apply_user_deletion(friend_proto::ApplyUserDeletionRequest {
                saga_id,
                user_id,
                actor_id,
                retention_until,
            })
            .with_edge_deadline(Edge::AuthFriend, self.edge_deadlines.clone())
            .await?;
        Ok(())
    }
}

#[derive(Clone)]
struct PostDeletionClient {
    client: post_proto::post_service_client::PostServiceClient<ServiceChannel>,
    edge_deadlines: EdgeDeadlineMetrics,
}

#[async_trait]
impl UserDeletionTargetClientPort for PostDeletionClient {
    async fn apply(&self, request: UserDeletionDispatchRequest) -> Result<()> {
        let (saga_id, user_id, actor_id, retention_until) = request.strings();
        let mut client = self.client.clone();
        client
            .apply_user_deletion(post_proto::ApplyUserDeletionRequest {
                saga_id,
                user_id,
                actor_id,
                retention_until,
            })
            .with_edge_deadline(Edge::AuthPost, self.edge_deadlines.clone())
            .await?;
        Ok(())
    }
}

#[derive(Clone)]
struct NotificationDeletionClient {
    client:
        notification_proto::notification_service_client::NotificationServiceClient<ServiceChannel>,
    edge_deadlines: EdgeDeadlineMetrics,
}

#[async_trait]
impl UserDeletionTargetClientPort for NotificationDeletionClient {
    async fn apply(&self, request: UserDeletionDispatchRequest) -> Result<()> {
        let (saga_id, user_id, actor_id, retention_until) = request.strings();
        let mut client = self.client.clone();
        client
            .apply_user_deletion(notification_proto::ApplyUserDeletionRequest {
                saga_id,
                user_id,
                actor_id,
                retention_until,
            })
            .with_edge_deadline(Edge::AuthNotification, self.edge_deadlines.clone())
            .await?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct GrpcUserDeletionDispatcher {
    profile: Arc<dyn UserDeletionTargetClientPort>,
    friend: Arc<dyn UserDeletionTargetClientPort>,
    post: Arc<dyn UserDeletionTargetClientPort>,
    notification: Arc<dyn UserDeletionTargetClientPort>,
}

impl GrpcUserDeletionDispatcher {
    pub fn connect(
        profile_endpoint: String,
        friend_endpoint: String,
        post_endpoint: String,
        notification_endpoint: String,
        service_token: &str,
        tls_material: Option<&platform::grpc_tls::TlsMaterial>,
    ) -> Result<Self> {
        Self::connect_with_metrics(
            profile_endpoint,
            friend_endpoint,
            post_endpoint,
            notification_endpoint,
            service_token,
            tls_material,
            EdgeDeadlineMetrics::default(),
        )
    }

    pub fn connect_with_metrics(
        profile_endpoint: String,
        friend_endpoint: String,
        post_endpoint: String,
        notification_endpoint: String,
        service_token: &str,
        tls_material: Option<&platform::grpc_tls::TlsMaterial>,
        edge_deadlines: EdgeDeadlineMetrics,
    ) -> Result<Self> {
        // Each target is verified by its own workload name, so a certificate for
        // one service cannot be accepted in place of another.
        let profile_channel = channel(profile_endpoint, Workload::Profile, tls_material)?;
        let friend_channel = channel(friend_endpoint, Workload::Friend, tls_material)?;
        let post_channel = channel(post_endpoint, Workload::Post, tls_material)?;
        let notification_channel =
            channel(notification_endpoint, Workload::Notification, tls_material)?;
        let interceptor = ServiceIdentityClientInterceptor::new(Workload::Auth, service_token)?;
        Ok(Self::from_clients(
            Arc::new(ProfileDeletionClient {
                client: profile_proto::profile_service_client::ProfileServiceClient::with_interceptor(
                    GrpcClientTraceService::new(profile_channel),
                    interceptor.clone(),
                ),
                edge_deadlines: edge_deadlines.clone(),
            }),
            Arc::new(FriendDeletionClient {
                client: friend_proto::friend_service_client::FriendServiceClient::with_interceptor(
                    GrpcClientTraceService::new(friend_channel),
                    interceptor.clone(),
                ),
                edge_deadlines: edge_deadlines.clone(),
            }),
            Arc::new(PostDeletionClient {
                client: post_proto::post_service_client::PostServiceClient::with_interceptor(
                    GrpcClientTraceService::new(post_channel),
                    interceptor.clone(),
                ),
                edge_deadlines: edge_deadlines.clone(),
            }),
            Arc::new(NotificationDeletionClient {
                client: notification_proto::notification_service_client::NotificationServiceClient::with_interceptor(
                    GrpcClientTraceService::new(notification_channel),
                    interceptor,
                ),
                edge_deadlines,
            }),
        ))
    }

    pub fn from_clients(
        profile: Arc<dyn UserDeletionTargetClientPort>,
        friend: Arc<dyn UserDeletionTargetClientPort>,
        post: Arc<dyn UserDeletionTargetClientPort>,
        notification: Arc<dyn UserDeletionTargetClientPort>,
    ) -> Self {
        Self {
            profile,
            friend,
            post,
            notification,
        }
    }
}

#[async_trait]
impl UserDeletionDispatcherPort for GrpcUserDeletionDispatcher {
    async fn apply(
        &self,
        target: crate::application::dto::deletion_dto::AuthUserDeletionTargetDto,
        saga_id: Uuid,
        user_id: Uuid,
        actor_id: Uuid,
        retention_until: DateTime<Utc>,
    ) -> crate::application::errors::AuthResult<()> {
        let target = crate::domain::user_deletion::UserDeletionTarget::from(target);

        let result: anyhow::Result<_> = async {
            let request = UserDeletionDispatchRequest {
                saga_id,
                user_id,
                actor_id,
                retention_until,
            };
            match target {
                UserDeletionTarget::Profile => self.profile.apply(request).await,
                UserDeletionTarget::Friend => self.friend.apply(request).await,
                UserDeletionTarget::Post => self.post.apply(request).await,
                UserDeletionTarget::Notification => self.notification.apply(request).await,
                // No unauthenticated/ad-hoc Concert endpoint or successful no-op.
                // The owned composite dispatcher injects real typed owners;
                // until then canonical retries retain these required steps.
                UserDeletionTarget::ConcertTickets
                | UserDeletionTarget::ConcertSessions
                | UserDeletionTarget::ConcertSharing
                | UserDeletionTarget::ConcertPrivateData => {
                    return Err(crate::application::errors::AuthError::DependencyUnavailable.into());
                }
            }
        }
        .await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }
}

/// Build a lazily connecting channel to one deletion target.
///
/// Lazy connection is deliberate: a dependency being down must not stop this
/// service from starting. Mutual TLS is still configured up front, so the first
/// call is mutually authenticated.
fn channel(
    endpoint: String,
    peer: Workload,
    tls_material: Option<&platform::grpc_tls::TlsMaterial>,
) -> Result<Channel> {
    use platform::edge::Edge;
    let edge = match peer {
        Workload::Profile => Edge::AuthProfile,
        Workload::Friend => Edge::AuthFriend,
        Workload::Post => Edge::AuthPost,
        Workload::Notification => Edge::AuthNotification,
        _ => anyhow::bail!("Unsupported account-deletion target"),
    };
    platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
        tls_material,
        &endpoint,
        peer,
        edge.budget(),
    )
}
