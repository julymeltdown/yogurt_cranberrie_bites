use post_service::adapters::out::cache::feed_cache::RedisFeedCache;
use post_service::application::ports::FeedCachePort;
use redis::Client;
use std::time::{Duration, Instant};
#[path = "support/native_redis.rs"]
mod native_redis;
use native_redis::RedisFixture;
use uuid::Uuid;

#[tokio::test]
async fn cache_connection_recovers_after_redis_process_restart() {
    let container = RedisFixture::new().await.expect("Redis container");
    let host = container.get_host().await.expect("Redis host");
    let port = container
        .get_host_port_ipv4(6379)
        .await
        .expect("Redis port");
    let client = Client::open(format!("redis://{host}:{port}/")).expect("Redis client");
    let cache = RedisFeedCache::new(client).await.expect("Redis feed cache");
    let user_id = Uuid::new_v4();
    assert_eq!(cache.current_generation(user_id).await.unwrap(), 0);

    container
        .stop_with_timeout(Some(0))
        .await
        .expect("stop Redis process");
    let started = Instant::now();
    let unavailable =
        tokio::time::timeout(Duration::from_secs(5), cache.current_generation(user_id))
            .await
            .expect("Redis outage must not stall a cache call");
    assert!(
        unavailable.is_err(),
        "a stopped Redis process must fail the call"
    );
    assert!(started.elapsed() < Duration::from_secs(5));

    container.start().await.expect("restart Redis process");
    let restarted_host = container.get_host().await.expect("restarted Redis host");
    let restarted_port = container
        .get_host_port_ipv4(6379)
        .await
        .expect("restarted Redis port");
    assert_eq!(host, restarted_host, "container restart changed Redis host");
    assert_eq!(port, restarted_port, "container restart changed Redis port");
    container
        .wait_ready(Duration::from_secs(30))
        .await
        .expect("restarted Redis readiness");

    let mut last_error = String::from("no reconnect attempt completed");
    let mut recovered = false;
    for _ in 0..8 {
        match tokio::time::timeout(Duration::from_secs(2), cache.current_generation(user_id)).await
        {
            Ok(Ok(0)) => {
                recovered = true;
                break;
            }
            Ok(Ok(generation)) => last_error = format!("unexpected Redis generation: {generation}"),
            Ok(Err(error)) => last_error = error.to_string(),
            Err(error) => last_error = error.to_string(),
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    assert!(recovered, "Redis cache did not recover: {last_error}");
    container.rm().await.expect("dispose Redis fixture");
}
