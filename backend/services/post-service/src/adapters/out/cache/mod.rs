pub mod claimed_effects;
pub mod feed_cache;
pub mod post_cache;

use redis::aio::{ConnectionManager, ConnectionManagerConfig};
use redis::Client;
use std::future::Future;
use std::time::Duration;
use tokio::time::timeout;

const CACHE_CONNECTION_TIMEOUT: Duration = Duration::from_millis(100);
const CACHE_RESPONSE_TIMEOUT: Duration = Duration::from_millis(150);
const CACHE_OPERATION_TIMEOUT: Duration = Duration::from_millis(100);

pub(crate) async fn run_bounded_cache_operation<T, F>(
    operation: &'static str,
    future: F,
) -> anyhow::Result<T>
where
    F: Future<Output = anyhow::Result<T>>,
{
    timeout(CACHE_OPERATION_TIMEOUT, future)
        .await
        .map_err(|_| {
            anyhow::anyhow!(
                "{operation} timed out after {} ms",
                CACHE_OPERATION_TIMEOUT.as_millis()
            )
        })?
}

/// Create the shared Redis cache connection manager with bounded timing.
///
/// These limits let the outbox worker record a delivery failure and retry it
/// instead of stalling on a half-open connection. Cache contents are
/// reconstructible from Postgres, so an individual cache operation must fail
/// quickly enough for its caller to complete the authoritative database path
/// within the Gateway→Post request budget. Reconnection continues in the
/// shared manager after the failed operation.
pub async fn connect_manager(client: Client) -> redis::RedisResult<ConnectionManager> {
    let config = ConnectionManagerConfig::new()
        .set_number_of_retries(2)
        .set_max_delay(250)
        .set_connection_timeout(CACHE_CONNECTION_TIMEOUT)
        .set_response_timeout(CACHE_RESPONSE_TIMEOUT);
    ConnectionManager::new_with_config(client, config).await
}
