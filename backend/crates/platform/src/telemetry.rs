//! JSON logging and opt-in, allow-listed OTLP trace export.

use anyhow::{bail, Context as _};
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::SpanExporter;
use opentelemetry_sdk::{
    trace::{Sampler, SdkTracerProvider},
    Resource,
};
use std::{env, time::Duration};
use tracing::{Metadata, Subscriber};
use tracing_subscriber::{
    layer::{Context, Filter},
    prelude::*,
    EnvFilter,
};

const TRACE_ENDPOINT_ENV: &str = "OTEL_EXPORTER_OTLP_TRACES_ENDPOINT";
const OTLP_ENDPOINT_ENV: &str = "OTEL_EXPORTER_OTLP_ENDPOINT";
const TRACE_PROTOCOL_ENV: &str = "OTEL_EXPORTER_OTLP_TRACES_PROTOCOL";
const OTLP_PROTOCOL_ENV: &str = "OTEL_EXPORTER_OTLP_PROTOCOL";

/// Initialize JSON logs and (when an OTLP endpoint is explicitly configured)
/// bounded OpenTelemetry trace export.
///
/// The OTLP layer exports only the reviewed span names in [`is_exportable_span`]
/// and never exports tracing events. This prevents arbitrary application log
/// fields, request values, or error strings from becoming OTLP attributes.
pub fn init(service_name: &str) -> anyhow::Result<TelemetryGuard> {
    let provider = if has_otlp_endpoint()? {
        Some(build_provider(service_name)?)
    } else {
        None
    };

    let otel_layer = provider.as_ref().map(|provider| {
        tracing_opentelemetry::layer()
            .with_tracer(provider.tracer(service_name.to_owned()))
            .with_filter(SafeSpanFilter)
    });
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let json_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_thread_names(false)
        .json()
        .with_current_span(true)
        .with_filter(filter);

    tracing_subscriber::registry()
        .with(otel_layer)
        .with(json_layer)
        .try_init()
        .context("initialize tracing subscriber")?;

    tracing::info!(
        service = service_name,
        otlp_enabled = provider.is_some(),
        "telemetry initialized"
    );
    Ok(TelemetryGuard { provider })
}

/// Owns the provider until the service has drained its workers and listeners.
/// Dropping early still attempts a bounded flush, including on startup errors.
pub struct TelemetryGuard {
    provider: Option<SdkTracerProvider>,
}

impl TelemetryGuard {
    /// Flush and stop export after the application has completed graceful
    /// shutdown. A timeout or exporter failure is logged without exposing its
    /// endpoint or headers.
    pub fn shutdown(mut self) {
        self.shutdown_inner();
    }

    fn shutdown_inner(&mut self) {
        let Some(provider) = self.provider.take() else {
            return;
        };
        if provider
            .shutdown_with_timeout(Duration::from_secs(2))
            .is_err()
        {
            tracing::warn!("OTLP trace provider did not shut down cleanly");
        }
    }
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        self.shutdown_inner();
    }
}

fn has_otlp_endpoint() -> anyhow::Result<bool> {
    let traces_endpoint = optional_env(TRACE_ENDPOINT_ENV)?;
    let generic_endpoint = if traces_endpoint.is_none() {
        optional_env(OTLP_ENDPOINT_ENV)?
    } else {
        None
    };
    endpoint_is_configured(traces_endpoint.as_deref(), generic_endpoint.as_deref())
}

fn endpoint_is_configured(
    traces_endpoint: Option<&str>,
    generic_endpoint: Option<&str>,
) -> anyhow::Result<bool> {
    let Some((name, value)) = traces_endpoint
        .map(|value| (TRACE_ENDPOINT_ENV, value))
        .or_else(|| generic_endpoint.map(|value| (OTLP_ENDPOINT_ENV, value)))
    else {
        return Ok(false);
    };
    if value.trim().is_empty() {
        bail!("{name} must not be empty when set")
    }
    Ok(true)
}

fn build_provider(service_name: &str) -> anyhow::Result<SdkTracerProvider> {
    validate_protocol_from_env()?;
    let exporter = SpanExporter::builder()
        .with_tonic()
        .build()
        .context("build OTLP/gRPC trace exporter")?;
    Ok(SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_sampler(sampler_from_env()?)
        .with_resource(
            Resource::builder()
                .with_service_name(service_name.to_owned())
                .build(),
        )
        .with_max_attributes_per_span(16)
        .with_max_events_per_span(0)
        .with_max_links_per_span(0)
        .build())
}

fn validate_protocol_from_env() -> anyhow::Result<()> {
    let trace_protocol = optional_env(TRACE_PROTOCOL_ENV)?;
    let generic_protocol = if trace_protocol.is_none() {
        optional_env(OTLP_PROTOCOL_ENV)?
    } else {
        None
    };
    validate_protocol(trace_protocol.as_deref(), generic_protocol.as_deref())
}

fn validate_protocol(
    trace_protocol: Option<&str>,
    generic_protocol: Option<&str>,
) -> anyhow::Result<()> {
    let protocol = trace_protocol.or(generic_protocol);
    if protocol.is_some_and(|protocol| protocol != "grpc") {
        bail!("configured OTLP trace protocol must be grpc")
    }
    Ok(())
}

fn sampler_from_env() -> anyhow::Result<Sampler> {
    let sampler = env_or_default("OTEL_TRACES_SAMPLER", "parentbased_always_on")?;
    Ok(match sampler.as_str() {
        "always_on" => Sampler::AlwaysOn,
        "always_off" => Sampler::AlwaysOff,
        "traceidratio" => Sampler::TraceIdRatioBased(sample_ratio_from_env()?),
        "parentbased_always_on" => Sampler::ParentBased(Box::new(Sampler::AlwaysOn)),
        "parentbased_always_off" => Sampler::ParentBased(Box::new(Sampler::AlwaysOff)),
        "parentbased_traceidratio" => Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(
            sample_ratio_from_env()?,
        ))),
        _ => bail!("unsupported OTEL_TRACES_SAMPLER value"),
    })
}

fn sample_ratio_from_env() -> anyhow::Result<f64> {
    let ratio = env_or_default("OTEL_TRACES_SAMPLER_ARG", "1.0")?
        .parse::<f64>()
        .context("OTEL_TRACES_SAMPLER_ARG must be a number between 0 and 1")?;
    if !ratio.is_finite() || !(0.0..=1.0).contains(&ratio) {
        bail!("OTEL_TRACES_SAMPLER_ARG must be between 0 and 1")
    }
    Ok(ratio)
}

fn optional_env(name: &str) -> anyhow::Result<Option<String>> {
    match env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => bail!("{name} must be valid UTF-8"),
    }
}

fn env_or_default(name: &str, default: &str) -> anyhow::Result<String> {
    Ok(optional_env(name)?.unwrap_or_else(|| default.to_owned()))
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SafeSpanFilter;

impl<S> Filter<S> for SafeSpanFilter
where
    S: Subscriber,
{
    fn enabled(&self, metadata: &Metadata<'_>, _context: &Context<'_, S>) -> bool {
        metadata.is_span() && is_exportable_span(metadata.target(), metadata.name())
    }
}

/// OTLP attribute allow-list. Keep this list explicit: adding a new span here
/// requires reviewing every field it records for secrets and personal data.
fn is_exportable_span(target: &str, name: &str) -> bool {
    matches!(
        (target, name),
        ("platform::trace_context", "request")
            | ("platform::grpc_client_trace", "grpc_client_call")
            | (
                "api_gateway::adapters::in::http::operation_worker",
                "gateway_async_operation"
            )
            | (
                "api_gateway::adapters::out::telemetry_queue",
                "gateway_telemetry_write"
            )
            | (
                "auth_service::adapters::in::messaging::user_deletion_worker",
                "auth_user_deletion_dispatch"
            )
            | (
                "auth_service::adapters::in::messaging::mail_outbox_worker",
                "auth_mail_delivery"
            )
            | (
                "post_service::adapters::in::messaging::external_event_forwarder",
                "post_outbox_delivery"
            )
            | (
                "post_service::adapters::out::persistence::post_repository",
                "post_write_transaction"
            )
            | (
                "post_service::adapters::out::persistence::event_outbox",
                "post_outbox_claim"
            )
            | (
                "post_service::adapters::out::persistence::event_outbox",
                "post_outbox_insert_event"
            )
            | (
                "post_service::adapters::out::persistence::event_outbox",
                "post_outbox_publish_failure"
            )
            | (
                "post_service::adapters::out::persistence::event_outbox",
                "post_outbox_append_delivery_attempt"
            )
            | (
                "post_service::adapters::out::persistence::event_outbox",
                "post_outbox_delivery_check"
            )
            | (
                "post_service::adapters::out::persistence::event_outbox",
                "post_outbox_mark_delivery_completed"
            )
            | (
                "post_service::adapters::out::persistence::event_outbox",
                "post_outbox_mark_published"
            )
    )
}

#[cfg(test)]
mod tests {
    use super::{endpoint_is_configured, is_exportable_span, validate_protocol};
    use opentelemetry::trace::{Tracer as _, TracerProvider as _};
    use opentelemetry_otlp::{SpanExporter, WithExportConfig};
    use opentelemetry_sdk::trace::{Sampler, SdkTracerProvider};
    use std::time::Duration;
    use tokio_stream::wrappers::TcpListenerStream;
    use tonic::{Request, Response, Status};

    #[test]
    fn otlp_span_allow_list_is_name_and_target_specific() {
        assert!(is_exportable_span("platform::trace_context", "request"));
        assert!(is_exportable_span(
            "platform::grpc_client_trace",
            "grpc_client_call"
        ));
        assert!(!is_exportable_span("platform::trace_context", "user_login"));
        assert!(!is_exportable_span(
            "platform::shutdown",
            "background_worker_cycle"
        ));
        assert!(is_exportable_span(
            "auth_service::adapters::in::messaging::mail_outbox_worker",
            "auth_mail_delivery"
        ));
        assert!(!is_exportable_span(
            "auth_service::adapters::in::messaging::mail_outbox_worker",
            "unreviewed_mail_span"
        ));
        assert!(is_exportable_span(
            "post_service::adapters::in::messaging::external_event_forwarder",
            "post_outbox_delivery"
        ));
        assert!(!is_exportable_span(
            "post_service::adapters::in::messaging::external_event_forwarder",
            "unreviewed_outbox_span"
        ));
        assert!(!is_exportable_span("api_gateway::auth", "request"));
        assert!(is_exportable_span(
            "post_service::adapters::out::persistence::post_repository",
            "post_write_transaction"
        ));
        assert!(is_exportable_span(
            "post_service::adapters::out::persistence::event_outbox",
            "post_outbox_claim"
        ));
        assert!(!is_exportable_span(
            "post_service::adapters::out::persistence::event_outbox",
            "post_write_transaction"
        ));
        for span_name in [
            "post_outbox_insert_event",
            "post_outbox_publish_failure",
            "post_outbox_append_delivery_attempt",
            "post_outbox_delivery_check",
            "post_outbox_mark_delivery_completed",
            "post_outbox_mark_published",
        ] {
            assert!(is_exportable_span(
                "post_service::adapters::out::persistence::event_outbox",
                span_name
            ));
        }
    }

    #[test]
    fn signal_specific_protocol_overrides_the_generic_protocol() {
        assert!(validate_protocol(None, None).is_ok());
        assert!(validate_protocol(None, Some("grpc")).is_ok());
        assert!(validate_protocol(Some("grpc"), Some("http/protobuf")).is_ok());
        assert!(validate_protocol(None, Some("http/protobuf")).is_err());
        assert!(validate_protocol(Some("http/protobuf"), Some("grpc")).is_err());
    }

    #[test]
    fn traces_endpoint_overrides_the_generic_endpoint() {
        assert!(!endpoint_is_configured(None, None).unwrap());
        assert!(endpoint_is_configured(None, Some("http://collector:4317")).unwrap());
        assert!(
            endpoint_is_configured(Some("http://traces:4317"), Some("http://collector:4317"))
                .unwrap()
        );
        assert!(endpoint_is_configured(Some("  "), Some("http://collector:4317")).is_err());
    }

    #[tokio::test]
    async fn otlp_grpc_export_reaches_the_configured_loopback_collector() {
        use opentelemetry_proto::tonic::collector::trace::v1::{
            trace_service_server::{TraceService, TraceServiceServer},
            ExportTraceServiceRequest, ExportTraceServiceResponse,
        };
        use tokio::sync::mpsc;
        use tonic::transport::Server;

        struct Capture(mpsc::UnboundedSender<ExportTraceServiceRequest>);

        #[tonic::async_trait]
        impl TraceService for Capture {
            async fn export(
                &self,
                request: Request<ExportTraceServiceRequest>,
            ) -> Result<Response<ExportTraceServiceResponse>, Status> {
                self.0
                    .send(request.into_inner())
                    .map_err(|_| Status::unavailable("test receiver closed"))?;
                Ok(Response::new(ExportTraceServiceResponse::default()))
            }
        }

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("loopback listener");
        let endpoint = format!(
            "http://{}",
            listener.local_addr().expect("listener address")
        );
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let server = tokio::spawn(async move {
            Server::builder()
                .add_service(TraceServiceServer::new(Capture(sender)))
                .serve_with_incoming(TcpListenerStream::new(listener))
                .await
        });
        let exporter = SpanExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint)
            .build()
            .expect("build OTLP gRPC exporter");
        let provider = SdkTracerProvider::builder()
            .with_batch_exporter(exporter)
            .with_sampler(Sampler::AlwaysOn)
            .build();
        provider
            .tracer("platform-otlp-test")
            .in_span("otlp_export_probe", |_| {});
        let flush_provider = provider.clone();
        tokio::task::spawn_blocking(move || flush_provider.force_flush())
            .await
            .expect("OTLP flush task completes")
            .expect("flush OTLP trace batch");
        let exported = tokio::time::timeout(Duration::from_secs(3), receiver.recv())
            .await
            .expect("collector receives trace batch before timeout")
            .expect("collector trace batch");
        assert!(exported.resource_spans.iter().any(|resource_spans| {
            resource_spans.scope_spans.iter().any(|scope_spans| {
                scope_spans
                    .spans
                    .iter()
                    .any(|span| span.name == "otlp_export_probe")
            })
        }));
        provider
            .shutdown_with_timeout(Duration::from_secs(3))
            .expect("shut down OTLP provider");
        server.abort();
    }

    #[tokio::test]
    async fn reviewed_post_database_span_exports_only_bounded_child_metadata() {
        use crate::trace_context::{
            self, TraceContextLayer, TRACEPARENT_HEADER, TRACESTATE_HEADER,
        };
        use opentelemetry::trace::TracerProvider as _;
        use opentelemetry_sdk::trace::{InMemorySpanExporter, Sampler, SdkTracerProvider};
        use std::convert::Infallible;
        use tower::{Layer as _, ServiceExt as _};
        use tracing::instrument::WithSubscriber as _;
        use tracing::Instrument as _;
        use tracing_subscriber::prelude::*;

        const PARENT: &str = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
        const PRIVATE_STATE: &str = "vendor=private-state-marker";

        let exporter = InMemorySpanExporter::default();
        let provider = SdkTracerProvider::builder()
            .with_simple_exporter(exporter.clone())
            .with_sampler(Sampler::AlwaysOn)
            .build();
        let subscriber = tracing_subscriber::registry().with(
            tracing_opentelemetry::layer()
                .with_tracer(provider.tracer("platform-test"))
                .with_filter(super::SafeSpanFilter),
        );
        // This capture is scoped to one test, while callsites are shared with
        // other test threads. Avoid tracing-core's single-dispatcher shortcut
        // resolving a callsite against another thread's unscoped default.
        let _registration_peer = tracing::Dispatch::new(tracing_subscriber::registry());
        let dispatch = tracing::Dispatch::new(subscriber);
        let service =
            TraceContextLayer.layer(tower::service_fn(|_request: http::Request<()>| async {
                let request_context = trace_context::current().expect("request trace context");
                let span = tracing::info_span!(
                    target: "post_service::adapters::out::persistence::post_repository",
                    "post_write_transaction",
                    db_system = "postgresql",
                    db_operation = "post.create_with_outbox",
                    trace_id = tracing::field::Empty,
                    span_id = tracing::field::Empty,
                    parent_span_id = tracing::field::Empty,
                    sampled = tracing::field::Empty,
                    outcome = tracing::field::Empty,
                );
                trace_context::bind_current_child_to_span(&span);
                async {}.instrument(span.clone()).await;
                span.record("outcome", "success");
                Ok::<_, Infallible>(request_context)
            }));
        let request = http::Request::builder()
            .uri("/trace-probe")
            .header(TRACEPARENT_HEADER, PARENT)
            .header(TRACESTATE_HEADER, PRIVATE_STATE)
            .body(())
            .expect("probe request");
        let request_context = service
            .oneshot(request)
            .with_subscriber(dispatch)
            .await
            .expect("probe response");

        provider.force_flush().expect("flush in-memory spans");
        let spans = exporter.get_finished_spans().expect("read exported spans");
        let request_span = spans
            .iter()
            .find(|span| {
                span.name == "request"
                    && format!("{:016x}", span.span_context.span_id()) == request_context.span_id()
            })
            .expect("exported request span");
        let database_span = spans
            .iter()
            .find(|span| span.name == "post_write_transaction")
            .expect("exported database span");
        assert_eq!(
            format!("{:032x}", database_span.span_context.trace_id()),
            request_context.trace_id()
        );
        assert_eq!(
            format!("{:016x}", database_span.parent_span_id),
            format!("{:016x}", request_span.span_context.span_id())
        );
        assert!(database_span.events.is_empty());
        assert!(database_span.links.is_empty());
        let attributes = format!("{:?}", database_span.attributes);
        assert!(attributes.contains("db_system"));
        assert!(attributes.contains("postgresql"));
        assert!(attributes.contains("db_operation"));
        assert!(attributes.contains("post.create_with_outbox"));
        assert!(attributes.contains("success"));
        assert!(!attributes.contains(PRIVATE_STATE));
        assert!(!attributes.contains("db.statement"));
        assert!(!attributes.contains("query"));
        assert!(!attributes.contains("actor_id"));

        provider
            .shutdown_with_timeout(Duration::from_secs(3))
            .expect("shut down in-memory provider");
    }
}
