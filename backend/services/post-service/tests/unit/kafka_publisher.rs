//! Tests for the Kafka publisher that do not need a broker.
//!
//! The wire format and the aggregate-to-partition mapping are pure functions, so
//! they are checked here directly. The broker round trip lives in the parity test,
//! which needs a container and therefore runs with the rest of the containers.

use crate::support::{instant, stored_event, uuid};
use post_service::adapters::out::messaging::kafka_publisher::{
    stable_partition, EventEnvelope, EVENT_ENVELOPE_SCHEMA_VERSION, EVENT_ID_HEADER,
    EVENT_TYPE_HEADER, LEGACY_EVENT_ENVELOPE_SCHEMA_VERSION, SCHEMA_VERSION_HEADER,
    TRACEPARENT_HEADER,
};
use uuid::Uuid;

fn envelope() -> EventEnvelope {
    EventEnvelope::from_stored(&stored_event(
        uuid("33333333-3333-3333-3333-333333333301"),
        "POST_CREATED",
        serde_json::json!({
            "postId": "44444444-4444-4444-4444-444444444401",
            "authorId": "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"
        })
        .to_string(),
    ))
    .expect("envelope")
}

#[test]
fn envelope_round_trips_through_the_wire_format() {
    let envelope = envelope();
    let encoded = envelope.encode().expect("encode");
    let decoded = EventEnvelope::decode(&encoded).expect("decode");

    assert_eq!(decoded, envelope);
    assert_eq!(
        decoded.event_id,
        uuid("33333333-3333-3333-3333-333333333301")
    );
    assert_eq!(decoded.event_type, "POST_CREATED");
    assert_eq!(
        decoded.aggregate_id,
        uuid("44444444-4444-4444-4444-444444444401")
    );
    assert_eq!(
        decoded.payload["postId"],
        serde_json::json!("44444444-4444-4444-4444-444444444401")
    );
}

#[test]
fn envelope_stamps_the_schema_version_it_was_built_with() {
    let decoded = EventEnvelope::decode(&envelope().encode().expect("encode")).expect("decode");

    // Historical fixtures represent rows which predate aggregate sequencing.
    assert_eq!(decoded.schema_version, LEGACY_EVENT_ENVELOPE_SCHEMA_VERSION);
    assert_eq!(decoded.aggregate_version, None);
}

#[test]
fn sequenced_envelope_uses_v2_and_carries_the_database_version() {
    let mut event = stored_event(
        uuid("33333333-3333-3333-3333-333333333303"),
        "POST_CREATED",
        serde_json::json!({"postId": "44444444-4444-4444-4444-444444444401"}).to_string(),
    );
    event.aggregate_version = Some(7);

    let envelope = EventEnvelope::from_stored(&event).expect("v2 envelope");
    let value = serde_json::to_value(&envelope).expect("serialize envelope");

    assert_eq!(envelope.schema_version, EVENT_ENVELOPE_SCHEMA_VERSION);
    assert_eq!(envelope.aggregate_version, Some(7));
    assert_eq!(value["aggregate_version"], 7);
}

#[test]
fn envelope_headers_carry_the_deduplication_identity() {
    let envelope = envelope();
    let headers = envelope.headers();

    // A consumer deduplicates by event id, so the id has to be readable without
    // parsing the body.
    assert_eq!(
        headers
            .get(EVENT_ID_HEADER)
            .map(|value| String::from_utf8(value.clone())),
        Some(Ok(envelope.event_id.to_string()))
    );
    assert_eq!(
        headers
            .get(EVENT_TYPE_HEADER)
            .map(|value| String::from_utf8(value.clone())),
        Some(Ok("POST_CREATED".to_owned()))
    );
    assert_eq!(
        headers
            .get(SCHEMA_VERSION_HEADER)
            .map(|value| String::from_utf8(value.clone())),
        Some(Ok(envelope.schema_version.to_string()))
    );
    assert!(!headers.contains_key(TRACEPARENT_HEADER));
}

#[tokio::test]
async fn async_trace_context_is_a_kafka_header_not_event_body_or_receipt_data() {
    const PARENT: &str = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
    let mut event = stored_event(
        uuid("33333333-3333-3333-3333-333333333305"),
        "POST_CREATED",
        serde_json::json!({"postId": "44444444-4444-4444-4444-444444444401"}).to_string(),
    );
    event.origin_traceparent = Some(PARENT.to_owned());
    let serialized_event = serde_json::to_value(&event).expect("serialize stored event");
    assert!(serialized_event.get("origin_traceparent").is_none());

    let envelope = EventEnvelope::from_stored(&event).expect("event envelope");
    let serialized_envelope = serde_json::to_value(&envelope).expect("serialize envelope");
    assert!(serialized_envelope.get("traceparent").is_none());

    let context = platform::trace_context::TraceContext::child_from_traceparent(PARENT)
        .expect("valid asynchronous parent");
    let expected_traceparent = context.traceparent();
    let headers = platform::trace_context::scope(context, async { envelope.headers() }).await;

    assert_eq!(
        headers
            .get(TRACEPARENT_HEADER)
            .map(|value| String::from_utf8(value.clone())),
        Some(Ok(expected_traceparent))
    );
}

#[test]
fn envelope_rejects_a_payload_that_is_not_json() {
    let mut event = stored_event(
        uuid("33333333-3333-3333-3333-333333333302"),
        "POST_CREATED",
        String::new(),
    );
    event.payload = "not json".to_owned();

    // An unparsable payload is a defect in the producer, not a transport choice.
    // Sending it as an opaque string would hide the defect from every consumer.
    let error = EventEnvelope::from_stored(&event).expect_err("payload must be validated");
    assert!(error.to_string().contains("not valid JSON"));
}

#[test]
fn every_event_of_one_aggregate_maps_to_one_partition() {
    let aggregate_id = uuid("22222222-2222-2222-2222-222222222222");
    let expected = stable_partition(aggregate_id, 6);

    for _ in 0..64 {
        assert_eq!(stable_partition(aggregate_id, 6), expected);
    }
    // One partition is the degenerate case: every aggregate has to land there.
    assert_eq!(stable_partition(aggregate_id, 1), 0);
}

#[test]
fn partition_mapping_is_pinned_so_a_hasher_change_cannot_reorder_an_aggregate() {
    // The mapping is written out rather than delegated to a standard-library
    // hasher, whose output may change between releases. A silent change would
    // scatter one aggregate across two partitions and lose its order. These
    // values are therefore recorded as literals: a change breaks this test on
    // purpose, so it is reviewed rather than discovered in production.
    let cases = [
        ("11111111-1111-1111-1111-111111111111", 3),
        ("22222222-2222-2222-2222-222222222222", 3),
        ("33333333-3333-3333-3333-333333333333", 1),
        ("deadbeef-dead-beef-dead-beefdeadbeef", 3),
    ];

    for (value, expected) in cases {
        assert_eq!(
            stable_partition(Uuid::parse_str(value).expect("uuid"), 6),
            expected,
            "partition for aggregate {value}"
        );
    }
}

#[test]
fn partition_mapping_spreads_aggregates_across_partitions() {
    // A mapping that sent every aggregate to one partition would preserve order
    // but serialize all traffic. This checks only that the mapping spreads, not
    // that it is uniform, so it stays stable as the literal cases evolve.
    let mut partitions = std::collections::BTreeSet::new();
    for index in 0..256 {
        let aggregate_id = Uuid::from_u128(index as u128);
        partitions.insert(stable_partition(aggregate_id, 6));
    }

    assert_eq!(partitions.len(), 6, "partitions used: {partitions:?}");
}

#[test]
fn envelope_occurred_at_is_the_event_time_not_the_publish_time() {
    let envelope = envelope();

    assert_eq!(envelope.occurred_at, instant(1_700_000_200));
}
