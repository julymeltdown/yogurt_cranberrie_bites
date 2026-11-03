use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DownstreamCall {
    pub service: String,
    pub path: String,
    pub fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AggregationRecipe {
    pub id: String,
    pub route_key: String,
    pub downstream_calls: Vec<DownstreamCall>,
    pub join_strategy: String,
    pub timeout_ms: Option<i32>,
    pub cache_policy: Option<String>,
}
