use anyhow::{bail, Result};
use async_trait::async_trait;
use chrono::{DateTime, Duration, TimeZone, Utc};
use post_service::adapters::r#in::messaging::recommendation_retention_worker::FeedRecommendationRetentionWorker;
use post_service::application::ports::FeedRecommendationMaintenancePort;
use post_service::config::PostMetrics;
use post_service::domain::recommendation::FeedRecommendationRetentionResult;
use std::sync::{Arc, Mutex};

type FeedRetentionCall = (DateTime<Utc>, DateTime<Utc>, i32);

#[derive(Clone, Default)]
struct FakeMaintenanceRepository {
    calls: Arc<Mutex<Vec<FeedRetentionCall>>>,
    result: Arc<Mutex<Option<Result<FeedRecommendationRetentionResult, String>>>>,
}

#[async_trait]
impl FeedRecommendationMaintenancePort for FakeMaintenanceRepository {
    async fn purge_expired(
        &self,
        now: DateTime<Utc>,
        event_cutoff: DateTime<Utc>,
        limit: i32,
    ) -> Result<FeedRecommendationRetentionResult> {
        self.calls.lock().unwrap().push((now, event_cutoff, limit));
        match self.result.lock().unwrap().take() {
            Some(Ok(result)) => Ok(result),
            Some(Err(error)) => bail!(error),
            None => Ok(FeedRecommendationRetentionResult::default()),
        }
    }
}

#[tokio::test]
async fn feed_retention_worker_bounds_batch_and_records_rows_by_fixed_entity() {
    let repository = FakeMaintenanceRepository::default();
    *repository.result.lock().unwrap() = Some(Ok(FeedRecommendationRetentionResult {
        timeline_entries: 2,
        high_fanout_posts: 3,
        snapshot_items: 5,
        impressions: 7,
        snapshots: 11,
        events: 13,
        experiment_assignments: 17,
        embeddings: 19,
        fanout_jobs: 23,
    }));
    let metrics = Arc::new(PostMetrics::default());
    let worker = FeedRecommendationRetentionWorker::new(
        repository.clone(),
        metrics.clone(),
        Duration::days(90),
        99_999,
    );
    let now = Utc.with_ymd_and_hms(2026, 8, 4, 12, 0, 0).unwrap();

    let result = worker.tick(now).await.expect("feed retention tick");

    assert_eq!(result.total(), 100);
    assert_eq!(
        repository.calls.lock().unwrap().as_slice(),
        &[(now, now - Duration::days(90), 5_000)]
    );
    let rendered = metrics.render_recommendation_prometheus();
    assert!(rendered.contains(
        "recommendation_retention_runs_total{surface=\"home_feed\",outcome=\"success\"} 1\n"
    ));
    assert!(rendered.contains(
        "recommendation_retention_rows_total{surface=\"home_feed\",entity=\"timeline_entry\",outcome=\"success\"} 2\n"
    ));
    assert!(rendered.contains(
        "recommendation_retention_rows_total{surface=\"home_feed\",entity=\"snapshot_item\",outcome=\"success\"} 5\n"
    ));
    assert!(rendered.contains(
        "recommendation_retention_rows_total{surface=\"home_feed\",entity=\"impression\",outcome=\"success\"} 7\n"
    ));
    assert!(rendered.contains(
        "recommendation_retention_rows_total{surface=\"home_feed\",entity=\"fanout_job\",outcome=\"success\"} 23\n"
    ));
}

#[tokio::test]
async fn feed_retention_worker_records_failure_counter() {
    let repository = FakeMaintenanceRepository::default();
    *repository.result.lock().unwrap() = Some(Err("database unavailable".into()));
    let metrics = Arc::new(PostMetrics::default());
    let worker = FeedRecommendationRetentionWorker::new(
        repository,
        metrics.clone(),
        Duration::days(30),
        100,
    );

    assert!(worker.tick(Utc::now()).await.is_err());
    let rendered = metrics.render_recommendation_prometheus();
    assert!(rendered.contains(
        "recommendation_retention_runs_total{surface=\"home_feed\",outcome=\"failure\"} 1\n"
    ));
    assert!(!rendered
        .contains("recommendation_requests_total{surface=\"home_feed\",outcome=\"failure\"}"));
}

#[test]
fn account_deletion_repository_cleans_feed_recommendation_subject_tables() {
    // Cleanup lives in the deletion-receipt owner; the lifecycle repository
    // delegates to it. Inspect the actual implementation, not the delegator.
    let source = include_str!("../../src/deletion_receipt/mod.rs");
    for table in [
        "home_timeline_entries",
        "high_fanout_posts",
        "feed_recommendation_snapshots",
        "feed_recommendation_impressions",
        "feed_recommendation_events",
        "feed_author_affinities",
        "feed_recent_actions",
        "feed_experiment_assignments",
        "feed_embeddings",
    ] {
        assert!(
            source.contains(table),
            "missing lifecycle cleanup for {table}"
        );
    }
}

#[test]
fn feed_recommendation_retention_uses_skip_locked_claims() {
    let source =
        include_str!("../../src/adapters/out/persistence/feed_recommendation_repository.rs");
    let purge = source
        .split("impl FeedRecommendationMaintenancePort")
        .nth(1)
        .expect("maintenance repository implementation");

    assert!(purge.contains("delete from feed_recommendation_impressions"));
    assert!(purge.contains("delete from feed_recommendation_snapshot_items"));
    assert!(purge.contains(
        "not exists (\n                      select 1 from feed_recommendation_impressions"
    ));
    assert!(purge.contains(
        "not exists (\n                      select 1 from feed_recommendation_snapshot_items"
    ));
    assert!(
        purge.matches("skip locked").count() >= 9,
        "each bounded retention query must use SKIP LOCKED"
    );
}

#[test]
fn feed_recommendation_retention_migration_indexes_every_global_purge_order() {
    let migration = include_str!("../../migrations/0008_recommendation_retention.sql");
    for index in [
        "home_timeline_retention_idx",
        "feed_rec_impressions_snapshot_retention_idx",
        "feed_events_retention_idx",
        "feed_embeddings_retention_idx",
        "feed_fanout_terminal_retention_idx",
    ] {
        assert!(migration.contains(index), "missing retention index {index}");
    }
}
