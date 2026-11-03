//! Read-only INTERNAL owner and current created-source observation. No RPC or
//! credentials are activated, and no Friend policy is inferred from Post data.
use super::{
    delivery_authority_repository::PgDeliveryAuthorityRepository, event_outbox::PgEventOutbox,
};
use post_application::{
    delivery_authority::{
        DeliveryAuthorityErrorDto as Error, DeliveryAuthorityRepositoryPort,
        DeliveryClaimLookupDto, OwnedDeliveryClaimDto,
    },
    dto::delivery_schedule_dto::TargetDeliveryClaimDto,
    notification_authority::*,
};
use post_domain::post::DeliveryTarget;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};

pub fn notification_reference(
    owner: &OwnedDeliveryClaimDto,
    scope: NotificationScopeDto,
) -> Result<NotificationOwnerReferenceDto, Error> {
    let c = owner.lookup().claim();
    // Building a reference must not parse a created payload before the owner
    // observes current lifecycle. The existing listener suppresses an inactive
    // created source even when its retained payload cannot be parsed. This
    // reference is data; the repository validates the intent and exact scope.
    scope.validate()?;
    let r = NotificationOwnerReferenceDto {
        event_id: c.event.event_id,
        aggregate_id: c.event.aggregate_id,
        event_digest: Sha256::digest(notification_event_identity(&c.event)?).into(),
        attempt_digest: Sha256::digest(notification_attempt_identity(owner)?).into(),
        lease_token: c.lease_token,
        attempt_id: c.attempt_id,
        attempt_number: c.attempt_number,
        lease_expires_at: c.lease_expires_at,
        scope,
    };
    r.validate()?;
    Ok(r)
}

pub struct PgNotificationAuthorityRepository {
    pool: PgPool,
    owner: PgDeliveryAuthorityRepository,
    max_page_limit: i32,
}
impl PgNotificationAuthorityRepository {
    pub fn new(pool: PgPool, max_page_limit: i32) -> anyhow::Result<Self> {
        anyhow::ensure!(
            (1..=100).contains(&max_page_limit),
            "Invalid notification page ceiling"
        );
        Ok(Self {
            owner: PgDeliveryAuthorityRepository::new(pool.clone()),
            pool,
            max_page_limit,
        })
    }
}
impl PgNotificationAuthorityRepository {
    async fn resolve_source(
        &self,
        r: &NotificationOwnerReferenceDto,
    ) -> Result<OwnedNotificationSourceDto, Error> {
        r.validate()?;
        if matches!(r.scope, NotificationScopeDto::FollowerPage { limit, .. } if limit > self.max_page_limit)
        {
            return Err(Error::Denied);
        }
        let row=sqlx::query("select event_id,event_type,aggregate_id,aggregate_version,actor_id,attributes,payload,occurred_at,created_at,published,published_at,replay_resolution_id,origin_traceparent from post_event_outbox where event_id=$1")
            .bind(r.event_id).fetch_optional(&self.pool).await.map_err(|_|Error::Unavailable)?.ok_or(Error::Denied)?;
        let event = PgEventOutbox::map_event(row);
        if event.aggregate_id != r.aggregate_id
            || <[u8; 32]>::from(Sha256::digest(notification_event_identity(&event)?))
                != r.event_digest
        {
            return Err(Error::Denied);
        }
        let lookup = DeliveryClaimLookupDto::new(TargetDeliveryClaimDto {
            event,
            target: DeliveryTarget::Internal,
            lease_token: r.lease_token,
            attempt_id: r.attempt_id,
            attempt_number: r.attempt_number,
            lease_expires_at: r.lease_expires_at,
        })?;
        // The hint read above is not authority. This repository validates the
        // complete stored source, actual lease/provenance and post-wait clock.
        let initial = self.owner.resolve_claimed_delivery(&lookup).await?;
        if <[u8; 32]>::from(Sha256::digest(notification_attempt_identity(&initial)?))
            != r.attempt_digest
        {
            return Err(Error::Denied);
        }
        // Cap freshness at the beginning of the source read. A later ownership
        // wait must not re-date an older Post snapshot (including absence).
        let source_checked_at: Option<chrono::DateTime<chrono::Utc>> =
            if lookup.claim().event.event_type == "POST_CREATED" {
                Some(
                    sqlx::query_scalar("select clock_timestamp()")
                        .fetch_one(&self.pool)
                        .await
                        .map_err(|_| Error::Unavailable)?,
                )
            } else {
                None
            };
        let current_post = if source_checked_at.is_some() {
            sqlx::query("select id,author_id,visibility from posts where id=$1 and deleted_at is null and moderation_hidden_at is null")
                .bind(r.aggregate_id).fetch_optional(&self.pool).await.map_err(|_|Error::Unavailable)?
                .map(|row| NotificationPostAccessDto { post_id: row.get("id"), author_id: row.get("author_id"), visibility: row.get("visibility") })
        } else {
            None
        };
        // The Post await must not outlive or replace the original owner lease.
        // Re-observe without renewing or mutating the attempt, then compare the
        // frozen provenance again. Relationship/write admission remains separate.
        let fresh = self.owner.resolve_claimed_delivery(&lookup).await?;
        if <[u8; 32]>::from(Sha256::digest(notification_attempt_identity(&fresh)?))
            != r.attempt_digest
        {
            return Err(Error::Denied);
        }
        OwnedNotificationSourceDto::from_persisted(
            r.clone(),
            &fresh.lookup().claim().event,
            current_post.as_ref(),
            source_checked_at.map_or(fresh.checked_at(), |source| source.min(fresh.checked_at())),
        )
    }
}

#[async_trait::async_trait]
impl NotificationAuthorityRepositoryPort for PgNotificationAuthorityRepository {
    async fn resolve_notification_scope(
        &self,
        r: &NotificationOwnerReferenceDto,
    ) -> Result<Option<OwnedNotificationScopeDto>, Error> {
        Ok(self.resolve_source(r).await?.into_scope())
    }
}

#[async_trait::async_trait]
impl NotificationNetworkAuthorityRepositoryPort for PgNotificationAuthorityRepository {
    async fn resolve_notification_source(
        &self,
        r: &NotificationNetworkReferenceDto,
    ) -> Result<OwnedNotificationSourceDto, Error> {
        r.validate()?;
        // This unlocked credential read is only a hint. A rotated/completed or
        // otherwise stale row cannot authorize anything: resolve_source below
        // checks the actual full claim and provenance under the owner locks,
        // then rechecks ownership after reading current Post state.
        let token: Option<uuid::Uuid> = sqlx::query_scalar(
            "select lease_token from post_event_delivery_jobs where event_id=$1 and target='INTERNAL'",
        )
        .bind(r.event_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| Error::Unavailable)?
        .flatten();
        let token = token.filter(|t| !t.is_nil()).ok_or(Error::Denied)?;
        if <[u8; 32]>::from(Sha256::digest(token.as_bytes())) != r.lease_digest {
            return Err(Error::Denied);
        }
        self.resolve_source(&NotificationOwnerReferenceDto {
            event_id: r.event_id,
            aggregate_id: r.aggregate_id,
            event_digest: r.event_digest,
            attempt_digest: r.attempt_digest,
            lease_token: token,
            attempt_id: r.attempt_id,
            attempt_number: r.attempt_number,
            lease_expires_at: r.lease_expires_at,
            scope: r.scope.clone(),
        })
        .await
    }
}
