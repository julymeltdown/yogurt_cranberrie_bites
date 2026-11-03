use async_trait::async_trait;
use profile_service::adapters::r#in::messaging::mutation_receipt_retention_worker::ProfileMutationReceiptRetentionWorker;
use profile_service::application::errors::{ProfileError, ProfileResult as Result};
use profile_service::application::ports::ProfileMutationReceiptMaintenancePort;
use profile_service::application::use_cases::purge_expired_mutation_outcomes::PurgeExpiredMutationOutcomesUseCase;
use profile_service::config::ProfileMetrics;
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn worker_clamps_batch_size_and_records_success_and_purge_metrics() {
    let repository = Arc::new(FakeMaintenance::default());
    *repository.purged.lock().expect("purged") = 4;
    let metrics = Arc::new(ProfileMetrics::default());
    let worker = ProfileMutationReceiptRetentionWorker::new(
        PurgeExpiredMutationOutcomesUseCase::new(Arc::clone(&repository)),
        Arc::clone(&metrics),
        9_000,
    );

    assert_eq!(worker.tick().await.expect("retention tick"), 4);
    assert_eq!(*repository.limits.lock().expect("limits"), vec![5_000]);
    let rendered = metrics.render_prometheus();
    assert!(
        rendered.contains("profile_mutation_receipt_retention_runs_total{outcome=\"success\"} 1")
    );
    assert!(rendered.contains("profile_mutation_receipt_outcome_bodies_purged_total{} 4"));
}

#[tokio::test]
async fn worker_records_failure_without_counting_purged_bodies() {
    let repository = Arc::new(FakeMaintenance::default());
    *repository.error.lock().expect("error") = Some("database unavailable".into());
    let metrics = Arc::new(ProfileMetrics::default());
    let worker = ProfileMutationReceiptRetentionWorker::new(
        PurgeExpiredMutationOutcomesUseCase::new(repository),
        Arc::clone(&metrics),
        25,
    );

    assert_eq!(
        worker
            .tick()
            .await
            .expect_err("retention failure")
            .to_string(),
        "Profile persistence operation failed"
    );
    let rendered = metrics.render_prometheus();
    assert!(
        rendered.contains("profile_mutation_receipt_retention_runs_total{outcome=\"failure\"} 1")
    );
    assert!(!rendered.contains("profile_mutation_receipt_outcome_bodies_purged_total"));
}

#[derive(Default)]
struct FakeMaintenance {
    limits: Mutex<Vec<i32>>,
    purged: Mutex<i64>,
    error: Mutex<Option<String>>,
}

#[async_trait]
impl ProfileMutationReceiptMaintenancePort for FakeMaintenance {
    async fn purge_expired_mutation_outcomes(&self, limit: i32) -> Result<i64> {
        self.limits.lock().expect("limits").push(limit);
        if self.error.lock().expect("error").is_some() {
            return Err(ProfileError::Persistence);
        }
        Ok(*self.purged.lock().expect("purged"))
    }
}
