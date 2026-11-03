use crate::application::ports::{ProfileDiscoveryPort, ProfilePrivacyPort};
use crate::domain::follow_request::AccountVisibility;
use crate::domain::recommendation::ProfileDiscoverySnapshot;
use anyhow::Result;
use async_trait::async_trait;
use platform::edge::Edge;
use platform::edge_metrics::EdgeDeadlineMetrics;
use platform::grpc_auth::{ServiceIdentityClientInterceptor, Workload};
use platform::grpc_client_trace::GrpcClientTraceService;
use platform::grpc_deadline::EdgeDeadlineCallExt;
use tonic::service::interceptor::InterceptedService;
use tonic::transport::Channel;
use uuid::Uuid;

pub mod proto {
    tonic::include_proto!("profile.v1");
}

#[derive(Clone)]
pub struct GrpcProfilePrivacyClient {
    client: proto::profile_service_client::ProfileServiceClient<
        InterceptedService<GrpcClientTraceService<Channel>, ServiceIdentityClientInterceptor>,
    >,
    edge_deadlines: EdgeDeadlineMetrics,
}

impl GrpcProfilePrivacyClient {
    pub fn connect_lazy(
        endpoint: String,
        caller: Workload,
        service_token: &str,
        tls_material: Option<&platform::grpc_tls::TlsMaterial>,
    ) -> Result<Self> {
        Self::connect_lazy_with_metrics(
            endpoint,
            caller,
            service_token,
            tls_material,
            EdgeDeadlineMetrics::default(),
        )
    }

    pub fn connect_lazy_with_metrics(
        endpoint: String,
        caller: Workload,
        service_token: &str,
        tls_material: Option<&platform::grpc_tls::TlsMaterial>,
        edge_deadlines: EdgeDeadlineMetrics,
    ) -> Result<Self> {
        // Lazy connection keeps startup independent of the dependency, while
        // mutual TLS is still configured before the first call.
        // The declared budget owns both deadlines now, so the local constants
        // cannot drift away from the contract they were approximating.
        let channel = platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
            tls_material,
            &endpoint,
            Workload::Profile,
            platform::edge::Edge::FriendProfile.budget(),
        )?;
        Ok(Self {
            client: proto::profile_service_client::ProfileServiceClient::with_interceptor(
                GrpcClientTraceService::new(channel),
                ServiceIdentityClientInterceptor::new(caller, service_token)?,
            ),
            edge_deadlines,
        })
    }
}

#[async_trait]
impl ProfilePrivacyPort for GrpcProfilePrivacyClient {
    async fn account_visibility(&self, user_id: Uuid) -> Result<AccountVisibility> {
        let this = self;
        let response = platform::grpc_retry::read(
            Edge::FriendProfile,
            "/profile.v1.ProfileService/GetProfilePrivacy",
            &self.edge_deadlines,
            move || async move {
                this.client
                    .clone()
                    .get_profile_privacy(proto::GetProfilePrivacyRequest {
                        user_id: user_id.to_string(),
                    })
                    .with_edge_deadline(Edge::FriendProfile, this.edge_deadlines.clone())
                    .await
            },
        )
        .await?
        .into_inner();
        AccountVisibility::parse(&response.account_visibility)
    }
}

#[async_trait]
impl ProfileDiscoveryPort for GrpcProfilePrivacyClient {
    async fn batch_profiles(&self, user_ids: &[Uuid]) -> Result<Vec<ProfileDiscoverySnapshot>> {
        if user_ids.len() > 100 {
            anyhow::bail!("Profile discovery supports at most 100 user IDs");
        }
        let this = self;
        let response = platform::grpc_retry::read(
            Edge::FriendProfile,
            "/profile.v1.ProfileService/GetProfiles",
            &self.edge_deadlines,
            move || async move {
                this.client
                    .clone()
                    .get_profiles(proto::GetProfilesRequest {
                        user_ids: user_ids.iter().map(Uuid::to_string).collect(),
                    })
                    .with_edge_deadline(Edge::FriendProfile, this.edge_deadlines.clone())
                    .await
            },
        )
        .await?
        .into_inner();
        response
            .profiles
            .into_iter()
            .map(|profile| {
                Ok(ProfileDiscoverySnapshot {
                    user_id: Uuid::parse_str(&profile.user_id)?,
                    username: profile.username,
                    nickname: profile.nickname,
                    bio: profile.bio,
                    avatar_url: profile.avatar_url,
                    account_visibility: profile.account_visibility,
                    lifecycle_status: profile.lifecycle_status,
                })
            })
            .collect()
    }
}
