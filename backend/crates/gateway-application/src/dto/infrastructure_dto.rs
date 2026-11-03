//! Internal governance, aggregation and telemetry transfers. Wire serialization stays at the consumer.
use chrono::{DateTime, Utc};
use std::fmt;

#[derive(Clone, PartialEq, Eq)]
pub struct RouteContractDto {
    pub id: String,
    pub route_key: String,
    pub owner: String,
    pub sla_target: Option<String>,
    pub status: String,
    pub description: Option<String>,
    pub versions: Vec<RouteContractVersionDto>,
}
impl fmt::Debug for RouteContractDto {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RouteContractDto").finish_non_exhaustive()
    }
}
impl From<gateway_domain::contract::RouteContract> for RouteContractDto {
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
impl From<RouteContractDto> for gateway_domain::contract::RouteContract {
    fn from(value: RouteContractDto) -> Self {
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

#[derive(Clone, PartialEq, Eq)]
pub struct RouteContractVersionDto {
    pub version: String,
    pub status: String,
    pub deprecation_date: Option<DateTime<Utc>>,
    pub response_schema_ref: String,
}
impl fmt::Debug for RouteContractVersionDto {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RouteContractVersionDto")
            .finish_non_exhaustive()
    }
}
impl From<gateway_domain::contract::RouteContractVersion> for RouteContractVersionDto {
    fn from(value: gateway_domain::contract::RouteContractVersion) -> Self {
        Self {
            version: value.version,
            status: value.status,
            deprecation_date: value.deprecation_date,
            response_schema_ref: value.response_schema_ref,
        }
    }
}
impl From<RouteContractVersionDto> for gateway_domain::contract::RouteContractVersion {
    fn from(value: RouteContractVersionDto) -> Self {
        Self {
            version: value.version,
            status: value.status,
            deprecation_date: value.deprecation_date,
            response_schema_ref: value.response_schema_ref,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct PolicySetDto {
    pub id: String,
    pub name: String,
    pub rate_limit: Option<String>,
    pub quota: Option<String>,
    pub timeout_ms: Option<i32>,
    pub retry_policy: Option<String>,
    pub circuit_breaker_policy: Option<String>,
    pub cache_policy: Option<String>,
}
impl fmt::Debug for PolicySetDto {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PolicySetDto").finish_non_exhaustive()
    }
}
impl From<gateway_domain::policy::PolicySet> for PolicySetDto {
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
impl From<PolicySetDto> for gateway_domain::policy::PolicySet {
    fn from(value: PolicySetDto) -> Self {
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

#[derive(Clone, PartialEq, Eq)]
pub struct RolloutPolicyDto {
    pub id: String,
    pub contract_id: String,
    pub version: String,
    pub strategy: String,
    pub status: String,
    pub percent: Option<i32>,
}
impl fmt::Debug for RolloutPolicyDto {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RolloutPolicyDto").finish_non_exhaustive()
    }
}
impl From<gateway_domain::policy::RolloutPolicy> for RolloutPolicyDto {
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
impl From<RolloutPolicyDto> for gateway_domain::policy::RolloutPolicy {
    fn from(value: RolloutPolicyDto) -> Self {
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

#[derive(Clone, PartialEq, Eq)]
pub struct ClientProfileDto {
    pub id: String,
    pub client_type: String,
    pub allowed_contracts: Vec<String>,
    pub policy_set_id: Option<String>,
}
impl fmt::Debug for ClientProfileDto {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClientProfileDto").finish_non_exhaustive()
    }
}
impl From<gateway_domain::client::ClientProfile> for ClientProfileDto {
    fn from(value: gateway_domain::client::ClientProfile) -> Self {
        Self {
            id: value.id,
            client_type: value.client_type,
            allowed_contracts: value.allowed_contracts,
            policy_set_id: value.policy_set_id,
        }
    }
}
impl From<ClientProfileDto> for gateway_domain::client::ClientProfile {
    fn from(value: ClientProfileDto) -> Self {
        Self {
            id: value.id,
            client_type: value.client_type,
            allowed_contracts: value.allowed_contracts,
            policy_set_id: value.policy_set_id,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct DownstreamCallDto {
    pub service: String,
    pub path: String,
    pub fields: Vec<String>,
}
impl fmt::Debug for DownstreamCallDto {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DownstreamCallDto").finish_non_exhaustive()
    }
}
impl From<gateway_domain::aggregation::DownstreamCall> for DownstreamCallDto {
    fn from(value: gateway_domain::aggregation::DownstreamCall) -> Self {
        Self {
            service: value.service,
            path: value.path,
            fields: value.fields,
        }
    }
}
impl From<DownstreamCallDto> for gateway_domain::aggregation::DownstreamCall {
    fn from(value: DownstreamCallDto) -> Self {
        Self {
            service: value.service,
            path: value.path,
            fields: value.fields,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AggregationRecipeDto {
    pub id: String,
    pub route_key: String,
    pub downstream_calls: Vec<DownstreamCallDto>,
    pub join_strategy: String,
    pub timeout_ms: Option<i32>,
    pub cache_policy: Option<String>,
}
impl fmt::Debug for AggregationRecipeDto {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AggregationRecipeDto")
            .finish_non_exhaustive()
    }
}
impl From<gateway_domain::aggregation::AggregationRecipe> for AggregationRecipeDto {
    fn from(value: gateway_domain::aggregation::AggregationRecipe) -> Self {
        Self {
            id: value.id,
            route_key: value.route_key,
            downstream_calls: value.downstream_calls.into_iter().map(Into::into).collect(),
            join_strategy: value.join_strategy,
            timeout_ms: value.timeout_ms,
            cache_policy: value.cache_policy,
        }
    }
}
impl From<AggregationRecipeDto> for gateway_domain::aggregation::AggregationRecipe {
    fn from(value: AggregationRecipeDto) -> Self {
        Self {
            id: value.id,
            route_key: value.route_key,
            downstream_calls: value.downstream_calls.into_iter().map(Into::into).collect(),
            join_strategy: value.join_strategy,
            timeout_ms: value.timeout_ms,
            cache_policy: value.cache_policy,
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct TelemetrySummaryDto {
    pub total_requests: i64,
    pub error_rate: f64,
    pub p95_latency_ms: i64,
}
impl fmt::Debug for TelemetrySummaryDto {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TelemetrySummaryDto")
            .finish_non_exhaustive()
    }
}
impl From<gateway_domain::telemetry::TelemetrySummary> for TelemetrySummaryDto {
    fn from(value: gateway_domain::telemetry::TelemetrySummary) -> Self {
        Self {
            total_requests: value.total_requests,
            error_rate: value.error_rate,
            p95_latency_ms: value.p95_latency_ms,
        }
    }
}
impl From<TelemetrySummaryDto> for gateway_domain::telemetry::TelemetrySummary {
    fn from(value: TelemetrySummaryDto) -> Self {
        Self {
            total_requests: value.total_requests,
            error_rate: value.error_rate,
            p95_latency_ms: value.p95_latency_ms,
        }
    }
}
