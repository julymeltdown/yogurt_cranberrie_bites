use crate::adapters::out::grpc_call::{
    delegated, owner_request, replay_request, replay_safe_read, BoundedGrpcCallExt,
};
use crate::application::dto::post_dto::{MutationReceiptStateDto, MutationReceiptStatusDto};
use crate::application::dto::profile_dto::{
    AvatarContent, DeleteProfileResponse, ProfileBatchResponse, ProfilePrivacyUpdateRequest,
    ProfileResponse, ProfileSearchResponse, ProfileUpdateRequest,
};
use crate::application::ports::ProfileClientPort;
use crate::config::GatewayMetrics;
use async_trait::async_trait;
use platform::actor_delegation::{ActorDelegationClient, ActorDelegationSigner};
use platform::edge::Edge;
use platform::grpc_auth::{ServiceIdentityClientInterceptor, Workload};
use platform::grpc_client_trace::GrpcClientTraceService;
use platform::mutation_context;
use proto::replay_mutation_request::Command;
use proto::replay_mutation_response::Outcome;
use tonic::service::interceptor::InterceptedService;
use tonic::transport::Channel;

pub mod proto {
    tonic::include_proto!("profile.v1");
}

#[derive(Clone)]
pub struct GrpcProfileClient {
    client: proto::profile_service_client::ProfileServiceClient<
        InterceptedService<
            GrpcClientTraceService<ActorDelegationClient<Channel>>,
            ServiceIdentityClientInterceptor,
        >,
    >,
    metrics: GatewayMetrics,
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
    ) -> anyhow::Result<Self> {
        Self::connect_with_metrics(
            endpoint,
            caller,
            service_token,
            tls_material,
            ActorDelegationSigner::local_parity(caller),
            GatewayMetrics::default(),
        )
        .await
    }

    pub async fn connect_with_metrics(
        endpoint: String,
        caller: Workload,
        service_token: &str,
        tls_material: Option<&platform::grpc_tls::TlsMaterial>,
        signer: ActorDelegationSigner,
        metrics: GatewayMetrics,
    ) -> anyhow::Result<Self> {
        let channel = platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
            tls_material,
            &endpoint,
            Workload::Profile,
            platform::edge::Edge::GatewayProfile.budget(),
        )?;
        Ok(Self {
            client: proto::profile_service_client::ProfileServiceClient::with_interceptor(
                GrpcClientTraceService::new(ActorDelegationClient::new(
                    channel,
                    signer,
                    Workload::Profile,
                )),
                ServiceIdentityClientInterceptor::new(caller, service_token)?,
            ),
            metrics,
        })
    }
}

impl GrpcProfileClient {
    async fn get_mutation_receipt_status_profile_client_port_inner(
        &self,
        operation_id: uuid::Uuid,
    ) -> anyhow::Result<Option<MutationReceiptStatusDto>> {
        let mut client = self.client.clone();
        let response = match client
            .get_mutation_receipt_status(proto::GetMutationReceiptStatusRequest {
                operation_id: operation_id.to_string(),
            })
            .with_edge_deadline(platform::edge::Edge::GatewayProfile, self.metrics.clone())
            .await
        {
            Ok(response) => response.into_inner(),
            Err(status) if status.code() == tonic::Code::NotFound => return Ok(None),
            Err(status) => return Err(status.into()),
        };
        let response_operation_id = uuid::Uuid::parse_str(&response.operation_id)?;
        let actor_id = uuid::Uuid::parse_str(&response.actor_id)?;
        let state =
            match proto::get_mutation_receipt_status_response::State::try_from(response.state)? {
                proto::get_mutation_receipt_status_response::State::Replayable => {
                    MutationReceiptStateDto::Replayable
                }
                proto::get_mutation_receipt_status_response::State::Expired => {
                    MutationReceiptStateDto::Expired
                }
                proto::get_mutation_receipt_status_response::State::Incomplete => {
                    MutationReceiptStateDto::Incomplete
                }
                proto::get_mutation_receipt_status_response::State::Unspecified => {
                    anyhow::bail!("Profile service returned an unspecified mutation receipt state")
                }
            };
        Ok(Some(MutationReceiptStatusDto {
            operation_id: response_operation_id,
            actor_id,
            scope: response.scope,
            committed_at: chrono::DateTime::parse_from_rfc3339(&response.committed_at)?
                .with_timezone(&chrono::Utc),
            replay_until: chrono::DateTime::parse_from_rfc3339(&response.replay_until)?
                .with_timezone(&chrono::Utc),
            state,
        }))
    }

    async fn get_profile_profile_client_port_inner(
        &self,
        user_id: String,
    ) -> anyhow::Result<ProfileResponse> {
        let response = replay_safe_read(
            Edge::GatewayProfile,
            "/profile.v1.ProfileService/GetProfile",
            &self.metrics,
            || {
                Ok(tonic::Request::new(proto::GetProfileRequest {
                    user_id: user_id.clone(),
                }))
            },
            |request| {
                let mut client = self.client.clone();
                async move { client.get_profile(request).await }
            },
        )
        .await?
        .into_inner();
        Ok(map_profile(response.profile.expect("profile")))
    }

    async fn get_profile_by_username_profile_client_port_inner(
        &self,
        username: String,
    ) -> anyhow::Result<ProfileResponse> {
        let response = replay_safe_read(
            Edge::GatewayProfile,
            "/profile.v1.ProfileService/GetProfileByUsername",
            &self.metrics,
            || {
                Ok(tonic::Request::new(proto::GetProfileByUsernameRequest {
                    username: username.clone(),
                }))
            },
            |request| {
                let mut client = self.client.clone();
                async move { client.get_profile_by_username(request).await }
            },
        )
        .await?
        .into_inner();
        Ok(map_profile(response.profile.expect("profile")))
    }

    async fn get_profiles_profile_client_port_inner(
        &self,
        user_ids: Vec<String>,
    ) -> anyhow::Result<ProfileBatchResponse> {
        let response = replay_safe_read(
            Edge::GatewayProfile,
            "/profile.v1.ProfileService/GetProfiles",
            &self.metrics,
            || {
                Ok(tonic::Request::new(proto::GetProfilesRequest {
                    user_ids: user_ids.clone(),
                }))
            },
            |request| {
                let mut client = self.client.clone();
                async move { client.get_profiles(request).await }
            },
        )
        .await?
        .into_inner();
        Ok(ProfileBatchResponse {
            profiles: response.profiles.into_iter().map(map_profile).collect(),
        })
    }

    async fn search_profiles_profile_client_port_inner(
        &self,
        query: String,
        cursor: Option<String>,
        limit: i32,
    ) -> anyhow::Result<ProfileSearchResponse> {
        let cursor = cursor.unwrap_or_default();
        let response = replay_safe_read(
            Edge::GatewayProfile,
            "/profile.v1.ProfileService/SearchProfiles",
            &self.metrics,
            || {
                Ok(tonic::Request::new(proto::SearchProfilesRequest {
                    query: query.clone(),
                    cursor: cursor.clone(),
                    limit,
                }))
            },
            |request| {
                let mut client = self.client.clone();
                async move { client.search_profiles(request).await }
            },
        )
        .await?
        .into_inner();
        Ok(ProfileSearchResponse {
            profiles: response.profiles.into_iter().map(map_profile).collect(),
            next_cursor: empty_to_none(response.next_cursor),
        })
    }

    async fn upload_avatar_profile_client_port_inner(
        &self,
        user_id: String,
        content: Vec<u8>,
        content_type: String,
        filename: String,
    ) -> anyhow::Result<String> {
        let mut client = self.client.clone();
        let actor = user_id.clone();
        let response = client
            .upload_avatar(delegated(
                proto::UploadAvatarRequest {
                    user_id,
                    content,
                    content_type,
                    filename,
                },
                &actor,
            )?)
            .with_edge_deadline(platform::edge::Edge::GatewayProfile, self.metrics.clone())
            .await?
            .into_inner();
        Ok(response.avatar_path)
    }

    async fn get_avatar_profile_client_port_inner(
        &self,
        user_id: String,
    ) -> anyhow::Result<AvatarContent> {
        let response = replay_safe_read(
            Edge::GatewayProfile,
            "/profile.v1.ProfileService/GetAvatar",
            &self.metrics,
            || {
                Ok(tonic::Request::new(proto::GetAvatarRequest {
                    user_id: user_id.clone(),
                }))
            },
            |request| {
                let mut client = self.client.clone();
                async move { client.get_avatar(request).await }
            },
        )
        .await?
        .into_inner();
        Ok(AvatarContent {
            content: response.content,
            content_type: response.content_type,
        })
    }

    async fn update_profile_profile_client_port_inner(
        &self,
        user_id: String,
        request: ProfileUpdateRequest,
    ) -> anyhow::Result<ProfileResponse> {
        let mut client = self.client.clone();
        let actor = user_id.clone();
        let command = proto::UpdateProfileRequest {
            user_id,
            username: request.username,
            nickname: request.nickname,
            avatar_url: request.avatar_url,
            bio: request.bio,
        };
        let response = if lookup_only() {
            let response = client
                .replay_mutation(replay_request(proto::ReplayMutationRequest {
                    command: Some(Command::UpdateProfile(command)),
                })?)
                .with_edge_deadline(platform::edge::Edge::GatewayProfile, self.metrics.clone())
                .await?
                .into_inner();
            match response.outcome {
                Some(Outcome::UpdateProfile(response)) => response,
                None => anyhow::bail!("Profile service returned an empty replay outcome"),
                Some(_) => anyhow::bail!("Profile service returned a mismatched replay outcome"),
            }
        } else {
            client
                .update_profile(owner_request(command, &actor)?)
                .with_edge_deadline(platform::edge::Edge::GatewayProfile, self.metrics.clone())
                .await?
                .into_inner()
        };
        Ok(map_profile(response.profile.expect("profile")))
    }

    async fn update_profile_privacy_profile_client_port_inner(
        &self,
        user_id: String,
        request: ProfilePrivacyUpdateRequest,
    ) -> anyhow::Result<ProfileResponse> {
        let mut client = self.client.clone();
        let actor = user_id.clone();
        let command = proto::UpdateProfilePrivacyRequest {
            actor_id: user_id.clone(),
            user_id,
            account_visibility: request.account_visibility,
        };
        let response = if lookup_only() {
            let response = client
                .replay_mutation(replay_request(proto::ReplayMutationRequest {
                    command: Some(Command::UpdateProfilePrivacy(command)),
                })?)
                .with_edge_deadline(platform::edge::Edge::GatewayProfile, self.metrics.clone())
                .await?
                .into_inner();
            match response.outcome {
                Some(Outcome::UpdateProfilePrivacy(response)) => response,
                None => anyhow::bail!("Profile service returned an empty replay outcome"),
                Some(_) => anyhow::bail!("Profile service returned a mismatched replay outcome"),
            }
        } else {
            client
                .update_profile_privacy(owner_request(command, &actor)?)
                .with_edge_deadline(platform::edge::Edge::GatewayProfile, self.metrics.clone())
                .await?
                .into_inner()
        };
        Ok(map_profile(response.profile.expect("profile")))
    }

    async fn delete_profile_profile_client_port_inner(
        &self,
        user_id: String,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> anyhow::Result<DeleteProfileResponse> {
        let mut client = self.client.clone();
        let actor = user_id.clone();
        let response = client
            .delete_profile(delegated(
                proto::DeleteProfileRequest {
                    actor_id: user_id.clone(),
                    user_id,
                    request_id: request_id.unwrap_or_default(),
                    ip_address: ip_address.unwrap_or_default(),
                    user_agent: user_agent.unwrap_or_default(),
                },
                &actor,
            )?)
            .with_edge_deadline(platform::edge::Edge::GatewayProfile, self.metrics.clone())
            .await?
            .into_inner();
        Ok(DeleteProfileResponse {
            user_id: response.user_id,
            deleted: response.deleted,
            account_deleted: response.account_deleted,
        })
    }
}

#[async_trait]
impl ProfileClientPort for GrpcProfileClient {
    async fn get_mutation_receipt_status(
        &self,
        operation_id: uuid::Uuid,
    ) -> crate::application::client_error::GatewayClientResult<Option<MutationReceiptStatusDto>>
    {
        self.get_mutation_receipt_status_profile_client_port_inner(operation_id)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn get_profile(
        &self,
        user_id: String,
    ) -> crate::application::client_error::GatewayClientResult<ProfileResponse> {
        self.get_profile_profile_client_port_inner(user_id)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn get_profile_by_username(
        &self,
        username: String,
    ) -> crate::application::client_error::GatewayClientResult<ProfileResponse> {
        self.get_profile_by_username_profile_client_port_inner(username)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn get_profiles(
        &self,
        user_ids: Vec<String>,
    ) -> crate::application::client_error::GatewayClientResult<ProfileBatchResponse> {
        self.get_profiles_profile_client_port_inner(user_ids)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn search_profiles(
        &self,
        query: String,
        cursor: Option<String>,
        limit: i32,
    ) -> crate::application::client_error::GatewayClientResult<ProfileSearchResponse> {
        self.search_profiles_profile_client_port_inner(query, cursor, limit)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn upload_avatar(
        &self,
        user_id: String,
        content: Vec<u8>,
        content_type: String,
        filename: String,
    ) -> crate::application::client_error::GatewayClientResult<String> {
        self.upload_avatar_profile_client_port_inner(user_id, content, content_type, filename)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn get_avatar(
        &self,
        user_id: String,
    ) -> crate::application::client_error::GatewayClientResult<AvatarContent> {
        self.get_avatar_profile_client_port_inner(user_id)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn update_profile(
        &self,
        user_id: String,
        request: ProfileUpdateRequest,
    ) -> crate::application::client_error::GatewayClientResult<ProfileResponse> {
        self.update_profile_profile_client_port_inner(user_id, request)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn update_profile_privacy(
        &self,
        user_id: String,
        request: ProfilePrivacyUpdateRequest,
    ) -> crate::application::client_error::GatewayClientResult<ProfileResponse> {
        self.update_profile_privacy_profile_client_port_inner(user_id, request)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn delete_profile(
        &self,
        user_id: String,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> crate::application::client_error::GatewayClientResult<DeleteProfileResponse> {
        self.delete_profile_profile_client_port_inner(user_id, request_id, ip_address, user_agent)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
}

fn lookup_only() -> bool {
    mutation_context::current().is_some_and(|context| context.lookup_only)
}

fn map_profile(value: proto::Profile) -> ProfileResponse {
    ProfileResponse {
        user_id: value.user_id,
        username: value.username,
        nickname: value.nickname,
        avatar_url: value.avatar_url,
        bio: value.bio,
        follower_count: value.follower_count,
        following_count: value.following_count,
        post_count: value.post_count,
        account_visibility: value.account_visibility,
        lifecycle_status: value.lifecycle_status,
    }
}

fn empty_to_none(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}
