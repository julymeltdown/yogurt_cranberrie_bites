use super::connect_manager;
use crate::application::ports::FeedCachePort;
use crate::domain::feed::FeedCachePage;
use anyhow::Result;
use async_trait::async_trait;
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use redis::Client;
use uuid::Uuid;

const FEED_CACHE_TTL_SECONDS: u64 = 300;
pub(super) const FEED_CACHE_GENERATION_TTL_SECONDS: u64 = FEED_CACHE_TTL_SECONDS * 2;
pub const DEFAULT_FEED_CACHE_EPOCH: u64 = 1;

/// Store a page only if the generation it was computed against is still current.
///
/// This runs inside Redis so the comparison and the write cannot be separated.
/// A read-then-write from the client would leave a window in which a mutation
/// bumps the generation between the two calls, and the stale page would land
/// under the new generation. Returns 1 when stored and 0 when discarded.
const STORE_IF_GENERATION_MATCHES: &str = r#"
local observed = redis.call('GET', KEYS[1])
if not observed then
  observed = '0'
end
-- Compare Redis' decimal strings exactly. Lua numbers lose integer precision above 2^53,
-- and a malformed value must not be treated as a missing generation.
if observed ~= ARGV[1] then
  return 0
end
redis.call('SET', KEYS[2], ARGV[2], 'EX', ARGV[3])
return 1
"#;

#[derive(Clone)]
pub struct RedisFeedCache {
    /// A pooled, bounded Redis connection manager.
    ///
    /// Obtaining a fresh multiplexed connection per call opened a new TCP
    /// connection and handshake on every cache read and write, which is a
    /// per-request cost paid for a dependency that is supposed to make requests
    /// cheaper. The manager bounds connection and response timing, then retries
    /// after transport failures.
    connection: ConnectionManager,
    /// Rotated by operators after Redis reset/recovery. Old processes can still
    /// finish writes, but their keys are invisible to the new namespace.
    namespace_epoch: u64,
}

impl RedisFeedCache {
    /// Build a cache over a shared connection manager.
    pub fn with_connection(connection: ConnectionManager) -> Self {
        Self::with_namespace_epoch(connection, DEFAULT_FEED_CACHE_EPOCH)
    }

    /// Build a cache over a shared connection manager using a deployment epoch.
    pub fn with_namespace_epoch(connection: ConnectionManager, namespace_epoch: u64) -> Self {
        Self {
            connection,
            namespace_epoch,
        }
    }

    /// Build a cache from a client, creating its own connection manager.
    pub async fn new(client: Client) -> Result<Self> {
        Ok(Self::with_connection(connect_manager(client).await?))
    }

    /// Build a cache from a client and an explicit deployment epoch.
    pub async fn new_with_namespace_epoch(client: Client, namespace_epoch: u64) -> Result<Self> {
        Ok(Self::with_namespace_epoch(
            connect_manager(client).await?,
            namespace_epoch,
        ))
    }
}

pub fn feed_cache_generation_key(user_id: Uuid) -> String {
    feed_cache_generation_key_for_epoch(DEFAULT_FEED_CACHE_EPOCH, user_id)
}

pub fn feed_cache_generation_key_for_epoch(namespace_epoch: u64, user_id: Uuid) -> String {
    format!("feed-generation:{namespace_epoch}:{user_id}")
}

pub fn feed_cache_key(user_id: Uuid, generation: u64, cursor: Option<&str>, limit: i32) -> String {
    feed_cache_key_for_epoch(DEFAULT_FEED_CACHE_EPOCH, user_id, generation, cursor, limit)
}

pub fn feed_cache_key_for_epoch(
    namespace_epoch: u64,
    user_id: Uuid,
    generation: u64,
    cursor: Option<&str>,
    limit: i32,
) -> String {
    format!(
        "feed:{}:{}:{}:{}:{}",
        namespace_epoch,
        user_id,
        generation,
        cursor.unwrap_or("head").replace('|', "_"),
        limit
    )
}

/// Read the generation counter.
///
/// Generic over the connection type so the same logic works with a plain
/// connection and with the shared manager, which is what lets the adapter keep
/// one handle instead of opening a connection per call.
async fn current_generation<C>(
    connection: &mut C,
    namespace_epoch: u64,
    user_id: Uuid,
) -> Result<u64>
where
    C: redis::AsyncCommands + Send,
{
    let key = feed_cache_generation_key_for_epoch(namespace_epoch, user_id);
    let (generation, _): (Option<u64>, bool) = redis::pipe()
        .cmd("GET")
        .arg(&key)
        .cmd("EXPIRE")
        .arg(&key)
        .arg(FEED_CACHE_GENERATION_TTL_SECONDS)
        .query_async(connection)
        .await?;
    Ok(generation.unwrap_or(0))
}

#[async_trait]
impl FeedCachePort for RedisFeedCache {
    async fn get_feed(
        &self,
        user_id: Uuid,
        cursor: Option<&str>,
        limit: i32,
    ) -> Result<Option<FeedCachePage>> {
        let mut connection = self.connection.clone();
        let generation = current_generation(&mut connection, self.namespace_epoch, user_id).await?;
        let payload: Option<String> = connection
            .get(feed_cache_key_for_epoch(
                self.namespace_epoch,
                user_id,
                generation,
                cursor,
                limit,
            ))
            .await?;
        Ok(payload
            .map(|value| serde_json::from_str(&value))
            .transpose()?)
    }

    async fn set_feed(
        &self,
        user_id: Uuid,
        cursor: Option<&str>,
        limit: i32,
        page: &FeedCachePage,
        generation: u64,
    ) -> Result<()> {
        let mut connection = self.connection.clone();
        // The comparison and the write are one server-side operation, so a
        // mutation cannot slip between them.
        let stored: i64 = redis::Script::new(STORE_IF_GENERATION_MATCHES)
            .key(feed_cache_generation_key_for_epoch(
                self.namespace_epoch,
                user_id,
            ))
            .key(feed_cache_key_for_epoch(
                self.namespace_epoch,
                user_id,
                generation,
                cursor,
                limit,
            ))
            .arg(generation)
            .arg(serde_json::to_string(page)?)
            .arg(FEED_CACHE_TTL_SECONDS)
            .invoke_async(&mut connection)
            .await?;
        if stored == 0 {
            tracing::debug!(
                user_id = %user_id,
                expected = generation,
                "discarded a feed cache fill that raced a mutation"
            );
        }
        Ok(())
    }

    async fn current_generation(&self, user_id: Uuid) -> Result<u64> {
        let mut connection = self.connection.clone();
        current_generation(&mut connection, self.namespace_epoch, user_id).await
    }

    async fn invalidate_users(&self, user_ids: &[Uuid]) -> Result<()> {
        if user_ids.is_empty() {
            return Ok(());
        }
        let mut connection = self.connection.clone();
        let mut pipeline = redis::pipe();
        for user_id in user_ids {
            let generation_key =
                feed_cache_generation_key_for_epoch(self.namespace_epoch, *user_id);
            pipeline
                .cmd("INCR")
                .arg(&generation_key)
                .ignore()
                .cmd("EXPIRE")
                .arg(generation_key)
                .arg(FEED_CACHE_GENERATION_TTL_SECONDS)
                .ignore();
        }
        let _: () = pipeline.query_async(&mut connection).await?;
        Ok(())
    }
}
