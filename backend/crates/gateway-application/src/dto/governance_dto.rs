use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RouteContractCreateRequest {
    pub route_key: String,
    pub owner: String,
    pub sla_target: Option<String>,
    pub status: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RouteContractVersionCreateRequest {
    pub version: String,
    pub response_schema_ref: String,
    pub status: Option<String>,
    pub deprecation_date: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RouteContractVersionUpdateRequest {
    pub status: Option<String>,
    pub deprecation_date: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PolicySetRequest {
    pub name: String,
    pub rate_limit: Option<String>,
    pub quota: Option<String>,
    pub timeout_ms: Option<i32>,
    pub retry_policy: Option<String>,
    pub circuit_breaker_policy: Option<String>,
    pub cache_policy: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClientProfileRequest {
    pub client_type: String,
    pub allowed_contracts: Vec<String>,
    pub policy_set_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RolloutCreateRequest {
    pub contract_id: String,
    pub version: String,
    pub strategy: String,
    pub status: Option<String>,
    pub percent: Option<i32>,
}
