use crate::delivery_effect::{DeliveryEffectDeadlineDto, DeliveryEffectFailure};
use crate::dto::timeline_fanout_dto::{
    TimelineFanoutClaimDto, TimelineFanoutClaimRequestDto, TimelineFanoutFailureDto,
    TimelineFanoutOutcomeDto, TimelineFanoutPageDto,
};
use crate::fanout_authority::{ClaimedFanoutApplyPort, ClaimedFanoutReadPort, FanoutPageLookupDto};
use crate::ports::{FriendClientPort, TimelineFanoutRepositoryPort};
use anyhow::Result;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct TimelineFanoutConfig {
    pub claim_limit: i32,
    pub follower_page_size: i32,
    pub lease_timeout: Duration,
    pub max_attempts: i32,
    pub base_retry: Duration,
    pub entry_ttl: Duration,
}

impl Default for TimelineFanoutConfig {
    fn default() -> Self {
        Self {
            claim_limit: 20,
            follower_page_size: 100,
            lease_timeout: Duration::from_secs(60),
            max_attempts: 8,
            base_retry: Duration::from_secs(2),
            entry_ttl: Duration::from_secs(14 * 24 * 60 * 60),
        }
    }
}

#[derive(Clone)]
pub struct ProcessTimelineFanoutUseCase {
    repository: Arc<dyn TimelineFanoutRepositoryPort>,
    reader: FanoutReader,
    config: TimelineFanoutConfig,
}

#[derive(Clone)]
enum FanoutReader {
    Legacy(Arc<dyn FriendClientPort>),
    Claimed {
        reader: Arc<dyn ClaimedFanoutReadPort>,
        writer: Arc<dyn ClaimedFanoutApplyPort>,
    },
}

impl ProcessTimelineFanoutUseCase {
    pub fn new(
        repository: Arc<dyn TimelineFanoutRepositoryPort>,
        friend_client: Arc<dyn FriendClientPort>,
        config: TimelineFanoutConfig,
    ) -> Self {
        Self {
            repository,
            reader: FanoutReader::Legacy(friend_client),
            config,
        }
    }

    /// Explicit composition selects system authority. A claimed worker never
    /// holds a generic Friend client, even when admission is unavailable.
    pub fn new_claimed(
        repository: Arc<dyn TimelineFanoutRepositoryPort>,
        reader: Arc<dyn ClaimedFanoutReadPort>,
        writer: Arc<dyn ClaimedFanoutApplyPort>,
        config: TimelineFanoutConfig,
    ) -> Result<Self> {
        let lease_ms = i64::try_from(config.lease_timeout.as_millis())?;
        let retry_ms = i64::try_from(config.base_retry.as_millis())?;
        anyhow::ensure!(
            (1..=100).contains(&config.claim_limit),
            "Invalid fanout claim limit"
        );
        anyhow::ensure!(
            (1..=100).contains(&config.follower_page_size),
            "Invalid fanout page size"
        );
        anyhow::ensure!(
            (1..=100).contains(&config.max_attempts),
            "Invalid fanout attempt limit"
        );
        anyhow::ensure!(
            (1..=86_400_000).contains(&lease_ms)
                && (100..=3_600_000).contains(&retry_ms)
                && !config.entry_ttl.is_zero(),
            "Invalid fanout duration"
        );
        ChronoDuration::from_std(config.entry_ttl)?;
        Ok(Self {
            repository,
            reader: FanoutReader::Claimed { reader, writer },
            config,
        })
    }

    pub async fn execute(&self, now: DateTime<Utc>) -> Result<i64> {
        let mut completed_pages = 0_i64;
        let mut seen_jobs = Vec::new();
        for _ in 0..self.config.claim_limit.clamp(1, 100) {
            // Claim immediately before the page fetch. Later jobs must not
            // spend their leases waiting behind this page's network call.
            let observed = std::time::Instant::now();
            let Some(job) = self
                .repository
                .claim_jobs(&TimelineFanoutClaimRequestDto {
                    lease_timeout: self.config.lease_timeout,
                    limit: 1,
                    max_page_attempts: self.config.max_attempts,
                    retry_base: self.config.base_retry,
                    excluded_job_ids: seen_jobs.clone(),
                })
                .await?
                .into_iter()
                .next()
            else {
                break;
            };
            seen_jobs.push(job.job.job_id);
            let stop = observed + self.config.lease_timeout;
            let remaining = stop.saturating_duration_since(std::time::Instant::now());
            let expires_at = job
                .lease_expires_at
                .min(Utc::now() + ChronoDuration::from_std(remaining)?);
            let deadline = DeliveryEffectDeadlineDto::new(expires_at, stop)?;
            match self.process_job(&job, now, deadline).await {
                Ok(TimelineFanoutOutcomeDto::Applied) => completed_pages += 1,
                Ok(TimelineFanoutOutcomeDto::Lost | TimelineFanoutOutcomeDto::Suspended) => {}
                Err(error) => {
                    if matches!(&self.reader, FanoutReader::Claimed { .. })
                        && DeliveryEffectFailure::classify(&error)
                            == DeliveryEffectFailure::Uncertain
                    {
                        tracing::warn!(job_id=%job.job.job_id,"timeline fanout outcome uncertain; preserve original lease");
                        continue;
                    }
                    self.repository
                        .record_job_failure(
                            &job,
                            &TimelineFanoutFailureDto {
                                message: error.to_string(),
                            },
                        )
                        .await?;
                    tracing::warn!(error = ?error, job_id = %job.job.job_id, "timeline fanout page failed");
                }
            }
        }
        Ok(completed_pages)
    }

    async fn process_job(
        &self,
        claim: &TimelineFanoutClaimDto,
        _now: DateTime<Utc>,
        deadline: DeliveryEffectDeadlineDto,
    ) -> Result<TimelineFanoutOutcomeDto> {
        let job = &claim.job;
        let (follower_ids, next_cursor, authority_expiry) = match &self.reader {
            FanoutReader::Legacy(friend) => {
                let page = friend
                    .fetch_follower_page(
                        job.author_id,
                        job.cursor.as_deref(),
                        self.config.follower_page_size,
                    )
                    .await?;
                (page.follower_ids, page.next_cursor, deadline.expires_at())
            }
            FanoutReader::Claimed { reader, .. } => {
                let request =
                    FanoutPageLookupDto::new(claim.clone(), self.config.follower_page_size)
                        .map_err(anyhow::Error::from)
                        .and_then(|request| request.within(deadline))
                        .map_err(DeliveryEffectFailure::confirmed)?;
                let page = reader
                    .fetch_claimed_page(&request)
                    .await
                    .map_err(DeliveryEffectFailure::confirmed)?;
                page.validate(&request)
                    .map_err(DeliveryEffectFailure::confirmed)?;
                page.into_parts()
            }
        };
        let page = TimelineFanoutPageDto {
            follower_ids,
            next_cursor,
            expires_at: job.post_created_at + ChronoDuration::from_std(self.config.entry_ttl)?,
        };
        match &self.reader {
            FanoutReader::Legacy(_) => self.repository.apply_page(claim, &page).await,
            FanoutReader::Claimed { writer, .. } => writer
                .apply_claimed_page(claim, &page, deadline.clipped_to(authority_expiry))
                .await
                .map_err(|error| {
                    if DeliveryEffectFailure::classify(&error) == DeliveryEffectFailure::Uncertain {
                        DeliveryEffectFailure::uncertain(error)
                    } else {
                        error
                    }
                }),
        }
    }
}
