use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AggregationResponse {
    pub route_key: String,
    pub cached: bool,
    pub payload: Value,
}
