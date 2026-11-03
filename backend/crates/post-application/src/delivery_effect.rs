use std::fmt;

/// An adapter may confirm failure only when no effect of this target attempt
/// could have committed. Timeouts, lost acknowledgements and partial fanout are
/// uncertain. An unclassified error is also uncertain, never proof of absence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryEffectFailure {
    ConfirmedNoEffect,
    Uncertain,
}

impl fmt::Display for DeliveryEffectFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ConfirmedNoEffect => "delivery failed before any effect",
            Self::Uncertain => "delivery effect outcome is uncertain",
        })
    }
}

impl std::error::Error for DeliveryEffectFailure {}

impl DeliveryEffectFailure {
    pub fn confirmed(error: anyhow::Error) -> anyhow::Error {
        if error.downcast_ref::<Self>() == Some(&Self::Uncertain) {
            return error;
        }
        error.context(Self::ConfirmedNoEffect)
    }

    pub fn uncertain(error: anyhow::Error) -> anyhow::Error {
        error.context(Self::Uncertain)
    }

    pub fn classify(error: &anyhow::Error) -> Self {
        // An outer uncertain context supersedes a nested pre-effect failure,
        // e.g. a later recipient failing after earlier fanout effects committed.
        error
            .downcast_ref::<Self>()
            .copied()
            .unwrap_or(Self::Uncertain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shorter_wall_expiry_freezes_monotonic_stop_without_extending_original() {
        let now = std::time::Instant::now();
        let parent = DeliveryEffectDeadlineDto::new(
            chrono::Utc::now() + chrono::Duration::seconds(60),
            now + std::time::Duration::from_secs(60),
        )
        .unwrap();
        let wall = chrono::Utc::now() + chrono::Duration::milliseconds(100);
        let short = parent.clipped_to(wall);
        assert_eq!(short.expires_at(), wall);
        assert!(short.monotonic_stop() <= now + std::time::Duration::from_millis(110));
        assert!(short.monotonic_stop() < parent.monotonic_stop());
        let already = DeliveryEffectDeadlineDto::new(
            parent.expires_at(),
            now - std::time::Duration::from_secs(1),
        )
        .unwrap();
        assert_eq!(
            already.clipped_to(wall).monotonic_stop(),
            already.monotonic_stop()
        );
        let expired = parent.clipped_to(chrono::Utc::now() - chrono::Duration::seconds(1));
        assert!(expired.monotonic_stop() <= std::time::Instant::now());
    }

    #[test]
    fn unknown_and_nested_uncertain_errors_cannot_be_confirmed() {
        let unknown = anyhow::anyhow!("unclassified result");
        assert_eq!(
            DeliveryEffectFailure::classify(&unknown),
            DeliveryEffectFailure::Uncertain
        );
        let no_effect = DeliveryEffectFailure::confirmed(unknown);
        assert_eq!(
            DeliveryEffectFailure::classify(&no_effect),
            DeliveryEffectFailure::ConfirmedNoEffect
        );
        let partial = DeliveryEffectFailure::uncertain(no_effect).context("later recipient failed");
        let cannot_downgrade = DeliveryEffectFailure::confirmed(partial);
        assert_eq!(
            DeliveryEffectFailure::classify(&cannot_downgrade),
            DeliveryEffectFailure::Uncertain
        );
    }
}

/// Data handed from the owning delivery fence to a required effect capability.
/// The monotonic stop is opaque here: adapters must retain it in their runtime's
/// clock domain, rather than granting a fresh budget after earlier awaits.
#[derive(Clone, Copy)]
pub struct DeliveryEffectDeadlineDto {
    expires_at: chrono::DateTime<chrono::Utc>,
    stop: std::time::Instant,
}
impl DeliveryEffectDeadlineDto {
    pub fn new(
        expires_at: chrono::DateTime<chrono::Utc>,
        stop: std::time::Instant,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            expires_at.timestamp_micros() > 0,
            "Invalid delivery effect deadline"
        );
        Ok(Self { expires_at, stop })
    }
    pub fn expires_at(self) -> chrono::DateTime<chrono::Utc> {
        self.expires_at
    }
    pub fn monotonic_stop(self) -> std::time::Instant {
        self.stop
    }
    pub fn clipped_to(self, expires_at: chrono::DateTime<chrono::Utc>) -> Self {
        let expires_at = self.expires_at.min(expires_at);
        let remaining = (expires_at - chrono::Utc::now())
            .to_std()
            .unwrap_or_default();
        let stop = std::time::Instant::now()
            .checked_add(remaining)
            .unwrap_or(self.stop);
        Self {
            expires_at,
            stop: self.stop.min(stop),
        }
    }
}
