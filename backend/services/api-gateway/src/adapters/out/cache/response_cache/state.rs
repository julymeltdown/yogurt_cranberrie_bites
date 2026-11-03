use super::{CacheLimits, CacheStats};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;
use std::time::{Duration, Instant};

struct Entry {
    bytes: Arc<[u8]>,
    expires_at: Instant,
    access: u64,
}

/// Each live entry has exactly one expiration and one recency-index entry.
/// No append-only access queue or per-key background task can grow unbounded.
#[derive(Default)]
pub(super) struct CacheState {
    entries: HashMap<String, Entry>,
    expiration: BTreeSet<(Instant, String)>,
    recency: BTreeMap<u64, String>,
    access: u64,
    counters: CacheStats,
}

impl CacheState {
    fn tick(&mut self) -> u64 {
        if self.access == u64::MAX {
            // Preserve exact LRU ordering even after the logical clock wraps.
            let mut compact = BTreeMap::new();
            for (index, key) in self.recency.values().enumerate() {
                let sequence = index as u64 + 1;
                if let Some(entry) = self.entries.get_mut(key) {
                    entry.access = sequence;
                }
                compact.insert(sequence, key.clone());
            }
            self.access = compact.len() as u64;
            self.recency = compact;
        }
        self.access += 1;
        self.access
    }

    fn remove(&mut self, key: &str) -> bool {
        let Some(entry) = self.entries.remove(key) else {
            return false;
        };
        self.counters.stored_bytes -= key.len() + entry.bytes.len();
        self.expiration.remove(&(entry.expires_at, key.to_owned()));
        self.recency.remove(&entry.access);
        true
    }

    pub(super) fn invalidate(&mut self, key: &str) -> bool {
        let removed = self.remove(key);
        if removed {
            self.counters.invalidated = self.counters.invalidated.saturating_add(1);
        }
        removed
    }

    pub(super) fn reject(&mut self, key: &str) {
        self.counters.rejected = self.counters.rejected.saturating_add(1);
        self.invalidate(key);
    }

    pub(super) fn clear(&mut self) {
        self.counters.invalidated = self
            .counters
            .invalidated
            .saturating_add(self.entries.len() as u64);
        self.entries.clear();
        self.expiration.clear();
        self.recency.clear();
        self.counters.stored_bytes = 0;
        self.access = 0;
    }

    pub(super) fn expire(&mut self, now: Instant) {
        while let Some((deadline, key)) = self.expiration.first().cloned() {
            if deadline > now {
                break;
            }
            self.remove(&key);
            self.counters.expired = self.counters.expired.saturating_add(1);
        }
    }

    pub(super) fn get(&mut self, key: &str, now: Instant) -> Option<Arc<[u8]>> {
        if self
            .entries
            .get(key)
            .is_some_and(|entry| entry.expires_at <= now)
        {
            self.remove(key);
            self.counters.expired = self.counters.expired.saturating_add(1);
        }
        if !self.entries.contains_key(key) {
            self.counters.misses = self.counters.misses.saturating_add(1);
            return None;
        }
        let access = self.tick();
        let entry = self.entries.get_mut(key)?;
        self.recency.remove(&entry.access);
        entry.access = access;
        self.recency.insert(access, key.to_owned());
        self.counters.hits = self.counters.hits.saturating_add(1);
        Some(entry.bytes.clone())
    }

    pub(super) fn put(
        &mut self,
        key: &str,
        bytes: Arc<[u8]>,
        ttl: Duration,
        now: Instant,
        limits: CacheLimits,
    ) {
        let Some(deadline) = now.checked_add(ttl) else {
            self.reject(key);
            return;
        };
        let Some(cost) = key.len().checked_add(bytes.len()) else {
            self.reject(key);
            return;
        };
        if ttl.is_zero()
            || key.len() > limits.max_key_bytes
            || bytes.len() > limits.max_value_bytes
            || cost > limits.max_total_bytes
        {
            self.reject(key);
            return;
        }
        self.expire(now);
        self.remove(key);
        while self.entries.len() >= limits.max_entries
            || self.counters.stored_bytes > limits.max_total_bytes - cost
        {
            let Some((_, oldest)) = self.recency.first_key_value() else {
                self.reject(key);
                return;
            };
            let oldest = oldest.clone();
            self.remove(&oldest);
            self.counters.evicted = self.counters.evicted.saturating_add(1);
        }
        let access = self.tick();
        self.expiration.insert((deadline, key.to_owned()));
        self.recency.insert(access, key.to_owned());
        self.entries.insert(
            key.to_owned(),
            Entry {
                bytes,
                expires_at: deadline,
                access,
            },
        );
        self.counters.stored_bytes += cost;
        self.counters.writes = self.counters.writes.saturating_add(1);
    }

    pub(super) fn stats(&self) -> CacheStats {
        CacheStats {
            entries: self.entries.len(),
            ..self.counters
        }
    }

    #[cfg(test)]
    pub(super) fn assert_consistent(&self) {
        assert_eq!(self.entries.len(), self.expiration.len());
        assert_eq!(self.entries.len(), self.recency.len());
        assert_eq!(
            self.counters.stored_bytes,
            self.entries
                .iter()
                .map(|(key, entry)| key.len() + entry.bytes.len())
                .sum::<usize>()
        );
        for (key, entry) in &self.entries {
            assert!(self.expiration.contains(&(entry.expires_at, key.clone())));
            assert_eq!(self.recency.get(&entry.access), Some(key));
        }
    }

    #[cfg(test)]
    pub(super) fn exhaust_clock(&mut self) {
        self.access = u64::MAX;
    }
}
