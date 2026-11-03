use api_gateway::adapters::out::cache::response_cache::InMemoryResponseCache;
use api_gateway::application::ports::ResponseCachePort;
use serde_json::json;
use std::time::Duration;

#[tokio::test]
async fn response_cache_returns_payload_before_ttl_expires() {
    let cache = InMemoryResponseCache::new();

    cache
        .put(
            "aggregation:feed",
            json!({"posts": [{"id": "post-1"}]}).into(),
            Duration::from_secs(60),
        )
        .await
        .expect("cache put");
    let cached = cache
        .get("aggregation:feed")
        .await
        .expect("cache get")
        .expect("cached payload");

    assert_eq!(cached.value, json!({"posts": [{"id": "post-1"}]}));
}

#[tokio::test]
async fn response_cache_evicts_payload_after_ttl_expires() {
    let cache = InMemoryResponseCache::new();

    cache
        .put(
            "aggregation:feed",
            json!({"stale": true}).into(),
            Duration::ZERO,
        )
        .await
        .expect("cache put");
    let cached = cache.get("aggregation:feed").await.expect("cache get");
    let cached_after_removal = cache.get("aggregation:feed").await.expect("cache get");

    assert!(cached.is_none());
    assert!(cached_after_removal.is_none());
}
