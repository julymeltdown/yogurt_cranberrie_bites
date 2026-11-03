use super::{TestClients, USER_ID};
use api_gateway::application::dto::friend_dto::FollowCountsResponse;
use api_gateway::application::dto::profile_dto::{
    AvatarContent, DeleteProfileResponse, ProfileBatchResponse, ProfileResponse,
    ProfileSearchResponse, ProfileUpdateRequest,
};
use api_gateway::application::ports::ProfileClientPort;
use async_trait::async_trait;

impl TestClients {
    async fn get_profile_profile_client_port_inner(
        &self,
        user_id: String,
    ) -> anyhow::Result<ProfileResponse> {
        Ok(profile(&user_id, counts()))
    }

    async fn get_profile_by_username_profile_client_port_inner(
        &self,
        _username: String,
    ) -> anyhow::Result<ProfileResponse> {
        Ok(profile(USER_ID, counts()))
    }

    async fn get_profiles_profile_client_port_inner(
        &self,
        user_ids: Vec<String>,
    ) -> anyhow::Result<ProfileBatchResponse> {
        Ok(ProfileBatchResponse {
            profiles: user_ids
                .iter()
                .map(|user_id| profile(user_id, counts()))
                .collect(),
        })
    }

    async fn search_profiles_profile_client_port_inner(
        &self,
        _query: String,
        cursor: Option<String>,
        _limit: i32,
    ) -> anyhow::Result<ProfileSearchResponse> {
        Ok(ProfileSearchResponse {
            profiles: vec![profile(USER_ID, counts())],
            next_cursor: cursor,
        })
    }

    async fn upload_avatar_profile_client_port_inner(
        &self,
        user_id: String,
        _content: Vec<u8>,
        _content_type: String,
        _filename: String,
    ) -> anyhow::Result<String> {
        Ok(format!("/api/profile/avatar/{user_id}"))
    }

    async fn get_avatar_profile_client_port_inner(
        &self,
        _user_id: String,
    ) -> anyhow::Result<AvatarContent> {
        Ok(AvatarContent {
            content: b"avatar-bytes".to_vec(),
            content_type: self.current_avatar_content_type(),
        })
    }

    async fn update_profile_profile_client_port_inner(
        &self,
        user_id: String,
        request: ProfileUpdateRequest,
    ) -> anyhow::Result<ProfileResponse> {
        Ok(ProfileResponse {
            user_id,
            username: request.username,
            nickname: request.nickname,
            avatar_url: request.avatar_url,
            bio: request.bio,
            follower_count: 12,
            following_count: 7,
            post_count: 3,
            account_visibility: "PUBLIC".into(),
            lifecycle_status: "ACTIVE".into(),
        })
    }

    async fn delete_profile_profile_client_port_inner(
        &self,
        user_id: String,
        _request_id: Option<String>,
        _ip_address: Option<String>,
        _user_agent: Option<String>,
    ) -> anyhow::Result<DeleteProfileResponse> {
        Ok(DeleteProfileResponse {
            user_id,
            deleted: true,
            account_deleted: false,
        })
    }

    // Explicit recording behavior formerly inherited from the port.
    async fn get_mutation_receipt_status_profile_client_port_inner(
        &self,
        _operation_id: uuid::Uuid,
    ) -> anyhow::Result<Option<api_gateway::application::dto::post_dto::MutationReceiptStatusDto>>
    {
        anyhow::bail!("Profile mutation status lookup is unavailable")
    }

    async fn update_profile_privacy_profile_client_port_inner(
        &self,
        _user_id: String,
        _request: api_gateway::application::dto::profile_dto::ProfilePrivacyUpdateRequest,
    ) -> anyhow::Result<api_gateway::application::dto::profile_dto::ProfileResponse> {
        anyhow::bail!("Profile privacy operation is not implemented")
    }
}

#[async_trait]
impl ProfileClientPort for TestClients {
    async fn get_profile(
        &self,
        user_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<ProfileResponse> {
        self.get_profile_profile_client_port_inner(user_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn get_profile_by_username(
        &self,
        _username: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<ProfileResponse> {
        self.get_profile_by_username_profile_client_port_inner(_username)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn get_profiles(
        &self,
        user_ids: Vec<String>,
    ) -> api_gateway::application::client_error::GatewayClientResult<ProfileBatchResponse> {
        self.get_profiles_profile_client_port_inner(user_ids)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn search_profiles(
        &self,
        _query: String,
        cursor: Option<String>,
        _limit: i32,
    ) -> api_gateway::application::client_error::GatewayClientResult<ProfileSearchResponse> {
        self.search_profiles_profile_client_port_inner(_query, cursor, _limit)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn upload_avatar(
        &self,
        user_id: String,
        _content: Vec<u8>,
        _content_type: String,
        _filename: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<String> {
        self.upload_avatar_profile_client_port_inner(user_id, _content, _content_type, _filename)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn get_avatar(
        &self,
        _user_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<AvatarContent> {
        self.get_avatar_profile_client_port_inner(_user_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn update_profile(
        &self,
        user_id: String,
        request: ProfileUpdateRequest,
    ) -> api_gateway::application::client_error::GatewayClientResult<ProfileResponse> {
        self.update_profile_profile_client_port_inner(user_id, request)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn delete_profile(
        &self,
        user_id: String,
        _request_id: Option<String>,
        _ip_address: Option<String>,
        _user_agent: Option<String>,
    ) -> api_gateway::application::client_error::GatewayClientResult<DeleteProfileResponse> {
        self.delete_profile_profile_client_port_inner(
            user_id,
            _request_id,
            _ip_address,
            _user_agent,
        )
        .await
        .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn get_mutation_receipt_status(
        &self,
        _operation_id: uuid::Uuid,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        Option<api_gateway::application::dto::post_dto::MutationReceiptStatusDto>,
    > {
        self.get_mutation_receipt_status_profile_client_port_inner(_operation_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn update_profile_privacy(
        &self,
        _user_id: String,
        _request: api_gateway::application::dto::profile_dto::ProfilePrivacyUpdateRequest,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        api_gateway::application::dto::profile_dto::ProfileResponse,
    > {
        self.update_profile_privacy_profile_client_port_inner(_user_id, _request)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
}

fn counts() -> FollowCountsResponse {
    FollowCountsResponse {
        follower_count: 12,
        following_count: 7,
    }
}

fn profile(user_id: &str, counts: FollowCountsResponse) -> ProfileResponse {
    ProfileResponse {
        user_id: user_id.into(),
        username: "hana_ani".into(),
        nickname: "Hana Akiyama".into(),
        avatar_url: "/avatars/hana.png".into(),
        bio: "profile bio".into(),
        follower_count: counts.follower_count,
        following_count: counts.following_count,
        post_count: 3,
        account_visibility: "PUBLIC".into(),
        lifecycle_status: "ACTIVE".into(),
    }
}
