use anyhow::{bail, Result};
use async_trait::async_trait;
use chrono::{DateTime, Duration, TimeZone, Utc};
use friend_service::adapters::r#in::messaging::recommendation_retention_worker::PeopleRecommendationRetentionWorker;
use friend_service::application::ports::PeopleRecommendationMaintenancePort;
use friend_service::domain::recommendation::PeopleRecommendationRetentionResult;
use platform::recommendation_metrics::RecommendationMetrics;
use std::sync::{Arc, Mutex};

type PeopleRetentionCall = (DateTime<Utc>, DateTime<Utc>, i32);

#[derive(Clone, Default)]
struct FakeMaintenanceRepository {
    calls: Arc<Mutex<Vec<PeopleRetentionCall>>>,
    result: Arc<Mutex<Option<Result<PeopleRecommendationRetentionResult, String>>>>,
}

#[async_trait]
impl PeopleRecommendationMaintenancePort for FakeMaintenanceRepository {
    async fn purge_expired(
        &self,
        now: DateTime<Utc>,
        event_cutoff: DateTime<Utc>,
        limit: i32,
    ) -> Result<PeopleRecommendationRetentionResult> {
        self.calls.lock().unwrap().push((now, event_cutoff, limit));
        match self.result.lock().unwrap().take() {
            Some(Ok(result)) => Ok(result),
            Some(Err(error)) => bail!(error),
            None => Ok(PeopleRecommendationRetentionResult::default()),
        }
    }
}

#[tokio::test]
async fn retention_worker_bounds_batch_and_records_fixed_label_metrics() {
    let repository = FakeMaintenanceRepository::default();
    *repository.result.lock().unwrap() = Some(Ok(PeopleRecommendationRetentionResult {
        candidates: 2,
        snapshot_items: 3,
        impressions: 5,
        snapshots: 7,
        events: 11,
        experiment_assignments: 13,
        embeddings: 17,
        refresh_jobs: 19,
    }));
    let metrics = Arc::new(RecommendationMetrics::default());
    let worker = PeopleRecommendationRetentionWorker::new(
        repository.clone(),
        metrics.clone(),
        Duration::days(90),
        50_000,
    );
    let now = Utc.with_ymd_and_hms(2026, 8, 4, 12, 0, 0).unwrap();

    let result = worker.tick(now).await.expect("retention tick");

    assert_eq!(result.total(), 77);
    assert_eq!(
        repository.calls.lock().unwrap().as_slice(),
        &[(now, now - Duration::days(90), 5_000)]
    );
    let rendered = metrics.render_prometheus();
    assert!(rendered.contains(
        "recommendation_retention_runs_total{surface=\"people\",outcome=\"success\"} 1\n"
    ));
    assert!(rendered.contains(
        "recommendation_retention_rows_total{surface=\"people\",entity=\"candidate\",outcome=\"success\"} 2\n"
    ));
    assert!(rendered.contains(
        "recommendation_retention_rows_total{surface=\"people\",entity=\"snapshot_item\",outcome=\"success\"} 3\n"
    ));
    assert!(rendered.contains(
        "recommendation_retention_rows_total{surface=\"people\",entity=\"impression\",outcome=\"success\"} 5\n"
    ));
    assert!(rendered.contains(
        "recommendation_retention_rows_total{surface=\"people\",entity=\"embedding\",outcome=\"success\"} 17\n"
    ));
}

#[tokio::test]
async fn retention_worker_records_failure_without_claiming_success_rows() {
    let repository = FakeMaintenanceRepository::default();
    *repository.result.lock().unwrap() = Some(Err("database unavailable".into()));
    let metrics = Arc::new(RecommendationMetrics::default());
    let worker = PeopleRecommendationRetentionWorker::new(
        repository,
        metrics.clone(),
        Duration::days(30),
        100,
    );

    assert!(worker.tick(Utc::now()).await.is_err());
    let rendered = metrics.render_prometheus();
    assert!(rendered.contains(
        "recommendation_retention_runs_total{surface=\"people\",outcome=\"failure\"} 1\n"
    ));
    assert!(
        !rendered.contains("recommendation_requests_total{surface=\"people\",outcome=\"failure\"}")
    );
    assert!(!rendered.contains("entity=\"candidate\",outcome=\"success\""));
}

#[test]
fn account_deletion_repository_cleans_all_people_recommendation_subject_tables() {
    let source = include_str!("../../src/adapters/out/persistence/follow_repository.rs");
    for table in [
        "people_recommendation_candidates",
        "people_recommendation_snapshots",
        "people_recommendation_impressions",
        "people_recommendation_events",
        "people_recommendation_refresh_jobs",
        "people_experiment_assignments",
        "people_embeddings",
    ] {
        assert!(
            source.contains(table),
            "missing lifecycle cleanup for {table}"
        );
    }
}

#[test]
fn people_recommendation_retention_uses_skip_locked_claims() {
    let source =
        include_str!("../../src/adapters/out/persistence/people_recommendation_repository.rs");
    let purge = source
        .split("impl PeopleRecommendationMaintenancePort")
        .nth(1)
        .expect("maintenance repository implementation");

    assert!(purge.contains("delete from people_recommendation_impressions"));
    assert!(purge.contains("delete from people_recommendation_snapshot_items"));
    assert!(purge.contains(
        "not exists (\n                      select 1 from people_recommendation_impressions"
    ));
    assert!(purge.contains(
        "not exists (\n                      select 1 from people_recommendation_snapshot_items"
    ));
    assert!(
        purge.matches("skip locked").count() >= 8,
        "each bounded retention query must use SKIP LOCKED"
    );
}

#[test]
fn people_recommendation_retention_migration_indexes_every_global_purge_order() {
    let migration = include_str!("../../migrations/0009_recommendation_retention.sql");
    for index in [
        "people_candidates_retention_idx",
        "people_snapshots_retention_idx",
        "people_impressions_snapshot_retention_idx",
        "people_events_retention_idx",
        "people_embeddings_retention_idx",
        "people_refresh_dead_retention_idx",
    ] {
        assert!(migration.contains(index), "missing retention index {index}");
    }
}
