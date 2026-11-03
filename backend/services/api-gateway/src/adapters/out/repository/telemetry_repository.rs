use crate::application::ports::{TelemetryRecord, TelemetryRepositoryPort};
use crate::domain::telemetry::TelemetrySummary;
use async_trait::async_trait;
use sqlx::{PgPool, Row};

#[derive(Clone)]
pub struct PgTelemetryRepository {
    pool: PgPool,
}

impl PgTelemetryRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl PgTelemetryRepository {
    async fn record_infrastructure_inner(&self, record: TelemetryRecord) -> anyhow::Result<()> {
        let downstream_latencies = serde_json::to_string(&record.downstream_latencies)?;
        sqlx::query(
            r#"
            insert into telemetry_records (
                id, correlation_id, route_key, client_profile_id, outcome, latency_ms, error_code,
                downstream_latencies, recorded_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(&record.id)
        .bind(&record.correlation_id)
        .bind(&record.route_key)
        .bind(&record.client_profile_id)
        .bind(&record.outcome)
        .bind(record.latency_ms)
        .bind(&record.error_code)
        .bind(downstream_latencies)
        .bind(record.recorded_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn summary_infrastructure_inner(&self) -> anyhow::Result<TelemetrySummary> {
        let rows =
            sqlx::query("select outcome, latency_ms from telemetry_records order by latency_ms")
                .fetch_all(&self.pool)
                .await?;

        let total_requests = rows.len() as i64;
        if total_requests == 0 {
            return Ok(TelemetrySummary {
                total_requests: 0,
                error_rate: 0.0,
                p95_latency_ms: 0,
            });
        }

        let error_count = rows
            .iter()
            .filter(|row| row.get::<String, _>("outcome").to_uppercase() != "SUCCESS")
            .count() as i64;
        let latencies = rows
            .iter()
            .map(|row| row.get::<i64, _>("latency_ms"))
            .collect::<Vec<_>>();
        let p95_index = ((latencies.len() as f64) * 0.95).ceil() as usize;
        let p95_latency_ms = *latencies.get(p95_index.saturating_sub(1)).unwrap_or(&0);

        Ok(TelemetrySummary {
            total_requests,
            error_rate: error_count as f64 / total_requests as f64,
            p95_latency_ms,
        })
    }
}

#[async_trait]
impl TelemetryRepositoryPort for PgTelemetryRepository {
    async fn record(
        &self,
        record: TelemetryRecord,
    ) -> crate::application::infrastructure_error::GatewayInfrastructureResult<()> {
        self.record_infrastructure_inner(record)
            .await
            .map_err(crate::adapters::out::infrastructure_error::classify_infrastructure_error)
    }
    async fn summary(
        &self,
    ) -> crate::application::infrastructure_error::GatewayInfrastructureResult<
        crate::application::dto::infrastructure_dto::TelemetrySummaryDto,
    > {
        self.summary_infrastructure_inner()
            .await
            .map(Into::into)
            .map_err(crate::adapters::out::infrastructure_error::classify_infrastructure_error)
    }
}
