use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PolicySet {
    pub id: String,
    pub name: String,
    pub rate_limit: Option<String>,
    pub quota: Option<String>,
    pub timeout_ms: Option<i32>,
    pub retry_policy: Option<String>,
    pub circuit_breaker_policy: Option<String>,
    pub cache_policy: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RolloutPolicy {
    pub id: String,
    pub contract_id: String,
    pub version: String,
    pub strategy: String,
    pub status: String,
    pub percent: Option<i32>,
}
