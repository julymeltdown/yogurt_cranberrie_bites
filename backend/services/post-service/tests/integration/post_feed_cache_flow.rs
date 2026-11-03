use crate::support::native_redis::RedisFixture;
use anyhow::Context;
use post_service::adapters::out::cache::feed_cache::{
    feed_cache_generation_key, feed_cache_key, RedisFeedCache,
};
use post_service::application::ports::FeedCachePort;
use post_service::domain::feed::FeedCachePage;
use redis::AsyncCommands;
use uuid::Uuid;

/// Exercise the real Redis adapter, whose fill guard is a server-side script.
///
/// The unit tests model this with a fake, which cannot prove the Lua comparison
/// and the write really are one atomic operation against a live server.
async fn cache() -> anyhow::Result<(RedisFixture, RedisFeedCache, redis::Client)> {
    let container = RedisFixture::new().await?;
    let host = container.get_host().await?;
    let port = container.get_host_port_ipv4(6379).await?;
    let url = format!("redis://{host}:{port}/");
    let client = redis::Client::open(url)?;
    let cache = RedisFeedCache::new(client.clone()).await?;
    // The caller owns the container for the entire test, including failure
    // unwinding. Process exit does not remove a forgotten Docker container.
    Ok((container, cache, client))
}

async fn total_connections(
    observer: &mut redis::aio::MultiplexedConnection,
) -> anyhow::Result<u64> {
    let stats: String = redis::cmd("INFO")
        .arg("stats")
        .query_async(observer)
        .await?;
    stats
        .lines()
        .find_map(|line| line.strip_prefix("total_connections_received:"))
        .context("Redis INFO stats must include total_connections_received")?
        .trim()
        .parse()
        .context("Redis connection counter must be an unsigned integer")
}

#[tokio::test]
async fn cache_calls_reuse_one_connection() -> anyhow::Result<()> {
    // The regression this guards: a multiplexed connection was obtained per
    // call, so every cache read and write opened a new connection and handshake.
    // Count cumulative accepted connections, not current clients: opening and
    // closing one connection per call could leave the live client count flat.
    let (container, cache, client) = cache().await?;
    let mut observer = client.get_multiplexed_async_connection().await?;
    let user_id = Uuid::from_u128(0x9030);

    // A first call establishes the managed connection.
    let generation = FeedCachePort::current_generation(&cache, user_id).await?;
    let after_first = total_connections(&mut observer).await?;
    assert!(
        after_first >= 2,
        "Redis must count at least the cache and observer connections, saw {after_first}"
    );

    // Many more calls must not raise the count.
    for _ in 0..5 {
        FeedCachePort::current_generation(&cache, user_id).await?;
        let page = FeedCachePage {
            post_ids: vec![user_id],
            next_cursor: None,
        };
        FeedCachePort::set_feed(&cache, user_id, None, 10, &page, generation).await?;
        FeedCachePort::get_feed(&cache, user_id, None, 10).await?;
    }

    let after_many = total_connections(&mut observer).await?;
    assert_eq!(
        after_many, after_first,
        "cache calls must not open new connections"
    );

    // Prove the oracle detects a connection even after it has been closed.
    let mut probe = client.get_multiplexed_async_connection().await?;
    let _: String = redis::cmd("PING").query_async(&mut probe).await?;
    drop(probe);
    assert!(
        total_connections(&mut observer).await? > after_many,
        "the cumulative counter must detect the opened-and-closed probe connection"
    );
    container.rm().await?;
    Ok(())
}

#[tokio::test]
async fn a_fill_is_discarded_when_the_generation_moved_on() -> anyhow::Result<()> {
    let (container, cache, _client) = cache().await?;
    let user_id = Uuid::from_u128(0x9001);
    let page = FeedCachePage {
        post_ids: vec![Uuid::from_u128(0x9002)],
        next_cursor: None,
    };

    // Given: a mutation bumps the generation after the page was computed.
    cache.invalidate_users(&[user_id]).await?;
    let generation = cache.current_generation(user_id).await?;
    assert_eq!(generation, 1);

    // When: a second mutation lands while the page is being computed, and the
    // fill is then stored against the generation it read at the start.
    cache.invalidate_users(&[user_id]).await?;
    cache.set_feed(user_id, None, 10, &page, generation).await?;

    // Then: the fill is refused because the generation moved on. Storing it
    // would land the page under the newer generation and serve content from
    // before the mutation.
    assert!(
        cache.get_feed(user_id, None, 10).await?.is_none(),
        "a fill computed before the mutation must not be served after it"
    );
    container.rm().await?;
    Ok(())
}

#[tokio::test]
async fn fill_guard_compares_generation_exactly_and_rejects_invalid_state() -> anyhow::Result<()> {
    let (container, cache, client) = cache().await?;
    let user_id = Uuid::from_u128(0x9003);
    let page = FeedCachePage {
        post_ids: vec![Uuid::from_u128(0x9004)],
        next_cursor: None,
    };

    let observed: anyhow::Result<(bool, bool)> = async {
        let mut connection = client.get_multiplexed_async_connection().await?;
        let generation_key = feed_cache_generation_key(user_id);

        // These adjacent integers collapse to the same Lua number above 2^53.
        let stale_generation = 9_007_199_254_740_993_u64;
        let _: () = connection
            .set(&generation_key, 9_007_199_254_740_992_u64)
            .await?;
        cache
            .set_feed(user_id, None, 10, &page, stale_generation)
            .await?;
        let high_precision_fill_exists: bool = connection
            .exists(feed_cache_key(user_id, stale_generation, None, 10))
            .await?;

        // A corrupt counter must not be interpreted as the missing-key value 0.
        let _: () = connection.set(&generation_key, "corrupt").await?;
        cache.set_feed(user_id, None, 10, &page, 0).await?;
        let corrupt_fill_exists: bool = connection
            .exists(feed_cache_key(user_id, 0, None, 10))
            .await?;
        Ok((high_precision_fill_exists, corrupt_fill_exists))
    }
    .await;

    container.rm().await?;
    let (high_precision_fill_exists, corrupt_fill_exists) = observed?;
    assert!(
        !high_precision_fill_exists,
        "an adjacent generation above Lua's exact-integer range must not be accepted"
    );
    assert!(
        !corrupt_fill_exists,
        "a malformed Redis generation must not authorize a generation-zero fill"
    );
    Ok(())
}

#[tokio::test]
async fn a_rotated_namespace_ignores_a_late_fill_from_the_previous_epoch() -> anyhow::Result<()> {
    let (container, old_cache, client) = cache().await?;
    let new_cache = RedisFeedCache::new_with_namespace_epoch(client, 2).await?;
    let user_id = Uuid::from_u128(0x9008);
    let old_page = FeedCachePage {
        post_ids: vec![Uuid::from_u128(0x9009)],
        next_cursor: None,
    };

    // An old request captured this generation before operators rotated the
    // deployment epoch after a Redis reset.
    let old_generation = old_cache.current_generation(user_id).await?;

    // A process that has already restarted can still receive a late write from
    // an old in-flight request, but that write lands only in epoch 1.
    old_cache
        .set_feed(user_id, None, 10, &old_page, old_generation)
        .await?;
    assert_eq!(old_cache.get_feed(user_id, None, 10).await?, Some(old_page));

    // The replacement process reads epoch 2 and cannot observe the old page.
    let new_generation = new_cache.current_generation(user_id).await?;
    assert_eq!(new_generation, 0);
    assert!(new_cache.get_feed(user_id, None, 10).await?.is_none());
    container.rm().await?;
    Ok(())
}

#[tokio::test]
async fn a_fill_is_stored_when_no_mutation_intervened() -> anyhow::Result<()> {
    let (container, cache, client) = cache().await?;
    let user_id = Uuid::from_u128(0x9010);
    let page = FeedCachePage {
        post_ids: vec![Uuid::from_u128(0x9011)],
        next_cursor: Some("next".to_owned()),
    };

    let generation = cache.current_generation(user_id).await?;
    cache.set_feed(user_id, None, 10, &page, generation).await?;

    // The guard must not reject an ordinary fill, or the cache would never fill.
    assert_eq!(cache.get_feed(user_id, None, 10).await?, Some(page));

    // And the entry lands under the generation-qualified key.
    let mut connection = client.get_multiplexed_async_connection().await?;
    let exists: bool = connection
        .exists(feed_cache_key(user_id, generation, None, 10))
        .await?;
    assert!(exists, "the page must be stored under its generation");
    let generation_key: Option<i64> = connection.get(feed_cache_generation_key(user_id)).await?;
    assert_eq!(
        generation_key.unwrap_or(0),
        0,
        "no mutation should have bumped the generation"
    );
    container.rm().await?;
    Ok(())
}

#[tokio::test]
async fn invalidating_bumps_the_generation_and_hides_the_previous_page() -> anyhow::Result<()> {
    let (container, cache, client) = cache().await?;
    let user_id = Uuid::from_u128(0x9020);
    let page = FeedCachePage {
        post_ids: vec![Uuid::from_u128(0x9021)],
        next_cursor: None,
    };

    let generation = cache.current_generation(user_id).await?;
    cache.set_feed(user_id, None, 10, &page, generation).await?;
    assert!(cache.get_feed(user_id, None, 10).await?.is_some());

    // Invalidation is a generation bump, not a key scan, so the page becomes
    // unreachable by construction rather than by deleting each key.
    cache.invalidate_users(&[user_id]).await?;
    assert!(
        cache.get_feed(user_id, None, 10).await?.is_none(),
        "a bump must make the previous generation unreachable"
    );
    assert_eq!(cache.current_generation(user_id).await?, generation + 1);
    let mut connection = client.get_multiplexed_async_connection().await?;
    let generation_ttl: i64 = connection.ttl(feed_cache_generation_key(user_id)).await?;
    assert!((1..=600).contains(&generation_ttl));
    container.rm().await?;
    Ok(())
}
