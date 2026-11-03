use chrono::Utc;
use post_service::application::dto::post_dto::CreatePostDto;
use uuid::Uuid;

const VIEWER_ID: &str = "11111111-1111-1111-1111-111111111111";

#[tokio::test]
async fn ranked_feed_visibility_requires_outbox_and_timeline_worker_ticks() {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("app");
    let viewer_id = Uuid::parse_str(VIEWER_ID).expect("uuid");
    let public_post = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: "33333333-3333-3333-3333-333333333333".into(),
            content: "Public ranked probe".into(),
            visibility: "PUBLIC".into(),
        })
        .await
        .expect("public");
    let friends_post = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: "22222222-2222-2222-2222-222222222222".into(),
            content: "Friends ranked probe".into(),
            visibility: "FRIENDS".into(),
        })
        .await
        .expect("friends");
    let private_post = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: "22222222-2222-2222-2222-222222222222".into(),
            content: "Private ranked probe".into(),
            visibility: "PRIVATE".into(),
        })
        .await
        .expect("private");

    let feed_before_worker_ticks = app
        .state
        .get_feed
        .execute(viewer_id, None, 10)
        .await
        .expect("ranked feed before worker ticks");
    assert_eq!(
        feed_before_worker_ticks.rank_version.as_deref(),
        Some("feed-linear-v1")
    );
    assert!(!feed_before_worker_ticks.fallback_used);
    assert!(feed_before_worker_ticks
        .posts
        .iter()
        .all(|post| post.id != friends_post.id));

    assert_eq!(
        app.state
            .outbox_worker
            .tick(Utc::now(), 10)
            .await
            .expect("outbox worker tick"),
        3
    );
    assert_eq!(
        app.state
            .timeline_fanout_worker
            .tick()
            .await
            .expect("timeline fanout worker tick"),
        2 // Only PUBLIC/FRIENDS pages apply; PRIVATE work is suspended.
    );
    let private_status: String =
        sqlx::query_scalar("select status from feed_fanout_jobs where post_id=$1")
            .bind(private_post.id.parse::<Uuid>().expect("private post id"))
            .fetch_one(&app.pool)
            .await
            .expect("private fanout state");
    assert_eq!(private_status, "SUSPENDED");
    let private_entries: i64 = sqlx::query_scalar(
        "select count(*) from home_timeline_entries where post_id=$1 and viewer_id<>author_id",
    )
    .bind(private_post.id.parse::<Uuid>().expect("private post id"))
    .fetch_one(&app.pool)
    .await
    .expect("private entries visible to other viewers");
    assert_eq!(private_entries, 0);

    let ranked_feed = app
        .state
        .get_feed
        .execute(viewer_id, None, 10)
        .await
        .expect("ranked feed after worker ticks");
    let expected_ranked_ids = [public_post.id.clone(), friends_post.id.clone()];
    assert_eq!(ranked_feed.rank_version.as_deref(), Some("feed-linear-v1"));
    assert!(!ranked_feed.fallback_used);
    assert_eq!(ranked_feed.posts.len(), expected_ranked_ids.len());
    assert!(expected_ranked_ids
        .iter()
        .all(|post_id| ranked_feed.posts.iter().any(|post| &post.id == post_id)));
    assert!(expected_ranked_ids.iter().all(|post_id| {
        ranked_feed
            .items
            .iter()
            .any(|item| &item.post_id == post_id && item.source == "MATERIALIZED_TIMELINE")
    }));
    assert!(ranked_feed
        .posts
        .iter()
        .all(|post| post.id != private_post.id));
    let metrics = app.metrics().await.expect("published metrics");
    assert!(metrics
        .contains("recommendation_requests_total{surface=\"home_feed\",outcome=\"success\"} 2"));
    assert!(metrics.contains("recommendation_source_runs_total{surface=\"home_feed\",source=\"timeline\",outcome=\"success\"} 2"));
    assert!(metrics.contains(
        "recommendation_stage_latency_milliseconds_count{surface=\"home_feed\",stage=\"ranking\"} 2"
    ));
    assert!(
        !metrics.contains(VIEWER_ID),
        "metrics must not carry viewer identity"
    );
}

#[tokio::test]
async fn chronological_fallback_uses_redis_cache_after_an_explicit_cache_warmup() {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("app");
    let viewer_id = Uuid::parse_str(VIEWER_ID).expect("uuid");
    let public_post = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: "22222222-2222-2222-2222-222222222222".into(),
            content: "Chronological fallback cache probe".into(),
            visibility: "PUBLIC".into(),
        })
        .await
        .expect("post");

    let warmed = app
        .chronological_feed
        .execute(viewer_id, None, 10)
        .await
        .expect("explicit chronological fallback warmup");
    assert_eq!(warmed.rank_version, None);
    assert!(!warmed.fallback_used);
    assert!(warmed.posts.iter().any(|post| post.id == public_post.id));
    assert!(app
        .feed_cache_key_exists(viewer_id, None, 10)
        .await
        .expect("chronological cache key"));

    // Keep authoritative lifecycle/visibility state present so the cache's
    // privacy recheck still permits the post. Changing the body makes the
    // cached response distinguishable from a database refetch.
    sqlx::query(
        "update posts set content = 'Source changed after cache warmup' where id = $1::uuid",
    )
    .bind(&public_post.id)
    .execute(&app.pool)
    .await
    .expect("change source body after cache warmup");

    let cached = app
        .chronological_feed
        .execute(viewer_id, None, 10)
        .await
        .expect("explicit chronological fallback cache hit");
    let cached_post = cached
        .posts
        .iter()
        .find(|post| post.id == public_post.id)
        .expect("cached feed retains the warmed public post");
    assert_eq!(cached_post.content, "Chronological fallback cache probe");
}
