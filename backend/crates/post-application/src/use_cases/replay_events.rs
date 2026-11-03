use crate::dto::post_dto::{ReplayRequestDto, ReplayResponseDto};
use crate::ports::{EventOutboxPort, ReplayEventDeliveryPort};
use anyhow::{ensure, Result};
use chrono::{DateTime, Utc};
use post_domain::post::{DeliveryTarget, EventReplayFilter, ReplayTarget};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct ReplayEventsUseCase {
    outbox: Arc<dyn EventOutboxPort>,
    delivery: Arc<dyn ReplayEventDeliveryPort>,
}
impl ReplayEventsUseCase {
    pub fn new(
        outbox: Arc<dyn EventOutboxPort>,
        delivery: Arc<dyn ReplayEventDeliveryPort>,
    ) -> Self {
        Self { outbox, delivery }
    }
    pub async fn execute(&self, request: ReplayRequestDto) -> Result<ReplayResponseDto> {
        let limit = request.limit.unwrap_or(100);
        ensure!(
            (1..=500).contains(&limit),
            "Replay limit must be between 1 and 500"
        );
        let filter = EventReplayFilter {
            actor_id: request
                .actor_id
                .as_deref()
                .map(Uuid::parse_str)
                .transpose()?,
            event_type: request.event_type,
            attribute: request.attribute,
            from: request
                .from
                .as_deref()
                .map(DateTime::parse_from_rfc3339)
                .transpose()?
                .map(|v| v.with_timezone(&Utc)),
            to: request
                .to
                .as_deref()
                .map(DateTime::parse_from_rfc3339)
                .transpose()?
                .map(|v| v.with_timezone(&Utc)),
            published: request.published,
        };
        let target = request.target.unwrap_or(ReplayTarget::Internal);
        let events = self.outbox.find_events(&filter, limit).await?;
        for event in &events {
            if matches!(target, ReplayTarget::Internal | ReplayTarget::Both) {
                self.delivery
                    .replay_target(event.event_id, DeliveryTarget::Internal)
                    .await?;
            }
            if matches!(target, ReplayTarget::External | ReplayTarget::Both) {
                self.delivery
                    .replay_target(event.event_id, DeliveryTarget::External)
                    .await?;
            }
        }
        Ok(ReplayResponseDto {
            replayed_count: events.len() as i64,
        })
    }
}
