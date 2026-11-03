use crate::dto::facade_dto::TelemetrySummaryResponseDto;
use crate::ports::{GatewayRuntimePort, TelemetryRecord, TelemetryRepositoryPort};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone)]
pub struct TelemetryService {
    repository: Arc<dyn TelemetryRepositoryPort>,
    runtime: Arc<dyn GatewayRuntimePort>,
}

impl TelemetryService {
    pub fn new(
        repository: Arc<dyn TelemetryRepositoryPort>,
        runtime: Arc<dyn GatewayRuntimePort>,
    ) -> Self {
        Self {
            repository,
            runtime,
        }
    }

    pub async fn record_request(
        &self,
        correlation_id: String,
        route_key: String,
        client_profile_id: Option<String>,
        outcome: String,
        latency_ms: i64,
        error_code: Option<String>,
        downstream_latencies: HashMap<String, i64>,
    ) -> crate::service_error::GatewayServiceResult<()> {
        self.repository
            .record(TelemetryRecord {
                id: self.runtime.new_id().to_string(),
                correlation_id,
                route_key,
                client_profile_id,
                outcome,
                latency_ms,
                error_code,
                downstream_latencies,
                recorded_at: self.runtime.utc_now(),
            })
            .await
            .map_err(Into::into)
    }

    pub async fn summary(
        &self,
    ) -> crate::service_error::GatewayServiceResult<TelemetrySummaryResponseDto> {
        self.repository
            .summary()
            .await
            .map(Into::into)
            .map_err(Into::into)
    }
}
