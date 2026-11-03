//! Verifies that Post outbox delivery traces preserve retry causality while
//! exporting only bounded worker metadata.

#[path = "unit/support.rs"]
#[allow(dead_code)]
mod support;

use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::trace::{InMemorySpanExporter, Sampler, SdkTracerProvider};
use post_service::adapters::r#in::messaging::external_event_forwarder::ExternalEventForwarder;
use post_service::domain::post::StoredEvent;
use support::{instant, stored_event, uuid, Harness};
use tracing_subscriber::prelude::*;

const DELIVERY_TARGET: &str = "post_service::adapters::in::messaging::external_event_forwarder";

fn forwarder(harness: &Harness) -> ExternalEventForwarder {
    ExternalEventForwarder::new(
        harness.outbox.clone(),
        harness.publisher.clone(),
        harness.internal.clone(),
        harness.feed_cache.clone(),
        harness.post_cache.clone(),
        harness.friend.clone(),
        std::sync::Arc::new(support::RecordingTimelineProjection::default()),
    )
    .with_delivery_metrics(harness.metrics.clone())
}

fn traced_event(event_id: uuid::Uuid, post_id: uuid::Uuid, payload: &str) -> StoredEvent {
    let mut event = stored_event(
        event_id,
        "POST_LIKED",
        serde_json::json!({ "postId": post_id, "content": payload }).to_string(),
    );
    event.origin_traceparent =
        Some("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01".to_owned());
    event
}

#[test]
fn external_forwarder_exports_bounded_outcomes_for_retry_and_recovery() {
    const PRIVATE_PAYLOAD: &str = "private-post-outbox-payload-marker";
    const PRIVATE_ERROR: &str = "private-outbox-error-marker";

    let exporter = InMemorySpanExporter::default();
    let provider = SdkTracerProvider::builder()
        .with_simple_exporter(exporter.clone())
        .with_sampler(Sampler::AlwaysOn)
        .with_max_events_per_span(0)
        .with_max_links_per_span(0)
        .build();
    let subscriber = tracing_subscriber::registry().with(
        tracing_opentelemetry::layer()
            .with_tracer(provider.tracer("post-outbox-outcome-test"))
            .with_filter(tracing_subscriber::filter::filter_fn(|metadata| {
                metadata.target() == DELIVERY_TARGET && metadata.name() == "post_outbox_delivery"
            })),
    );
    let dispatch = tracing::Dispatch::new(subscriber);
    let test_result = tracing::dispatcher::with_default(&dispatch, || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tracing test runtime");
        runtime.block_on(async {
            let harness = Harness::new();
            let event_id = uuid("33333333-3333-3333-3333-333333333390");
            let post_id = uuid("33333333-3333-3333-3333-333333333391");
            *harness.outbox.unpublished.lock().unwrap() =
                vec![traced_event(event_id, post_id, PRIVATE_PAYLOAD)];
            *harness.post_cache.error.lock().unwrap() = Some("cache unavailable".to_owned());
            let forwarder = forwarder(&harness);

            assert_eq!(
                forwarder
                    .republish_unpublished(instant(1_700_000_300), 1)
                    .await
                    .expect("target failure is persisted for retry"),
                0
            );
            *harness.post_cache.error.lock().unwrap() = None;
            assert_eq!(
                forwarder
                    .republish_unpublished(instant(1_700_000_301), 1)
                    .await
                    .expect("retry completes after cache recovery"),
                1
            );

            let dependency_event_id = uuid("33333333-3333-3333-3333-333333333392");
            let dependency_post_id = uuid("33333333-3333-3333-3333-333333333393");
            harness
                .outbox
                .unpublished
                .lock()
                .unwrap()
                .push(traced_event(
                    dependency_event_id,
                    dependency_post_id,
                    PRIVATE_PAYLOAD,
                ));
            *harness.outbox.lease_status_error.lock().unwrap() = Some(PRIVATE_ERROR.to_owned());
            let error = forwarder
                .republish_unpublished(instant(1_700_000_302), 1)
                .await
                .expect_err("outbox persistence error is surfaced for retry");
            assert!(error.to_string().contains(PRIVATE_ERROR));

            provider
                .force_flush()
                .expect("flush captured delivery spans");
            let spans = exporter
                .get_finished_spans()
                .expect("read captured delivery spans");
            let delivery_spans = spans
                .iter()
                .filter(|span| span.name == "post_outbox_delivery")
                .collect::<Vec<_>>();
            anyhow::ensure!(delivery_spans.len() == 6);
            let trace_id = "4bf92f3577b34da6a3ce929d0e0e4736";
            let parent_span_id = "00f067aa0ba902b7";
            let event_id_value = event_id.to_string();
            let post_id_value = post_id.to_string();
            let dependency_event_id_value = dependency_event_id.to_string();
            let dependency_post_id_value = dependency_post_id.to_string();
            let span_ids: std::collections::HashSet<_> = delivery_spans
                .iter()
                .map(|span| format!("{:016x}", span.span_context.span_id()))
                .collect();
            anyhow::ensure!(
                span_ids.len() == delivery_spans.len(),
                "each delivery attempt gets a fresh child span"
            );
            let mut saw_target_failure = false;
            let mut saw_published = false;
            let mut saw_dependency_error = false;
            for span in delivery_spans {
                anyhow::ensure!(
                    format!("{:032x}", span.span_context.trace_id()) == trace_id,
                    "delivery keeps the originating trace"
                );
                anyhow::ensure!(
                    format!("{:016x}", span.parent_span_id) == parent_span_id,
                    "delivery is a child of the persisted parent"
                );
                anyhow::ensure!(span.events.is_empty());
                anyhow::ensure!(span.links.is_empty());
                anyhow::ensure!(span.span_context.trace_state().header().is_empty());
                let attributes = format!("{:?}", span.attributes);
                saw_target_failure |= attributes.contains("target_failure");
                saw_published |= attributes.contains("published");
                saw_dependency_error |= attributes.contains("dependency_error");
                for private_value in [
                    PRIVATE_PAYLOAD,
                    PRIVATE_ERROR,
                    event_id_value.as_str(),
                    post_id_value.as_str(),
                    dependency_event_id_value.as_str(),
                    dependency_post_id_value.as_str(),
                    "POST_LIKED",
                ] {
                    anyhow::ensure!(
                        !attributes.contains(private_value),
                        "delivery span excludes event data"
                    );
                }
            }
            anyhow::ensure!(saw_target_failure, "failed target has a bounded outcome");
            anyhow::ensure!(saw_published, "recovered event has a bounded outcome");
            anyhow::ensure!(
                saw_dependency_error,
                "outbox dependency error has a bounded outcome"
            );
            Ok::<_, anyhow::Error>(())
        })
    });
    let shutdown_result = provider.shutdown_with_timeout(std::time::Duration::from_secs(3));
    test_result.expect("verify Post outbox trace outcomes");
    shutdown_result.expect("shut down Post outbox trace provider");
}
