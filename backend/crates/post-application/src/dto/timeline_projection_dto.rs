use anyhow::{ensure, Result};
use chrono::{DateTime, Utc};
use post_domain::post::StoredEvent;
use post_domain::ranking::FeedEngagementAction;
use serde::Serialize;
use uuid::Uuid;

/// Only immutable owner-event fields participate in projection identity.
#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct TimelineProjectionEventDto {
    pub event_id: Uuid,
    pub event_type: String,
    pub aggregate_id: Uuid,
    pub aggregate_version: Option<i64>,
    pub actor_id: Option<Uuid>,
    pub attributes: Vec<String>,
    pub payload: String,
    pub occurred_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}
impl From<&StoredEvent> for TimelineProjectionEventDto {
    fn from(event: &StoredEvent) -> Self {
        Self {
            event_id: event.event_id,
            event_type: event.event_type.clone(),
            aggregate_id: event.aggregate_id,
            aggregate_version: event.aggregate_version,
            actor_id: event.actor_id,
            attributes: event.attributes.clone(),
            payload: event.payload.clone(),
            occurred_at: event.occurred_at,
            created_at: event.created_at,
        }
    }
}
#[derive(Clone, PartialEq, Eq)]
pub enum TimelineProjectionEffectDto {
    Create {
        author_id: Uuid,
    },
    Delete {
        author_id: Uuid,
    },
    Engagement {
        actor_id: Uuid,
        author_id: Uuid,
        action: FeedEngagementAction,
    },
    /// Existing PostgreSQL unlike writer stores authorId=actor and omits the
    /// post-owner fields. Resolve the owner from the locked post, never a fallback.
    StoredUnlike {
        actor_id: Uuid,
    },
    Noop,
}
impl TimelineProjectionEffectDto {
    pub fn subjects(&self) -> Vec<Uuid> {
        match *self {
            Self::Create { author_id } | Self::Delete { author_id } => vec![author_id],
            Self::Engagement {
                actor_id,
                author_id,
                ..
            } => vec![actor_id, author_id],
            Self::StoredUnlike { actor_id } => vec![actor_id],
            Self::Noop => vec![],
        }
    }
}
impl TimelineProjectionEventDto {
    /// Producer-specific author semantics; missing or conflicting fields fail closed.
    pub fn effect(&self) -> Result<TimelineProjectionEffectDto> {
        use TimelineProjectionEffectDto::*;
        if !matches!(
            self.event_type.as_str(),
            "POST_CREATED"
                | "POST_DELETED"
                | "POST_LIKED"
                | "POST_UNLIKED"
                | "POST_COMMENTED"
                | "POST_COMMENT_DELETED"
        ) {
            anyhow::bail!(
                "Unsupported timeline event type requires an explicit projection contract"
            );
        }
        let payload: serde_json::Value = serde_json::from_str(&self.payload)?;
        let field = |name: &str| -> Result<Uuid> {
            Ok(Uuid::parse_str(
                payload
                    .get(name)
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| anyhow::anyhow!("Missing projection {name}"))?,
            )?)
        };
        ensure!(
            field("postId")? == self.aggregate_id,
            "Projection aggregate does not match postId"
        );
        let actor = self
            .actor_id
            .ok_or_else(|| anyhow::anyhow!("Missing projection actor"))?;
        if payload.get("actorId").is_some() {
            ensure!(field("actorId")? == actor, "Conflicting projection actor");
        }
        if self.event_type == "POST_UNLIKED"
            && payload.get("actorId").is_none()
            && payload.get("notifyUserId").is_none()
        {
            ensure!(
                field("authorId")? == actor,
                "Conflicting stored unlike actor"
            );
            return Ok(StoredUnlike { actor_id: actor });
        }
        Ok(match self.event_type.as_str() {
            "POST_CREATED" | "POST_DELETED" => {
                let author_id = field("authorId")?;
                ensure!(author_id == actor, "Conflicting projection owner");
                if self.event_type == "POST_CREATED" {
                    Create { author_id }
                } else {
                    Delete { author_id }
                }
            }
            kind => {
                let (author_id, action) = match kind {
                    "POST_COMMENTED" => {
                        ensure!(field("authorId")? == actor, "Conflicting comment actor");
                        (field("notifyUserId")?, FeedEngagementAction::Comment)
                    }
                    "POST_COMMENT_DELETED" => {
                        ensure!(field("authorId")? == actor, "Conflicting comment actor");
                        (field("postAuthorId")?, FeedEngagementAction::DeleteComment)
                    }
                    "POST_LIKED" => (field("authorId")?, FeedEngagementAction::Like),
                    _ => (field("authorId")?, FeedEngagementAction::Unlike),
                };
                if payload.get("notifyUserId").is_some() {
                    ensure!(
                        field("notifyUserId")? == author_id,
                        "Conflicting projection notification owner"
                    );
                }
                Engagement {
                    actor_id: actor,
                    author_id,
                    action,
                }
            }
        })
    }
}
#[derive(Clone, Debug)]
pub struct TimelineProjectionWorkDto {
    pub event: TimelineProjectionEventDto,
    pub effect: TimelineProjectionEffectDto,
}
#[derive(Clone, Debug)]
pub enum TimelineProjectionPlanDto {
    Duplicate,
    Events(Vec<TimelineProjectionWorkDto>),
}
#[derive(Clone)]
pub struct TimelineProjectionApplyDto {
    pub authority: Option<crate::dto::delivery_schedule_dto::TargetDeliveryClaimDto>,
    pub work: TimelineProjectionWorkDto,
    pub follower_count: Option<i64>,
    pub high_fanout_threshold: i64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimelineProjectionOutcomeDto {
    Applied,
    Duplicate,
    Adopted,
    Suppressed,
}

impl std::fmt::Debug for TimelineProjectionEventDto {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TimelineProjectionEventDto")
            .field("details", &"[REDACTED]")
            .finish()
    }
}
impl std::fmt::Debug for TimelineProjectionEffectDto {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Create { .. } => "Create",
            Self::Delete { .. } => "Delete",
            Self::Engagement { .. } => "Engagement",
            Self::StoredUnlike { .. } => "StoredUnlike",
            Self::Noop => "Noop",
        })
    }
}
#[cfg(test)]
mod privacy_tests {
    use super::*;
    #[test]
    fn projection_debug_omits_payload_actor_and_owner_identity() {
        let actor = Uuid::new_v4();
        let owner = Uuid::new_v4();
        let event = TimelineProjectionEventDto {
            event_id: Uuid::new_v4(),
            event_type: "POST_LIKED".into(),
            aggregate_id: Uuid::new_v4(),
            aggregate_version: Some(1),
            actor_id: Some(actor),
            attributes: vec!["private-attribute".into()],
            payload: "private-projection-payload".into(),
            occurred_at: Utc::now(),
            created_at: Utc::now(),
        };
        let plan = TimelineProjectionPlanDto::Events(vec![TimelineProjectionWorkDto {
            event,
            effect: TimelineProjectionEffectDto::Engagement {
                actor_id: actor,
                author_id: owner,
                action: FeedEngagementAction::Like,
            },
        }]);
        let rendered = format!("{plan:?}");
        for secret in [
            actor.to_string(),
            owner.to_string(),
            "private-attribute".into(),
            "private-projection-payload".into(),
        ] {
            assert!(!rendered.contains(&secret));
        }
    }
}
