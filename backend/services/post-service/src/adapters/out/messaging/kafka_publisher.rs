//! External event publication.
//!
//! Two publishers implement [`ExternalEventPublisherPort`]:
//!
//! * [`KafkaEventPublisher`] writes an [`EventEnvelope`] to a Kafka topic.
//! * [`DisabledEventPublisher`] publishes nothing and says so.
//!
//! The distinction is the point. A publisher that reports success while sending
//! nothing makes a switched-off deployment indistinguishable from a working one.

use crate::application::delivery_effect::DeliveryEffectFailure;
use crate::application::ports::ExternalEventPublisherPort;
use crate::domain::post::StoredEvent;
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use rskafka::client::error::{Error as KafkaClientError, ProtocolError};
use rskafka::client::partition::{Compression, PartitionClient, UnknownTopicHandling};
use rskafka::client::Client as KafkaClient;
use rskafka::record::Record;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use uuid::Uuid;

async fn existing_topic_partition_count(
    client: &KafkaClient,
    topic_name: &str,
) -> Result<Option<usize>> {
    Ok(client
        .list_topics()
        .await?
        .into_iter()
        .find(|topic| topic.name == topic_name)
        .map(|topic| topic.partitions.len()))
}

fn ensure_topic_partition_count(
    topic_name: &str,
    existing_count: usize,
    configured_count: i32,
) -> Result<()> {
    anyhow::ensure!(
        existing_count == configured_count as usize,
        "Kafka topic {topic_name} has {existing_count} partitions, but POST_KAFKA_PARTITION_COUNT configures {configured_count}"
    );
    Ok(())
}

/// External publication that is switched off, stated as such.
///
/// This type previously marked the outbox row published without publishing
/// anything, which made an unpublishing deployment look identical to a working
/// one from the outside: rows advanced, the queue drained, and no event ever
/// left the process. It also marked the whole event published from inside a
/// single delivery target, so a successful external step would declare every
/// other target complete.
///
/// The decision that an event is fully delivered belongs to the forwarder, which
/// is the only component that knows the state of all targets. This adapter
/// therefore reports its own outcome and nothing more. Leaving the target
/// outstanding is the honest result of not publishing, and it keeps the row
/// retryable so events are not silently discarded while the mode is off.
#[derive(Clone, Default)]
pub struct DisabledEventPublisher;

impl DisabledEventPublisher {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ExternalEventPublisherPort for DisabledEventPublisher {
    async fn publish(&self, event: &StoredEvent) -> Result<()> {
        // Logged at error level on every attempt rather than once at startup,
        // because the failure to publish is ongoing and must remain visible
        // while external delivery is off.
        tracing::error!(
            event_id = %event.event_id,
            event_type = %event.event_type,
            "external event publication is disabled; the event was not delivered to any broker"
        );
        anyhow::bail!(
            "external event publication is disabled; configure a publisher to deliver events"
        )
    }

    fn is_enabled(&self) -> bool {
        false
    }
}

/// Version of new envelopes written to the external event stream.
///
/// A consumer has to know which fields it may rely on, so the version travels
/// with every record rather than being implied by the topic name. A payload
/// shape change without a version bump is the case this field exists to prevent.
pub const EVENT_ENVELOPE_SCHEMA_VERSION: u32 = 2;
/// Legacy events created before the database assigned aggregate versions.
pub const LEGACY_EVENT_ENVELOPE_SCHEMA_VERSION: u32 = 1;

/// Header carrying the event id outside the body.
///
/// Consumers deduplicate by event id. A header lets a consumer make that
/// decision before it parses the body.
pub const EVENT_ID_HEADER: &str = "event-id";
/// Header carrying the event type, for routing and for inspection.
pub const EVENT_TYPE_HEADER: &str = "event-type";
/// Header carrying [`EVENT_ENVELOPE_SCHEMA_VERSION`].
pub const SCHEMA_VERSION_HEADER: &str = "schema-version";
/// Optional W3C context header for tracing asynchronous delivery.
pub const TRACEPARENT_HEADER: &str = "traceparent";

/// The record written to the external event stream.
///
/// An event id is the unit of deduplication, so it has to be inside the record
/// rather than inferred from the topic offset. Offsets change on replay, and a
/// consumer that deduplicated on offsets would treat a replay as new work.
///
/// New envelopes carry the outbox's database-assigned aggregate version so
/// consumers can detect stale events and gaps. Legacy rows remain unsequenced
/// and use schema version 1; assigning them a version at publish time would
/// invent ordering evidence. All events still use the aggregate as their Kafka
/// key so one aggregate stays on one partition.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EventEnvelope {
    pub event_id: Uuid,
    pub schema_version: u32,
    pub event_type: String,
    pub aggregate_id: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aggregate_version: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<String>,
    pub occurred_at: chrono::DateTime<chrono::Utc>,
    pub payload: serde_json::Value,
}

impl EventEnvelope {
    /// Convert a stored outbox event into the wire envelope.
    ///
    /// The stored payload is JSON by construction, so a payload that does not
    /// parse is a defect rather than a formatting choice. Sending it as an
    /// opaque string would hide that defect from every consumer.
    pub fn from_stored(event: &StoredEvent) -> Result<Self> {
        let payload = serde_json::from_str(&event.payload).with_context(|| {
            format!("outbox event {} payload is not valid JSON", event.event_id)
        })?;
        Ok(Self {
            event_id: event.event_id,
            schema_version: if event.aggregate_version.is_some() {
                EVENT_ENVELOPE_SCHEMA_VERSION
            } else {
                LEGACY_EVENT_ENVELOPE_SCHEMA_VERSION
            },
            event_type: event.event_type.clone(),
            aggregate_id: event.aggregate_id,
            aggregate_version: event.aggregate_version,
            actor_id: event.actor_id,
            attributes: event.attributes.clone(),
            occurred_at: event.occurred_at,
            payload,
        })
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(self)?)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        Ok(serde_json::from_slice(bytes)?)
    }

    /// Headers a consumer can read without parsing the body.
    pub fn headers(&self) -> BTreeMap<String, Vec<u8>> {
        let mut headers = BTreeMap::from([
            (
                EVENT_ID_HEADER.to_owned(),
                self.event_id.to_string().into_bytes(),
            ),
            (
                EVENT_TYPE_HEADER.to_owned(),
                self.event_type.clone().into_bytes(),
            ),
            (
                SCHEMA_VERSION_HEADER.to_owned(),
                self.schema_version.to_string().into_bytes(),
            ),
        ]);
        if let Some(context) = platform::trace_context::current() {
            headers.insert(
                TRACEPARENT_HEADER.to_owned(),
                context.traceparent().into_bytes(),
            );
        }
        headers
    }
}

/// Map an aggregate to a partition with a written-out stable hash.
///
/// Every event of one aggregate must reach one partition for the broker to
/// preserve their order. The choice is therefore hash-based rather than
/// round-robin, and the hash is spelled out here instead of delegating to a
/// standard-library hasher whose output is free to change between releases: a
/// changed mapping would silently reorder one aggregate across two partitions.
/// FNV-1a, chosen only because it is trivial to reimplement and verify.
pub fn stable_partition(aggregate_id: Uuid, partition_count: i32) -> i32 {
    debug_assert!(partition_count > 0, "partition count must be positive");
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in aggregate_id.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    (hash % partition_count as u64) as i32
}

/// A Kafka producer that publishes outbox events to one topic.
///
/// The producer holds the only connection state; the forwarder still owns the
/// decision that an event is delivered. This adapter reports broker success and
/// nothing else, so a crash between the broker acknowledgement and the
/// checkpoint write is visible as outstanding work rather than as silent loss.
///
/// Delivery to a broker is at-least-once. The acknowledgement is required before
/// this method returns. An error may mean the acknowledgement was lost after
/// the broker accepted the record. A duplicate remains possible when
/// the process dies after the acknowledgement and before the checkpoint, which
/// is why consumers deduplicate by event id.
#[derive(Clone)]
pub struct KafkaEventPublisher {
    inner: Arc<KafkaPublisherInner>,
}

struct KafkaPublisherInner {
    client: KafkaClient,
    topic: String,
    partition_count: i32,
    publish_timeout: Duration,
    /// One leader connection per partition, created on first use.
    ///
    /// Resolving a partition client performs a metadata lookup and a leader
    /// handshake, so doing it per event would add a round trip to every publish.
    partition_clients: Mutex<HashMap<i32, Arc<PartitionClient>>>,
}

impl KafkaEventPublisher {
    /// Connect and, when asked, create the topic.
    ///
    /// Topic creation is opt-in because a producer that creates topics on its
    /// own can mask a deployment mistake: a typo in the topic name would look
    /// like a healthy deployment writing to an empty, automatically created
    /// topic. Deployments that own their topic layout leave this off.
    /// Connect and, when asked, create the topic.
    ///
    /// `connect_timeout` bounds the initial metadata refresh, which is where the
    /// client otherwise retries an unreachable broker without limit. A service
    /// that could not reach its broker would hang before it served anything, so
    /// the failure is turned into a startup error instead.
    pub async fn connect(
        brokers: Vec<String>,
        topic: String,
        partition_count: i32,
        connect_timeout: Duration,
        publish_timeout: Duration,
        create_topic: bool,
    ) -> Result<Self> {
        if brokers.is_empty() {
            bail!("at least one Kafka bootstrap broker is required");
        }
        if partition_count < 1 {
            bail!("Kafka partition count must be at least 1");
        }
        // The client retries metadata discovery forever by default, so the
        // attempt is bounded from the outside. Failing here reports a broker
        // that is unreachable at startup, which is a deployment problem, rather
        // than blocking the process before it can serve an unhealthy status.
        let client = tokio::time::timeout(
            connect_timeout,
            rskafka::client::ClientBuilder::new(brokers.clone())
                .client_id("post-service")
                .build(),
        )
        .await
        .map_err(|_| {
            anyhow::anyhow!(
                "connecting to Kafka brokers {brokers:?} timed out after {connect_timeout:?}"
            )
        })?
        .with_context(|| format!("connecting to Kafka brokers {brokers:?}"))?;

        if create_topic {
            match existing_topic_partition_count(&client, &topic).await? {
                Some(existing_count) => {
                    ensure_topic_partition_count(&topic, existing_count, partition_count)?;
                }
                None => {
                    match client
                        .controller_client()?
                        .create_topic(topic.clone(), partition_count, 1, 5_000)
                        .await
                    {
                        Ok(()) => {}
                        Err(KafkaClientError::ServerError {
                            protocol_error: ProtocolError::TopicAlreadyExists,
                            ..
                        }) => {
                            // Another service instance may have created the topic
                            // after the metadata check above. Validate its layout
                            // instead of treating the startup race as a failure.
                            let existing_count = existing_topic_partition_count(&client, &topic)
                                .await?
                                .context(
                                    "Kafka reported an existing topic but metadata omitted it",
                                )?;
                            ensure_topic_partition_count(&topic, existing_count, partition_count)?;
                        }
                        Err(error) => {
                            return Err(error)
                                .with_context(|| format!("creating Kafka topic {topic}"));
                        }
                    }
                }
            }
        }

        Ok(Self {
            inner: Arc::new(KafkaPublisherInner {
                client,
                topic,
                partition_count,
                publish_timeout,
                partition_clients: Mutex::new(HashMap::new()),
            }),
        })
    }

    pub fn topic(&self) -> &str {
        &self.inner.topic
    }

    /// Publish one envelope and return the offset the broker assigned.
    pub async fn publish_envelope(&self, envelope: &EventEnvelope) -> Result<i64> {
        // The whole path is bounded, not only the produce request. Resolving a
        // partition performs leader discovery, which retries an unknown topic or
        // an unreachable leader without limit by design. A publisher that can
        // wait forever holds the outbox row and its lock for as long as the
        // broker stays unreachable, so the deadline covers both steps.
        bounded_publication(self.inner.publish_timeout, self.produce(envelope))
            .await
            .with_context(|| {
                format!(
                    "publishing event {} to topic {}",
                    envelope.event_id, self.inner.topic
                )
            })
    }

    async fn produce(&self, envelope: &EventEnvelope) -> Result<i64> {
        let partition = stable_partition(envelope.aggregate_id, self.inner.partition_count);
        let client = self.inner.partition_client(partition).await?;
        let record = Record {
            // Keyed by aggregate, so every event of one aggregate shares a
            // partition and therefore keeps its append order.
            key: Some(envelope.aggregate_id.as_bytes().to_vec()),
            value: Some(envelope.encode()?),
            headers: envelope.headers(),
            timestamp: envelope.occurred_at,
        };

        let offsets = client
            .produce(vec![record], Compression::Gzip)
            .await
            .with_context(|| {
                format!(
                    "publishing event {} to topic {}",
                    envelope.event_id, self.inner.topic
                )
            })?;

        match offsets.as_slice() {
            [offset] => Ok(*offset),
            other => bail!(
                "broker acknowledged {} offsets for one record on topic {}; an acknowledgement was expected for exactly one",
                other.len(),
                self.inner.topic
            ),
        }
    }
}

async fn bounded_publication(
    budget: Duration,
    effect: impl std::future::Future<Output = Result<i64>>,
) -> Result<i64> {
    tokio::time::timeout(budget, effect)
        .await
        .map_err(|_| {
            DeliveryEffectFailure::uncertain(anyhow::anyhow!("Kafka publication timed out"))
        })?
        .map_err(DeliveryEffectFailure::uncertain)
}

impl KafkaPublisherInner {
    async fn partition_client(&self, partition: i32) -> Result<Arc<PartitionClient>> {
        let mut clients = self.partition_clients.lock().await;
        if let Some(client) = clients.get(&partition) {
            return Ok(Arc::clone(client));
        }
        // A topic that does not exist yet is retried rather than reported as a
        // permanent error, because a broker may still be creating it. The
        // caller's timeout is what keeps that retry finite.
        let client = Arc::new(
            self.client
                .partition_client(self.topic.clone(), partition, UnknownTopicHandling::Retry)
                .await
                .with_context(|| {
                    format!(
                        "resolving leader for topic {} partition {partition}",
                        self.topic
                    )
                })?,
        );
        clients.insert(partition, Arc::clone(&client));
        Ok(client)
    }
}

#[async_trait]
impl ExternalEventPublisherPort for KafkaEventPublisher {
    async fn publish(&self, event: &StoredEvent) -> Result<()> {
        let envelope =
            EventEnvelope::from_stored(event).map_err(DeliveryEffectFailure::confirmed)?;
        self.publish_envelope(&envelope).await?;
        Ok(())
    }

    fn is_enabled(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod delivery_result_tests {
    use super::*;

    #[tokio::test]
    async fn inner_publication_timeout_preserves_uncertainty() {
        let error = bounded_publication(Duration::from_millis(5), async {
            std::future::pending::<()>().await;
            Ok(1)
        })
        .await
        .unwrap_err();
        assert_eq!(
            DeliveryEffectFailure::classify(&error),
            DeliveryEffectFailure::Uncertain
        );
    }

    #[tokio::test]
    async fn publication_transport_error_preserves_uncertainty() {
        let error = bounded_publication(Duration::from_secs(1), async {
            Err(anyhow::anyhow!("connection closed without acknowledgement"))
        })
        .await
        .unwrap_err();
        assert_eq!(
            DeliveryEffectFailure::classify(&error),
            DeliveryEffectFailure::Uncertain
        );
    }
}
