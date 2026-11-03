use super::authority_exchange_client::GrpcAuthorizationExchangeClient;
use crate::adapters::r#in::grpc::resource_authority::proto as authority;
use crate::application::ports::ProfileClientPort;
use anyhow::Result;
use async_trait::async_trait;
use platform::edge::Edge;
use platform::edge_metrics::EdgeDeadlineMetrics;
use platform::grpc_auth::{ServiceIdentityClientInterceptor, Workload};
use platform::grpc_client_trace::GrpcClientTraceService;
use platform::grpc_deadline::EdgeDeadlineCallExt;
use post_application::profile_author::ExistingProfileAuthorDto;
use std::sync::Arc;
use tonic::service::interceptor::InterceptedService;
use tonic::transport::Channel;
use uuid::Uuid;

const CHECK_AUTHOR_PROFILE: &str = "/authority.v1.ProfileAuthorAuthorityService/CheckAuthorProfile";

pub mod proto {
    tonic::include_proto!("profile.v1");
}

#[derive(Clone)]
pub struct GrpcProfileClient {
    client: proto::profile_service_client::ProfileServiceClient<
        InterceptedService<GrpcClientTraceService<Channel>, ServiceIdentityClientInterceptor>,
    >,
    projection:
        authority::profile_author_authority_service_client::ProfileAuthorAuthorityServiceClient<
            InterceptedService<GrpcClientTraceService<Channel>, ServiceIdentityClientInterceptor>,
        >,
    exchange: Option<Arc<GrpcAuthorizationExchangeClient>>,
    edge_deadlines: EdgeDeadlineMetrics,
}

impl GrpcProfileClient {
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
        // The peer is the profile service, not this caller: the client verifies
        // the name it dialled. `caller` stays this workload's own identity.
        let channel = platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
            tls_material,
            &endpoint,
            Workload::Profile,
            platform::edge::Edge::PostProfile.budget(),
        )?;
        Ok(Self {
            client: proto::profile_service_client::ProfileServiceClient::with_interceptor(
                GrpcClientTraceService::new(channel.clone()),
                ServiceIdentityClientInterceptor::new(caller, service_token)?,
            ),
            projection: authority::profile_author_authority_service_client::ProfileAuthorAuthorityServiceClient::with_interceptor(
                GrpcClientTraceService::new(channel),
                ServiceIdentityClientInterceptor::new(caller, service_token)?,
            ),
            exchange: None,
            edge_deadlines,
        })
    }
    /// Production request handling composes this. Without it (tests and
    /// fixtures only) the author check falls back to GetProfile.
    pub fn with_exchange(mut self, exchange: Arc<GrpcAuthorizationExchangeClient>) -> Self {
        self.exchange = Some(exchange);
        self
    }
}

#[async_trait]
impl ProfileClientPort for GrpcProfileClient {
    async fn require_author(&self, user_id: Uuid) -> Result<ExistingProfileAuthorDto> {
        if let Some(exchange) = &self.exchange {
            // Each attempt asks for a fresh child authority, because
            // delegations are single-use; that spends one more of the parent's
            // bounded child issuances. An exchange failure is returned at once.
            let this = self;
            let response = platform::grpc_retry::read(
                Edge::PostProfile,
                CHECK_AUTHOR_PROFILE,
                &self.edge_deadlines,
                move || async move {
                    let request = exchange
                        .authorize(
                            CHECK_AUTHOR_PROFILE,
                            Workload::Profile,
                            authority::CheckAuthorProfileRequest {
                                user_id: user_id.to_string(),
                            },
                            None,
                        )
                        .await
                        .map_err(platform::grpc_retry::not_retried)?;
                    anyhow::Ok(
                        this.projection
                            .clone()
                            .check_author_profile(request)
                            .with_edge_deadline(Edge::PostProfile, this.edge_deadlines.clone())
                            .await?,
                    )
                },
            )
            .await?
            .into_inner();
            anyhow::ensure!(
                response.user_id.parse::<Uuid>().ok() == Some(user_id),
                "Profile author binding denied"
            );
            return ExistingProfileAuthorDto::new(user_id);
        }
        // Composition without the exchange. Profile contents never enter the
        // application port; configured forwarding has no fallback.
        let this = self;
        platform::grpc_retry::read(
            Edge::PostProfile,
            "/profile.v1.ProfileService/GetProfile",
            &self.edge_deadlines,
            move || async move {
                this.client
                    .clone()
                    .get_profile(proto::GetProfileRequest {
                        user_id: user_id.to_string(),
                    })
                    .with_edge_deadline(Edge::PostProfile, this.edge_deadlines.clone())
                    .await
            },
        )
        .await?
        .into_inner()
        .profile
        .ok_or_else(|| anyhow::anyhow!("Profile not found"))?;
        ExistingProfileAuthorDto::new(user_id)
    }
}
