//! INTERNAL source ownership and notification intent. These DTOs are data,
//! never Friend relationship evidence or Notification write capabilities.
use crate::delivery_authority::{
    DeliveryAuthorityErrorDto as Error, DeliveryLeaseKindDto, DeliveryReplayRestoreDto,
    OwnedDeliveryClaimDto,
};
use crate::dto::timeline_projection_dto::TimelineProjectionEventDto;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use post_domain::post::{DeliveryTarget, NotificationRequest, StoredEvent};
use uuid::Uuid;

/// Preserve exact retained source bytes, including trace and ordered attributes.
/// Publication and release metadata are mutable and are not event identity.
pub fn notification_event_identity(event: &StoredEvent) -> Result<Vec<u8>, Error> {
    #[derive(serde::Serialize)]
    struct Identity<'a> {
        event: TimelineProjectionEventDto,
        trace: &'a Option<String>,
    }
    serde_json::to_vec(&Identity {
        event: TimelineProjectionEventDto::from(event),
        trace: &event.origin_traceparent,
    })
    .map_err(|_| Error::InvalidBinding)
}

/// Explicit replay does not consume an automatic attempt for completed/skipped
/// targets. Bind its actual restore state instead of applying CACHE AUTO rules.
pub fn notification_attempt_identity(owner: &OwnedDeliveryClaimDto) -> Result<Vec<u8>, Error> {
    if owner.lookup().claim().target != DeliveryTarget::Internal {
        return Err(Error::Denied);
    }
    let p = owner.provenance();
    let (kind, restore) = match p.lease_kind {
        DeliveryLeaseKindDto::Automatic => ("AUTO", None),
        DeliveryLeaseKindDto::Replay(state) => (
            "REPLAY",
            Some(match state {
                DeliveryReplayRestoreDto::Pending => "PENDING",
                DeliveryReplayRestoreDto::Retry => "RETRY",
                DeliveryReplayRestoreDto::Succeeded => "SUCCEEDED",
                DeliveryReplayRestoreDto::Skipped => "SKIPPED",
            }),
        ),
    };
    serde_json::to_vec(&serde_json::json!({
        "target": "INTERNAL", "leaseKind": kind, "restoreState": restore,
        "source": p.source.as_str(), "resolutionId": p.resolution_id,
        "maxAttempts": p.max_attempts, "progressCredits": p.progress_credits,
        "targetRequired": p.target_required
    }))
    .map_err(|_| Error::InvalidBinding)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NotificationKindDto {
    PostCreated,
    PostLiked,
    PostCommented,
    LegacyComment,
}
impl NotificationKindDto {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PostCreated => "POST_CREATED",
            Self::PostLiked => "POST_LIKED",
            Self::PostCommented => "POST_COMMENTED",
            Self::LegacyComment => "COMMENT",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NotificationAudienceDto {
    Followers { author_id: Uuid },
    Direct { recipient_id: Uuid },
}

#[derive(Clone, PartialEq, Eq)]
pub struct NotificationIntentDto {
    event_id: Uuid,
    post_id: Uuid,
    actor_id: Uuid,
    kind: NotificationKindDto,
    occurred_at: DateTime<Utc>,
    audience: NotificationAudienceDto,
}
impl NotificationIntentDto {
    /// No expansion to FRIEND_REQUEST, SYSTEM or future notification types.
    /// Legacy COMMENT retains its payload postId independently of aggregate_id:
    /// its historical producer schema is not established by current sources.
    pub fn from_event(event: &StoredEvent) -> Result<Option<Self>, Error> {
        let kind = match event.event_type.as_str() {
            "POST_CREATED" => NotificationKindDto::PostCreated,
            "POST_LIKED" => NotificationKindDto::PostLiked,
            "POST_COMMENTED" => NotificationKindDto::PostCommented,
            "COMMENT" => NotificationKindDto::LegacyComment,
            _ => return Ok(None),
        };
        let payload: serde_json::Value =
            serde_json::from_str(&event.payload).map_err(|_| Error::Denied)?;
        let field = |name: &str| {
            payload
                .get(name)
                .and_then(serde_json::Value::as_str)
                .and_then(|s| Uuid::parse_str(s).ok())
                .filter(|id| !id.is_nil())
                .ok_or(Error::Denied)
        };
        let actor_id = event
            .actor_id
            .filter(|id| !id.is_nil())
            .ok_or(Error::Denied)?;
        let post_id = field("postId")?;
        if event.event_id.is_nil()
            || event.aggregate_id.is_nil()
            || (kind != NotificationKindDto::LegacyComment && post_id != event.aggregate_id)
            || (payload.get("actorId").is_some() && field("actorId")? != actor_id)
        {
            return Err(Error::Denied);
        }
        let audience = if kind == NotificationKindDto::PostCreated {
            if field("authorId")? != actor_id {
                return Err(Error::Denied);
            }
            NotificationAudienceDto::Followers {
                author_id: actor_id,
            }
        } else {
            let recipient_id = field("notifyUserId")?;
            // Actual application producers use authorId differently for likes
            // and comments. Historical minimal direct payloads omit it.
            if payload.get("authorId").is_some()
                && match kind {
                    NotificationKindDto::PostLiked => field("authorId")? != recipient_id,
                    NotificationKindDto::PostCommented => field("authorId")? != actor_id,
                    _ => false,
                }
            {
                return Err(Error::Denied);
            }
            NotificationAudienceDto::Direct { recipient_id }
        };
        Ok(Some(Self {
            event_id: event.event_id,
            post_id,
            actor_id,
            kind,
            occurred_at: event.occurred_at,
            audience,
        }))
    }
    pub fn post_id(&self) -> Uuid {
        self.post_id
    }
    pub fn actor_id(&self) -> Uuid {
        self.actor_id
    }
    pub fn audience(&self) -> NotificationAudienceDto {
        self.audience
    }
    /// This reference-only transfer is not a write capability. The eventual
    /// destination must independently admit the exact recipient and policy.
    pub fn notification_for(&self, recipient: Uuid) -> Result<NotificationRequest, Error> {
        if recipient.is_nil()
            || recipient == self.actor_id
            || matches!(self.audience, NotificationAudienceDto::Direct { recipient_id } if recipient_id != recipient)
        {
            return Err(Error::Denied);
        }
        Ok(NotificationRequest {
            user_id: recipient,
            notification_type: self.kind.as_str().to_owned(),
            payload_json: serde_json::json!({"payloadVersion":1,"postId":self.post_id}).to_string(),
            event_id: self.event_id,
            actor_id: Some(self.actor_id),
            occurred_at: self.occurred_at.to_rfc3339(),
        })
    }
    pub fn validate_scope(&self, scope: &NotificationScopeDto) -> Result<(), Error> {
        scope.validate()?;
        match (self.audience, scope) {
            (
                NotificationAudienceDto::Followers { author_id },
                NotificationScopeDto::FollowerPage {
                    author_id: requested,
                    ..
                },
            ) if author_id == *requested => Ok(()),
            (
                NotificationAudienceDto::Followers { .. },
                NotificationScopeDto::RecipientPolicy { recipient_id },
            ) if *recipient_id != self.actor_id => Ok(()),
            (
                NotificationAudienceDto::Direct { recipient_id },
                NotificationScopeDto::RecipientPolicy {
                    recipient_id: requested,
                },
            ) if recipient_id == *requested && recipient_id != self.actor_id => Ok(()),
            _ => Err(Error::Denied),
        }
    }
}

/// Distinct purposes. Neither a page nor a policy lookup authorizes delivery.
#[derive(Clone, PartialEq, Eq)]
pub enum NotificationScopeDto {
    FollowerPage {
        author_id: Uuid,
        cursor: Option<String>,
        limit: i32,
    },
    RecipientPolicy {
        recipient_id: Uuid,
    },
}
impl NotificationScopeDto {
    pub fn validate(&self) -> Result<(), Error> {
        match self {
            Self::FollowerPage {
                author_id,
                cursor,
                limit,
            } => {
                if author_id.is_nil() || !(1..=100).contains(limit) {
                    return Err(Error::InvalidBinding);
                }
                if let Some(c) = cursor {
                    if c.len() > 128
                        || c.trim() != c
                        || !c.split_once('|').is_some_and(|(t, id)| {
                            DateTime::parse_from_rfc3339(t).is_ok()
                                && Uuid::parse_str(id).is_ok_and(|id| !id.is_nil())
                        })
                    {
                        return Err(Error::InvalidBinding);
                    }
                }
            }
            Self::RecipientPolicy { recipient_id } if recipient_id.is_nil() => {
                return Err(Error::InvalidBinding)
            }
            _ => {}
        }
        Ok(())
    }
}

/// Shape checked, not proof. Only owner persistence may resolve this reference.
/// No Debug/serialization: raw lease credentials must not enter diagnostics.
#[derive(Clone)]
pub struct NotificationOwnerReferenceDto {
    pub event_id: Uuid,
    pub aggregate_id: Uuid,
    pub event_digest: [u8; 32],
    pub attempt_digest: [u8; 32],
    pub lease_token: Uuid,
    pub attempt_id: Uuid,
    pub attempt_number: i32,
    pub lease_expires_at: DateTime<Utc>,
    pub scope: NotificationScopeDto,
}
impl NotificationOwnerReferenceDto {
    pub fn validate(&self) -> Result<(), Error> {
        if [
            self.event_id,
            self.aggregate_id,
            self.lease_token,
            self.attempt_id,
        ]
        .iter()
        .any(Uuid::is_nil)
            || self.attempt_number < 0
        {
            return Err(Error::InvalidBinding);
        }
        self.scope.validate()
    }
}

/// Current Post observation is required only for created-post notification.
#[derive(Clone)]
pub struct NotificationPostAccessDto {
    pub post_id: Uuid,
    pub author_id: Uuid,
    pub visibility: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationVisibilityDto {
    Public,
    Friends,
}

pub struct OwnedNotificationScopeDto {
    reference: NotificationOwnerReferenceDto,
    intent: NotificationIntentDto,
    visibility: Option<NotificationVisibilityDto>,
    checked_at: DateTime<Utc>,
}
impl OwnedNotificationScopeDto {
    /// Persistence observation only. None means a currently suppressed created
    /// source. Direct notifications intentionally require no live Post lookup.
    pub fn from_persisted(
        reference: NotificationOwnerReferenceDto,
        event: &StoredEvent,
        current_post: Option<&NotificationPostAccessDto>,
        checked_at: DateTime<Utc>,
    ) -> Result<Option<Self>, Error> {
        reference.validate()?;
        if reference.event_id != event.event_id
            || reference.aggregate_id != event.aggregate_id
            || checked_at >= reference.lease_expires_at
        {
            return Err(Error::Denied);
        }
        // Match the existing listener's lifecycle check before parsing a
        // created payload: inactive/private sources are successful suppression.
        let visibility = if event.event_type == "POST_CREATED" {
            let Some(post) = current_post else {
                return Ok(None);
            };
            if post.post_id != event.aggregate_id {
                return Err(Error::Denied);
            }
            if !matches!(post.visibility.as_str(), "PUBLIC" | "FRIENDS") {
                return Ok(None);
            }
            if event.actor_id != Some(post.author_id) {
                return Err(Error::Denied);
            }
            Some(if post.visibility == "FRIENDS" {
                NotificationVisibilityDto::Friends
            } else {
                NotificationVisibilityDto::Public
            })
        } else {
            None
        };
        let intent = NotificationIntentDto::from_event(event)?.ok_or(Error::Denied)?;
        intent.validate_scope(&reference.scope)?;
        Ok(Some(Self {
            reference,
            intent,
            visibility,
            checked_at,
        }))
    }
    pub fn reference(&self) -> &NotificationOwnerReferenceDto {
        &self.reference
    }
    pub fn intent(&self) -> &NotificationIntentDto {
        &self.intent
    }
    pub fn visibility(&self) -> Option<NotificationVisibilityDto> {
        self.visibility
    }
    pub fn requires_following(&self) -> bool {
        self.visibility == Some(NotificationVisibilityDto::Friends)
    }
    pub fn checked_at(&self) -> DateTime<Utc> {
        self.checked_at
    }
}

#[async_trait]
pub trait NotificationAuthorityRepositoryPort: Send + Sync {
    /// Read-only source observation, not relationship truth or delivery proof.
    async fn resolve_notification_scope(
        &self,
        reference: &NotificationOwnerReferenceDto,
    ) -> Result<Option<OwnedNotificationScopeDto>, Error>;
}

/// Digest-only cross-service reference. The lease credential stays inside Post.
/// Like the local reference, these fields are untrusted lookup data, not proof.
#[derive(Clone)]
pub struct NotificationNetworkReferenceDto {
    pub event_id: Uuid,
    pub aggregate_id: Uuid,
    pub event_digest: [u8; 32],
    pub attempt_digest: [u8; 32],
    pub lease_digest: [u8; 32],
    pub attempt_id: Uuid,
    pub attempt_number: i32,
    pub lease_expires_at: DateTime<Utc>,
    pub scope: NotificationScopeDto,
}
impl NotificationNetworkReferenceDto {
    pub fn validate(&self) -> Result<(), Error> {
        if [self.event_id, self.aggregate_id, self.attempt_id]
            .iter()
            .any(Uuid::is_nil)
            || self.attempt_number < 0
            || self.lease_expires_at.timestamp_micros() <= 0
            || self.lease_expires_at.timestamp_subsec_nanos() % 1000 != 0
        {
            return Err(Error::InvalidBinding);
        }
        self.scope.validate()
    }
}

/// An owning observation retains its checked time even when the created source
/// is suppressed. Suppression is not an unavailable read or an unbounded ACK.
/// This remains persistence data; authenticated evidence is minted by adapters.
pub struct OwnedNotificationSourceDto {
    reference: NotificationOwnerReferenceDto,
    event: StoredEvent,
    scope: Option<OwnedNotificationScopeDto>,
    checked_at: DateTime<Utc>,
}
impl OwnedNotificationSourceDto {
    pub fn from_persisted(
        reference: NotificationOwnerReferenceDto,
        event: &StoredEvent,
        current_post: Option<&NotificationPostAccessDto>,
        checked_at: DateTime<Utc>,
    ) -> Result<Self, Error> {
        Ok(Self {
            reference: reference.clone(),
            event: event.clone(),
            scope: OwnedNotificationScopeDto::from_persisted(
                reference,
                event,
                current_post,
                checked_at,
            )?,
            checked_at,
        })
    }
    pub fn scope(&self) -> Option<&OwnedNotificationScopeDto> {
        self.scope.as_ref()
    }
    pub fn reference(&self) -> &NotificationOwnerReferenceDto {
        &self.reference
    }
    pub fn event(&self) -> &StoredEvent {
        &self.event
    }
    pub fn checked_at(&self) -> DateTime<Utc> {
        self.checked_at
    }
    pub fn into_scope(self) -> Option<OwnedNotificationScopeDto> {
        self.scope
    }
}

#[async_trait]
pub trait NotificationNetworkAuthorityRepositoryPort: Send + Sync {
    /// Must compare the digest with the stored INTERNAL lease and independently
    /// validate full source, attempt provenance, current lifecycle and expiry.
    async fn resolve_notification_source(
        &self,
        reference: &NotificationNetworkReferenceDto,
    ) -> Result<OwnedNotificationSourceDto, Error>;
}
