//! Deadline-aware selected writes share the legacy SQL and lock order. Before
//! COMMIT, cancellation drops the transaction for rollback. After COMMIT is sent,
//! a timeout or late acknowledgement is uncertain; it does not prove rollback.
use super::*;
use platform::actor_delegation::authority_deadline::AuthorityRpcDeadline;
use post_application::{
    delivery_effect::{DeliveryEffectDeadlineDto, DeliveryEffectFailure},
    fanout_authority::ClaimedFanoutApplyPort,
};
use std::time::Duration;

pub(super) struct FanoutWriteDeadline {
    pub(super) expires: DateTime<Utc>,
    rpc: AuthorityRpcDeadline,
}
impl FanoutWriteDeadline {
    fn new(dto: DeliveryEffectDeadlineDto) -> Result<Self> {
        let remaining = dto
            .expires_at()
            .signed_duration_since(Utc::now())
            .to_std()?;
        Ok(Self {
            expires: dto.expires_at(),
            rpc: AuthorityRpcDeadline::within_original(
                dto.expires_at().timestamp_micros(),
                dto.monotonic_stop(),
                remaining,
            )?,
        })
    }
    pub(super) async fn arm(&self, tx: &mut sqlx::Transaction<'_, sqlx::Postgres>) -> Result<()> {
        let millis = self
            .rpc
            .remaining(Duration::from_secs(86_400), self.expires.timestamp_micros())?
            .as_millis();
        anyhow::ensure!(millis > 0, "Fanout authority expired");
        sqlx::query(
            "select set_config('statement_timeout',$1,true),set_config('lock_timeout',$1,true)",
        )
        .bind(format!("{millis}ms"))
        .execute(&mut **tx)
        .await?;
        let live: bool = sqlx::query_scalar("select clock_timestamp()<$1")
            .bind(self.expires)
            .fetch_one(&mut **tx)
            .await?;
        anyhow::ensure!(live, "Fanout authority expired at database");
        self.rpc
            .remaining(Duration::from_secs(86_400), self.expires.timestamp_micros())?;
        Ok(())
    }
}

pub(super) async fn commit_bound(
    mut tx: sqlx::Transaction<'_, sqlx::Postgres>,
    bound: Option<&FanoutWriteDeadline>,
    commit_started: &mut bool,
) -> Result<()> {
    if let Some(bound) = bound {
        bound.arm(&mut tx).await?;
    }
    *commit_started = true;
    tx.commit().await?;
    if let Some(bound) = bound {
        bound.rpc.remaining(
            Duration::from_secs(86_400),
            bound.expires.timestamp_micros(),
        )?;
    }
    Ok(())
}

pub(super) async fn lock_complete_claim(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    claim: &TimelineFanoutClaimDto,
) -> Result<bool> {
    let found: Option<Uuid> = sqlx::query_scalar(
        "select job_id from feed_fanout_jobs where job_id=$1 and event_id=$2 \
         and post_id=$3 and author_id=$4 and lease_token=$5 and attempt_count=$6 \
         and cursor is not distinct from $7 and lease_expires_at=$8 \
         and post_created_at=$9 and page_attempt_count=$10 and max_page_attempts=$11 \
         and page_budget_provenance='CURRENT_PAGE' \
         and page_attempt_count between 1 and max_page_attempts \
         and status='PROCESSING' and completed_at is null and dead_lettered_at is null for update",
    )
    .bind(claim.job.job_id)
    .bind(claim.job.event_id)
    .bind(claim.job.post_id)
    .bind(claim.job.author_id)
    .bind(claim.lease_token)
    .bind(claim.job.attempt_count)
    .bind(&claim.job.cursor)
    .bind(claim.lease_expires_at)
    .bind(claim.job.post_created_at)
    .bind(claim.page_attempt_count)
    .bind(claim.max_page_attempts)
    .fetch_optional(&mut **tx)
    .await?;
    if found.is_none() {
        return Ok(false);
    }
    Ok(sqlx::query_scalar(
        "select lease_expires_at>clock_timestamp() from feed_fanout_jobs where job_id=$1",
    )
    .bind(claim.job.job_id)
    .fetch_one(&mut **tx)
    .await?)
}

#[async_trait]
impl ClaimedFanoutApplyPort for PgFeedRecommendationRepository {
    async fn apply_claimed_page(
        &self,
        claim: &TimelineFanoutClaimDto,
        page: &TimelineFanoutPageDto,
        deadline: DeliveryEffectDeadlineDto,
    ) -> Result<TimelineFanoutOutcomeDto> {
        let bound = (|| -> Result<_> {
            anyhow::ensure!(
                deadline.expires_at() <= claim.lease_expires_at,
                "Extended fanout authority"
            );
            validate_fanout_page_progress(claim.job.cursor.as_deref(), page)?;
            let mut seen = std::collections::HashSet::new();
            anyhow::ensure!(
                page.follower_ids
                    .iter()
                    .all(|id| !id.is_nil() && *id != claim.job.author_id && seen.insert(*id)),
                "Invalid fanout recipients"
            );
            FanoutWriteDeadline::new(deadline)
        })()
        .map_err(DeliveryEffectFailure::confirmed)?;
        let mut commit_started = false;
        let result = tokio::time::timeout_at(
            bound.rpc.stop(),
            self.apply_fanout_page(claim, page, Some(&bound), &mut commit_started),
        )
        .await;
        let result = match result {
            Ok(result) => result,
            Err(error) => Err(error.into()),
        };
        // Check again even if timeout_at returned a ready future after its stop.
        let result = result.and_then(|outcome| {
            bound.rpc.remaining(
                Duration::from_secs(86_400),
                bound.expires.timestamp_micros(),
            )?;
            Ok(outcome)
        });
        result.map_err(|error| {
            if commit_started {
                DeliveryEffectFailure::uncertain(error)
            } else {
                DeliveryEffectFailure::confirmed(error)
            }
        })
    }
}
