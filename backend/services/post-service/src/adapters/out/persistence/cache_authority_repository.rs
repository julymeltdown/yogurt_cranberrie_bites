//! CACHE author derives from the authoritative event, including after purge.
use super::{
    delivery_authority_repository::PgDeliveryAuthorityRepository, event_outbox::PgEventOutbox,
};
use platform::actor_delegation::cache::CacheScope;
use post_application::{
    cache_authority::*,
    delivery_authority::{
        DeliveryAuthorityErrorDto as Error, DeliveryAuthorityRepositoryPort, DeliveryClaimLookupDto,
    },
    dto::delivery_schedule_dto::TargetDeliveryClaimDto,
};
use post_domain::post::DeliveryTarget;
use sqlx::PgPool;

pub struct PgCacheAuthorityRepository {
    pool: PgPool,
    owner: PgDeliveryAuthorityRepository,
    max_limit: i32,
}
impl PgCacheAuthorityRepository {
    pub fn new(pool: PgPool, max_limit: i32) -> anyhow::Result<Self> {
        anyhow::ensure!(
            (1..=100).contains(&max_limit),
            "Invalid cache owner page ceiling"
        );
        Ok(Self {
            owner: PgDeliveryAuthorityRepository::new(pool.clone()),
            pool,
            max_limit,
        })
    }
}
#[async_trait::async_trait]
impl CacheAuthorityRepositoryPort for PgCacheAuthorityRepository {
    async fn resolve_cache_page(
        &self,
        r: &CachePageReferenceDto,
    ) -> Result<OwnedCachePageDto, Error> {
        r.validate()?;
        if r.limit > self.max_limit {
            return Err(Error::Denied);
        }
        let row=sqlx::query("select event_id,event_type,aggregate_id,aggregate_version,actor_id,attributes,payload,occurred_at,created_at,published,published_at,replay_resolution_id,origin_traceparent from post_event_outbox where event_id=$1")
            .bind(r.event_id).fetch_optional(&self.pool).await.map_err(|_|Error::Unavailable)?.ok_or(Error::Denied)?;
        let event = PgEventOutbox::map_event(row);
        if event.aggregate_id != r.post_id
            || CacheScope::digest(&cache_event_identity(&event).map_err(|_| Error::Denied)?)
                != r.event_digest
            || CacheInvalidationPlanDto::from_event(&event)?.feed_author_id() != Some(r.author_id)
        {
            return Err(Error::Denied);
        }
        // This first read is not provenance. The real owning repository locks
        // and compares its complete immutable identity again, checks the stored
        // CACHE lease/attempt/budget/marker, and samples time after all waits.
        let owned = self
            .owner
            .resolve_claimed_delivery(&DeliveryClaimLookupDto::new(TargetDeliveryClaimDto {
                event,
                target: DeliveryTarget::Cache,
                lease_token: r.lease_token,
                attempt_id: r.attempt_id,
                attempt_number: r.attempt_number,
                lease_expires_at: r.lease_expires_at,
            })?)
            .await?;
        if CacheScope::digest(&cache_attempt_identity(&owned).map_err(|_| Error::Denied)?)
            != r.attempt_digest
        {
            return Err(Error::Denied);
        }
        OwnedCachePageDto::from_persisted(r.clone(), owned.checked_at())
    }
}
