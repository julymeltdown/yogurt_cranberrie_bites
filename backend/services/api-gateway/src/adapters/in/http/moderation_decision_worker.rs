//! Restart-safe reconciliation for Gateway-owned content moderation decisions.

use crate::application::service::ModerationService;
use crate::config::GatewayMetrics;
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_millis(500);
const METRICS_REFRESH_INTERVAL: Duration = Duration::from_secs(15);

pub async fn run(service: ModerationService, metrics: GatewayMetrics) -> anyhow::Result<()> {
    let mut last_metrics_refresh = None;
    platform::shutdown::periodic("gateway-moderation-decision", POLL_INTERVAL, move || {
        let service = service.clone();
        let metrics = metrics.clone();
        let refresh_metrics = last_metrics_refresh
            .map(|last: Instant| last.elapsed() >= METRICS_REFRESH_INTERVAL)
            .unwrap_or(true);
        if refresh_metrics {
            last_metrics_refresh = Some(Instant::now());
        }
        async move {
            if refresh_metrics {
                match service.content_decision_backlog().await {
                    Ok(backlog) => metrics.set_content_moderation_backlog(
                        backlog.pending,
                        backlog.oldest_pending_age_seconds,
                        backlog.target_unavailable,
                        backlog.retry_exhausted,
                        chrono::Utc::now().timestamp(),
                    ),
                    Err(error) => tracing::warn!(
                        error = ?error,
                        "failed to refresh content moderation outbox metrics"
                    ),
                }
            }
            service.reconcile_one_content_decision().await?;
            Ok(())
        }
    })
    .await
    .map_err(|error| anyhow::anyhow!("Gateway moderation decision worker stopped: {error}"))
}
