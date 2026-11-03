use crate::dto::post_dto::{ResolveDeadLetterReplayRequestDto, ResolveDeadLetterReplayResponseDto};
use crate::ports::EventOutboxPort;
use chrono::Utc;
use post_domain::post::{
    DeadLetterReplayError, DeadLetterReplayOutcome, DeadLetterReplayResolution,
};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct ResolveDeadLetterReplayUseCase {
    outbox: Arc<dyn EventOutboxPort>,
}

impl ResolveDeadLetterReplayUseCase {
    pub fn new(outbox: Arc<dyn EventOutboxPort>) -> Self {
        Self { outbox }
    }

    pub async fn execute(
        &self,
        request: ResolveDeadLetterReplayRequestDto,
    ) -> anyhow::Result<ResolveDeadLetterReplayResponseDto> {
        let event_id = parse_uuid(&request.event_id, "Event ID must be a UUID")?;
        let resolution_id = parse_uuid(&request.resolution_id, "Resolution ID must be a UUID")?;
        let actor_id = parse_uuid(&request.actor_id, "Actor ID must be a UUID")?;
        let reason = request.reason.trim();
        if !(20..=1024).contains(&reason.chars().count())
            || reason.len() > 1024
            || reason.chars().any(char::is_control)
        {
            return Err(anyhow::Error::new(DeadLetterReplayError::InvalidReason));
        }
        let evidence_reference = request.evidence_reference.trim();
        if !(3..=256).contains(&evidence_reference.len())
            || !evidence_reference.is_ascii()
            || !evidence_reference
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._:/#-".contains(&byte))
            || !evidence_reference
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphanumeric)
        {
            return Err(anyhow::Error::new(
                DeadLetterReplayError::InvalidEvidenceReference,
            ));
        }

        let resolution = DeadLetterReplayResolution {
            resolution_id,
            event_id,
            actor_id,
            reason: reason.to_owned(),
            evidence_reference: evidence_reference.to_owned(),
            created_at: Utc::now(),
        };
        let outcome = self.outbox.resolve_dead_letter(&resolution).await?;
        Ok(ResolveDeadLetterReplayResponseDto {
            event_id: event_id.to_string(),
            resolution_id: resolution_id.to_string(),
            created: outcome == DeadLetterReplayOutcome::Created,
        })
    }
}

fn parse_uuid(value: &str, message: &str) -> anyhow::Result<Uuid> {
    Uuid::parse_str(value.trim()).map_err(|_| anyhow::anyhow!(message.to_owned()))
}
