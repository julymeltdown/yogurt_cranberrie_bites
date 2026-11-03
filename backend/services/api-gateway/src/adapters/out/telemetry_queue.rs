//! Bounded, non-blocking hand-off for telemetry records.
//!
//! Recording a request used to be awaited before the response was returned, so
//! every response ended when a database insert ended: a slow or locked telemetry
//! table added its latency directly to the request, and the request timeout had to
//! cover a write that only observes the request. That inverts the purpose of
//! telemetry.
//!
//! The queue is an adapter concern rather than an application rule, so it lives
//! here: the application service keeps its straight-through write, and the runtime
//! decides whether that write happens on the request path.
//!
//! Shedding is counted rather than silent. Telemetry that quietly stops being
//! recorded looks exactly like a system under no load, which is the one report an
//! observability pipeline must not produce.

use crate::application::dto::facade_dto::TelemetrySummaryResponseDto;
use crate::application::ports::{TelemetryRecord, TelemetryRepositoryPort};
use crate::application::service::TelemetryService;
use crate::domain::telemetry::TelemetrySummary;
use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::Instrument;
use uuid::Uuid;

/// Default number of records that may wait for the writer.
pub const DEFAULT_QUEUE_CAPACITY: usize = 4096;

/// Telemetry that is written on the calling task.
///
/// This is the application service behind the queue. It is a distinct name so a
/// reader cannot confuse the straight-through writer with the queued handler that
/// schedules it.
pub type TelemetryWriter = TelemetryService;

/// Queues telemetry and writes it in the background.
#[derive(Clone)]
pub struct QueuedTelemetry {
    repository: Arc<dyn TelemetryRepositoryPort>,
    sender: mpsc::Sender<QueuedTelemetryRecord>,
    capacity: usize,
    shed: Arc<AtomicU64>,
}

/// In-memory handoff metadata; trace context is intentionally not part of the
/// telemetry row or persisted schema.
struct QueuedTelemetryRecord {
    record: TelemetryRecord,
    traceparent: Option<String>,
}

impl QueuedTelemetryRecord {
    fn capture(record: TelemetryRecord) -> Self {
        // Persist only the normalized, fixed-size W3C parent. Vendor state and
        // baggage do not cross even this asynchronous queue boundary.
        let traceparent = platform::trace_context::current().map(|context| context.traceparent());
        Self {
            record,
            traceparent,
        }
    }
}

impl QueuedTelemetry {
    /// Start the writer and return a handler that admits records.
    ///
    /// `capacity` is the number of records allowed to wait. It is configuration
    /// rather than a constant because the right bound depends on how much latency
    /// the writer can absorb before shedding: a deployment that prefers a visible
    /// drop to a growing queue picks a small one.
    pub fn with_capacity(repository: Arc<dyn TelemetryRepositoryPort>, capacity: usize) -> Self {
        let capacity = capacity.max(1);
        let (sender, receiver) = mpsc::channel(capacity);
        let shed = Arc::new(AtomicU64::new(0));
        let writer = repository.clone();
        let writer_shed = shed.clone();
        tokio::spawn(async move {
            write_records(writer, receiver, writer_shed).await;
        });
        Self {
            repository,
            sender,
            capacity,
            shed,
        }
    }

    /// The queue bound this handler was created with.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// The repository the queued writes go to.
    pub fn repository(&self) -> Arc<dyn TelemetryRepositoryPort> {
        self.repository.clone()
    }

    /// Admit a record without waiting for the write.
    ///
    /// Returns whether it was accepted. A full queue is not something the request
    /// path can act on, so the record is shed and counted instead of failing a
    /// request that already succeeded.
    pub fn admit(
        &self,
        correlation_id: String,
        route_key: String,
        client_profile_id: Option<String>,
        outcome: String,
        latency_ms: i64,
        error_code: Option<String>,
        downstream_latencies: HashMap<String, i64>,
    ) -> bool {
        let record = TelemetryRecord {
            id: Uuid::new_v4().to_string(),
            correlation_id,
            route_key,
            client_profile_id,
            outcome,
            latency_ms,
            error_code,
            downstream_latencies,
            recorded_at: Utc::now(),
        };
        match self.sender.try_send(QueuedTelemetryRecord::capture(record)) {
            Ok(()) => true,
            // Full means the writer is behind; Closed means it has stopped and the
            // queue cannot recover. Both lose the record, so both are counted
            // rather than being reported as accepted.
            Err(mpsc::error::TrySendError::Full(_)) | Err(mpsc::error::TrySendError::Closed(_)) => {
                self.shed.fetch_add(1, Ordering::Relaxed);
                false
            }
        }
    }

    /// Records shed because the queue was full or the writer stopped.
    pub fn shed_records(&self) -> u64 {
        self.shed.load(Ordering::Relaxed)
    }

    /// Read the aggregate straight from the repository, where the rows are.
    pub async fn summary(
        &self,
    ) -> crate::application::service_error::GatewayServiceResult<TelemetrySummaryResponseDto> {
        self.repository
            .summary()
            .await
            .map(Into::into)
            .map_err(Into::into)
    }

    /// Wait until every admitted record has been written.
    ///
    /// A shutdown that returns while the writer still holds records loses them,
    /// so a caller that cares about the tail drains first. The wait is bounded so
    /// a stuck writer cannot hold shutdown forever.
    pub async fn drain(&self) {
        for _ in 0..200 {
            if self.sender.capacity() == self.capacity {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }
}

impl QueuedTelemetry {
    /// Admit the record. The write happens in the background writer.
    async fn record_infrastructure_inner(&self, record: TelemetryRecord) -> anyhow::Result<()> {
        match self.sender.try_send(QueuedTelemetryRecord::capture(record)) {
            Ok(()) => Ok(()),
            Err(mpsc::error::TrySendError::Full(_)) | Err(mpsc::error::TrySendError::Closed(_)) => {
                self.shed.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
        }
    }

    /// Read through to the repository, which is where the rows are.
    async fn summary_infrastructure_inner(&self) -> anyhow::Result<TelemetrySummary> {
        self.repository
            .summary()
            .await
            .map(Into::into)
            .map_err(Into::into)
    }
}

#[async_trait]
impl TelemetryRepositoryPort for QueuedTelemetry {
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

/// Write queued records until every sender is gone.
async fn write_records(
    repository: Arc<dyn TelemetryRepositoryPort>,
    mut receiver: mpsc::Receiver<QueuedTelemetryRecord>,
    shed: Arc<AtomicU64>,
) {
    while let Some(queued) = receiver.recv().await {
        let trace_context = queued
            .traceparent
            .as_deref()
            .and_then(platform::trace_context::TraceContext::child_from_traceparent);
        let record = queued.record;
        let write = async {
            if let Err(error) = repository.record(record).await {
                // Counted per failed write rather than once, because the failure
                // is ongoing and would otherwise be reported only at first.
                shed.fetch_add(1, Ordering::Relaxed);
                tracing::error!(error = ?error, "failed to write telemetry record");
            }
        };
        match trace_context {
            Some(mut context) => {
                let span = tracing::info_span!(
                    parent: None,
                    "gateway_telemetry_write",
                    trace_id = tracing::field::Empty,
                    span_id = tracing::field::Empty,
                    parent_span_id = tracing::field::Empty,
                    sampled = tracing::field::Empty,
                );
                context.bind_to_span(&span);
                context.record_on_span(&span);
                platform::trace_context::scope(context, write.instrument(span)).await;
            }
            None => write.await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use tokio::sync::mpsc as test_mpsc;
    use tower::{Layer, ServiceExt};

    struct TraceCapture(test_mpsc::UnboundedSender<Option<platform::trace_context::TraceContext>>);

    impl TraceCapture {
        async fn record_infrastructure_inner(
            &self,
            _record: TelemetryRecord,
        ) -> anyhow::Result<()> {
            self.0
                .send(platform::trace_context::current())
                .map_err(|_| anyhow::anyhow!("trace capture receiver closed"))
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
    impl TelemetryRepositoryPort for TraceCapture {
        async fn record(
            &self,
            _record: TelemetryRecord,
        ) -> crate::application::infrastructure_error::GatewayInfrastructureResult<()> {
            self.record_infrastructure_inner(_record)
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

    #[tokio::test]
    async fn detached_writer_continues_only_normalized_request_trace_context() {
        let (captured_tx, mut captured_rx) = test_mpsc::unbounded_channel();
        let queue = QueuedTelemetry::with_capacity(Arc::new(TraceCapture(captured_tx)), 1);
        let (request_context_tx, mut request_context_rx) = test_mpsc::unbounded_channel();
        let service = platform::trace_context::TraceContextLayer.layer(tower::service_fn(
            move |_request: http::Request<()>| {
                let queue = queue.clone();
                let request_context_tx = request_context_tx.clone();
                async move {
                    request_context_tx
                        .send(platform::trace_context::current())
                        .expect("request context receiver is open");
                    assert!(queue.admit(
                        "correlation-id".into(),
                        "GET /health".into(),
                        None,
                        "SUCCESS".into(),
                        1,
                        None,
                        HashMap::new(),
                    ));
                    Ok::<_, std::convert::Infallible>(http::Response::new(()))
                }
            },
        ));
        let request = http::Request::builder()
            .header(
                platform::trace_context::TRACEPARENT_HEADER,
                "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
            )
            .header(platform::trace_context::TRACESTATE_HEADER, "vendor=value")
            .body(())
            .expect("valid trace request");
        service
            .oneshot(request)
            .await
            .expect("trace test service responds");

        let request_context = request_context_rx
            .recv()
            .await
            .expect("HTTP trace layer reports the request context")
            .expect("HTTP trace layer establishes a request context");
        let trace_id = request_context.trace_id().to_owned();
        let request_span_id = request_context.span_id().to_owned();

        let worker_context =
            tokio::time::timeout(std::time::Duration::from_secs(1), captured_rx.recv())
                .await
                .expect("background telemetry writer completes")
                .expect("background telemetry writer reports its context")
                .expect("validated request trace context is continued");
        assert_eq!(worker_context.trace_id(), trace_id);
        assert_ne!(worker_context.span_id(), request_span_id);
        assert_eq!(
            worker_context.parent_span_id(),
            Some(request_span_id.as_str())
        );
        assert!(worker_context.sampled());
        assert_eq!(worker_context.tracestate(), None);
    }
}
