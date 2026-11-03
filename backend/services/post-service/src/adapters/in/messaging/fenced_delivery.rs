use crate::application::delivery_effect::{DeliveryEffectDeadlineDto, DeliveryEffectFailure};
use crate::application::dto::delivery_schedule_dto::{
    DeliveryExecutionConfigDto, DeliveryLeaseOutcomeDto, DeliveryLeaseStatusDto,
    TargetDeliveryClaimDto,
};
use crate::application::ports::{DeliverySchedulePort, EventDeliveryMetricsPort};
use anyhow::Result;
use std::time::Duration;
use tokio::time::{timeout, Instant};

pub(crate) enum ExecutionOutcome {
    Unrecorded(&'static str),
    Recorded {
        target_succeeded: bool,
        event_published: bool,
    },
}
impl ExecutionOutcome {
    pub(crate) fn target_succeeded(&self) -> bool {
        matches!(
            self,
            Self::Recorded {
                target_succeeded: true,
                ..
            }
        )
    }
    pub(crate) fn event_published(&self) -> bool {
        matches!(
            self,
            Self::Recorded {
                event_published: true,
                ..
            }
        )
    }
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::Unrecorded(label) => label,
            Self::Recorded {
                target_succeeded: false,
                ..
            } => "target_failure",
            Self::Recorded {
                event_published: true,
                ..
            } => "published",
            Self::Recorded { .. } => "target_succeeded",
        }
    }
}

pub(crate) async fn execute<F, Fut>(
    schedule: &dyn DeliverySchedulePort,
    config: &DeliveryExecutionConfigDto,
    metrics: Option<&dyn EventDeliveryMetricsPort>,
    claim: &TargetDeliveryClaimDto,
    deliver: F,
) -> Result<ExecutionOutcome>
where
    F: FnOnce(DeliveryEffectDeadlineDto) -> Fut,
    Fut: std::future::Future<Output = Result<()>>,
{
    let reserve = Duration::from_millis(config.completion_reserve_ms);
    let observed_at = Instant::now();
    let status = timeout(reserve, schedule.lease_status(claim))
        .await
        .map_err(|_| anyhow::anyhow!("delivery lease validation timed out"))??;
    let DeliveryLeaseStatusDto::Owned { remaining_ms } = status else {
        return Ok(ExecutionOutcome::Unrecorded("lease_lost"));
    };
    // Subtract the whole observation round trip conservatively. Neither
    // remote I/O nor completion receives a new lease/deadline budget.
    let deadline = observed_at + Duration::from_millis(remaining_ms);
    let Some(effect_budget) = deadline
        .checked_duration_since(Instant::now())
        .and_then(|remaining| remaining.checked_sub(reserve))
    else {
        return Ok(ExecutionOutcome::Unrecorded("lease_budget_exhausted"));
    };
    let effect_stop =
        (deadline - reserve).min(Instant::now() + Duration::from_millis(config.effect_timeout_ms));
    let effect_budget = effect_budget.min(effect_stop.saturating_duration_since(Instant::now()));
    if effect_budget.is_zero() {
        return Ok(ExecutionOutcome::Unrecorded("lease_budget_exhausted"));
    }
    let expires_at = (chrono::Utc::now() + chrono::Duration::from_std(effect_budget)?)
        .min(claim.lease_expires_at);
    let effect_deadline = DeliveryEffectDeadlineDto::new(expires_at, effect_stop.into_std())?;
    let effect = tokio::time::timeout_at(effect_stop, deliver(effect_deadline)).await;
    if Instant::now() >= effect_stop {
        if let Some(metrics) = metrics {
            metrics.record_target_failure(claim.target);
        }
        return Ok(ExecutionOutcome::Unrecorded("effect_uncertain"));
    }
    let success = match effect {
        Ok(Ok(())) => true,
        Ok(Err(error)) => {
            if let Some(metrics) = metrics {
                metrics.record_target_failure(claim.target);
            }
            if DeliveryEffectFailure::classify(&error) == DeliveryEffectFailure::Uncertain {
                return Ok(ExecutionOutcome::Unrecorded("effect_uncertain"));
            }
            false
        }
        Err(_) => {
            if let Some(metrics) = metrics {
                metrics.record_target_failure(claim.target);
            }
            // Cancellation cannot prove that an owner/broker did not commit.
            // Preserve STARTED and the lease for crash-style expiry recovery.
            return Ok(ExecutionOutcome::Unrecorded("effect_uncertain"));
        }
    };
    let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
        return Ok(ExecutionOutcome::Unrecorded("lease_lost"));
    };
    let completion = async {
        if success {
            schedule.complete_target(claim).await
        } else {
            schedule.fail_target(claim).await
        }
    };
    let result = timeout(remaining.min(reserve), completion)
        .await
        .map_err(|_| anyhow::anyhow!("delivery completion outcome is uncertain"))??;
    Ok(match result {
        DeliveryLeaseOutcomeDto::Lost => ExecutionOutcome::Unrecorded("lease_lost"),
        DeliveryLeaseOutcomeDto::Recorded { event_published } => ExecutionOutcome::Recorded {
            target_succeeded: success,
            event_published,
        },
    })
}
