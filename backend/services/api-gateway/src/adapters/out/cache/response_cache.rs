mod state;
#[cfg(test)]
mod tests;

use crate::application::ports::ResponseCachePort;
use async_trait::async_trait;
use serde_json::Value;
use state::CacheState;
use std::io::{self, Write};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// Limits apply to retained UTF-8 keys and serialized JSON, not process RSS.
/// In-flight values, allocator overhead, indexes and caller clones are separate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheLimits {
    pub max_entries: usize,
    pub max_key_bytes: usize,
    pub max_value_bytes: usize,
    pub max_total_bytes: usize,
}

impl Default for CacheLimits {
    fn default() -> Self {
        Self {
            max_entries: 4096,
            max_key_bytes: 512,
            max_value_bytes: 1024 * 1024,
            max_total_bytes: 16 * 1024 * 1024,
        }
    }
}

impl CacheLimits {
    fn validate(self) -> anyhow::Result<Self> {
        anyhow::ensure!(
            self.max_entries > 0 && self.max_key_bytes > 0 && self.max_value_bytes > 0,
            "cache entry, key and value limits must be positive"
        );
        let maximum_entry = self
            .max_key_bytes
            .checked_add(self.max_value_bytes)
            .ok_or_else(|| anyhow::anyhow!("cache entry limits overflow"))?;
        anyhow::ensure!(
            maximum_entry <= self.max_total_bytes,
            "cache total budget must fit a maximum-sized key and value"
        );
        Ok(self)
    }
}

/// Bounded counters have no resource IDs, keys, payloads or unbounded labels.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CacheStats {
    pub entries: usize,
    pub stored_bytes: usize,
    pub hits: u64,
    pub misses: u64,
    pub writes: u64,
    pub expired: u64,
    pub evicted: u64,
    pub rejected: u64,
    pub invalidated: u64,
}

/// A process-local, bounded LRU cache with atomic expiration decisions.
/// Clones share one store. This is not a distributed invalidation mechanism.
#[derive(Clone)]
pub struct InMemoryResponseCache {
    state: Arc<Mutex<CacheState>>,
    limits: CacheLimits,
}

impl Default for InMemoryResponseCache {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryResponseCache {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(CacheState::default())),
            limits: CacheLimits::default(),
        }
    }

    pub fn with_limits(limits: CacheLimits) -> anyhow::Result<Self> {
        Ok(Self {
            state: Arc::new(Mutex::new(CacheState::default())),
            limits: limits.validate()?,
        })
    }

    pub fn limits(&self) -> CacheLimits {
        self.limits
    }

    /// Expire stale entries before reporting retained occupancy.
    pub async fn stats(&self) -> CacheStats {
        let mut state = self.state.lock().await;
        state.expire(Instant::now());
        state.stats()
    }

    pub async fn invalidate(&self, cache_key: &str) -> bool {
        self.state.lock().await.invalidate(cache_key)
    }

    /// Invalidates currently retained entries; in-flight puts may repopulate it.
    pub async fn clear(&self) {
        self.state.lock().await.clear();
    }
}

/// Stop serialization before retaining an over-budget encoded value.
/// The input Value already exists; this does not bound upstream allocations.
struct LimitedWriter {
    bytes: Vec<u8>,
    limit: usize,
    exceeded: bool,
}

impl Write for LimitedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            self.exceeded = true;
            return Err(io::Error::other(
                "cache value exceeds serialized byte budget",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn encode(payload: &Value, limit: usize) -> anyhow::Result<Option<Arc<[u8]>>> {
    let mut writer = LimitedWriter {
        bytes: Vec::new(),
        limit,
        exceeded: false,
    };
    match serde_json::to_writer(&mut writer, payload) {
        Ok(()) => Ok(Some(Arc::from(writer.bytes.into_boxed_slice()))),
        Err(_) if writer.exceeded => Ok(None),
        Err(error) => Err(error.into()),
    }
}

impl InMemoryResponseCache {
    async fn get_infrastructure_inner(&self, cache_key: &str) -> anyhow::Result<Option<Value>> {
        let encoded = {
            let mut state = self.state.lock().await;
            // Sample time after acquiring the lock. A delayed reader must not
            // use a stale timestamp, or delete a value replaced by another task.
            state.get(cache_key, Instant::now())
        };
        // Decode outside the lock. The Arc protects an in-flight read from
        // eviction without keeping that value in retained-cache accounting.
        encoded
            .map(|bytes| serde_json::from_slice(&bytes).map_err(anyhow::Error::from))
            .transpose()
    }

    async fn put_infrastructure_inner(
        &self,
        cache_key: &str,
        payload: Value,
        ttl: Duration,
    ) -> anyhow::Result<()> {
        let encoded = if ttl.is_zero() || cache_key.len() > self.limits.max_key_bytes {
            None
        } else {
            encode(&payload, self.limits.max_value_bytes)?
        };
        drop(payload);
        let mut state = self.state.lock().await;
        let now = Instant::now();
        state.expire(now);
        match encoded {
            Some(bytes) => state.put(cache_key, bytes, ttl, now, self.limits),
            None => state.reject(cache_key),
        }
        // An uncacheable response is not a failed business response. Invalidate
        // its former value instead of leaving stale data under the same key.
        Ok(())
    }
}

#[async_trait]
impl ResponseCachePort for InMemoryResponseCache {
    async fn get(
        &self,
        cache_key: &str,
    ) -> crate::application::infrastructure_error::GatewayInfrastructureResult<
        Option<crate::application::dto::cache_dto::CachePayloadDto>,
    > {
        self.get_infrastructure_inner(cache_key)
            .await
            .map(|value| value.map(Into::into))
            .map_err(crate::adapters::out::infrastructure_error::classify_infrastructure_error)
    }
    async fn put(
        &self,
        cache_key: &str,
        payload: crate::application::dto::cache_dto::CachePayloadDto,
        ttl: Duration,
    ) -> crate::application::infrastructure_error::GatewayInfrastructureResult<()> {
        self.put_infrastructure_inner(cache_key, payload.into(), ttl)
            .await
            .map_err(crate::adapters::out::infrastructure_error::classify_infrastructure_error)
    }
}
