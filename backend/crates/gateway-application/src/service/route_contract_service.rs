use crate::dto::facade_dto::{RouteContractResponseDto, RouteContractVersionResponseDto};
use crate::ports::RouteContractRepositoryPort;
use std::sync::Arc;

#[derive(Clone)]
pub struct RouteContractService {
    repository: Arc<dyn RouteContractRepositoryPort>,
}

impl RouteContractService {
    pub fn new(repository: Arc<dyn RouteContractRepositoryPort>) -> Self {
        Self { repository }
    }

    pub async fn list_contracts(
        &self,
    ) -> crate::service_error::GatewayServiceResult<Vec<RouteContractResponseDto>> {
        self.repository
            .find_all()
            .await
            .map(|values| values.into_iter().map(Into::into).collect())
            .map_err(Into::into)
    }

    pub async fn get_contract(
        &self,
        contract_id: &str,
    ) -> crate::service_error::GatewayServiceResult<RouteContractResponseDto> {
        self.repository
            .find_by_id(contract_id)
            .await?
            .map(Into::into)
            .ok_or(crate::service_error::GatewayServiceError::RouteContractMissing)
    }

    pub async fn get_version(
        &self,
        contract_id: &str,
        version: &str,
    ) -> crate::service_error::GatewayServiceResult<RouteContractVersionResponseDto> {
        self.repository
            .find_version(contract_id, version)
            .await?
            .map(Into::into)
            .ok_or(crate::service_error::GatewayServiceError::RouteContractVersionMissing)
    }

    pub async fn create_contract(
        &self,
        contract: RouteContractResponseDto,
    ) -> crate::service_error::GatewayServiceResult<RouteContractResponseDto> {
        self.repository
            .save(contract.into())
            .await
            .map(Into::into)
            .map_err(Into::into)
    }

    pub async fn create_version(
        &self,
        contract_id: &str,
        version: RouteContractVersionResponseDto,
    ) -> crate::service_error::GatewayServiceResult<RouteContractVersionResponseDto> {
        self.repository
            .save_version(contract_id, version.into())
            .await
            .map(Into::into)
            .map_err(Into::into)
    }
}
