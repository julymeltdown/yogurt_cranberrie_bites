//! A single wall/monotonic budget shared across authority awaits.
use std::time::Duration;
use tonic::Status;

#[derive(Clone, Copy)]
pub struct AuthorityRpcDeadline {
    expires_micros: i64,
    stop: tokio::time::Instant,
}
impl AuthorityRpcDeadline {
    pub fn new(expires_micros: i64, ceiling: Duration) -> Result<Self, Status> {
        if ceiling.is_zero() || ceiling.as_micros() > i64::MAX as u128 {
            return Err(Status::invalid_argument("Invalid authority budget"));
        }
        let now = chrono::Utc::now().timestamp_micros();
        let expires_micros = expires_micros.min(now.saturating_add(ceiling.as_micros() as i64));
        let remaining = expires_micros.saturating_sub(now);
        if remaining <= 0 {
            return Err(Status::deadline_exceeded("Authority budget expired"));
        }
        let stop = tokio::time::Instant::now()
            .checked_add(Duration::from_micros(remaining as u64))
            .ok_or_else(|| Status::invalid_argument("Invalid authority budget"))?;
        Ok(Self {
            expires_micros,
            stop,
        })
    }
    /// Preserve an outer effect's original monotonic clock-domain stop across
    /// local owning reads. Constructor/edge ceilings can only shorten it.
    pub fn within_original(
        expires_micros: i64,
        stop: std::time::Instant,
        ceiling: Duration,
    ) -> Result<Self, Status> {
        let mut deadline = Self::new(expires_micros, ceiling)?;
        deadline.stop = deadline.stop.min(tokio::time::Instant::from_std(stop));
        deadline.remaining(ceiling, expires_micros)?;
        Ok(deadline)
    }
    pub fn stop(self) -> tokio::time::Instant {
        self.stop
    }
    pub fn expires_micros(self) -> i64 {
        self.expires_micros
    }
    /// Include channel readiness and all earlier waits. Neither a later wall
    /// clock nor a fresh dependency budget may reset this original stop time.
    pub fn remaining(self, ceiling: Duration, evidence_expiry: i64) -> Result<Duration, Status> {
        let micros = self
            .expires_micros
            .min(evidence_expiry)
            .saturating_sub(chrono::Utc::now().timestamp_micros());
        let mono = self
            .stop
            .saturating_duration_since(tokio::time::Instant::now());
        if micros <= 0 || mono.is_zero() || ceiling.is_zero() {
            return Err(Status::deadline_exceeded("Authority budget expired"));
        }
        Ok(ceiling.min(mono).min(Duration::from_micros(micros as u64)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn imported_effect_stop_cannot_be_reset_after_local_owning_waits() {
        let stop = tokio::time::Instant::now() - Duration::from_millis(1);
        let result = AuthorityRpcDeadline::within_original(
            chrono::Utc::now().timestamp_micros() + 60_000_000,
            stop.into_std(),
            Duration::from_secs(60),
        );
        assert!(matches!(result, Err(status) if status.code() == tonic::Code::DeadlineExceeded));

        let stop = tokio::time::Instant::now() + Duration::from_millis(100);
        let deadline = AuthorityRpcDeadline::within_original(
            chrono::Utc::now().timestamp_micros() + 60_000_000,
            stop.into_std(),
            Duration::from_secs(60),
        )
        .unwrap();
        assert_eq!(deadline.stop, stop);
        assert!(
            deadline
                .remaining(Duration::from_secs(60), deadline.expires_micros())
                .unwrap()
                <= Duration::from_millis(100)
        );
    }

    #[tokio::test]
    async fn original_monotonic_stop_survives_new_edge_budgets_and_live_wall_expiry() {
        let deadline = AuthorityRpcDeadline {
            expires_micros: chrono::Utc::now().timestamp_micros() + 10_000_000,
            stop: tokio::time::Instant::now() - Duration::from_millis(1),
        };
        for ceiling in [Duration::from_millis(1), Duration::from_secs(60)] {
            assert_eq!(
                deadline
                    .remaining(ceiling, deadline.expires_micros)
                    .unwrap_err()
                    .code(),
                tonic::Code::DeadlineExceeded
            );
        }
    }

    #[tokio::test]
    async fn effective_budget_clips_wall_monotonic_evidence_and_edge_limits() {
        let now = chrono::Utc::now().timestamp_micros();
        let deadline = AuthorityRpcDeadline::new(now + 60_000_000, Duration::from_secs(2)).unwrap();
        assert!(deadline.expires_micros() <= chrono::Utc::now().timestamp_micros() + 2_000_000);
        assert!(
            deadline
                .remaining(Duration::from_secs(9), now + 60_000_000)
                .unwrap()
                <= Duration::from_secs(2)
        );
        assert!(
            deadline
                .remaining(Duration::from_secs(9), now + 500_000)
                .unwrap()
                <= Duration::from_millis(500)
        );
        assert!(
            deadline
                .remaining(Duration::from_millis(5), now + 60_000_000)
                .unwrap()
                <= Duration::from_millis(5)
        );
        assert!(deadline.remaining(Duration::from_secs(9), now - 1).is_err());
        assert!(deadline
            .remaining(Duration::ZERO, now + 60_000_000)
            .is_err());
        assert!(AuthorityRpcDeadline::new(now - 1, Duration::from_secs(1)).is_err());
        assert!(AuthorityRpcDeadline::new(now + 60_000_000, Duration::ZERO).is_err());
    }
}
