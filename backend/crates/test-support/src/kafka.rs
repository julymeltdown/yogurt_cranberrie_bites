use crate::container::{
    is_host_port_conflict, is_transient_container_start_failure, reserve_loopback_host_port,
    CONTAINER_START_ATTEMPTS, CONTAINER_START_BACKOFF, HOST_PORT_BIND_ATTEMPTS,
};
use anyhow::Result;
use testcontainers::{core::IntoContainerPort, runners::AsyncRunner, ContainerAsync, ImageExt};
use testcontainers_modules::kafka::apache::Kafka;
use tokio::sync::Mutex;

pub type KafkaContainer = ContainerAsync<Kafka>;

/// The port the broker listens on inside the container.
pub const KAFKA_PORT: u16 = 9092;

static KAFKA_START_LOCK: Mutex<()> = Mutex::const_new(());

/// Start a single-node Kafka broker.
///
/// The module configures the broker to advertise the mapped loopback address on
/// its PLAINTEXT listener, so a test connects to the address this helper returns
/// rather than to the container-internal hostname.
pub async fn start_kafka() -> Result<KafkaContainer> {
    // Docker may choose a different host port when a stopped container is
    // restarted. Pin one available port for the container's full lifecycle so
    // producers created before the restart keep a valid bootstrap address.
    let _start_guard = KAFKA_START_LOCK.lock().await;
    let mut last_port_conflict = None;
    let mut last_start_failure = None;
    // Kafka is the heaviest fixture in the suite, so it is the first container a
    // loaded host kills outright or fails to create. Both shapes are the same
    // transient infrastructure class the other fixtures already retry.
    let attempts = HOST_PORT_BIND_ATTEMPTS.max(CONTAINER_START_ATTEMPTS);
    for attempt in 0..attempts {
        if attempt > 0 {
            tokio::time::sleep(CONTAINER_START_BACKOFF).await;
        }
        let host_port = reserve_loopback_host_port()?;
        match Kafka::default()
            .with_mapped_port(host_port, KAFKA_PORT.tcp())
            .start()
            .await
        {
            Ok(container) => return Ok(container),
            Err(start_error) => {
                let error: anyhow::Error = start_error.into();
                if is_host_port_conflict(&error) {
                    last_port_conflict = Some(error);
                    continue;
                }
                if is_transient_container_start_failure(&error) {
                    last_start_failure = Some(error);
                    continue;
                }
                return Err(error);
            }
        }
    }

    Err(last_start_failure
        .or(last_port_conflict)
        .unwrap_or_else(|| {
            anyhow::anyhow!("failed to start Kafka testcontainer after {attempts} attempts")
        }))
}

/// Restart a stopped Kafka testcontainer while tolerating transient host-port
/// bind races from Docker or another test process.
///
/// The existing container keeps its mapped port so clients configured before
/// the outage remain valid. Unlike [`start_kafka`], a restart cannot select a
/// different port; retry only the narrow host-port-conflict error and fail
/// immediately for every other Docker/startup error.
pub async fn start_stopped_kafka(container: &KafkaContainer) -> Result<()> {
    let mut last_port_conflict = None;
    for attempt in 0..HOST_PORT_BIND_ATTEMPTS {
        match container.start().await {
            Ok(()) => return Ok(()),
            Err(start_error) => {
                let error: anyhow::Error = start_error.into();
                if !is_host_port_conflict(&error) {
                    return Err(error);
                }

                last_port_conflict = Some(error);
                if attempt + 1 < HOST_PORT_BIND_ATTEMPTS {
                    let backoff_ms = 100 * (attempt as u64 + 1);
                    tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
                }
            }
        }
    }

    Err(last_port_conflict.unwrap_or_else(|| {
        anyhow::anyhow!(
            "failed to restart Kafka testcontainer after {HOST_PORT_BIND_ATTEMPTS} host-port attempts"
        )
    }))
}

/// The `host:port` a client should bootstrap against.
pub async fn kafka_bootstrap(container: &KafkaContainer) -> Result<String> {
    let host = container.get_host().await?;
    let port = container.get_host_port_ipv4(KAFKA_PORT).await?;
    Ok(format!("{host}:{port}"))
}

/// Wait until the broker answers a real metadata request.
///
/// The container reports ready on a broker log line, but the listener can still
/// refuse the first connection. Polling the request the test actually needs is
/// stronger than trusting a proxy for it.
pub async fn await_kafka_ready(bootstrap: &str, timeout: std::time::Duration) -> Result<()> {
    let deadline = std::time::Instant::now() + timeout;
    let mut last_error = None;
    while std::time::Instant::now() < deadline {
        let attempt = rskafka::client::ClientBuilder::new(vec![bootstrap.to_owned()])
            .build()
            .await;
        match attempt {
            Ok(client) => match client.list_topics().await {
                Ok(_) => return Ok(()),
                Err(error) => last_error = Some(error.to_string()),
            },
            Err(error) => last_error = Some(error.to_string()),
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    let detail = last_error.unwrap_or_else(|| "no error reported".to_owned());
    anyhow::bail!("kafka at {bootstrap} was not ready within {timeout:?}: {detail}")
}
