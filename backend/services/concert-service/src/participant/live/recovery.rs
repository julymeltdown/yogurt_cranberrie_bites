//! Restartable settlement of existing operations only. The original persisted
//! binding is not renewed; errors leave prepared work and visibility quarantined.
use super::{
    coordinator::{ParticipantProfileJournal, RecoveryCursor},
    profile::GrpcCanonicalSelfProfile,
};
use std::{sync::Arc, time::Duration};

pub struct ParticipantRecovery {
    journal: Arc<ParticipantProfileJournal>,
    profile: GrpcCanonicalSelfProfile,
}
impl ParticipantRecovery {
    pub fn new(journal: Arc<ParticipantProfileJournal>, profile: GrpcCanonicalSelfProfile) -> Self {
        Self { journal, profile }
    }
    /// Root startup owns spawning and shutdown. Reconstructing this worker after
    /// process restart scans the same durable operations, never reissues PATCH.
    pub async fn run(self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        let mut cursor: Option<RecoveryCursor> = None;
        let mut interval = tokio::time::interval(Duration::from_millis(500));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            if *shutdown.borrow() {
                return;
            }
            tokio::select! {
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() { return; }
                }
                _ = interval.tick() => {}
            }
            let page = tokio::select! {
                _ = shutdown.changed() => return,
                page = tokio::time::timeout(Duration::from_secs(2), self.journal.recovery_page(cursor)) => page,
            };
            let page = match page {
                Ok(Ok(page)) => page,
                _ => {
                    tracing::warn!(
                        "Participant recovery scan unavailable; existing operations retained"
                    );
                    continue;
                }
            };
            if page.is_empty() {
                cursor = None;
                continue;
            }
            for item in page {
                // Advance even on an unresolved operation, then wrap on the next
                // empty page. An old unknown cannot starve a newer settlement.
                cursor = Some(item);
                let operation = item.1;
                let settle = async {
                    let original = self.journal.recovery_operation(operation).await?;
                    let terminal = self.profile.recover(&original).await?;
                    self.journal.record_terminal(&terminal).await
                };
                tokio::select! {
                    _ = shutdown.changed() => return,
                    result = tokio::time::timeout(Duration::from_secs(4), settle) => {
                        if !matches!(result, Ok(Ok(()))) {
                            tracing::debug!("Participant operation remains unresolved; no visibility or authority released");
                        }
                    }
                }
            }
        }
    }
}
