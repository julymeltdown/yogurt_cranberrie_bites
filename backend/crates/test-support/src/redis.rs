use crate::container::{
    is_host_port_conflict, is_transient_container_start_failure, reserve_loopback_host_port,
    CONTAINER_START_ATTEMPTS, CONTAINER_START_BACKOFF, HOST_PORT_BIND_ATTEMPTS,
};
use anyhow::Result;
use testcontainers::{core::IntoContainerPort, runners::AsyncRunner, ContainerAsync, ImageExt};
use testcontainers_modules::redis::{Redis, REDIS_PORT};
use tokio::time::{sleep, Duration, Instant};

pub type RedisContainer = ContainerAsync<Redis>;

static REDIS_START_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub async fn start_redis() -> Result<RedisContainer> {
    // Dynamic host ports are reassigned by Docker when a stopped testcontainer
    // is restarted. Pin a host port for this container's full lifecycle so
    // clients configured before the restart keep the same URL. The OS port
    // probe cannot reserve that port for Docker, so retry only the resulting
    // address-in-use race with a newly selected port.
    let _start_guard = REDIS_START_LOCK.lock().await;
    let mut last_port_conflict = None;
    let mut last_start_timeout = None;
    let attempts = HOST_PORT_BIND_ATTEMPTS.max(CONTAINER_START_ATTEMPTS);
    for attempt in 0..attempts {
        if attempt > 0 {
            tokio::time::sleep(CONTAINER_START_BACKOFF).await;
        }
        let host_port = reserve_loopback_host_port()?;

        // The module's default tag is redis:5.0, which cannot answer
        // `CLIENT PAUSE ... WRITE` (Redis 6.2+). The parity and release
        // stacks run `redis:7`, so the fixture follows the deployed version:
        // a test that needs pause/failover semantics would otherwise pass or
        // fail for a reason the product never meets.
        let container = match Redis::default()
            .with_tag("7")
            .with_mapped_port(host_port, REDIS_PORT.tcp())
            .start()
            .await
        {
            Ok(container) => container,
            Err(start_error) => {
                let error: anyhow::Error = start_error.into();
                if is_host_port_conflict(&error) {
                    last_port_conflict = Some(error);
                    continue;
                }
                // A loaded host can time out Docker's create call even though
                // the daemon is healthy. Retry only that transient error; the
                // container still has to start and answer PING.
                if is_transient_container_start_failure(&error) {
                    last_start_timeout = Some(error);
                    continue;
                }
                return Err(error);
            }
        };

        match await_redis_ready(&container).await {
            Ok(()) => return Ok(container),
            Err(ready_error) => {
                // A readiness probe that times out is the same transient
                // infrastructure class as the create timeout: drop this
                // container and try a fresh one rather than failing the suite.
                drop(container);
                if is_transient_container_start_failure(&ready_error) {
                    last_start_timeout = Some(ready_error);
                    continue;
                }
                return Err(ready_error);
            }
        }
    }

    Err(last_start_timeout.or(last_port_conflict).unwrap_or_else(|| {
        anyhow::anyhow!("failed to start Redis testcontainer after {HOST_PORT_BIND_ATTEMPTS} host-port attempts")
    }))
}

pub async fn await_redis_ready(container: &RedisContainer) -> Result<()> {
    await_redis_ready_with_timeout(container, Duration::from_secs(10)).await
}

pub async fn await_redis_ready_with_timeout(
    container: &RedisContainer,
    timeout: Duration,
) -> Result<()> {
    let host = container.get_host().await?;
    let port = container.get_host_port_ipv4(6379).await?;
    let url = format!("redis://{host}:{port}/");
    let deadline = Instant::now() + timeout;
    let client = redis::Client::open(url)?;

    loop {
        if let Ok(mut connection) = client.get_multiplexed_async_connection().await {
            let ping: redis::RedisResult<String> =
                redis::cmd("PING").query_async(&mut connection).await;
            if matches!(ping.as_deref(), Ok("PONG")) {
                return Ok(());
            }
        }
        if Instant::now() >= deadline {
            anyhow::bail!("Redis testcontainer did not become ready within {timeout:?}");
        }
        sleep(Duration::from_millis(50)).await;
    }
}
