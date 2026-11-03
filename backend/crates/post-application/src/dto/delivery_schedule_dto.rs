use chrono::{DateTime, Utc};
use post_domain::post::{DeliveryTarget, StoredEvent};
use uuid::Uuid;

/// Technical defaults are configurable, not deployment/SLO acceptance evidence.
#[derive(Clone, Debug)]
pub struct DeliveryScheduleConfigDto {
    pub external_enabled: bool,
    pub lease_duration_ms: u64,
    pub max_attempts: i32,
    pub retry_base_ms: u64,
    pub retry_cap_ms: u64,
}

impl Default for DeliveryScheduleConfigDto {
    fn default() -> Self {
        Self {
            external_enabled: false,
            lease_duration_ms: 300_000,
            max_attempts: 5,
            retry_base_ms: 1000,
            retry_cap_ms: 300_000,
        }
    }
}

impl DeliveryScheduleConfigDto {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            (1..=86_400_000).contains(&self.lease_duration_ms),
            "invalid delivery lease duration"
        );
        anyhow::ensure!(
            (1..=1000).contains(&self.max_attempts),
            "invalid delivery attempt limit"
        );
        anyhow::ensure!(
            self.retry_base_ms > 0
                && self.retry_base_ms <= self.retry_cap_ms
                && self.retry_cap_ms <= 86_400_000,
            "invalid delivery retry bounds"
        );
        Ok(())
    }
}

#[derive(Clone)]
pub struct TargetDeliveryClaimDto {
    pub event: StoredEvent,
    pub target: DeliveryTarget,
    pub lease_token: Uuid,
    pub attempt_id: Uuid,
    pub attempt_number: i32,
    pub lease_expires_at: DateTime<Utc>,
}

#[derive(Clone, Default)]
pub struct TargetDeliveryBatchDto {
    pub claims: Vec<TargetDeliveryClaimDto>,
    /// Parents finalized solely by inherited authoritative checkpoints.
    pub reconciled_published_events: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryLeaseOutcomeDto {
    Lost,
    Recorded { event_published: bool },
}

/// Snapshot from the database clock. It does not extend or reacquire a lease.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryLeaseStatusDto {
    Lost,
    Owned { remaining_ms: u64 },
}

#[derive(Clone, Debug)]
pub struct DeliveryExecutionConfigDto {
    pub schedule: DeliveryScheduleConfigDto,
    pub effect_timeout_ms: u64,
    pub completion_reserve_ms: u64,
}

impl Default for DeliveryExecutionConfigDto {
    fn default() -> Self {
        Self {
            schedule: DeliveryScheduleConfigDto::default(),
            effect_timeout_ms: 25_000,
            completion_reserve_ms: 1000,
        }
    }
}

impl DeliveryExecutionConfigDto {
    pub fn validate(&self) -> anyhow::Result<()> {
        self.schedule.validate()?;
        anyhow::ensure!(
            (1..=86_400_000).contains(&self.effect_timeout_ms),
            "invalid delivery effect timeout"
        );
        anyhow::ensure!(
            self.completion_reserve_ms > 0
                && self.completion_reserve_ms < self.schedule.lease_duration_ms,
            "delivery lease must exceed its completion reserve"
        );
        Ok(())
    }
}
