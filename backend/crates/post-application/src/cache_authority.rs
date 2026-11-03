//! Claimed CACHE scope and execution. Owner data is not network admission proof.
use crate::delivery_authority::{
    DeliveryAuthorityErrorDto as Error, DeliveryAuthorityRepositoryPort, DeliveryClaimLookupDto,
    OwnedDeliveryClaimDto,
};
use crate::delivery_effect::DeliveryEffectDeadlineDto;
use crate::delivery_effect::DeliveryEffectFailure;
use crate::dto::delivery_schedule_dto::TargetDeliveryClaimDto;
use crate::dto::timeline_projection_dto::TimelineProjectionEventDto;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use post_domain::post::{DeliveryTarget, StoredEvent};
use std::{collections::HashSet, sync::Arc};
use uuid::Uuid;

/// Stable exact source identity, including retained trace, excluding mutable
/// publication/release metadata. Digesting belongs to the infrastructure edge.
pub fn cache_event_identity(event: &StoredEvent) -> anyhow::Result<Vec<u8>> {
    #[derive(serde::Serialize)]
    struct Identity<'a> {
        event: TimelineProjectionEventDto,
        trace: &'a Option<String>,
    }
    Ok(serde_json::to_vec(&Identity {
        event: TimelineProjectionEventDto::from(event),
        trace: &event.origin_traceparent,
    })?)
}

/// Stored scheduling provenance is distinct from event and raw claim identity.
pub fn cache_attempt_identity(owned: &OwnedDeliveryClaimDto) -> anyhow::Result<Vec<u8>> {
    let p = owned.provenance();
    #[derive(serde::Serialize)]
    struct Provenance<'a> {
        lease_kind: &'a str,
        source: &'a str,
        resolution_id: Option<Uuid>,
        max_attempts: i32,
        progress_credits: i32,
        target_required: bool,
    }
    anyhow::ensure!(
        matches!(
            p.lease_kind,
            crate::delivery_authority::DeliveryLeaseKindDto::Automatic
        ),
        "CACHE requires automatic lease"
    );
    Ok(serde_json::to_vec(&Provenance {
        lease_kind: "AUTO",
        source: p.source.as_str(),
        resolution_id: p.resolution_id,
        max_attempts: p.max_attempts,
        progress_credits: p.progress_credits,
        target_required: p.target_required,
    })?)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct CacheInvalidationPlanDto {
    post_id: Option<Uuid>,
    feed_author_id: Option<Uuid>,
}
impl CacheInvalidationPlanDto {
    /// Call only after independently validating the owning delivery. No live
    /// resource requirement: invalidation may follow deletion and hard purge.
    pub fn from_event(event: &StoredEvent) -> Result<Self, Error> {
        let payload: serde_json::Value =
            serde_json::from_str(&event.payload).map_err(|_| Error::Denied)?;
        let known = matches!(
            event.event_type.as_str(),
            "POST_CREATED"
                | "POST_COMMENTED"
                | "POST_COMMENT_DELETED"
                | "POST_DELETED"
                | "POST_LIKED"
                | "POST_UNLIKED"
        );
        if !known {
            return Ok(Self {
                post_id: None,
                feed_author_id: None,
            });
        }
        let field = |name: &str| {
            payload
                .get(name)
                .and_then(serde_json::Value::as_str)
                .and_then(|v| Uuid::parse_str(v).ok())
                .filter(|id| !id.is_nil())
                .ok_or(Error::Denied)
        };
        if field("postId")? != event.aggregate_id || event.aggregate_id.is_nil() {
            return Err(Error::Denied);
        }
        let actor = event
            .actor_id
            .filter(|id| !id.is_nil())
            .ok_or(Error::Denied)?;
        if matches!(event.event_type.as_str(), "POST_LIKED" | "POST_UNLIKED") {
            // Stored unlike has authorId=actor and intentionally lacks the
            // application producer's actorId/notifyUserId. Neither is needed
            // to invalidate the exact owning Post key. Validate the stored
            // producer's actor relationship without requiring its live Post.
            if event.event_type == "POST_UNLIKED"
                && payload.get("actorId").is_none()
                && payload.get("notifyUserId").is_none()
            {
                if field("authorId")? != actor {
                    return Err(Error::Denied);
                }
            } else if field("actorId")? != actor || field("authorId")? != field("notifyUserId")? {
                return Err(Error::Denied);
            }
            return Ok(Self {
                post_id: Some(event.aggregate_id),
                feed_author_id: None,
            });
        }
        if field("authorId")? != actor
            || (payload.get("actorId").is_some() && field("actorId")? != actor)
        {
            return Err(Error::Denied);
        }
        let author = match event.event_type.as_str() {
            "POST_COMMENTED" => field("notifyUserId")?,
            "POST_COMMENT_DELETED" => {
                if field("actorId")? != actor {
                    return Err(Error::Denied);
                }
                field("postAuthorId")?
            }
            "POST_DELETED" => {
                if field("actorId")? != actor {
                    return Err(Error::Denied);
                }
                actor
            }
            _ => actor,
        };
        Ok(Self {
            post_id: (event.event_type != "POST_CREATED").then_some(event.aggregate_id),
            feed_author_id: Some(author),
        })
    }
    pub fn post_id(self) -> Option<Uuid> {
        self.post_id
    }
    pub fn feed_author_id(self) -> Option<Uuid> {
        self.feed_author_id
    }
}

/// Minimal reference presented on the dedicated wire. Raw payload/attributes
/// and trace never go to Friend. This reference is not a capability.
#[derive(Clone)]
pub struct CachePageReferenceDto {
    pub event_id: Uuid,
    pub post_id: Uuid,
    pub author_id: Uuid,
    pub event_digest: [u8; 32],
    pub attempt_digest: [u8; 32],
    pub lease_token: Uuid,
    pub attempt_id: Uuid,
    pub attempt_number: i32,
    pub lease_expires_at: DateTime<Utc>,
    pub cursor: Option<String>,
    pub limit: i32,
}
impl CachePageReferenceDto {
    pub fn validate(&self) -> Result<(), Error> {
        if [
            self.event_id,
            self.post_id,
            self.author_id,
            self.lease_token,
            self.attempt_id,
        ]
        .iter()
        .any(Uuid::is_nil)
            || self.attempt_number < 1
            || !(1..=100).contains(&self.limit)
        {
            return Err(Error::InvalidBinding);
        }
        validate_cursor(self.cursor.as_deref())
    }
}
fn validate_cursor(cursor: Option<&str>) -> Result<(), Error> {
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
    Ok(())
}
pub struct OwnedCachePageDto {
    reference: CachePageReferenceDto,
    checked_at: DateTime<Utc>,
}
impl OwnedCachePageDto {
    /// Persistence projection, not proof of origin. Adapter must validate all
    /// source/plan/lease bindings before constructing this owner observation.
    pub fn from_persisted(
        reference: CachePageReferenceDto,
        checked_at: DateTime<Utc>,
    ) -> Result<Self, Error> {
        reference.validate()?;
        if checked_at >= reference.lease_expires_at {
            return Err(Error::Denied);
        }
        Ok(Self {
            reference,
            checked_at,
        })
    }
    pub fn reference(&self) -> &CachePageReferenceDto {
        &self.reference
    }
    pub fn checked_at(&self) -> DateTime<Utc> {
        self.checked_at
    }
}
#[async_trait]
pub trait CacheAuthorityRepositoryPort: Send + Sync {
    async fn resolve_cache_page(
        &self,
        reference: &CachePageReferenceDto,
    ) -> Result<OwnedCachePageDto, Error>;
}

#[derive(Clone)]
pub struct CachePageLookupDto {
    deadline: crate::delivery_effect::DeliveryEffectDeadlineDto,
    owner: OwnedDeliveryClaimDto,
    author_id: Uuid,
    cursor: Option<String>,
    limit: i32,
}
impl CachePageLookupDto {
    pub fn within(
        mut self,
        deadline: crate::delivery_effect::DeliveryEffectDeadlineDto,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            deadline.expires_at() <= self.deadline.expires_at(),
            "Invalid authority deadline"
        );
        self.deadline = deadline;
        Ok(self)
    }
    pub fn deadline(&self) -> crate::delivery_effect::DeliveryEffectDeadlineDto {
        self.deadline
    }

    pub fn new(
        owner: OwnedDeliveryClaimDto,
        author_id: Uuid,
        cursor: Option<String>,
        limit: i32,
    ) -> anyhow::Result<Self> {
        let claim = owner.lookup();
        anyhow::ensure!(
            claim.claim().target == DeliveryTarget::Cache
                && !author_id.is_nil()
                && (1..=100).contains(&limit),
            "Invalid cache page scope"
        );
        validate_cursor(cursor.as_deref())?;
        anyhow::ensure!(
            CacheInvalidationPlanDto::from_event(&claim.claim().event)?.feed_author_id()
                == Some(author_id),
            "Invalid cache author binding"
        );
        let expires_at = owner.expires_at();
        let budget = (expires_at - Utc::now()).to_std().unwrap_or_default();
        let deadline = crate::delivery_effect::DeliveryEffectDeadlineDto::new(
            expires_at,
            std::time::Instant::now() + budget,
        )?;
        Ok(Self {
            deadline,
            owner,
            author_id,
            cursor,
            limit,
        })
    }
    pub fn claim(&self) -> &DeliveryClaimLookupDto {
        self.owner.lookup()
    }
    pub fn owner(&self) -> &OwnedDeliveryClaimDto {
        &self.owner
    }
    pub fn author_id(&self) -> Uuid {
        self.author_id
    }
    pub fn cursor(&self) -> Option<&str> {
        self.cursor.as_deref()
    }
    pub fn limit(&self) -> i32 {
        self.limit
    }
}
pub struct CacheFollowerPageDto {
    expires_at: DateTime<Utc>,
    author_id: Uuid,
    followers: Vec<Uuid>,
    next_cursor: Option<String>,
}
impl CacheFollowerPageDto {
    pub fn new(
        request: &CachePageLookupDto,
        author_id: Uuid,
        followers: Vec<Uuid>,
        next_cursor: Option<String>,
        expires_at: DateTime<Utc>,
    ) -> anyhow::Result<Self> {
        let page = Self {
            expires_at,
            author_id,
            followers,
            next_cursor,
        };
        page.validate(request)?;
        Ok(page)
    }
    /// A constructible response DTO must be checked against the actual port
    /// request, even when an adapter constructed it against a different scope.
    pub fn validate(&self, request: &CachePageLookupDto) -> anyhow::Result<()> {
        validate_cursor(self.next_cursor.as_deref())?;
        let mut seen = HashSet::new();
        anyhow::ensure!(
            self.expires_at <= request.deadline().expires_at()
                && Utc::now() < self.expires_at
                && self.author_id == request.author_id
                && self.followers.len() <= request.limit as usize
                && self
                    .followers
                    .iter()
                    .all(|id| !id.is_nil() && *id != self.author_id && seen.insert(*id)),
            "Invalid cache follower page"
        );
        crate::dto::timeline_fanout_dto::validate_fanout_page_progress(
            request.cursor(),
            &crate::dto::timeline_fanout_dto::TimelineFanoutPageDto {
                follower_ids: self.followers.clone(),
                next_cursor: self.next_cursor.clone(),
                expires_at: request.claim().claim().lease_expires_at,
            },
        )?;
        Ok(())
    }
    pub fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at
    }
    pub fn into_parts(self) -> (Vec<Uuid>, Option<String>, DateTime<Utc>) {
        (self.followers, self.next_cursor, self.expires_at)
    }
}
#[async_trait]
pub trait ClaimedCachePageReadPort: Send + Sync {
    async fn fetch_cache_page(
        &self,
        request: &CachePageLookupDto,
    ) -> anyhow::Result<CacheFollowerPageDto>;
}
#[async_trait]
pub trait ClaimedCacheInvalidationPort: Send + Sync {
    async fn invalidate_claimed_cache(
        &self,
        claim: &TargetDeliveryClaimDto,
        deadline: DeliveryEffectDeadlineDto,
    ) -> anyhow::Result<()>;
}

#[async_trait]
pub trait ClaimedCacheEffectsPort: Send + Sync {
    async fn evict_post(
        &self,
        post_id: Uuid,
        deadline: DeliveryEffectDeadlineDto,
    ) -> anyhow::Result<()>;
    async fn invalidate_feeds(
        &self,
        user_ids: &[Uuid],
        deadline: DeliveryEffectDeadlineDto,
    ) -> anyhow::Result<()>;
}

/// Explicit required owner and dedicated page ports. It has no general Friend
/// dependency, fallback or scheduler mutation. Outer delivery owns its deadline.
pub struct InvalidateClaimedCacheUseCase {
    owner: Arc<dyn DeliveryAuthorityRepositoryPort>,
    pages: Arc<dyn ClaimedCachePageReadPort>,
    effects: Arc<dyn ClaimedCacheEffectsPort>,
    limit: i32,
}
impl InvalidateClaimedCacheUseCase {
    pub fn new(
        owner: Arc<dyn DeliveryAuthorityRepositoryPort>,
        pages: Arc<dyn ClaimedCachePageReadPort>,
        effects: Arc<dyn ClaimedCacheEffectsPort>,
        limit: i32,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!((1..=100).contains(&limit), "Invalid cache page limit");
        Ok(Self {
            owner,
            pages,
            effects,
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
            cache_attempt_identity(&fresh)? == cache_attempt_identity(frozen)?,
            "Cache attempt provenance changed"
        );
        anyhow::ensure!(
            Utc::now() < deadline.expires_at()
                && std::time::Instant::now() < deadline.monotonic_stop(),
            "Cache authority expired"
        );
        Ok(())
    }
    async fn run(
        &self,
        claim: &TargetDeliveryClaimDto,
        deadline: DeliveryEffectDeadlineDto,
        started: &mut bool,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            claim.target == DeliveryTarget::Cache
                && deadline.expires_at() <= claim.lease_expires_at
                && Utc::now() < deadline.expires_at()
                && std::time::Instant::now() < deadline.monotonic_stop(),
            "CACHE delivery required"
        );
        let lookup = DeliveryClaimLookupDto::new(claim.clone())?;
        let owned = self.owner.resolve_claimed_delivery(&lookup).await?;
        let plan = CacheInvalidationPlanDto::from_event(&owned.lookup().claim().event)?;
        anyhow::ensure!(
            plan == CacheInvalidationPlanDto::from_event(&claim.event)?,
            "Cache owner plan mismatch"
        );
        if let Some(post) = plan.post_id() {
            self.recheck(&lookup, &owned, deadline).await?;
            self.effects
                .evict_post(post, deadline)
                .await
                .map_err(effect_error)?;
            *started = true;
        }
        let Some(author) = plan.feed_author_id() else {
            return Ok(());
        };
        let mut cursor = None;
        let mut seen = HashSet::new();
        let mut first = true;
        loop {
            let page_request =
                CachePageLookupDto::new(owned.clone(), author, cursor.clone(), self.limit)?
                    .within(deadline)?;
            let page = self.pages.fetch_cache_page(&page_request).await?;
            page.validate(&page_request)?;
            let (mut ids, next, expiry) = page.into_parts();
            let child_deadline = deadline.clipped_to(expiry);
            anyhow::ensure!(
                ids.iter().all(|id| seen.insert(*id)),
                "Repeated cache recipient across pages"
            );
            // Projection is an observation, not the independent local effect
            // fence. Loss after a successful read cannot select stale effects.
            self.recheck(&lookup, &owned, child_deadline).await?;
            if first {
                ids.push(author);
                first = false;
            }
            if !ids.is_empty() {
                self.effects
                    .invalidate_feeds(&ids, child_deadline)
                    .await
                    .map_err(effect_error)?;
                *started = true;
            }
            let Some(next) = next else {
                return Ok(());
            };
            cursor = Some(next);
        }
    }
}
fn effect_error(error: anyhow::Error) -> anyhow::Error {
    if DeliveryEffectFailure::classify(&error) == DeliveryEffectFailure::Uncertain {
        DeliveryEffectFailure::uncertain(error)
    } else {
        error
    }
}
#[async_trait]
impl ClaimedCacheInvalidationPort for InvalidateClaimedCacheUseCase {
    async fn invalidate_claimed_cache(
        &self,
        claim: &TargetDeliveryClaimDto,
        deadline: DeliveryEffectDeadlineDto,
    ) -> anyhow::Result<()> {
        let mut started = false;
        let result = self.run(claim, deadline, &mut started).await;
        result.map_err(|e| {
            if started {
                DeliveryEffectFailure::uncertain(e)
            } else {
                DeliveryEffectFailure::confirmed(e)
            }
        })
    }
}
