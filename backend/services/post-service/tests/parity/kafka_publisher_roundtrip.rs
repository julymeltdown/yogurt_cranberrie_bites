//! Real-broker proof for external event publication.
//!
//! The unit tests cover the envelope and the partition mapping without a broker.
//! This test is the other half: it starts a real Kafka fixture, publishes through
//! the adapter the service selects in `kafka` mode, and reads the record back
//! with an independent consumer. A test that only published would not show that
//! the bytes on the wire are the envelope, so the assertion is on the consumed
//! record rather than on the producer returning success.

use post_service::adapters::out::messaging::kafka_publisher::{
    EventEnvelope, KafkaEventPublisher, EVENT_ID_HEADER, EVENT_TYPE_HEADER, SCHEMA_VERSION_HEADER,
};
use post_service::application::ports::ExternalEventPublisherPort;
use post_service::domain::post::StoredEvent;
use rskafka::client::consumer::{StartOffset, StreamConsumerBuilder};
use rskafka::client::partition::UnknownTopicHandling;
use rskafka::client::ClientBuilder;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use tokio_stream::StreamExt;
use uuid::Uuid;

const TOPIC: &str = "post-events-parity";

fn stored_event(event_id: Uuid, aggregate_id: Uuid) -> StoredEvent {
    let occurred_at = chrono::DateTime::from_timestamp(1_700_000_200, 0).expect("timestamp");
    StoredEvent {
        event_id,
        event_type: "POST_CREATED".to_owned(),
        aggregate_id,
        aggregate_version: Some(1),
        origin_traceparent: None,
        actor_id: Some(Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").expect("uuid")),
        attributes: vec!["PUBLIC".to_owned()],
        payload: serde_json::json!({
            "postId": aggregate_id,
            "authorId": "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"
        })
        .to_string(),
        occurred_at,
        created_at: occurred_at,
        published: false,
        published_at: None,
        replay_resolution_id: None,
    }
}

/// Read one record from the topic, starting at the earliest offset.
async fn consume_one(bootstrap: &str, expected_key: Uuid) -> (BTreeMap<String, Vec<u8>>, Vec<u8>) {
    let client = ClientBuilder::new(vec![bootstrap.to_owned()])
        .build()
        .await
        .expect("consumer client");
    // The producer maps an aggregate to a partition with a stable hash. The test
    // resolves the partition the same way through the public helper rather than
    // assuming partition 0, because assuming it would pass while the mapping
    // silently changed.
    let partition =
        post_service::adapters::out::messaging::kafka_publisher::stable_partition(expected_key, 1);
    let partition_client = Arc::new(
        client
            .partition_client(TOPIC.to_owned(), partition, UnknownTopicHandling::Retry)
            .await
            .expect("partition client"),
    );
    let mut stream = StreamConsumerBuilder::new(partition_client, StartOffset::Earliest)
        .with_max_wait_ms(500)
        .build();

    // The fixture verifies listener and metadata readiness, so the first fetch can still
    // race the listener. The stream refetches internally until records arrive,
    // so one bounded await is enough; the bound is what turns a broken producer
    // into a failure rather than a hung test.
    match tokio::time::timeout(Duration::from_secs(30), stream.next()).await {
        Ok(Some(Ok((record, _watermark)))) => (
            record.record.headers.clone(),
            record.record.value.clone().expect("record value"),
        ),
        Ok(Some(Err(error))) => panic!("consume failed: {error}"),
        Ok(None) => panic!("consumer stream ended before a record arrived"),
        Err(_) => panic!("no record was produced within the timeout"),
    }
}

#[tokio::test]
async fn kafka_publisher_writes_an_envelope_that_an_independent_consumer_reads_back() {
    let container = crate::support::native_kafka::KafkaFixture::new()
        .await
        .expect("Kafka fixture");
    let bootstrap = container.bootstrap().await.expect("bootstrap address");
    container
        .wait_ready(Duration::from_secs(60))
        .await
        .expect("broker ready");

    let publisher = KafkaEventPublisher::connect(
        vec![bootstrap.clone()],
        TOPIC.to_owned(),
        // One partition keeps the read simple; the mapping itself is covered by
        // the unit tests.
        1,
        Duration::from_secs(10),
        Duration::from_secs(10),
        // Creating the topic is what a deployment owning its topic layout would
        // do out of band. The test asks the publisher to create it so the test
        // controls when the topic exists.
        true,
    )
    .await
    .expect("publisher");
    assert!(publisher.is_enabled());
    assert_eq!(publisher.topic(), TOPIC);

    // A restarted service can use the same create-topic configuration; topic
    // provisioning must treat an already-existing topic as a healthy no-op.
    let restarted_publisher = KafkaEventPublisher::connect(
        vec![bootstrap.clone()],
        TOPIC.to_owned(),
        1,
        Duration::from_secs(10),
        Duration::from_secs(10),
        true,
    )
    .await
    .expect("publisher restart must accept its existing topic");
    assert!(restarted_publisher.is_enabled());

    let wrong_layout = KafkaEventPublisher::connect(
        vec![bootstrap.clone()],
        TOPIC.to_owned(),
        2,
        Duration::from_secs(10),
        Duration::from_secs(10),
        true,
    )
    .await;
    let layout_error = match wrong_layout {
        Ok(_) => panic!("publisher accepted an existing topic with the wrong partition count"),
        Err(error) => error,
    };
    assert!(layout_error
        .to_string()
        .contains("POST_KAFKA_PARTITION_COUNT"));

    let event = stored_event(
        Uuid::parse_str("33333333-3333-3333-3333-333333333301").expect("uuid"),
        Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").expect("uuid"),
    );
    ExternalEventPublisherPort::publish(&publisher, &event)
        .await
        .expect("publish");

    let (headers, value) = consume_one(&bootstrap, event.aggregate_id).await;
    let envelope = EventEnvelope::decode(&value).expect("envelope");

    // The record that left the process is the envelope, not the raw payload.
    assert_eq!(envelope.event_id, event.event_id);
    assert_eq!(envelope.aggregate_id, event.aggregate_id);
    assert_eq!(envelope.event_type, "POST_CREATED");
    assert_eq!(envelope.occurred_at, event.occurred_at);
    assert_eq!(envelope.schema_version, 2);
    assert_eq!(envelope.aggregate_version, Some(1));
    assert_eq!(
        envelope.payload["postId"],
        serde_json::json!(event.aggregate_id)
    );

    // The headers a consumer deduplicates on travel outside the body.
    let decode = |name: &str| {
        headers
            .get(name)
            .map(|value| String::from_utf8(value.clone()).expect("utf8 header"))
    };
    assert_eq!(decode(EVENT_ID_HEADER), Some(event.event_id.to_string()));
    assert_eq!(decode(EVENT_TYPE_HEADER), Some("POST_CREATED".to_owned()));
    assert_eq!(decode(SCHEMA_VERSION_HEADER), Some("2".to_owned()));
}

#[tokio::test]
async fn kafka_publisher_reports_failure_when_no_broker_answers() {
    // A publisher that returned success without a broker would be the exact
    // defect the disabled mode exists to remove, so a connection failure has to
    // surface as an error. The reserved port below accepts the bootstrap attempt
    // and then never completes a produce request.
    let publisher = KafkaEventPublisher::connect(
        vec!["127.0.0.1:1".to_owned()],
        TOPIC.to_owned(),
        1,
        Duration::from_millis(500),
        Duration::from_millis(500),
        false,
    )
    .await;

    // The client refreshes metadata at build time, so an unreachable broker
    // either fails the construction or the first publish. Both are honest
    // failures; silently succeeding is not an outcome this test allows.
    match publisher {
        Err(error) => {
            assert!(!error.to_string().is_empty());
        }
        Ok(publisher) => {
            let event = stored_event(
                Uuid::parse_str("33333333-3333-3333-3333-333333333302").expect("uuid"),
                Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").expect("uuid"),
            );
            let error = tokio::time::timeout(
                Duration::from_secs(20),
                ExternalEventPublisherPort::publish(&publisher, &event),
            )
            .await
            .expect("publish must not hang past the timeout")
            .expect_err("an unreachable broker must not report success");
            assert!(!error.to_string().is_empty());
        }
    }
}
