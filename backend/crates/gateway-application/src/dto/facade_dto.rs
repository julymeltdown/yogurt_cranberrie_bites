//! Application-owned facade records and policy selections. Repository transfers
//! remain separate; wire-compatible response serialization is explicit here.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RouteContractResponseDto {
    pub id: String,
    pub route_key: String,
    pub owner: String,
    pub sla_target: Option<String>,
    pub status: String,
    pub description: Option<String>,
    pub versions: Vec<RouteContractVersionResponseDto>,
}
impl fmt::Debug for RouteContractResponseDto {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RouteContractResponseDto")
            .finish_non_exhaustive()
    }
}
impl From<gateway_domain::contract::RouteContract> for RouteContractResponseDto {
    fn from(value: gateway_domain::contract::RouteContract) -> Self {
        Self {
            id: value.id,
            route_key: value.route_key,
            owner: value.owner,
            sla_target: value.sla_target,
            status: value.status,
            description: value.description,
            versions: value.versions.into_iter().map(Into::into).collect(),
        }
    }
}
impl From<RouteContractResponseDto> for gateway_domain::contract::RouteContract {
    fn from(value: RouteContractResponseDto) -> Self {
        Self {
            id: value.id,
            route_key: value.route_key,
            owner: value.owner,
            sla_target: value.sla_target,
            status: value.status,
            description: value.description,
            versions: value.versions.into_iter().map(Into::into).collect(),
        }
    }
}
impl From<crate::dto::infrastructure_dto::RouteContractDto> for RouteContractResponseDto {
    fn from(value: crate::dto::infrastructure_dto::RouteContractDto) -> Self {
        Self {
            id: value.id,
            route_key: value.route_key,
            owner: value.owner,
            sla_target: value.sla_target,
            status: value.status,
            description: value.description,
            versions: value.versions.into_iter().map(Into::into).collect(),
        }
    }
}
impl From<RouteContractResponseDto> for crate::dto::infrastructure_dto::RouteContractDto {
    fn from(value: RouteContractResponseDto) -> Self {
        Self {
            id: value.id,
            route_key: value.route_key,
            owner: value.owner,
            sla_target: value.sla_target,
            status: value.status,
            description: value.description,
            versions: value.versions.into_iter().map(Into::into).collect(),
        }
    }
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RouteContractVersionResponseDto {
    pub version: String,
    pub status: String,
    pub deprecation_date: Option<DateTime<Utc>>,
    pub response_schema_ref: String,
}
impl fmt::Debug for RouteContractVersionResponseDto {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RouteContractVersionResponseDto")
            .finish_non_exhaustive()
    }
}
impl From<gateway_domain::contract::RouteContractVersion> for RouteContractVersionResponseDto {
    fn from(value: gateway_domain::contract::RouteContractVersion) -> Self {
        Self {
            version: value.version,
            status: value.status,
            deprecation_date: value.deprecation_date,
            response_schema_ref: value.response_schema_ref,
        }
    }
}
impl From<RouteContractVersionResponseDto> for gateway_domain::contract::RouteContractVersion {
    fn from(value: RouteContractVersionResponseDto) -> Self {
        Self {
            version: value.version,
            status: value.status,
            deprecation_date: value.deprecation_date,
            response_schema_ref: value.response_schema_ref,
        }
    }
}
impl From<crate::dto::infrastructure_dto::RouteContractVersionDto>
    for RouteContractVersionResponseDto
{
    fn from(value: crate::dto::infrastructure_dto::RouteContractVersionDto) -> Self {
        Self {
            version: value.version,
            status: value.status,
            deprecation_date: value.deprecation_date,
            response_schema_ref: value.response_schema_ref,
        }
    }
}
impl From<RouteContractVersionResponseDto>
    for crate::dto::infrastructure_dto::RouteContractVersionDto
{
    fn from(value: RouteContractVersionResponseDto) -> Self {
        Self {
            version: value.version,
            status: value.status,
            deprecation_date: value.deprecation_date,
            response_schema_ref: value.response_schema_ref,
        }
    }
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PolicySetResponseDto {
    pub id: String,
    pub name: String,
    pub rate_limit: Option<String>,
    pub quota: Option<String>,
    pub timeout_ms: Option<i32>,
    pub retry_policy: Option<String>,
    pub circuit_breaker_policy: Option<String>,
    pub cache_policy: Option<String>,
}
impl fmt::Debug for PolicySetResponseDto {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PolicySetResponseDto")
            .finish_non_exhaustive()
    }
}
impl From<gateway_domain::policy::PolicySet> for PolicySetResponseDto {
    fn from(value: gateway_domain::policy::PolicySet) -> Self {
        Self {
            id: value.id,
            name: value.name,
            rate_limit: value.rate_limit,
            quota: value.quota,
            timeout_ms: value.timeout_ms,
            retry_policy: value.retry_policy,
            circuit_breaker_policy: value.circuit_breaker_policy,
            cache_policy: value.cache_policy,
        }
    }
}
impl From<PolicySetResponseDto> for gateway_domain::policy::PolicySet {
    fn from(value: PolicySetResponseDto) -> Self {
        Self {
            id: value.id,
            name: value.name,
            rate_limit: value.rate_limit,
            quota: value.quota,
            timeout_ms: value.timeout_ms,
            retry_policy: value.retry_policy,
            circuit_breaker_policy: value.circuit_breaker_policy,
            cache_policy: value.cache_policy,
        }
    }
}
impl From<crate::dto::infrastructure_dto::PolicySetDto> for PolicySetResponseDto {
    fn from(value: crate::dto::infrastructure_dto::PolicySetDto) -> Self {
        Self {
            id: value.id,
            name: value.name,
            rate_limit: value.rate_limit,
            quota: value.quota,
            timeout_ms: value.timeout_ms,
            retry_policy: value.retry_policy,
            circuit_breaker_policy: value.circuit_breaker_policy,
            cache_policy: value.cache_policy,
        }
    }
}
impl From<PolicySetResponseDto> for crate::dto::infrastructure_dto::PolicySetDto {
    fn from(value: PolicySetResponseDto) -> Self {
        Self {
            id: value.id,
            name: value.name,
            rate_limit: value.rate_limit,
            quota: value.quota,
            timeout_ms: value.timeout_ms,
            retry_policy: value.retry_policy,
            circuit_breaker_policy: value.circuit_breaker_policy,
            cache_policy: value.cache_policy,
        }
    }
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RolloutPolicyResponseDto {
    pub id: String,
    pub contract_id: String,
    pub version: String,
    pub strategy: String,
    pub status: String,
    pub percent: Option<i32>,
}
impl fmt::Debug for RolloutPolicyResponseDto {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RolloutPolicyResponseDto")
            .finish_non_exhaustive()
    }
}
impl From<gateway_domain::policy::RolloutPolicy> for RolloutPolicyResponseDto {
    fn from(value: gateway_domain::policy::RolloutPolicy) -> Self {
        Self {
            id: value.id,
            contract_id: value.contract_id,
            version: value.version,
            strategy: value.strategy,
            status: value.status,
            percent: value.percent,
        }
    }
}
impl From<RolloutPolicyResponseDto> for gateway_domain::policy::RolloutPolicy {
    fn from(value: RolloutPolicyResponseDto) -> Self {
        Self {
            id: value.id,
            contract_id: value.contract_id,
            version: value.version,
            strategy: value.strategy,
            status: value.status,
            percent: value.percent,
        }
    }
}
impl From<crate::dto::infrastructure_dto::RolloutPolicyDto> for RolloutPolicyResponseDto {
    fn from(value: crate::dto::infrastructure_dto::RolloutPolicyDto) -> Self {
        Self {
            id: value.id,
            contract_id: value.contract_id,
            version: value.version,
            strategy: value.strategy,
            status: value.status,
            percent: value.percent,
        }
    }
}
impl From<RolloutPolicyResponseDto> for crate::dto::infrastructure_dto::RolloutPolicyDto {
    fn from(value: RolloutPolicyResponseDto) -> Self {
        Self {
            id: value.id,
            contract_id: value.contract_id,
            version: value.version,
            strategy: value.strategy,
            status: value.status,
            percent: value.percent,
        }
    }
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClientProfileResponseDto {
    pub id: String,
    pub client_type: String,
    pub allowed_contracts: Vec<String>,
    pub policy_set_id: Option<String>,
}
impl fmt::Debug for ClientProfileResponseDto {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClientProfileResponseDto")
            .finish_non_exhaustive()
    }
}
impl From<gateway_domain::client::ClientProfile> for ClientProfileResponseDto {
    fn from(value: gateway_domain::client::ClientProfile) -> Self {
        Self {
            id: value.id,
            client_type: value.client_type,
            allowed_contracts: value.allowed_contracts,
            policy_set_id: value.policy_set_id,
        }
    }
}
impl From<ClientProfileResponseDto> for gateway_domain::client::ClientProfile {
    fn from(value: ClientProfileResponseDto) -> Self {
        Self {
            id: value.id,
            client_type: value.client_type,
            allowed_contracts: value.allowed_contracts,
            policy_set_id: value.policy_set_id,
        }
    }
}
impl From<crate::dto::infrastructure_dto::ClientProfileDto> for ClientProfileResponseDto {
    fn from(value: crate::dto::infrastructure_dto::ClientProfileDto) -> Self {
        Self {
            id: value.id,
            client_type: value.client_type,
            allowed_contracts: value.allowed_contracts,
            policy_set_id: value.policy_set_id,
        }
    }
}
impl From<ClientProfileResponseDto> for crate::dto::infrastructure_dto::ClientProfileDto {
    fn from(value: ClientProfileResponseDto) -> Self {
        Self {
            id: value.id,
            client_type: value.client_type,
            allowed_contracts: value.allowed_contracts,
            policy_set_id: value.policy_set_id,
        }
    }
}
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TelemetrySummaryResponseDto {
    pub total_requests: i64,
    pub error_rate: f64,
    pub p95_latency_ms: i64,
}
impl fmt::Debug for TelemetrySummaryResponseDto {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TelemetrySummaryResponseDto")
            .finish_non_exhaustive()
    }
}
impl From<gateway_domain::telemetry::TelemetrySummary> for TelemetrySummaryResponseDto {
    fn from(value: gateway_domain::telemetry::TelemetrySummary) -> Self {
        Self {
            total_requests: value.total_requests,
            error_rate: value.error_rate,
            p95_latency_ms: value.p95_latency_ms,
        }
    }
}
impl From<TelemetrySummaryResponseDto> for gateway_domain::telemetry::TelemetrySummary {
    fn from(value: TelemetrySummaryResponseDto) -> Self {
        Self {
            total_requests: value.total_requests,
            error_rate: value.error_rate,
            p95_latency_ms: value.p95_latency_ms,
        }
    }
}
impl From<crate::dto::infrastructure_dto::TelemetrySummaryDto> for TelemetrySummaryResponseDto {
    fn from(value: crate::dto::infrastructure_dto::TelemetrySummaryDto) -> Self {
        Self {
            total_requests: value.total_requests,
            error_rate: value.error_rate,
            p95_latency_ms: value.p95_latency_ms,
        }
    }
}
impl From<TelemetrySummaryResponseDto> for crate::dto::infrastructure_dto::TelemetrySummaryDto {
    fn from(value: TelemetrySummaryResponseDto) -> Self {
        Self {
            total_requests: value.total_requests,
            error_rate: value.error_rate,
            p95_latency_ms: value.p95_latency_ms,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IdentityProviderDto {
    Email,
    Google,
    Apple,
    X,
}
impl From<gateway_domain::IdentityProvider> for IdentityProviderDto {
    fn from(value: gateway_domain::IdentityProvider) -> Self {
        match value {
            gateway_domain::IdentityProvider::Email => Self::Email,
            gateway_domain::IdentityProvider::Google => Self::Google,
            gateway_domain::IdentityProvider::Apple => Self::Apple,
            gateway_domain::IdentityProvider::X => Self::X,
        }
    }
}
impl From<IdentityProviderDto> for gateway_domain::IdentityProvider {
    fn from(value: IdentityProviderDto) -> Self {
        match value {
            IdentityProviderDto::Email => Self::Email,
            IdentityProviderDto::Google => Self::Google,
            IdentityProviderDto::Apple => Self::Apple,
            IdentityProviderDto::X => Self::X,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HighRiskActionDto {
    Administration,
    AccountSecurity,
}
impl From<gateway_domain::high_risk::HighRiskAction> for HighRiskActionDto {
    fn from(value: gateway_domain::high_risk::HighRiskAction) -> Self {
        match value {
            gateway_domain::high_risk::HighRiskAction::Administration => Self::Administration,
            gateway_domain::high_risk::HighRiskAction::AccountSecurity => Self::AccountSecurity,
        }
    }
}
impl From<HighRiskActionDto> for gateway_domain::high_risk::HighRiskAction {
    fn from(value: HighRiskActionDto) -> Self {
        match value {
            HighRiskActionDto::Administration => Self::Administration,
            HighRiskActionDto::AccountSecurity => Self::AccountSecurity,
        }
    }
}
impl HighRiskActionDto {
    /// Labels remain owned by the domain policy.
    pub fn as_str(self) -> &'static str {
        gateway_domain::high_risk::HighRiskAction::from(self).as_str()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionDecisionDto {
    Allowed,
    SessionEnded,
    RoleRevoked,
}
impl From<gateway_domain::high_risk::SessionDecision> for SessionDecisionDto {
    fn from(value: gateway_domain::high_risk::SessionDecision) -> Self {
        match value {
            gateway_domain::high_risk::SessionDecision::Allowed => Self::Allowed,
            gateway_domain::high_risk::SessionDecision::SessionEnded => Self::SessionEnded,
            gateway_domain::high_risk::SessionDecision::RoleRevoked => Self::RoleRevoked,
        }
    }
}
impl From<SessionDecisionDto> for gateway_domain::high_risk::SessionDecision {
    fn from(value: SessionDecisionDto) -> Self {
        match value {
            SessionDecisionDto::Allowed => Self::Allowed,
            SessionDecisionDto::SessionEnded => Self::SessionEnded,
            SessionDecisionDto::RoleRevoked => Self::RoleRevoked,
        }
    }
}
