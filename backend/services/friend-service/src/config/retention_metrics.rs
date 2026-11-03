use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub struct MutationReceiptRetentionMetrics {
    inner: Arc<Mutex<MutationReceiptRetentionCounters>>,
}

#[derive(Default)]
struct MutationReceiptRetentionCounters {
    successful_runs: u64,
    failed_runs: u64,
    purged_outcome_bodies: u64,
}

impl MutationReceiptRetentionMetrics {
    pub fn record_success(&self, purged_outcome_bodies: i64) {
        let mut counters = self
            .inner
            .lock()
            .expect("mutation receipt retention metrics");
        counters.successful_runs = counters.successful_runs.saturating_add(1);
        counters.purged_outcome_bodies = counters
            .purged_outcome_bodies
            .saturating_add(purged_outcome_bodies.max(0) as u64);
    }

    pub fn record_failure(&self) {
        let mut counters = self
            .inner
            .lock()
            .expect("mutation receipt retention metrics");
        counters.failed_runs = counters.failed_runs.saturating_add(1);
    }

    pub fn render_prometheus(&self) -> String {
        let counters = self
            .inner
            .lock()
            .expect("mutation receipt retention metrics");
        format!(
            "# HELP friend_mutation_receipt_retention_runs_total Receipt outcome retention ticks by result.\n\
             # TYPE friend_mutation_receipt_retention_runs_total counter\n\
             friend_mutation_receipt_retention_runs_total{{outcome=\"success\"}} {}\n\
             friend_mutation_receipt_retention_runs_total{{outcome=\"failure\"}} {}\n\
             # HELP friend_mutation_receipt_outcome_bodies_purged_total Expired FriendService receipt outcome bodies cleared.\n\
             # TYPE friend_mutation_receipt_outcome_bodies_purged_total counter\n\
             friend_mutation_receipt_outcome_bodies_purged_total {}\n",
            counters.successful_runs,
            counters.failed_runs,
            counters.purged_outcome_bodies,
        )
    }
}
