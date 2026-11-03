use crate::application::dto::delivery_schedule_dto::DeliveryExecutionConfigDto;
use crate::application::ports::{
    ExternalEventPublisherPort, InternalEventDispatcherPort, ReplayEventDeliveryPort,
    ReplaySchedulePort,
};
use crate::domain::post::DeliveryTarget;
use anyhow::{ensure, Result};
use async_trait::async_trait;
use std::sync::Arc;
use tracing::Instrument;
use uuid::Uuid;

#[derive(Clone)]
pub struct FencedReplayDelivery {
    claimed_notifications:
        Option<Arc<dyn post_application::claimed_notification::ClaimedNotificationDeliveryPort>>,
    schedule: Arc<dyn ReplaySchedulePort>,
    publisher: Arc<dyn ExternalEventPublisherPort>,
    dispatcher: Arc<dyn InternalEventDispatcherPort>,
    config: DeliveryExecutionConfigDto,
}

impl FencedReplayDelivery {
    pub fn new(
        schedule: Arc<dyn ReplaySchedulePort>,
        publisher: Arc<dyn ExternalEventPublisherPort>,
        dispatcher: Arc<dyn InternalEventDispatcherPort>,
        config: DeliveryExecutionConfigDto,
    ) -> Result<Self> {
        config.validate()?;
        Ok(Self {
            claimed_notifications: None,
            schedule,
            publisher,
            dispatcher,
            config,
        })
    }
    /// Explicit INTERNAL selection has no legacy fallback on denial/failure.
    pub fn with_claimed_notifications(
        mut self,
        notifications: Arc<
            dyn post_application::claimed_notification::ClaimedNotificationDeliveryPort,
        >,
    ) -> Self {
        self.claimed_notifications = Some(notifications);
        self
    }
}

#[async_trait]
impl ReplayEventDeliveryPort for FencedReplayDelivery {
    async fn replay_target(&self, event_id: Uuid, target: DeliveryTarget) -> Result<()> {
        ensure!(
            matches!(target, DeliveryTarget::Internal | DeliveryTarget::External),
            "unsupported replay target"
        );
        let mut config = self.config.schedule.clone();
        config.external_enabled = self.publisher.is_enabled();
        let claim = self
            .schedule
            .claim_replay_target(event_id, target, &config)
            .await?;
        ensure!(
            claim.event.event_id == event_id && claim.target == target,
            "invalid replay lease identity"
        );
        let span = tracing::info_span!(
            "post_event_replay",
            delivery_target = target.as_str(),
            outcome = tracing::field::Empty
        );
        let effect_claim = &claim;
        let effect = |deadline| async move {
            match target {
                DeliveryTarget::Internal => match &self.claimed_notifications {
                    Some(notifications) => {
                        notifications
                            .deliver_claimed_notifications(effect_claim, deadline)
                            .await
                    }
                    None => self.dispatcher.dispatch(&effect_claim.event).await,
                },
                DeliveryTarget::External => self.publisher.publish(&effect_claim.event).await,
                _ => unreachable!(),
            }
        };
        let result = super::fenced_delivery::execute(
            self.schedule.as_ref(),
            &self.config,
            None,
            &claim,
            effect,
        )
        .instrument(span.clone())
        .await?;
        span.record("outcome", result.label());
        ensure!(
            result.target_succeeded(),
            "replay delivery did not complete: {}",
            result.label()
        );
        Ok(())
    }
}
