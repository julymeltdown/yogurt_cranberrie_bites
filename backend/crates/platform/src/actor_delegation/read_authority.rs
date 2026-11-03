//! Verified request-local context for bounded owner reads. Not an exportable proof.
use super::authority_deadline::AuthorityRpcDeadline;
use std::time::{Duration, Instant};
use tonic::Status;
use uuid::Uuid;

#[derive(Clone, Copy)]
pub struct VerifiedReadAuthority {
    actor: Uuid,
    proof: AuthorityRpcDeadline,
    started_at: Instant,
    original_deadline: Option<Instant>,
}

impl VerifiedReadAuthority {
    // Called only after signature, caller/audience/body/actor and replay checks.
    // Legacy methods retain their tolerated clock skew; a new strict owner read
    // receives no usable context after the actual signed expiry.
    pub(super) fn capture(
        actor: Uuid,
        expires_seconds: i64,
        started_at: Instant,
        original_deadline: Option<Instant>,
    ) -> Option<Self> {
        let proof = AuthorityRpcDeadline::new(
            expires_seconds.checked_mul(1_000_000)?,
            Duration::from_secs(super::MAX_LIFETIME_SECONDS as u64),
        )
        .ok()?;
        Some(Self {
            actor,
            proof,
            started_at,
            original_deadline,
        })
    }

    pub fn actor(self) -> Uuid {
        self.actor
    }

    /// Require an original caller deadline and preserve time spent buffering,
    /// verifying and awaiting dispatch. A handler cannot start a fresh ceiling.
    /// Both the captured proof clock and current wall expiry can only shorten it.
    pub fn deadline(self, ceiling: Duration) -> Result<AuthorityRpcDeadline, Status> {
        let original = self
            .original_deadline
            .ok_or_else(|| Status::invalid_argument("Original caller deadline required"))?;
        let resource_stop = self
            .started_at
            .checked_add(ceiling)
            .ok_or_else(|| Status::invalid_argument("Invalid authority budget"))?;
        AuthorityRpcDeadline::within_original(
            self.proof.expires_micros(),
            original
                .min(resource_stop)
                .min(self.proof.stop().into_std()),
            ceiling,
        )
    }
}

impl std::fmt::Debug for VerifiedReadAuthority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("VerifiedReadAuthority([REDACTED])")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn capture(original: Option<Instant>, started: Instant) -> VerifiedReadAuthority {
        VerifiedReadAuthority::capture(
            Uuid::new_v4(),
            chrono::Utc::now().timestamp() + 30,
            started,
            original,
        )
        .unwrap()
    }

    #[test]
    fn missing_original_deadline_is_not_replaced_with_a_fresh_budget() {
        let context = capture(None, Instant::now());
        assert!(matches!(context.deadline(Duration::from_secs(2)), Err(e)
            if e.code() == tonic::Code::InvalidArgument));
    }

    #[test]
    fn expired_and_overflowed_proof_cannot_install_context() {
        for exp in [chrono::Utc::now().timestamp() - 1, i64::MAX] {
            assert!(
                VerifiedReadAuthority::capture(Uuid::new_v4(), exp, Instant::now(), None).is_none()
            );
        }
    }

    #[test]
    fn buffering_and_dispatch_waits_consume_the_same_resource_ceiling() {
        let started = Instant::now() - Duration::from_secs(3);
        let context = capture(Some(Instant::now() + Duration::from_secs(20)), started);
        assert!(matches!(context.deadline(Duration::from_secs(2)), Err(e)
            if e.code() == tonic::Code::DeadlineExceeded));
    }

    #[test]
    fn original_transport_and_captured_proof_stops_only_shorten() {
        let start = Instant::now();
        let original = start + Duration::from_millis(100);
        let context = capture(Some(original), start);
        assert_eq!(
            context
                .deadline(Duration::from_secs(2))
                .unwrap()
                .stop()
                .into_std(),
            original
        );
        let expired = capture(Some(start - Duration::from_millis(1)), start);
        assert!(expired.deadline(Duration::from_secs(2)).is_err());
        // A captured stop, rather than a newly computed wall interval, is retained.
        assert!(context.deadline(Duration::from_secs(2)).unwrap().stop() <= context.proof.stop());
    }

    #[tokio::test]
    async fn captured_proof_monotonic_expiry_survives_a_later_live_wall_interval() {
        let start = Instant::now();
        let mut context = capture(Some(start + Duration::from_secs(20)), start);
        context.proof = AuthorityRpcDeadline::within_original(
            chrono::Utc::now().timestamp_micros() + 20_000_000,
            start + Duration::from_millis(10),
            Duration::from_secs(20),
        )
        .unwrap();
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(context.proof.expires_micros() > chrono::Utc::now().timestamp_micros());
        assert!(matches!(context.deadline(Duration::from_secs(2)), Err(e)
            if e.code() == tonic::Code::DeadlineExceeded));
    }

    #[test]
    fn debug_does_not_export_the_actor_or_expiry() {
        let context = capture(
            Some(Instant::now() + Duration::from_secs(1)),
            Instant::now(),
        );
        assert_eq!(format!("{context:?}"), "VerifiedReadAuthority([REDACTED])");
    }
}
