//! Required full-claim notification delivery. No transport/framework imports.
use crate::{
    delivery_authority::{
        DeliveryAuthorityRepositoryPort, DeliveryClaimLookupDto, OwnedDeliveryClaimDto,
    },
    delivery_effect::{DeliveryEffectDeadlineDto, DeliveryEffectFailure},
    dto::{
        delivery_schedule_dto::TargetDeliveryClaimDto,
        timeline_fanout_dto::{validate_fanout_page_progress, TimelineFanoutPageDto},
    },
    notification_authority::{
        NotificationAudienceDto, NotificationIntentDto, NotificationScopeDto,
    },
    ports::PostRepositoryPort,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use post_domain::post::{DeliveryTarget, NotificationRequest};
use std::{collections::HashSet, sync::Arc};
use uuid::Uuid;

#[derive(Clone)]
pub struct NotificationPageRequestDto {
    owner: OwnedDeliveryClaimDto,
    author: Uuid,
    cursor: Option<String>,
    limit: i32,
    deadline: DeliveryEffectDeadlineDto,
}
impl NotificationPageRequestDto {
    pub fn new(
        owner: OwnedDeliveryClaimDto,
        author: Uuid,
        cursor: Option<String>,
        limit: i32,
        deadline: DeliveryEffectDeadlineDto,
    ) -> anyhow::Result<Self> {
        NotificationScopeDto::FollowerPage {
            author_id: author,
            cursor: cursor.clone(),
            limit,
        }
        .validate()?;
        anyhow::ensure!(
            owner.lookup().claim().target == DeliveryTarget::Internal
                && deadline.expires_at() <= owner.expires_at(),
            "Invalid notification page claim"
        );
        Ok(Self {
            owner,
            author,
            cursor,
            limit,
            deadline,
        })
    }
    pub fn owner(&self) -> &OwnedDeliveryClaimDto {
        &self.owner
    }
    pub fn author(&self) -> Uuid {
        self.author
    }
    pub fn cursor(&self) -> Option<&str> {
        self.cursor.as_deref()
    }
    pub fn limit(&self) -> i32 {
        self.limit
    }
    pub fn deadline(&self) -> DeliveryEffectDeadlineDto {
        self.deadline
    }
}
/// Verified transport data, never an application-minted write capability.
pub struct SelectedNotificationPageDto {
    author: Uuid,
    recipients: Vec<Uuid>,
    next: Option<String>,
    witness: String,
    expires_at: DateTime<Utc>,
}
impl SelectedNotificationPageDto {
    pub fn new(
        request: &NotificationPageRequestDto,
        author: Uuid,
        recipients: Vec<Uuid>,
        next: Option<String>,
        witness: String,
        expires_at: DateTime<Utc>,
    ) -> anyhow::Result<Self> {
        let page = Self {
            author,
            recipients,
            next,
            witness,
            expires_at,
        };
        page.validate(request)?;
        Ok(page)
    }
    pub fn validate(&self, r: &NotificationPageRequestDto) -> anyhow::Result<()> {
        let mut seen = HashSet::new();
        anyhow::ensure!(
            self.author == r.author
                && self.recipients.len() <= r.limit as usize
                && self
                    .recipients
                    .iter()
                    .all(|id| !id.is_nil() && *id != r.author && seen.insert(*id))
                && !self.witness.is_empty()
                && self.witness.len() <= 8192
                && self.expires_at <= r.deadline.expires_at()
                && Utc::now() < self.expires_at,
            "Invalid selected notification page"
        );
        validate_fanout_page_progress(
            r.cursor(),
            &TimelineFanoutPageDto {
                follower_ids: self.recipients.clone(),
                next_cursor: self.next.clone(),
                expires_at: self.expires_at,
            },
        )
    }
    pub fn recipients(&self) -> &[Uuid] {
        &self.recipients
    }
    pub fn next_cursor(&self) -> Option<&str> {
        self.next.as_deref()
    }
    pub fn witness(&self) -> &str {
        &self.witness
    }
    pub fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at
    }
}
pub enum NotificationPageResultDto {
    Selected(SelectedNotificationPageDto),
    Suppressed,
}
#[derive(Clone)]
pub struct ClaimedNotificationWriteDto {
    owner: OwnedDeliveryClaimDto,
    request: NotificationRequest,
    membership: Option<String>,
    deadline: DeliveryEffectDeadlineDto,
}
impl ClaimedNotificationWriteDto {
    pub fn new(
        owner: OwnedDeliveryClaimDto,
        intent: &NotificationIntentDto,
        recipient: Uuid,
        membership: Option<String>,
        deadline: DeliveryEffectDeadlineDto,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            owner.lookup().claim().target == DeliveryTarget::Internal
                && deadline.expires_at() <= owner.expires_at(),
            "Invalid notification write claim"
        );
        anyhow::ensure!(
            matches!(intent.audience(), NotificationAudienceDto::Followers { .. })
                == membership.is_some(),
            "Invalid notification membership purpose"
        );
        Ok(Self {
            owner,
            request: intent.notification_for(recipient)?,
            membership,
            deadline,
        })
    }
    pub fn owner(&self) -> &OwnedDeliveryClaimDto {
        &self.owner
    }
    pub fn request(&self) -> &NotificationRequest {
        &self.request
    }
    pub fn membership(&self) -> Option<&str> {
        self.membership.as_deref()
    }
    pub fn deadline(&self) -> DeliveryEffectDeadlineDto {
        self.deadline
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClaimedNotificationAckDto {
    /// The original wire response acknowledges a new or immutable existing row.
    Delivered,
    Suppressed,
}
#[async_trait]
pub trait ClaimedNotificationTransportPort: Send + Sync {
    async fn select_page(
        &self,
        request: &NotificationPageRequestDto,
    ) -> anyhow::Result<NotificationPageResultDto>;
    /// Only explicitly classified pre-write errors prove no effect.
    /// Unclassified errors after entering this boundary are uncertain.
    async fn write(
        &self,
        request: &ClaimedNotificationWriteDto,
    ) -> anyhow::Result<ClaimedNotificationAckDto>;
}
#[async_trait]
pub trait ClaimedNotificationDeliveryPort: Send + Sync {
    async fn deliver_claimed_notifications(
        &self,
        claim: &TargetDeliveryClaimDto,
        deadline: DeliveryEffectDeadlineDto,
    ) -> anyhow::Result<()>;
}
pub struct DeliverClaimedNotificationsUseCase {
    owner: Arc<dyn DeliveryAuthorityRepositoryPort>,
    posts: Arc<dyn PostRepositoryPort>,
    transport: Arc<dyn ClaimedNotificationTransportPort>,
    limit: i32,
}
impl DeliverClaimedNotificationsUseCase {
    pub fn new(
        owner: Arc<dyn DeliveryAuthorityRepositoryPort>,
        posts: Arc<dyn PostRepositoryPort>,
        transport: Arc<dyn ClaimedNotificationTransportPort>,
        limit: i32,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            (1..=100).contains(&limit),
            "Invalid notification page limit"
        );
        Ok(Self {
            owner,
            posts,
            transport,
            limit,
        })
    }
    async fn recheck(
        &self,
        lookup: &DeliveryClaimLookupDto,
        frozen: &OwnedDeliveryClaimDto,
        deadline: DeliveryEffectDeadlineDto,
    ) -> anyhow::Result<()> {
        let fresh = self.owner.resolve_claimed_delivery(lookup).await?;
        anyhow::ensure!(
            crate::notification_authority::notification_attempt_identity(&fresh)?
                == crate::notification_authority::notification_attempt_identity(frozen)?,
            "Notification attempt provenance changed"
        );
        anyhow::ensure!(
            Utc::now() < deadline.expires_at(),
            "Notification effect expired"
        );
        Ok(())
    }
    async fn write(
        &self,
        request: &ClaimedNotificationWriteDto,
    ) -> anyhow::Result<ClaimedNotificationAckDto> {
        self.transport.write(request).await.map_err(|error| {
            if DeliveryEffectFailure::classify(&error) == DeliveryEffectFailure::Uncertain {
                DeliveryEffectFailure::uncertain(error)
            } else {
                error
            }
        })
    }
    async fn run(
        &self,
        claim: &TargetDeliveryClaimDto,
        deadline: DeliveryEffectDeadlineDto,
        effects: &mut bool,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            claim.target == DeliveryTarget::Internal
                && deadline.expires_at() <= claim.lease_expires_at
                && Utc::now() < deadline.expires_at(),
            "Invalid notification delivery deadline"
        );
        let lookup = DeliveryClaimLookupDto::new(claim.clone())?;
        let owned = self.owner.resolve_claimed_delivery(&lookup).await?;
        anyhow::ensure!(
            Utc::now() < deadline.expires_at(),
            "Notification effect expired"
        );
        let event = &owned.lookup().claim().event;
        if !matches!(
            event.event_type.as_str(),
            "POST_CREATED" | "POST_LIKED" | "POST_COMMENTED" | "COMMENT"
        ) {
            return Ok(());
        }
        if event.event_type == "POST_CREATED" {
            // Preserve lifecycle-before-payload/actor parsing, including malformed
            // inactive retained events that cannot form a signed page request.
            let posts = self
                .posts
                .fetch_post_access_snapshots(&[event.aggregate_id])
                .await?;
            self.recheck(&lookup, &owned, deadline).await?;
            let Some(post) = posts.get(&event.aggregate_id) else {
                return Ok(());
            };
            if !matches!(post.visibility.as_str(), "PUBLIC" | "FRIENDS") {
                return Ok(());
            }
            anyhow::ensure!(
                event.actor_id == Some(post.author_id),
                "Notification author mismatch"
            );
        }
        let intent = NotificationIntentDto::from_event(event)?
            .ok_or_else(|| anyhow::anyhow!("Notification intent missing"))?;
        match intent.audience() {
            NotificationAudienceDto::Direct { recipient_id } => {
                if recipient_id == intent.actor_id() {
                    return Ok(());
                }
                self.recheck(&lookup, &owned, deadline).await?;
                let request =
                    ClaimedNotificationWriteDto::new(owned, &intent, recipient_id, None, deadline)?;
                *effects = self.write(&request).await? == ClaimedNotificationAckDto::Delivered;
            }
            NotificationAudienceDto::Followers { author_id } => {
                let mut cursor = None;
                let mut seen = HashSet::new();
                loop {
                    let request = NotificationPageRequestDto::new(
                        owned.clone(),
                        author_id,
                        cursor.clone(),
                        self.limit,
                        deadline,
                    )?;
                    let page = self.transport.select_page(&request).await?;
                    let NotificationPageResultDto::Selected(page) = page else {
                        return Ok(());
                    };
                    page.validate(&request)?;
                    anyhow::ensure!(
                        page.recipients().iter().all(|id| seen.insert(*id)),
                        "Repeated notification recipient across pages"
                    );
                    for recipient in page.recipients() {
                        self.recheck(&lookup, &owned, deadline).await?;
                        anyhow::ensure!(
                            Utc::now() < page.expires_at() && Utc::now() < deadline.expires_at(),
                            "Notification membership expired"
                        );
                        let request = ClaimedNotificationWriteDto::new(
                            owned.clone(),
                            &intent,
                            *recipient,
                            Some(page.witness().to_owned()),
                            deadline.clipped_to(page.expires_at()),
                        )?;
                        if self.write(&request).await? == ClaimedNotificationAckDto::Delivered {
                            *effects = true;
                        }
                    }
                    let Some(next) = page.next_cursor() else {
                        return Ok(());
                    };
                    cursor = Some(next.to_owned());
                }
            }
        }
        Ok(())
    }
}
#[async_trait]
impl ClaimedNotificationDeliveryPort for DeliverClaimedNotificationsUseCase {
    async fn deliver_claimed_notifications(
        &self,
        c: &TargetDeliveryClaimDto,
        d: DeliveryEffectDeadlineDto,
    ) -> anyhow::Result<()> {
        let mut effects = false;
        self.run(c, d, &mut effects).await.map_err(|error| {
            if effects {
                DeliveryEffectFailure::uncertain(error)
            } else {
                DeliveryEffectFailure::confirmed(error)
            }
        })
    }
}
