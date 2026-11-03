use crate::dto::facade_dto::ClientProfileResponseDto;
use crate::ports::ClientProfileRepositoryPort;
use std::sync::Arc;

#[derive(Clone)]
pub struct ClientProfileService {
    repository: Arc<dyn ClientProfileRepositoryPort>,
}

impl ClientProfileService {
    pub fn new(repository: Arc<dyn ClientProfileRepositoryPort>) -> Self {
        Self { repository }
    }

    pub async fn list_profiles(
        &self,
    ) -> crate::service_error::GatewayServiceResult<Vec<ClientProfileResponseDto>> {
        self.repository
            .find_all()
            .await
            .map(|values| values.into_iter().map(Into::into).collect())
            .map_err(Into::into)
    }

    pub async fn get_profile(
        &self,
        profile_id: &str,
    ) -> crate::service_error::GatewayServiceResult<ClientProfileResponseDto> {
        self.repository
            .find_by_id(profile_id)
            .await?
            .map(Into::into)
            .ok_or(crate::service_error::GatewayServiceError::ClientProfileMissing)
    }

    pub async fn create_profile(
        &self,
        profile: ClientProfileResponseDto,
    ) -> crate::service_error::GatewayServiceResult<ClientProfileResponseDto> {
        self.repository
            .save(profile.into())
            .await
            .map(Into::into)
            .map_err(Into::into)
    }
}
