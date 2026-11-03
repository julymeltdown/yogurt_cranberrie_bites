use super::*;
use serde_json::json;

fn limits(entries: usize, total: usize) -> CacheLimits {
    CacheLimits {
        max_entries: entries,
        max_key_bytes: 8,
        max_value_bytes: 32,
        max_total_bytes: total,
    }
}

fn bytes(value: &Value) -> Arc<[u8]> {
    encode(value, 1024).unwrap().unwrap()
}

fn insert(state: &mut CacheState, key: &str, value: Value, now: Instant) {
    state.put(
        key,
        bytes(&value),
        Duration::from_secs(10),
        now,
        limits(2, 64),
    );
    state.assert_consistent();
}

#[test]
fn default_limits_are_valid_and_explicit() {
    assert_eq!(CacheLimits::default().max_entries, 4096);
    assert_eq!(CacheLimits::default().max_total_bytes, 16 * 1024 * 1024);
    CacheLimits::default().validate().unwrap();
}

#[test]
fn invalid_limits_are_rejected() {
    for invalid in [
        CacheLimits {
            max_entries: 0,
            ..CacheLimits::default()
        },
        CacheLimits {
            max_key_bytes: 0,
            ..CacheLimits::default()
        },
        CacheLimits {
            max_value_bytes: 0,
            ..CacheLimits::default()
        },
        CacheLimits {
            max_total_bytes: 1,
            ..CacheLimits::default()
        },
        CacheLimits {
            max_key_bytes: usize::MAX,
            ..CacheLimits::default()
        },
    ] {
        assert!(InMemoryResponseCache::with_limits(invalid).is_err());
    }
}

#[test]
fn expiration_at_exact_deadline_is_a_miss() {
    let mut state = CacheState::default();
    let now = Instant::now();
    insert(&mut state, "a", json!(1), now);
    assert!(state.get("a", now + Duration::from_secs(10)).is_none());
    assert_eq!(state.stats().expired, 1);
    assert_eq!(state.stats().stored_bytes, 0);
    state.assert_consistent();
}

#[test]
fn live_read_preserves_the_original_deadline() {
    let mut state = CacheState::default();
    let now = Instant::now();
    insert(&mut state, "a", json!(1), now);
    assert!(state.get("a", now + Duration::from_secs(9)).is_some());
    assert!(state.get("a", now + Duration::from_secs(10)).is_none());
}

#[test]
fn replacement_removes_the_old_expiration_record() {
    let mut state = CacheState::default();
    let now = Instant::now();
    insert(&mut state, "a", json!(1), now);
    insert(&mut state, "a", json!(2), now + Duration::from_secs(5));
    state.expire(now + Duration::from_secs(10));
    assert_eq!(
        &*state.get("a", now + Duration::from_secs(10)).unwrap(),
        b"2"
    );
    assert_eq!(state.stats().entries, 1);
    assert_eq!(state.stats().stored_bytes, 2);
    state.assert_consistent();
}

#[test]
fn expired_entries_are_reclaimed_before_live_entries_are_evicted() {
    let mut state = CacheState::default();
    let now = Instant::now();
    insert(&mut state, "a", json!(1), now);
    insert(&mut state, "b", json!(2), now + Duration::from_secs(2));
    insert(&mut state, "c", json!(3), now + Duration::from_secs(10));
    assert_eq!(state.stats().expired, 1);
    assert_eq!(state.stats().evicted, 0);
    assert!(state.get("b", now + Duration::from_secs(10)).is_some());
}

#[test]
fn access_order_not_clock_resolution_defines_lru() {
    let mut state = CacheState::default();
    let now = Instant::now();
    insert(&mut state, "a", json!(1), now);
    insert(&mut state, "b", json!(2), now);
    assert!(state.get("a", now).is_some());
    insert(&mut state, "c", json!(3), now);
    assert!(state.get("b", now).is_none());
    assert!(state.get("a", now).is_some());
    assert_eq!(state.stats().evicted, 1);
    state.assert_consistent();
}

#[test]
fn sequence_wrap_preserves_lru_order() {
    let mut state = CacheState::default();
    let now = Instant::now();
    insert(&mut state, "a", json!(1), now);
    insert(&mut state, "b", json!(2), now);
    state.exhaust_clock();
    assert!(state.get("a", now).is_some());
    insert(&mut state, "c", json!(3), now);
    assert!(state.get("b", now).is_none());
    state.assert_consistent();
}

#[test]
fn serialized_byte_budget_can_evict_before_entry_limit() {
    let mut state = CacheState::default();
    let now = Instant::now();
    let policy = limits(100, 40);
    for key in ["a", "b", "c"] {
        state.put(
            key,
            bytes(&json!("12345678901234567890")),
            Duration::from_secs(1),
            now,
            policy,
        );
        state.assert_consistent();
        assert!(state.stats().stored_bytes <= 40);
    }
    assert_eq!(state.stats().entries, 1);
    assert_eq!(state.stats().evicted, 2);
}

#[test]
fn exact_total_byte_budget_is_accepted() {
    let mut state = CacheState::default();
    let now = Instant::now();
    let policy = CacheLimits {
        max_entries: 1,
        max_key_bytes: 1,
        max_value_bytes: 3,
        max_total_bytes: 4,
    };
    state.put("a", bytes(&json!(123)), Duration::from_secs(1), now, policy);
    assert_eq!(state.stats().stored_bytes, 4);
    assert_eq!(state.stats().rejected, 0);
    state.assert_consistent();
}

#[test]
fn zero_ttl_invalidates_existing_value() {
    let mut state = CacheState::default();
    let now = Instant::now();
    insert(&mut state, "a", json!(1), now);
    state.put("a", bytes(&json!(2)), Duration::ZERO, now, limits(2, 64));
    assert!(state.get("a", now).is_none());
    assert_eq!(state.stats().invalidated, 1);
    state.assert_consistent();
}

#[test]
fn overflowing_ttl_does_not_panic_or_retain_old_value() {
    let mut state = CacheState::default();
    let now = Instant::now();
    insert(&mut state, "a", json!(1), now);
    state.put("a", bytes(&json!(2)), Duration::MAX, now, limits(2, 64));
    assert!(state.get("a", now).is_none());
    assert_eq!(state.stats().rejected, 1);
    state.assert_consistent();
}

#[test]
fn repeated_replacements_do_not_grow_indexes_or_accounting() {
    let mut state = CacheState::default();
    let now = Instant::now();
    for value in 0..10_000 {
        insert(&mut state, "a", json!(value), now);
        assert_eq!(state.stats().entries, 1);
    }
}

#[test]
fn uncacheable_replacement_removes_old_but_keeps_unrelated_value() {
    let mut state = CacheState::default();
    let now = Instant::now();
    insert(&mut state, "a", json!(1), now);
    insert(&mut state, "b", json!(2), now);
    state.put(
        "a",
        bytes(&json!("x".repeat(64))),
        Duration::from_secs(1),
        now,
        limits(2, 64),
    );
    assert!(state.get("a", now).is_none());
    assert!(state.get("b", now).is_some());
    assert_eq!(state.stats().evicted, 0);
    state.assert_consistent();
}

#[test]
fn limited_writer_rejects_without_appending_partial_chunk() {
    let mut writer = LimitedWriter {
        bytes: Vec::new(),
        limit: 3,
        exceeded: false,
    };
    writer.write_all(b"ab").unwrap();
    assert!(writer.write_all(b"cd").is_err());
    assert_eq!(&writer.bytes, b"ab");
    assert!(writer.exceeded);
}

#[test]
fn encoded_limits_count_utf8_and_escaping() {
    assert!(encode(&json!("é"), 3).unwrap().is_none());
    assert!(encode(&json!("é"), 4).unwrap().is_some());
    assert!(encode(&json!("\n"), 3).unwrap().is_none());
    assert!(encode(&json!("\n"), 4).unwrap().is_some());
}

#[test]
fn missing_read_does_not_remove_another_key() {
    let mut state = CacheState::default();
    let now = Instant::now();
    insert(&mut state, "present", json!(1), now);
    assert!(state.get("absent", now).is_none());
    assert!(state.get("present", now).is_some());
    state.assert_consistent();
}

#[test]
fn clear_removes_all_indices_and_preserves_cumulative_counters() {
    let mut state = CacheState::default();
    let now = Instant::now();
    insert(&mut state, "a", json!(1), now);
    insert(&mut state, "b", json!(2), now);
    state.clear();
    assert_eq!(state.stats().entries, 0);
    assert_eq!(state.stats().stored_bytes, 0);
    assert_eq!(state.stats().writes, 2);
    assert_eq!(state.stats().invalidated, 2);
    state.assert_consistent();
}

#[test]
fn in_flight_reader_can_finish_after_eviction() {
    let mut state = CacheState::default();
    let now = Instant::now();
    insert(&mut state, "a", json!({"value": 1}), now);
    let reader = state.get("a", now).unwrap();
    state.invalidate("a");
    assert_eq!(
        serde_json::from_slice::<Value>(&reader).unwrap(),
        json!({"value": 1})
    );
    assert_eq!(state.stats().stored_bytes, 0);
}

#[tokio::test]
async fn public_cache_round_trips_json_without_sharing_mutable_values() {
    let cache = InMemoryResponseCache::new();
    let payload = json!({"items": [null, true, 42, "sample"], "cursor": null});
    cache
        .put("test", payload.clone().into(), Duration::from_secs(60))
        .await
        .unwrap();
    let mut first = cache
        .get("test")
        .await
        .unwrap()
        .map(|payload| payload.value)
        .unwrap();
    first["items"] = json!([]);
    assert_eq!(
        cache
            .get("test")
            .await
            .unwrap()
            .map(|payload| payload.value),
        Some(payload)
    );
    assert_eq!(cache.stats().await.hits, 2);
}

#[tokio::test]
async fn clones_share_limits_entries_and_invalidation() {
    let cache = InMemoryResponseCache::with_limits(limits(2, 64)).unwrap();
    let other = cache.clone();
    assert_eq!(cache.limits(), other.limits());
    cache
        .put("a", json!(1).into(), Duration::from_secs(60))
        .await
        .unwrap();
    assert_eq!(
        other.get("a").await.unwrap().map(|payload| payload.value),
        Some(json!(1))
    );
    assert!(other.invalidate("a").await);
    assert!(cache
        .get("a")
        .await
        .unwrap()
        .map(|payload| payload.value)
        .is_none());
    assert!(!other.invalidate("a").await);
}

#[tokio::test]
async fn over_budget_put_is_a_successful_cache_bypass() {
    let cache = InMemoryResponseCache::with_limits(limits(2, 64)).unwrap();
    cache
        .put("a", json!(1).into(), Duration::from_secs(60))
        .await
        .unwrap();
    cache
        .put("a", json!("x".repeat(128)).into(), Duration::from_secs(60))
        .await
        .unwrap();
    cache
        .put("123456789", json!(1).into(), Duration::from_secs(60))
        .await
        .unwrap();
    assert!(cache
        .get("a")
        .await
        .unwrap()
        .map(|payload| payload.value)
        .is_none());
    assert_eq!(cache.stats().await.rejected, 2);
}

#[tokio::test]
async fn zero_ttl_through_public_port_invalidates() {
    let cache = InMemoryResponseCache::new();
    cache
        .put("a", json!(1).into(), Duration::from_secs(60))
        .await
        .unwrap();
    cache
        .put("a", json!(2).into(), Duration::ZERO)
        .await
        .unwrap();
    assert!(cache
        .get("a")
        .await
        .unwrap()
        .map(|payload| payload.value)
        .is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_readers_and_writers_remain_bounded() {
    let cache = InMemoryResponseCache::with_limits(limits(8, 80)).unwrap();
    let start = Arc::new(tokio::sync::Barrier::new(16));
    let mut tasks = Vec::new();
    for worker in 0..16 {
        let clone = cache.clone();
        let start = start.clone();
        tasks.push(tokio::spawn(async move {
            start.wait().await;
            for step in 0..100 {
                let key = format!("{worker}-{step}");
                clone
                    .put(&key, json!(step).into(), Duration::from_secs(60))
                    .await
                    .unwrap();
                let _ = clone.get(&key).await.unwrap().map(|payload| payload.value);
                let stats = clone.stats().await;
                assert!(stats.entries <= 8);
                assert!(stats.stored_bytes <= 80);
            }
        }));
    }
    for task in tasks {
        task.await.unwrap();
    }
    cache
        .put("final", json!("new").into(), Duration::from_secs(60))
        .await
        .unwrap();
    assert_eq!(
        cache
            .get("final")
            .await
            .unwrap()
            .map(|payload| payload.value),
        Some(json!("new"))
    );
    cache.state.lock().await.assert_consistent();
}

#[tokio::test]
async fn cancellation_waiting_for_cache_lock_has_no_partial_write() {
    let cache = InMemoryResponseCache::new();
    cache
        .put("a", json!(1).into(), Duration::from_secs(60))
        .await
        .unwrap();
    let guard = cache.state.lock().await;
    let clone = cache.clone();
    let task = tokio::spawn(async move {
        clone
            .put("a", json!(2).into(), Duration::from_secs(60))
            .await
    });
    tokio::task::yield_now().await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    drop(guard);
    assert_eq!(
        cache.get("a").await.unwrap().map(|payload| payload.value),
        Some(json!(1))
    );
}
