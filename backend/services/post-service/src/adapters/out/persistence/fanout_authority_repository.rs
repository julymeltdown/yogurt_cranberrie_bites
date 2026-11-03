//! Read-only owner validation before system fanout authority may be issued.
//! This adapter signs nothing and is not registered on a production listener.
use async_trait::async_trait;
use post_application::dto::timeline_fanout_dto::TimelineFanoutClaimDto;
use post_application::fanout_authority::{
    FanoutAuthorityErrorDto, FanoutAuthorityRepositoryPort, FanoutPageLookupDto,
    FanoutPagePolicyDto, OwnedFanoutPageDto,
};
use post_domain::recommendation::TimelineFanoutJob;
use sqlx::{PgPool, Row};

#[derive(Clone)]
pub struct PgFanoutAuthorityRepository {
    pool: PgPool,
    policy: FanoutPagePolicyDto,
}
impl PgFanoutAuthorityRepository {
    pub fn new(pool: PgPool, policy: FanoutPagePolicyDto) -> Self {
        Self { pool, policy }
    }
}
fn unavailable(_: sqlx::Error) -> FanoutAuthorityErrorDto {
    // Never expose lease tokens, SQL details or source data in denial errors.
    FanoutAuthorityErrorDto::Unavailable
}

#[async_trait]
impl FanoutAuthorityRepositoryPort for PgFanoutAuthorityRepository {
    async fn resolve_claimed_page(
        &self,
        request: &FanoutPageLookupDto,
    ) -> Result<OwnedFanoutPageDto, FanoutAuthorityErrorDto> {
        let expected = request.claim();
        let job = &expected.job;
        if request.page_limit() > self.policy.max_page_size() {
            return Err(FanoutAuthorityErrorDto::Denied);
        }
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        // Match page application/deletion: sorted owner subject, Post, job.
        // Observe the owner before subject fencing and deny if it changes.
        let observed: uuid::Uuid = sqlx::query_scalar("select author_id from posts where id=$1")
            .bind(job.post_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(unavailable)?
            .ok_or(FanoutAuthorityErrorDto::Denied)?;
        sqlx::query("select post_lock_subject_ids($1::uuid[])")
            .bind(vec![observed])
            .execute(&mut *tx)
            .await
            .map_err(unavailable)?;
        // Shared row locks stabilize validation only; no lock survives lookup.
        let source = sqlx::query(
            "select author_id, deleted_at is null and moderation_hidden_at is null \
             and visibility in ('PUBLIC','FRIENDS') as eligible \
             from posts where id=$1 for share",
        )
        .bind(job.post_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(unavailable)?
        .ok_or(FanoutAuthorityErrorDto::Denied)?;
        let author: uuid::Uuid = source.try_get("author_id").map_err(unavailable)?;
        if author != observed
            || author != job.author_id
            || !source.try_get::<bool, _>("eligible").map_err(unavailable)?
        {
            return Err(FanoutAuthorityErrorDto::Denied);
        }
        // Expiry is deliberately evaluated in a SECOND statement after any
        // post/job row wait. A pre-wait predicate cannot authorize later work.
        let row = sqlx::query(
            "select * from feed_fanout_jobs where job_id=$1 and event_id=$2 \
             and post_id=$3 and author_id=$4 and lease_token=$5 and attempt_count=$6 \
             and cursor is not distinct from $7 and lease_expires_at=$8 \
             and post_created_at=$9 and page_attempt_count=$10 and max_page_attempts=$11 \
             and page_budget_provenance='CURRENT_PAGE' \
             and page_attempt_count between 1 and max_page_attempts \
             and status='PROCESSING' and completed_at is null and dead_lettered_at is null \
             for share",
        )
        .bind(job.job_id)
        .bind(job.event_id)
        .bind(job.post_id)
        .bind(job.author_id)
        .bind(expected.lease_token)
        .bind(job.attempt_count)
        .bind(&job.cursor)
        .bind(expected.lease_expires_at)
        .bind(job.post_created_at)
        .bind(expected.page_attempt_count)
        .bind(expected.max_page_attempts)
        .fetch_optional(&mut *tx)
        .await
        .map_err(unavailable)?
        .ok_or(FanoutAuthorityErrorDto::Denied)?;
        let checked_at = sqlx::query_scalar("select clock_timestamp()")
            .fetch_one(&mut *tx)
            .await
            .map_err(unavailable)?;
        let claim = TimelineFanoutClaimDto {
            job: TimelineFanoutJob {
                job_id: row.try_get("job_id").map_err(unavailable)?,
                event_id: row.try_get("event_id").map_err(unavailable)?,
                post_id: row.try_get("post_id").map_err(unavailable)?,
                author_id: row.try_get("author_id").map_err(unavailable)?,
                post_created_at: row.try_get("post_created_at").map_err(unavailable)?,
                cursor: row.try_get("cursor").map_err(unavailable)?,
                attempt_count: row.try_get("attempt_count").map_err(unavailable)?,
            },
            lease_token: row.try_get("lease_token").map_err(unavailable)?,
            lease_expires_at: row.try_get("lease_expires_at").map_err(unavailable)?,
            page_attempt_count: row.try_get("page_attempt_count").map_err(unavailable)?,
            max_page_attempts: row.try_get("max_page_attempts").map_err(unavailable)?,
        };
        let result =
            OwnedFanoutPageDto::from_persisted_claim(claim, request.page_limit(), checked_at)?;
        tx.commit().await.map_err(unavailable)?;
        // A slow commit must not return a locally expired observation either.
        // This does not replace the independent issuer's fresh expiry check.
        if result.expires_at() <= chrono::Utc::now() {
            return Err(FanoutAuthorityErrorDto::Denied);
        }
        Ok(result)
    }
}
