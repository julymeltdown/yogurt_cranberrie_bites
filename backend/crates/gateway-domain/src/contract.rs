use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RouteContract {
    pub id: String,
    pub route_key: String,
    pub owner: String,
    pub sla_target: Option<String>,
    pub status: String,
    pub description: Option<String>,
    pub versions: Vec<RouteContractVersion>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RouteContractVersion {
    pub version: String,
    pub status: String,
    pub deprecation_date: Option<DateTime<Utc>>,
    pub response_schema_ref: String,
}
