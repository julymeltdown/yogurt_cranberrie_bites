use crate::dto::facade_dto::{PolicySetResponseDto, RolloutPolicyResponseDto};
use crate::ports::{GatewayRuntimePort, PolicyRepositoryPort, RolloutRepositoryPort};
use std::sync::Arc;

#[derive(Clone)]
pub struct PolicyService {
    policy_repository: Arc<dyn PolicyRepositoryPort>,
    rollout_repository: Arc<dyn RolloutRepositoryPort>,
    runtime: Arc<dyn GatewayRuntimePort>,
}

impl PolicyService {
    pub fn new(
        policy_repository: Arc<dyn PolicyRepositoryPort>,
        rollout_repository: Arc<dyn RolloutRepositoryPort>,
        runtime: Arc<dyn GatewayRuntimePort>,
    ) -> Self {
        Self {
            policy_repository,
            rollout_repository,
            runtime,
        }
    }

    pub async fn list_policies(
        &self,
    ) -> crate::service_error::GatewayServiceResult<Vec<PolicySetResponseDto>> {
        self.policy_repository
            .find_all()
            .await
            .map(|values| values.into_iter().map(Into::into).collect())
            .map_err(Into::into)
    }

    pub async fn get_policy(
        &self,
        policy_id: &str,
    ) -> crate::service_error::GatewayServiceResult<PolicySetResponseDto> {
        self.policy_repository
            .find_by_id(policy_id)
            .await?
            .map(Into::into)
            .ok_or(crate::service_error::GatewayServiceError::PolicyMissing)
    }

    pub async fn create_policy(
        &self,
        policy: PolicySetResponseDto,
    ) -> crate::service_error::GatewayServiceResult<PolicySetResponseDto> {
        self.policy_repository
            .save(policy.into())
            .await
            .map(Into::into)
            .map_err(Into::into)
    }

    pub async fn default_policy(
        &self,
    ) -> crate::service_error::GatewayServiceResult<Option<PolicySetResponseDto>> {
        Ok(self
            .policy_repository
            .find_all()
            .await?
            .into_iter()
            .next()
            .map(Into::into))
    }

    pub async fn create_rollout(
        &self,
        rollout: RolloutPolicyResponseDto,
    ) -> crate::service_error::GatewayServiceResult<RolloutPolicyResponseDto> {
        let rollout = RolloutPolicyResponseDto {
            id: if rollout.id.trim().is_empty() {
                self.runtime.new_id().to_string()
            } else {
                rollout.id
            },
            ..rollout
        };
        self.rollout_repository
            .save(rollout.into())
            .await
            .map(Into::into)
            .map_err(Into::into)
    }

    pub async fn get_rollout(
        &self,
        rollout_id: &str,
    ) -> crate::service_error::GatewayServiceResult<RolloutPolicyResponseDto> {
        self.rollout_repository
            .find_by_id(rollout_id)
            .await?
            .map(Into::into)
            .ok_or(crate::service_error::GatewayServiceError::RolloutMissing)
    }
}
