use crate::application::ports::{UserDeletionDispatcherPort, UserDeletionSagaPort};
use chrono::{DateTime, Duration, Utc};
use platform::trace_context::{self, TraceContext};
use std::time::Duration as StdDuration;
use tracing::{field, Instrument};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UserDeletionWorkerPolicy {
    pub batch_size: i32,
    pub max_attempts: i32,
    pub lease_timeout: Duration,
    pub base_retry: Duration,
}

impl UserDeletionWorkerPolicy {
    pub fn validate(self) -> anyhow::Result<Self> {
        if !(1..=100).contains(&self.batch_size) {
            anyhow::bail!("User deletion worker batch size must be between 1 and 100");
        }
        if self.max_attempts < 1 {
            anyhow::bail!("User deletion worker max attempts must be positive");
        }
        if self.lease_timeout <= Duration::zero() || self.base_retry <= Duration::zero() {
            anyhow::bail!("User deletion worker durations must be positive");
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UserDeletionWorkerRun {
    pub claimed: i64,
    pub succeeded: i64,
    pub retried: i64,
    pub dead_lettered: i64,
}

#[derive(Clone)]
pub struct UserDeletionWorker<R, D> {
    repository: R,
    dispatcher: D,
    policy: UserDeletionWorkerPolicy,
}

impl<R, D> UserDeletionWorker<R, D>
where
    R: UserDeletionSagaPort,
    D: UserDeletionDispatcherPort,
{
    pub fn new(repository: R, dispatcher: D, policy: UserDeletionWorkerPolicy) -> Self {
        Self {
            repository,
            dispatcher,
            policy: policy
                .validate()
                .expect("valid user deletion worker policy"),
        }
    }

    pub async fn run_once(&self, now: DateTime<Utc>) -> anyhow::Result<UserDeletionWorkerRun> {
        let steps = self
            .repository
            .claim_due(now, self.policy.lease_timeout, self.policy.batch_size)
            .await?
            .into_iter()
            .map(crate::domain::user_deletion::UserDeletionStep::from)
            .collect::<Vec<_>>();
        let mut result = UserDeletionWorkerRun {
            claimed: steps.len() as i64,
            ..Default::default()
        };
        for step in steps {
            let mut context = step
                .origin_traceparent
                .as_deref()
                .and_then(TraceContext::child_from_traceparent);
            let dispatch_span = tracing::info_span!(
                parent: None,
                "auth_user_deletion_dispatch",
                dispatch_target = step.target.as_str(),
                trace_id = field::Empty,
                span_id = field::Empty,
                parent_span_id = field::Empty,
                sampled = field::Empty,
                outcome = field::Empty,
            );
            if let Some(context) = context.as_mut() {
                context.bind_to_span(&dispatch_span);
                context.record_on_span(&dispatch_span);
            }
            let dispatch = self.dispatcher.apply(
                (step.target).into(),
                step.saga_id,
                step.user_id,
                step.actor_id,
                step.retention_until,
            );
            let dispatch_result = match context {
                Some(context) => {
                    trace_context::scope(context, dispatch.instrument(dispatch_span.clone())).await
                }
                None => dispatch.instrument(dispatch_span.clone()).await,
            };
            match dispatch_result {
                Ok(()) => {
                    dispatch_span.record("outcome", "success");
                    self.repository
                        .mark_step_succeeded(&(&step).into(), now)
                        .await?;
                    result.succeeded += 1;
                }
                Err(error) => {
                    dispatch_span.record("outcome", "dependency_error");
                    // claim_due has already incremented this dispatch's
                    // attempt_count. Do not count the same attempt twice.
                    let dead_lettered = step.attempt_count >= self.policy.max_attempts;
                    let exponent =
                        u32::try_from(step.attempt_count.saturating_sub(1).min(20)).unwrap_or(20);
                    let multiplier = 1_i32.checked_shl(exponent).unwrap_or(i32::MAX);
                    let next_attempt_at = now
                        + self
                            .policy
                            .base_retry
                            .checked_mul(multiplier)
                            .unwrap_or(Duration::days(1));
                    self.repository
                        .mark_step_failed(
                            &(&step).into(),
                            &safe_error(&error),
                            next_attempt_at,
                            dead_lettered,
                            now,
                        )
                        .await?;
                    if dead_lettered {
                        result.dead_lettered += 1;
                    } else {
                        result.retried += 1;
                    }
                }
            }
        }
        Ok(result)
    }

    pub async fn run(&self, interval: StdDuration) -> anyhow::Result<()> {
        platform::shutdown::periodic("auth-user-deletion", interval, || self.run_once(Utc::now()))
            .await
    }
}

fn safe_error(error: &crate::application::errors::AuthError) -> String {
    error.to_string().chars().take(512).collect()
}
