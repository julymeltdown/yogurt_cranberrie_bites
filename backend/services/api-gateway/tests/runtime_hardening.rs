mod support;

use api_gateway::adapters::out::telemetry_queue::QueuedTelemetry;
use api_gateway::adapters::r#in::http;
use api_gateway::application::ports::{TelemetryRecord, TelemetryRepositoryPort};
use api_gateway::config::RuntimeMode;
use api_gateway::domain::telemetry::TelemetrySummary;
use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;
use uuid::Uuid;

struct SlowTelemetry;

/// Records what the middleware actually stored, so a test can assert on the
/// persisted row rather than on the response.
#[derive(Clone, Default)]
struct RecordingTelemetry {
    records: Arc<std::sync::Mutex<Vec<TelemetryRecord>>>,
}

impl RecordingTelemetry {
    async fn record_infrastructure_inner(&self, record: TelemetryRecord) -> anyhow::Result<()> {
        self.records.lock().expect("telemetry lock").push(record);
        Ok(())
    }

    async fn summary_infrastructure_inner(&self) -> anyhow::Result<TelemetrySummary> {
        Ok(TelemetrySummary {
            total_requests: 0,
            error_rate: 0.0,
            p95_latency_ms: 0,
        })
    }
}

#[async_trait]
impl TelemetryRepositoryPort for RecordingTelemetry {
    async fn record(
        &self,
        record: TelemetryRecord,
    ) -> api_gateway::application::infrastructure_error::GatewayInfrastructureResult<()> {
        self.record_infrastructure_inner(record).await.map_err(
            api_gateway::adapters::out::infrastructure_error::classify_infrastructure_error,
        )
    }
    async fn summary(
        &self,
    ) -> api_gateway::application::infrastructure_error::GatewayInfrastructureResult<
        api_gateway::application::dto::infrastructure_dto::TelemetrySummaryDto,
    > {
        self.summary_infrastructure_inner()
            .await
            .map(Into::into)
            .map_err(
                api_gateway::adapters::out::infrastructure_error::classify_infrastructure_error,
            )
    }
}

impl SlowTelemetry {
    async fn record_infrastructure_inner(&self, _record: TelemetryRecord) -> anyhow::Result<()> {
        tokio::time::sleep(Duration::from_secs(5)).await;
        Ok(())
    }

    async fn summary_infrastructure_inner(&self) -> anyhow::Result<TelemetrySummary> {
        Ok(TelemetrySummary {
            total_requests: 0,
            error_rate: 0.0,
            p95_latency_ms: 0,
        })
    }
}

#[async_trait]
impl TelemetryRepositoryPort for SlowTelemetry {
    async fn record(
        &self,
        _record: TelemetryRecord,
    ) -> api_gateway::application::infrastructure_error::GatewayInfrastructureResult<()> {
        self.record_infrastructure_inner(_record).await.map_err(
            api_gateway::adapters::out::infrastructure_error::classify_infrastructure_error,
        )
    }
    async fn summary(
        &self,
    ) -> api_gateway::application::infrastructure_error::GatewayInfrastructureResult<
        api_gateway::application::dto::infrastructure_dto::TelemetrySummaryDto,
    > {
        self.summary_infrastructure_inner()
            .await
            .map(Into::into)
            .map_err(
                api_gateway::adapters::out::infrastructure_error::classify_infrastructure_error,
            )
    }
}

#[tokio::test]
async fn a_slow_telemetry_writer_does_not_extend_the_request() -> anyhow::Result<()> {
    let app = support::start_gateway_app_with_runtime_mode(false, RuntimeMode::Production).await?;
    let mut state = (*app.state).clone();
    // The writer takes seconds to complete one insert. Recording used to run
    // inline, which made that latency part of every response: this assertion is
    // the difference between observing a request and extending it.
    state.request_timeout = Duration::from_millis(50);
    let slow = QueuedTelemetry::with_capacity(Arc::new(SlowTelemetry), 16);
    state.telemetry_service = slow.clone();
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        http::router(Arc::new(state)).oneshot(
            Request::builder()
                .uri("/docs")
                .header("x-correlation-id", "slow-telemetry")
                .body(Body::empty())?,
        ),
    )
    .await;
    app.shutdown().await?;
    let response = result??;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "a slow telemetry writer must not turn a successful request into a timeout"
    );
    assert_eq!(response.headers()["x-correlation-id"], "slow-telemetry");
    assert_eq!(
        slow.shed_records(),
        0,
        "a queue that accepts the record must not count a drop"
    );
    Ok(())
}

#[tokio::test]
async fn a_full_telemetry_queue_is_shed_and_counted() -> anyhow::Result<()> {
    let app = support::start_gateway_app_with_runtime_mode(false, RuntimeMode::Production).await?;
    let mut state = (*app.state).clone();
    // A one-slot queue with a writer that never finishes. The second request has
    // nowhere to go, and the point of the assertion is that the loss is reported
    // rather than absorbed: telemetry that silently stops being recorded looks
    // exactly like a system under no load.
    let telemetry = QueuedTelemetry::with_capacity(Arc::new(SlowTelemetry), 1);
    state.telemetry_service = telemetry.clone();
    let router = http::router(Arc::new(state));
    for _ in 0..3 {
        let response = router
            .clone()
            .oneshot(Request::builder().uri("/docs").body(Body::empty())?)
            .await?;
        assert!(response.status().is_success());
    }

    // The queue is drained asynchronously, so the drop count is observed after a
    // short wait rather than assumed to be exact for a given request count.
    tokio::time::sleep(Duration::from_millis(50)).await;
    let dropped = telemetry.shed_records();
    app.shutdown().await?;
    assert!(
        dropped > 0,
        "a bounded queue must report records it could not accept"
    );
    Ok(())
}

#[tokio::test]
async fn request_deadline_includes_audit_lock_wait() -> anyhow::Result<()> {
    let app = support::start_gateway_app_with_runtime_mode(false, RuntimeMode::Production).await?;
    let mut lock = app.pool.begin().await?;
    sqlx::query("lock table admin_audit_events in access exclusive mode")
        .execute(&mut *lock)
        .await?;
    let mut state = (*app.state).clone();
    state.request_timeout = Duration::from_millis(50);
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        http::router(Arc::new(state)).oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/unknown-mutation")
                .body(Body::empty())?,
        ),
    )
    .await;
    lock.rollback().await?;
    app.shutdown().await?;
    assert_eq!(result??.status(), StatusCode::GATEWAY_TIMEOUT);
    Ok(())
}

#[tokio::test]
async fn dynamic_post_ids_share_one_metric_series() -> anyhow::Result<()> {
    let app = support::start_gateway_app_with_rate_limit(false, 1).await?;
    let first = app
        .router
        .clone()
        .oneshot(Request::builder().uri("/docs").body(Body::empty())?)
        .await?;
    let mut statuses = Vec::new();
    let mut ids = Vec::new();
    for _ in 0..3 {
        let id = Uuid::new_v4().to_string();
        let response = app
            .router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/posts/{id}"))
                    .body(Body::empty())?,
            )
            .await?;
        statuses.push(response.status());
        ids.push(id);
    }
    let metrics = app.state.metrics.render_prometheus();
    app.shutdown().await?;
    assert_ne!(first.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(statuses
        .iter()
        .all(|status| *status == StatusCode::TOO_MANY_REQUESTS));
    assert!(
        metrics.contains("gateway_rate_limit_rejections_total{route=\"/api/posts/:post_id\"} 3")
    );
    for id in ids {
        assert!(!metrics.contains(&id));
    }
    Ok(())
}

#[tokio::test]
async fn an_unauthenticated_client_id_header_is_not_recorded_as_a_client() -> anyhow::Result<()> {
    // The header carries no credential, so a caller could otherwise attribute
    // its traffic to any client it named. Per-client telemetry and quota views
    // read this field, so a spoofable value is worse than a missing one.
    let app = support::start_gateway_app_with_runtime_mode(false, RuntimeMode::Production).await?;
    let recorder = Arc::new(RecordingTelemetry::default());
    let mut state = (*app.state).clone();
    state.telemetry_service = QueuedTelemetry::with_capacity(recorder.clone(), 16);

    let response = http::router(Arc::new(state))
        .oneshot(
            Request::builder()
                .uri("/docs")
                .header("x-client-id", "some-victim-client")
                .body(Body::empty())?,
        )
        .await?;
    app.shutdown().await?;

    // The request still succeeds; only the client attribution is withheld.
    assert!(response.status().is_success(), "unexpected status");
    let records = recorder.records.lock().expect("telemetry lock");
    assert_eq!(records.len(), 1, "the request must still be recorded");
    assert_eq!(
        records[0].client_profile_id, None,
        "an unauthenticated client id header must not become a client identity"
    );
    Ok(())
}
