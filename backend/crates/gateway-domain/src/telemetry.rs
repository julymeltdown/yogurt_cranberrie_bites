use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TelemetrySummary {
    pub total_requests: i64,
    pub error_rate: f64,
    pub p95_latency_ms: i64,
}
