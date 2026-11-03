mod claimed_fanout;
use super::fanout_budget_sql;
use super::subject_locks::{forget_affinity_sources, lock_post_subjects};
use crate::application::dto::mutation_dto::MutationError;
use crate::application::dto::recommendation_retry_dto::{
    RecommendationAdmissionDto, RecommendationRetryContextDto,
};
use crate::application::dto::timeline_fanout_dto::{
    validate_fanout_page_progress, TimelineFanoutClaimDto, TimelineFanoutClaimRequestDto,
    TimelineFanoutFailureDto, TimelineFanoutOutcomeDto, TimelineFanoutPageDto,
};
use crate::application::ports::{
    FeedCandidateSourceRepositoryPort, FeedFeatureRepositoryPort,
    FeedRecommendationMaintenancePort, FeedRecommendationRepositoryPort, RecommendationRollout,
    TimelineFanoutRepositoryPort,
};
use crate::domain::ranking::{AuthorDeliveryMode, FeedEngagementAction};
use crate::domain::recommendation::{
    FeedCandidate, FeedCandidateSource, FeedRecommendationImpression,
    FeedRecommendationRetentionResult, FeedSnapshot, FeedSnapshotCursor, FeedSnapshotItem,
    FeedSnapshotPage, RecommendationAction, RecommendationEvent, TimelineFanoutJob,
};
use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use claimed_fanout::{commit_bound, lock_complete_claim, FanoutWriteDeadline};
use platform::model_manifest::{ModelManifest, ModelStatus};
use platform::recommendation::{RecentActionLimit, RolloutMode};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Clone)]
pub struct PgFeedRecommendationRepository {
    pool: PgPool,
}

impl PgFeedRecommendationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn apply_fanout_page(
        &self,
        claim: &TimelineFanoutClaimDto,
        page: &TimelineFanoutPageDto,
        bound: Option<&FanoutWriteDeadline>,
        commit_started: &mut bool,
    ) -> Result<TimelineFanoutOutcomeDto> {
        let mut tx = self.pool.begin().await?;
        if let Some(bound) = bound {
            bound.arm(&mut tx).await?;
        }
        let mut subjects = page.follower_ids.clone();
        subjects.push(claim.job.author_id);
        lock_post_subjects(&mut tx, claim.job.post_id, &subjects).await?;
        // Post-before-job is shared with deletion. Holding the post row also
        // serializes with owner soft-delete/moderation writes.
        if let Some(bound) = bound {
            bound.arm(&mut tx).await?;
        }
        let parent=sqlx::query("select author_id,deleted_at is null and moderation_hidden_at is null and visibility in ('PUBLIC','FRIENDS') as eligible from posts where id=$1 for update")
            .bind(claim.job.post_id).fetch_optional(&mut *tx).await?;
        if let Some(bound) = bound {
            bound.arm(&mut tx).await?;
        }
        let owned = if bound.is_some() {
            lock_complete_claim(&mut tx, claim).await?
        } else {
            lock_fanout_claim(&mut tx, claim).await?
        };
        if parent.is_none() || !owned {
            tx.rollback().await?;
            return Ok(TimelineFanoutOutcomeDto::Lost);
        }
        if let Some(bound) = bound {
            bound.arm(&mut tx).await?;
        }
        let parent = parent.unwrap();
        if parent.get::<Uuid, _>("author_id") != claim.job.author_id
            || !parent.get::<bool, _>("eligible")
        {
            let changed=sqlx::query("update feed_fanout_jobs set status='SUSPENDED',locked_at=null,lease_token=null,lease_expires_at=null,updated_at=clock_timestamp() where job_id=$1 and lease_token=$2 and lease_expires_at>clock_timestamp() and ($3::timestamptz is null or $3>clock_timestamp())")
                .bind(claim.job.job_id).bind(claim.lease_token).bind(bound.map(|b| b.expires)).execute(&mut *tx).await?.rows_affected();
            commit_bound(tx, bound, commit_started).await?;
            return Ok(if changed == 1 {
                TimelineFanoutOutcomeDto::Suspended
            } else {
                TimelineFanoutOutcomeDto::Lost
            });
        }
        let previous: Option<String> =
            sqlx::query_scalar("select cursor from feed_fanout_jobs where job_id=$1")
                .bind(claim.job.job_id)
                .fetch_one(&mut *tx)
                .await?;
        validate_fanout_page_progress(previous.as_deref(), page)?;
        if let Some(bound) = bound {
            bound.arm(&mut tx).await?;
        }
        sqlx::query(r#"
            insert into home_timeline_entries(viewer_id,post_id,author_id,source,source_score,inserted_at,expires_at)
            select follower_id,$2,$3,'MATERIALIZED_TIMELINE',1,$4,$5
            from unnest($1::uuid[]) follower_id
            on conflict(viewer_id,post_id,source) do nothing
        "#).bind(&page.follower_ids).bind(claim.job.post_id).bind(claim.job.author_id)
            .bind(claim.job.post_created_at).bind(page.expires_at).execute(&mut *tx).await?;
        if let Some(bound) = bound {
            bound.arm(&mut tx).await?;
        }
        // Recheck expiry after insertion waits; failure rolls back every entry.
        let changed = sqlx::query(
            r#"
            update feed_fanout_jobs set cursor=$4,
                status=case when $4::text is null then 'COMPLETED' else 'PENDING' end,
                completed_at=case when $4::text is null then clock_timestamp() else null end,
                locked_at=null,lease_token=null,lease_expires_at=null,last_error=null,
                page_attempt_count=0,next_attempt_at=clock_timestamp(),updated_at=clock_timestamp()
            where job_id=$1 and lease_token=$2 and attempt_count=$3 and status='PROCESSING'
              and lease_expires_at>clock_timestamp()
              and ($5::timestamptz is null or $5>clock_timestamp())
        "#,
        )
        .bind(claim.job.job_id)
        .bind(claim.lease_token)
        .bind(claim.job.attempt_count)
        .bind(&page.next_cursor)
        .bind(bound.map(|b| b.expires))
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if changed != 1 {
            tx.rollback().await?;
            return Ok(TimelineFanoutOutcomeDto::Lost);
        }
        commit_bound(tx, bound, commit_started).await?;
        Ok(TimelineFanoutOutcomeDto::Applied)
    }
}

#[async_trait]
impl FeedCandidateSourceRepositoryPort for PgFeedRecommendationRepository {
    async fn materialized_timeline_candidates(
        &self,
        viewer_id: Uuid,
        limit: i32,
        now: DateTime<Utc>,
    ) -> Result<Vec<FeedCandidate>> {
        let rows = sqlx::query(
            r#"
            select p.id as post_id, p.author_id, h.source, h.source_score,
                   coalesce(a.affinity_score, 0)::float8 as author_affinity,
                   greatest(0::float8, least(1::float8,
                     exp(-extract(epoch from ($3 - p.created_at)) / 259200::float8))) as freshness,
                   0::float8 as social_proof,
                   greatest(0::float8, least(1::float8,
                     ln(1 + greatest(p.like_count, 0) + 2 * greatest(p.comment_count, 0)) / 10::float8)) as engagement_velocity,
                   0::float8 as topic_match,
                   least(1::float8, coalesce(i.impression_count, 0)::float8 / 5::float8) as repeated_impression_penalty,
                   0::float8 as author_saturation_penalty,
                   least(1::float8, coalesce(a.negative_score, 0)) as negative_feedback_penalty,
                   p.created_at as generated_at,
                   'FOLLOWING_RECENT'::text as reason_code,
                   true as in_network,
                   false as exploration
            from home_timeline_entries h
            join posts p on p.id = h.post_id
            left join feed_author_affinities a
              on a.viewer_id = h.viewer_id and a.author_id = p.author_id
            left join lateral (
                select count(*)::bigint as impression_count
                from feed_recommendation_impressions fi
                where fi.viewer_id = h.viewer_id
                  and fi.post_id = h.post_id
                  and fi.occurred_at >= $3 - interval '7 days'
            ) i on true
            where h.viewer_id = $1
              and h.expires_at > $3
              and p.deleted_at is null
              and p.moderation_hidden_at is null
            order by h.inserted_at desc, h.post_id desc
            limit $2
            "#,
        )
        .bind(viewer_id)
        .bind(limit.clamp(1, 500))
        .bind(now)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(map_candidate).collect()
    }

    async fn high_fanout_candidates(
        &self,
        viewer_id: Uuid,
        following_ids: &[Uuid],
        limit: i32,
        now: DateTime<Utc>,
    ) -> Result<Vec<FeedCandidate>> {
        if following_ids.is_empty() {
            return Ok(Vec::new());
        }
        let rows = sqlx::query(
            r#"
            select p.id as post_id, p.author_id, 'HIGH_FANOUT_PULL'::text as source,
                   0.9::float8 as source_score,
                   coalesce(a.affinity_score, 0)::float8 as author_affinity,
                   greatest(0::float8, least(1::float8,
                     exp(-extract(epoch from ($4 - p.created_at)) / 172800::float8))) as freshness,
                   0::float8 as social_proof,
                   greatest(0::float8, least(1::float8,
                     ln(1 + greatest(p.like_count, 0) + 2 * greatest(p.comment_count, 0)) / 10::float8)) as engagement_velocity,
                   0::float8 as topic_match,
                   0::float8 as repeated_impression_penalty,
                   0::float8 as author_saturation_penalty,
                   least(1::float8, coalesce(a.negative_score, 0)) as negative_feedback_penalty,
                   p.created_at as generated_at,
                   'HIGH_FANOUT_FOLLOWING'::text as reason_code,
                   true as in_network,
                   false as exploration
            from high_fanout_posts hp
            join posts p on p.id = hp.post_id
            left join feed_author_affinities a
              on a.viewer_id = $1 and a.author_id = p.author_id
            where hp.author_id = any($2::uuid[])
              and hp.expires_at > $4
              and p.deleted_at is null
              and p.moderation_hidden_at is null
            order by p.created_at desc, p.id desc
            limit $3
            "#,
        )
        .bind(viewer_id)
        .bind(following_ids.to_vec())
        .bind(limit.clamp(1, 500))
        .bind(now)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(map_candidate).collect()
    }

    async fn author_affinity_candidates(
        &self,
        viewer_id: Uuid,
        following_ids: &[Uuid],
        limit: i32,
        now: DateTime<Utc>,
    ) -> Result<Vec<FeedCandidate>> {
        if following_ids.is_empty() {
            return Ok(Vec::new());
        }
        let rows = sqlx::query(
            r#"
            select p.id as post_id, p.author_id, 'AUTHOR_AFFINITY'::text as source,
                   least(1::float8, greatest(0::float8, a.affinity_score)) as source_score,
                   least(1::float8, greatest(0::float8, a.affinity_score)) as author_affinity,
                   greatest(0::float8, least(1::float8,
                     exp(-extract(epoch from ($4 - p.created_at)) / 259200::float8))) as freshness,
                   0::float8 as social_proof,
                   greatest(0::float8, least(1::float8,
                     ln(1 + greatest(p.like_count, 0) + 2 * greatest(p.comment_count, 0)) / 10::float8)) as engagement_velocity,
                   0::float8 as topic_match,
                   0::float8 as repeated_impression_penalty,
                   0::float8 as author_saturation_penalty,
                   least(1::float8, greatest(0::float8, a.negative_score)) as negative_feedback_penalty,
                   p.created_at as generated_at,
                   'AUTHOR_AFFINITY'::text as reason_code,
                   true as in_network,
                   false as exploration
            from feed_author_affinities a
            join lateral (
                select p.*
                from posts p
                where p.author_id = a.author_id
                  and p.deleted_at is null
                  and p.moderation_hidden_at is null
                order by p.created_at desc, p.id desc
                limit 3
            ) p on true
            where a.viewer_id = $1
              and a.author_id = any($2::uuid[])
            order by a.affinity_score desc, p.created_at desc, p.id desc
            limit $3
            "#,
        )
        .bind(viewer_id)
        .bind(following_ids.to_vec())
        .bind(limit.clamp(1, 500))
        .bind(now)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(map_candidate).collect()
    }

    async fn social_proof_candidates(
        &self,
        viewer_id: Uuid,
        following_ids: &[Uuid],
        limit: i32,
        now: DateTime<Utc>,
    ) -> Result<Vec<FeedCandidate>> {
        if following_ids.is_empty() {
            return Ok(Vec::new());
        }
        let rows = sqlx::query(
            r#"
            with social as (
                select pl.post_id, count(distinct pl.user_id)::bigint as proof
                from post_likes pl
                where pl.user_id = any($2::uuid[])
                  and pl.deleted_at is null
                  and pl.created_at >= $4 - interval '7 days'
                group by pl.post_id
                union all
                select c.post_id, count(distinct c.author_id)::bigint as proof
                from comments c
                where c.author_id = any($2::uuid[])
                  and c.deleted_at is null
                  and c.moderation_hidden_at is null
                  and c.created_at >= $4 - interval '7 days'
                group by c.post_id
            ), aggregated as (
                select post_id, sum(proof)::bigint as proof
                from social
                group by post_id
            )
            select p.id as post_id, p.author_id, 'SOCIAL_PROOF'::text as source,
                   least(1::float8, ln(1 + aggregated.proof)::float8 / 5::float8) as source_score,
                   coalesce(a.affinity_score, 0)::float8 as author_affinity,
                   greatest(0::float8, least(1::float8,
                     exp(-extract(epoch from ($4 - p.created_at)) / 259200::float8))) as freshness,
                   least(1::float8, ln(1 + aggregated.proof)::float8 / 5::float8) as social_proof,
                   greatest(0::float8, least(1::float8,
                     ln(1 + greatest(p.like_count, 0) + 2 * greatest(p.comment_count, 0)) / 10::float8)) as engagement_velocity,
                   0::float8 as topic_match,
                   0::float8 as repeated_impression_penalty,
                   0::float8 as author_saturation_penalty,
                   least(1::float8, coalesce(a.negative_score, 0)) as negative_feedback_penalty,
                   p.created_at as generated_at,
                   'FOLLOWING_ENGAGED'::text as reason_code,
                   (p.author_id = any($2::uuid[])) as in_network,
                   false as exploration
            from aggregated
            join posts p on p.id = aggregated.post_id
            left join feed_author_affinities a
              on a.viewer_id = $1 and a.author_id = p.author_id
            where p.author_id <> $1
              and p.visibility = 'PUBLIC'
              and p.deleted_at is null
              and p.moderation_hidden_at is null
            order by aggregated.proof desc, p.created_at desc, p.id desc
            limit $3
            "#,
        )
        .bind(viewer_id)
        .bind(following_ids.to_vec())
        .bind(limit.clamp(1, 500))
        .bind(now)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(map_candidate).collect()
    }

    async fn fresh_trending_candidates(
        &self,
        viewer_id: Uuid,
        limit: i32,
        now: DateTime<Utc>,
    ) -> Result<Vec<FeedCandidate>> {
        let rows = sqlx::query(
            r#"
            select p.id as post_id, p.author_id, 'FRESH_TRENDING'::text as source,
                   greatest(0::float8, least(1::float8,
                     ln(1 + greatest(p.like_count, 0) + 2 * greatest(p.comment_count, 0)) /
                     sqrt(1 + extract(epoch from ($3 - p.created_at)) / 3600::float8) / 10::float8)) as source_score,
                   coalesce(a.affinity_score, 0)::float8 as author_affinity,
                   greatest(0::float8, least(1::float8,
                     exp(-extract(epoch from ($3 - p.created_at)) / 172800::float8))) as freshness,
                   0::float8 as social_proof,
                   greatest(0::float8, least(1::float8,
                     ln(1 + greatest(p.like_count, 0) + 2 * greatest(p.comment_count, 0)) / 10::float8)) as engagement_velocity,
                   0::float8 as topic_match,
                   0::float8 as repeated_impression_penalty,
                   0::float8 as author_saturation_penalty,
                   least(1::float8, coalesce(a.negative_score, 0)) as negative_feedback_penalty,
                   p.created_at as generated_at,
                   'FRESH_TRENDING'::text as reason_code,
                   false as in_network,
                   false as exploration
            from posts p
            left join feed_author_affinities a
              on a.viewer_id = $1 and a.author_id = p.author_id
            where p.author_id <> $1
              and p.visibility = 'PUBLIC'
              and p.created_at >= $3 - interval '7 days'
              and p.deleted_at is null
              and p.moderation_hidden_at is null
            order by source_score desc, p.created_at desc, p.id desc
            limit $2
            "#,
        )
        .bind(viewer_id)
        .bind(limit.clamp(1, 500))
        .bind(now)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(map_candidate).collect()
    }

    async fn exploration_candidates(
        &self,
        viewer_id: Uuid,
        excluded_author_ids: &[Uuid],
        limit: i32,
        now: DateTime<Utc>,
    ) -> Result<Vec<FeedCandidate>> {
        let rows = sqlx::query(
            r#"
            select p.id as post_id, p.author_id, 'EXPLORATION'::text as source,
                   0.35::float8 as source_score,
                   0::float8 as author_affinity,
                   greatest(0::float8, least(1::float8,
                     exp(-extract(epoch from ($4 - p.created_at)) / 86400::float8))) as freshness,
                   0::float8 as social_proof,
                   greatest(0::float8, least(1::float8,
                     ln(1 + greatest(p.like_count, 0) + 2 * greatest(p.comment_count, 0)) / 10::float8)) as engagement_velocity,
                   0::float8 as topic_match,
                   0::float8 as repeated_impression_penalty,
                   0::float8 as author_saturation_penalty,
                   0::float8 as negative_feedback_penalty,
                   p.created_at as generated_at,
                   'DISCOVERY_FRESH'::text as reason_code,
                   false as in_network,
                   true as exploration
            from posts p
            where p.author_id <> $1
              and not (p.author_id = any($2::uuid[]))
              and p.visibility = 'PUBLIC'
              and p.created_at >= $4 - interval '48 hours'
              and p.deleted_at is null
              and p.moderation_hidden_at is null
            order by p.created_at desc, p.id desc
            limit $3
            "#,
        )
        .bind(viewer_id)
        .bind(excluded_author_ids.to_vec())
        .bind(limit.clamp(1, 500))
        .bind(now)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(map_candidate).collect()
    }
}

#[async_trait]
impl FeedRecommendationRepositoryPort for PgFeedRecommendationRepository {
    async fn create_snapshot(&self, snapshot: &FeedSnapshot) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let mut subjects = vec![snapshot.viewer_id];
        subjects.extend(snapshot.items.iter().map(|item| item.author_id));
        sqlx::query("select post_lock_subject_ids($1::uuid[])")
            .bind(subjects)
            .execute(&mut *tx)
            .await?;

        sqlx::query(
            r#"
            insert into feed_recommendation_snapshots (
                snapshot_id, viewer_id, request_id, session_id, rank_version,
                fallback_used, failed_sources, created_at, expires_at
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            on conflict (snapshot_id) do nothing
            "#,
        )
        .bind(snapshot.snapshot_id)
        .bind(snapshot.viewer_id)
        .bind(&snapshot.request_id)
        .bind(snapshot.session_id)
        .bind(&snapshot.rank_version)
        .bind(snapshot.fallback_used)
        .bind(
            snapshot
                .failed_sources
                .iter()
                .map(|source| source.as_str().to_owned())
                .collect::<Vec<_>>(),
        )
        .bind(snapshot.created_at)
        .bind(snapshot.expires_at)
        .execute(&mut *tx)
        .await?;

        for item in &snapshot.items {
            sqlx::query(
                r#"
                insert into feed_recommendation_snapshot_items (
                    snapshot_id, position, post_id, author_id, source, reason_code,
                    score_micros, impression_id, propensity_micros
                ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                on conflict (snapshot_id, position) do nothing
                "#,
            )
            .bind(snapshot.snapshot_id)
            .bind(i32::try_from(item.position)?)
            .bind(item.post_id)
            .bind(item.author_id)
            .bind(item.source.as_str())
            .bind(&item.reason_code)
            .bind(item.score_micros)
            .bind(item.impression_id)
            .bind(i32::try_from(item.propensity_micros)?)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn load_snapshot_page(
        &self,
        viewer_id: Uuid,
        snapshot_id: Uuid,
        offset: u32,
        limit: i32,
        now: DateTime<Utc>,
    ) -> Result<Option<FeedSnapshotPage>> {
        let header = sqlx::query(
            r#"
            select request_id, session_id, rank_version, fallback_used, failed_sources
            from feed_recommendation_snapshots
            where snapshot_id = $1 and viewer_id = $2 and expires_at > $3
            "#,
        )
        .bind(snapshot_id)
        .bind(viewer_id)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?;
        let Some(header) = header else {
            return Ok(None);
        };
        let page_limit = limit.clamp(1, 100);
        let rows = sqlx::query(
            r#"
            select post_id, author_id, source, reason_code, score_micros,
                   position, impression_id, propensity_micros
            from feed_recommendation_snapshot_items
            where snapshot_id = $1 and position >= $2
            order by position asc
            limit $3
            "#,
        )
        .bind(snapshot_id)
        .bind(i32::try_from(offset)?)
        .bind(page_limit + 1)
        .fetch_all(&self.pool)
        .await?;
        let has_more = rows.len() > page_limit as usize;
        let items = rows
            .into_iter()
            .take(page_limit as usize)
            .map(map_snapshot_item)
            .collect::<Result<Vec<_>>>()?;
        let next_cursor = has_more.then(|| {
            FeedSnapshotCursor {
                snapshot_id,
                offset: offset + items.len() as u32,
            }
            .encode()
        });
        Ok(Some(FeedSnapshotPage {
            snapshot_id,
            request_id: header.get("request_id"),
            session_id: header.get("session_id"),
            rank_version: header.get("rank_version"),
            fallback_used: header.get("fallback_used"),
            failed_sources: header
                .get::<Vec<String>, _>("failed_sources")
                .into_iter()
                .map(|source| FeedCandidateSource::parse(&source).map_err(Into::into))
                .collect::<Result<Vec<_>>>()?,
            items,
            next_cursor,
        }))
    }

    async fn record_impressions(&self, impressions: &[FeedRecommendationImpression]) -> Result<()> {
        if impressions.is_empty() {
            return Ok(());
        }
        let mut tx = self.pool.begin().await?;
        // Automatic IMPRESSION event bodies share the global event-ID key.
        // Lock the full sorted batch before subjects, just like outcome writers.
        let event_ids: Vec<Uuid> = impressions.iter().map(|i| i.impression_id).collect();
        sqlx::query("select pg_advisory_xact_lock(1380270915,key) from (select distinct hashtext(id::text) as key from unnest($1::uuid[]) id order by key) event_keys")
            .bind(event_ids).execute(&mut *tx).await?;
        let mut subjects: Vec<Uuid> = impressions
            .iter()
            .flat_map(|i| [i.viewer_id, i.author_id])
            .collect();
        let posts: Vec<Uuid> = impressions.iter().map(|i| i.post_id).collect();
        let owners: Vec<Uuid> =
            sqlx::query_scalar("select author_id from posts where id=any($1::uuid[])")
                .bind(posts)
                .fetch_all(&mut *tx)
                .await?;
        subjects.extend(owners);
        sqlx::query("select post_lock_subject_ids($1::uuid[])")
            .bind(subjects)
            .execute(&mut *tx)
            .await?;

        for impression in impressions {
            sqlx::query(
                r#"
                insert into feed_recommendation_impressions (
                    impression_id, snapshot_id, viewer_id, post_id, author_id,
                    source, reason_code, position, rank_version, request_id,
                    session_id, score_micros, propensity_micros, occurred_at
                ) values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)
                on conflict (impression_id) do nothing
                "#,
            )
            .bind(impression.impression_id)
            .bind(impression.snapshot_id)
            .bind(impression.viewer_id)
            .bind(impression.post_id)
            .bind(impression.author_id)
            .bind(impression.source.as_str())
            .bind(&impression.reason_code)
            .bind(i32::try_from(impression.position)?)
            .bind(&impression.rank_version)
            .bind(&impression.request_id)
            .bind(impression.session_id)
            .bind(impression.score_micros)
            .bind(i32::try_from(impression.propensity_micros)?)
            .bind(impression.occurred_at)
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                r#"
                insert into feed_recommendation_events (
                    event_id, schema_version, occurred_at, viewer_id, subject_type,
                    subject_id, actor_id, surface, position, request_id, session_id,
                    action, source, model_version, impression_id, propensity_micros,
                    dwell_ms, metadata_json
                ) values ($1,$2,$3,$4,'POST',$5,$4,'HOME_FEED',$6,$7,$8,
                          'IMPRESSION',$9,$10,$1,$11,null,'{}')
                on conflict (event_id) do nothing
                "#,
            )
            .bind(impression.impression_id)
            .bind(i32::from(
                platform::recommendation::RECOMMENDATION_SCHEMA_VERSION,
            ))
            .bind(impression.occurred_at)
            .bind(impression.viewer_id)
            .bind(impression.post_id)
            .bind(i32::try_from(impression.position)?)
            .bind(&impression.request_id)
            .bind(impression.session_id)
            .bind(impression.source.as_str())
            .bind(&impression.rank_version)
            .bind(i32::try_from(impression.propensity_micros)?)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn append_event(&self, event: &RecommendationEvent) -> Result<bool> {
        let mut tx = self.pool.begin().await?;
        let accepted = append_event_in_transaction(&mut tx, event).await?;
        if accepted {
            tx.commit().await?;
        } else {
            tx.rollback().await?;
        }
        Ok(accepted)
    }

    async fn append_recommendation_request(
        &self,
        request: &RecommendationAdmissionDto,
    ) -> Result<bool> {
        let mut tx = self.pool.begin().await?;
        lock_recommendation_event(&mut tx, request.event_id).await?;
        let mut subjects = vec![request.viewer_id];
        subjects.extend(request.actor_id);
        let owners:Vec<Uuid>=sqlx::query_scalar("select author_id from posts where id=$1 union select author_id from (select author_id from feed_recommendation_impressions where viewer_id=$2 and post_id=$1 order by occurred_at desc,impression_id desc limit 1) latest")
            .bind(request.post_id).bind(request.viewer_id).fetch_all(&mut *tx).await?;
        subjects.extend(owners);
        if retry_context_installed(&mut tx).await? {
            let retained:Vec<Uuid>=sqlx::query_scalar("select id from feed_affinity_resolution_receipts r cross join lateral unnest(array[r.viewer_id,r.author_id,r.actor_id]) id where r.namespace='RECOMMENDATION' and r.source_id=$1 and id is not null")
                .bind(request.event_id).fetch_all(&mut *tx).await?;
            subjects.extend(retained);
        }
        sqlx::query("select post_lock_subject_ids($1::uuid[])")
            .bind(&subjects)
            .execute(&mut *tx)
            .await?;
        if retry_context_installed(&mut tx).await? {
            let same_subject:Option<bool>=sqlx::query_scalar("select viewer_id=$2 and post_id=$3 from feed_affinity_resolution_receipts where namespace='RECOMMENDATION' and source_id=$1")
                .bind(request.event_id).bind(request.viewer_id).bind(request.post_id).fetch_optional(&mut *tx).await?;
            if same_subject == Some(false) {
                return Err(MutationError::Conflict.into());
            }
        }
        let retained = retry_context(
            &mut tx,
            request.event_id,
            request.viewer_id,
            request.post_id,
        )
        .await?;
        let defaults = if let Some(retained) = retained {
            retained
        } else {
            let owner: Option<Uuid> =
                sqlx::query_scalar("select author_id from posts where id=$1 for update")
                    .bind(request.post_id)
                    .fetch_optional(&mut *tx)
                    .await?;
            anyhow::ensure!(
                owner.is_none_or(|id| subjects.contains(&id)),
                "Recommendation owner changed during locking"
            );
            let row=sqlx::query("select impression_id,snapshot_id,viewer_id,post_id,author_id,source,reason_code,position,rank_version,request_id,session_id,score_micros,propensity_micros,occurred_at from feed_recommendation_impressions where viewer_id=$1 and post_id=$2 order by occurred_at desc,impression_id desc limit 1 for share")
                .bind(request.viewer_id).bind(request.post_id).fetch_optional(&mut *tx).await?
                .ok_or_else(||anyhow::anyhow!("Feed impression not found"))?;
            let impression = map_impression(row)?;
            anyhow::ensure!(
                subjects.contains(&impression.author_id),
                "Recommendation impression owner changed during locking"
            );
            anyhow::ensure!(
                request
                    .impression_id
                    .is_none_or(|id| id == impression.impression_id),
                "Feed impression not found"
            );
            impression.into()
        };
        let event = request.normalize(defaults)?;
        let accepted = append_event_in_transaction(&mut tx, &event).await?;
        if accepted {
            tx.commit().await?;
        } else {
            tx.rollback().await?;
        }
        Ok(accepted)
    }

    async fn latest_impression(
        &self,
        viewer_id: Uuid,
        post_id: Uuid,
    ) -> Result<Option<FeedRecommendationImpression>> {
        let row = sqlx::query(
            r#"
            select impression_id, snapshot_id, viewer_id, post_id, author_id,
                   source, reason_code, position, rank_version, request_id,
                   session_id, score_micros, propensity_micros, occurred_at
            from feed_recommendation_impressions
            where viewer_id = $1 and post_id = $2
            order by occurred_at desc, impression_id desc
            limit 1
            "#,
        )
        .bind(viewer_id)
        .bind(post_id)
        .fetch_optional(&self.pool)
        .await?;
        row.map(map_impression).transpose()
    }

    async fn cleanup_user(&self, user_id: Uuid) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("select post_lock_subject_ids($1::uuid[])")
            .bind(vec![user_id])
            .execute(&mut *tx)
            .await?;
        forget_affinity_sources(&mut tx, user_id).await?;
        for query in [
            "delete from feed_fanout_jobs where author_id = $1",
            "delete from feed_recommendation_events where viewer_id = $1 or actor_id = $1",
            "delete from feed_recommendation_impressions where viewer_id = $1 or author_id = $1",
            "delete from feed_recommendation_snapshots where viewer_id = $1",
            "delete from feed_author_affinities where viewer_id = $1 or author_id = $1",
            "delete from feed_experiment_assignments where viewer_id = $1",
            "delete from feed_recent_actions where viewer_id = $1",
            "delete from feed_embeddings where subject_id = $1",
            "delete from home_timeline_entries where viewer_id = $1 or author_id = $1",
            "delete from high_fanout_posts where author_id = $1",
        ] {
            sqlx::query(query).bind(user_id).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn model_ready(&self, model_version: &str) -> Result<bool> {
        let row = sqlx::query(
            r#"
            select m.model_name, m.model_version, m.model_kind, m.rollout_mode,
                   m.status, m.artifact_uri, m.artifact_sha256,
                   m.feature_schema_version, m.embedding_dimension,
                   m.fallback_version, m.enabled, m.activated_at,
                   coalesce(readiness.ready, false) as data_ready,
                   exists(
                       select 1 from feed_recommendation_models fallback
                       where fallback.model_version = m.fallback_version
                         and fallback.model_kind = 'LINEAR'
                         and fallback.rollout_mode = 'DETERMINISTIC'
                         and fallback.status in ('READY', 'ACTIVE')
                         and fallback.enabled = true
                   ) as fallback_ready
            from feed_recommendation_models m
            left join feed_recommendation_model_readiness readiness
              on readiness.model_version = m.model_version
            where m.model_version = $1
            "#,
        )
        .bind(model_version)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(false);
        };
        if !row.get::<bool, _>("enabled") {
            return Ok(false);
        }
        let status = ModelStatus::parse(row.get::<String, _>("status").as_str())?;
        if !status.can_serve() {
            return Ok(false);
        }
        let model_kind = row.get::<String, _>("model_kind");
        if model_kind.eq_ignore_ascii_case("LINEAR") {
            return Ok(row
                .get::<String, _>("rollout_mode")
                .eq_ignore_ascii_case("DETERMINISTIC")
                && !row
                    .get::<String, _>("feature_schema_version")
                    .trim()
                    .is_empty());
        }
        if !row.get::<bool, _>("fallback_ready") || !row.get::<bool, _>("data_ready") {
            return Ok(false);
        }
        let manifest = feed_model_manifest_from_row(&row)?;
        Ok(manifest.validate(true).is_ok())
    }

    async fn rollout(&self, surface: &str) -> Result<RecommendationRollout> {
        let row = sqlx::query(
            r#"
            select r.mode, r.deterministic_version, r.learned_version, r.canary_percent,
                   m.model_name, m.model_kind, m.status, m.artifact_uri,
                   m.artifact_sha256, m.feature_schema_version, m.fallback_version,
                   m.embedding_dimension, m.enabled, m.activated_at,
                   coalesce(readiness.ready, false) as data_ready,
                   exists(
                       select 1 from feed_recommendation_models fallback
                       where fallback.model_version = m.fallback_version
                         and fallback.model_kind = 'LINEAR'
                         and fallback.status in ('READY', 'ACTIVE')
                         and fallback.enabled = true
                   ) as fallback_ready
            from feed_recommendation_rollouts r
            left join feed_recommendation_models m
              on m.model_version = r.learned_version
            left join feed_recommendation_model_readiness readiness
              on readiness.model_version = r.learned_version
            where r.surface = $1
            "#,
        )
        .bind(surface)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(RecommendationRollout::deterministic("feed-linear-v1"));
        };
        let learned_version = row.get::<Option<String>, _>("learned_version");
        let learned_ready = learned_version.as_ref().is_some_and(|version| {
            feed_model_manifest_from_row_with_version(version, &row)
                .map(|manifest| manifest.validate(true).is_ok() && manifest.status.can_serve())
                .unwrap_or(false)
                && row.get::<Option<bool>, _>("enabled").unwrap_or(false)
                && row.get::<bool, _>("data_ready")
                && row.get::<bool, _>("fallback_ready")
                && !row
                    .get::<Option<String>, _>("model_kind")
                    .unwrap_or_default()
                    .eq_ignore_ascii_case("LINEAR")
        });
        Ok(RecommendationRollout {
            mode: RolloutMode::parse(row.get::<String, _>("mode").as_str())?,
            deterministic_version: row.get("deterministic_version"),
            learned_version,
            canary_percent: row.get::<i32, _>("canary_percent").clamp(0, 100) as u8,
            learned_ready,
        })
    }

    async fn recent_actions(
        &self,
        viewer_id: Uuid,
        limit: RecentActionLimit,
    ) -> Result<Vec<RecommendationEvent>> {
        let rows = sqlx::query(
            r#"
            select event_id, schema_version, occurred_at, viewer_id, subject_type,
                   subject_id, actor_id, surface, position, request_id, session_id,
                   action, source, model_version, impression_id, propensity_micros,
                   dwell_ms, metadata_json::text as metadata_json
            from feed_recommendation_events
            where viewer_id = $1
            order by occurred_at desc, event_id desc
            limit $2
            "#,
        )
        .bind(viewer_id)
        .bind(i64::from(limit.get()))
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(map_recommendation_event).collect()
    }
}

async fn append_event_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    event: &RecommendationEvent,
) -> Result<bool> {
    event.validate()?;
    lock_recommendation_event(tx, event.event_id).await?;
    anyhow::ensure!(
        event.subject_type == crate::domain::recommendation::RecommendationSubjectType::Post,
        "Feed outcome requires a post subject"
    );
    let mut subjects = vec![event.viewer_id];
    subjects.extend(event.actor_id);
    let related:Vec<Uuid>=sqlx::query_scalar("select author_id from posts where $1='POST' and id=$2 union select author_id from comments where $1='COMMENT' and id=$2 union select $2::uuid where $1='USER' union select author_id from posts where id=$2 union select author_id from feed_recommendation_impressions where impression_id=$3 and viewer_id=$4 and post_id=$2")
            .bind(event.subject_type.as_str()).bind(event.subject_id).bind(event.impression_id).bind(event.viewer_id).fetch_all(&mut **tx).await?;
    subjects.extend(related);
    if retry_context_installed(tx).await? {
        let retained: Vec<Uuid> = sqlx::query_scalar("select id from feed_affinity_resolution_receipts r cross join lateral unnest(array[r.viewer_id,r.author_id,r.actor_id]) id where r.namespace='RECOMMENDATION' and r.source_id=$1 and id is not null")
                .bind(event.event_id).fetch_all(&mut **tx).await?;
        subjects.extend(retained);
    }
    sqlx::query("select post_lock_subject_ids($1::uuid[])")
        .bind(&subjects)
        .execute(&mut **tx)
        .await?;

    // Durable exact duplicates need no new impression admission. Compare
    // the persisted representation (PG timestamp precision / JSONB / micros)
    // rather than treating every collision as the same logical outcome.
    if let Some(matches) = recorded_recommendation_event_matches(tx, event).await? {
        if !matches {
            return Err(MutationError::Conflict.into());
        }
        return Ok(false);
    }
    // Source INSERT already allocates/locks the affinity pair. Establish
    // subject -> post -> impression/source -> pair order before that insert,
    // so a later ordered reducer never discovers a post lock afterward.
    // Presence/visibility of the Post is not a new admission condition:
    // the existing retained impression remains the authoritative source.
    let post_author: Option<Uuid> =
        sqlx::query_scalar("select author_id from posts where id=$1 for update")
            .bind(event.subject_id)
            .fetch_optional(&mut **tx)
            .await?;
    anyhow::ensure!(
        post_author.is_none_or(|author| subjects.contains(&author)),
        "Recommendation post owner changed during locking"
    );
    // Parsed requests normalize inside this transaction; direct normalized
    // callers may have read earlier. Lock and recheck the actual source
    // before inserting any event/recent-action/feature row; retain the lock
    // until all effects commit so retention must skip or wait.
    let impression = sqlx::query(
            "select author_id from feed_recommendation_impressions where impression_id=$1 and viewer_id=$2 and post_id=$3 for share",
        ).bind(event.impression_id).bind(event.viewer_id).bind(event.subject_id)
            .fetch_optional(&mut **tx).await?
            .ok_or_else(||anyhow::anyhow!("Feed impression not found"))?;
    let author_id: Uuid = impression.get("author_id");
    anyhow::ensure!(
        subjects.contains(&author_id),
        "Recommendation impression owner changed during locking"
    );

    let inserted = sqlx::query(
        r#"
            insert into feed_recommendation_events (
                event_id, schema_version, occurred_at, viewer_id, subject_type,
                subject_id, actor_id, surface, position, request_id, session_id,
                action, source, model_version, impression_id, propensity_micros,
                dwell_ms, metadata_json
            ) values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18::jsonb)
            on conflict (event_id) do nothing
            "#,
    )
    .bind(event.event_id)
    .bind(i32::from(event.schema_version))
    .bind(event.occurred_at)
    .bind(event.viewer_id)
    .bind(event.subject_type.as_str())
    .bind(event.subject_id)
    .bind(event.actor_id)
    .bind(event.surface.as_str())
    .bind(event.position)
    .bind(&event.request_id)
    .bind(event.session_id)
    .bind(event.action.as_str())
    .bind(&event.source)
    .bind(&event.model_version)
    .bind(event.impression_id)
    .bind(
        event
            .propensity
            .map(|value| (value * 1_000_000.0).round() as i32),
    )
    .bind(event.dwell_ms.map(|value| value as i32))
    .bind(&event.metadata_json)
    .execute(&mut **tx)
    .await?
    .rows_affected()
        == 1;
    if !inserted {
        // Different subject sets can concurrently race on the global event
        // ID. ON CONFLICT alone cannot prove their immutable identities match.
        if recorded_recommendation_event_matches(tx, event).await? != Some(true) {
            return Err(MutationError::Conflict.into());
        }
        return Ok(false);
    }

    sqlx::query(
        r#"
            insert into feed_recent_actions (event_id, viewer_id, post_id, action, occurred_at)
            values ($1, $2, $3, $4, $5)
            on conflict (event_id) do nothing
            "#,
    )
    .bind(event.event_id)
    .bind(event.viewer_id)
    .bind(event.subject_id)
    .bind(event.action.as_str())
    .bind(event.occurred_at)
    .execute(&mut **tx)
    .await?;

    {
        let (positive, negative) = affinity_delta(event.action, event.dwell_ms);
        if !super::affinity_provenance::apply_legacy_effect(
            tx,
            &super::affinity_provenance::LegacyAffinityMutationDto {
                namespace: "RECOMMENDATION",
                source_id: event.event_id,
                viewer_id: event.viewer_id,
                author_id,
                actor_id: event.actor_id,
                post_id: event.subject_id,
                action: event.action.as_str(),
                occurred_at: event.occurred_at,
                positive,
                negative,
            },
        )
        .await?
        {
            sqlx::query(
                r#"
                insert into feed_author_affinities (
                    viewer_id, author_id, affinity_score, negative_score,
                    last_interacted_at, feature_version
                ) values ($1, $2, $3, $4, $5, 'feed-affinity-v1')
                on conflict (viewer_id, author_id) do update
                set affinity_score = greatest(0, least(1,
                        feed_author_affinities.affinity_score * 0.95 + excluded.affinity_score)),
                    negative_score = greatest(0, least(1,
                        feed_author_affinities.negative_score * 0.95 + excluded.negative_score)),
                    last_interacted_at = greatest(feed_author_affinities.last_interacted_at, excluded.last_interacted_at),
                    feature_version = excluded.feature_version
                "#,
            )
            .bind(event.viewer_id)
            .bind(author_id)
            .bind(positive)
            .bind(negative)
            .bind(event.occurred_at)
            .execute(&mut **tx)
            .await?;
        }
    }
    Ok(true)
}

// Two-integer advisory keys use a separate PG lock namespace from the bigint
// subject keys. Hash collisions only serialize unrelated identities.
async fn lock_recommendation_event(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    event_id: Uuid,
) -> Result<()> {
    sqlx::query("select pg_advisory_xact_lock(1380270915,hashtext($1::text))")
        .bind(event_id.to_string())
        .execute(&mut **tx)
        .await?;
    Ok(())
}

async fn retry_context(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    event_id: Uuid,
    viewer_id: Uuid,
    post_id: Uuid,
) -> Result<Option<RecommendationRetryContextDto>> {
    if !retry_context_installed(tx).await? {
        return Ok(None);
    }
    let row=sqlx::query("select c.impression_id,c.position,c.request_id,c.session_id,c.source,c.model_version,c.propensity_micros from feed_recommendation_retry_contexts c join feed_affinity_resolution_receipts r on r.namespace=c.namespace and r.source_id=c.source_id where c.source_id=$1 and c.viewer_id=$2 and c.post_id=$3 and c.normalization_version='feed-recommendation-v1'")
        .bind(event_id).bind(viewer_id).bind(post_id).fetch_optional(&mut **tx).await?;
    row.map(|r| -> Result<_> {
        Ok(RecommendationRetryContextDto {
            impression_id: r.try_get("impression_id")?,
            position: r.try_get("position")?,
            request_id: r.try_get("request_id")?,
            session_id: r.try_get("session_id")?,
            source: FeedCandidateSource::parse(&r.try_get::<String, _>("source")?)?
                .as_str()
                .into(),
            model_version: r.try_get("model_version")?,
            propensity_micros: u32::try_from(r.try_get::<i32, _>("propensity_micros")?)?,
        })
    })
    .transpose()
}

/// Immutable event identity in its persisted normalized representation. Caller
/// and source linkage are part of identity; neither conflicts nor DB details
/// expose the previously recorded body.
async fn recorded_recommendation_event_matches(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    event: &RecommendationEvent,
) -> Result<Option<bool>> {
    let retained = sqlx::query_scalar(
        r#"select (schema_version,occurred_at,viewer_id,subject_type,subject_id,
                   actor_id,surface,position,request_id,session_id,action,source,
                   model_version,impression_id,propensity_micros,dwell_ms,metadata_json)
              is not distinct from
                  ($2::integer,$3::timestamptz,$4::uuid,$5::text,$6::uuid,
                   $7::uuid,$8::text,$9::integer,$10::text,$11::uuid,$12::text,$13::text,
                   $14::text,$15::uuid,$16::integer,$17::integer,$18::jsonb)
            from feed_recommendation_events where event_id=$1 for share"#,
    )
    .bind(event.event_id)
    .bind(i32::from(event.schema_version))
    .bind(event.occurred_at)
    .bind(event.viewer_id)
    .bind(event.subject_type.as_str())
    .bind(event.subject_id)
    .bind(event.actor_id)
    .bind(event.surface.as_str())
    .bind(event.position)
    .bind(&event.request_id)
    .bind(event.session_id)
    .bind(event.action.as_str())
    .bind(&event.source)
    .bind(&event.model_version)
    .bind(event.impression_id)
    .bind(event.propensity.map(|v| (v * 1_000_000.0).round() as i32))
    .bind(event.dwell_ms.map(|v| v as i32))
    .bind(&event.metadata_json)
    .fetch_optional(&mut **tx)
    .await?;
    if retained.is_some() {
        return Ok(retained);
    }
    if !retry_context_installed(tx).await? {
        return Ok(None);
    }
    sqlx::query_scalar(
        r#"select public.feed_recommendation_receipt_matches(row(
            $1::uuid,$2::integer,$3::timestamptz,$4::uuid,$5::text,$6::uuid,
            $7::uuid,$8::text,$9::integer,$10::text,$11::uuid,$12::text,$13::text,
            $14::text,$15::uuid,$16::integer,$17::integer,$18::jsonb
        )::public.feed_recommendation_events)"#,
    )
    .bind(event.event_id)
    .bind(i32::from(event.schema_version))
    .bind(event.occurred_at)
    .bind(event.viewer_id)
    .bind(event.subject_type.as_str())
    .bind(event.subject_id)
    .bind(event.actor_id)
    .bind(event.surface.as_str())
    .bind(event.position)
    .bind(&event.request_id)
    .bind(event.session_id)
    .bind(event.action.as_str())
    .bind(&event.source)
    .bind(&event.model_version)
    .bind(event.impression_id)
    .bind(event.propensity.map(|v| (v * 1_000_000.0).round() as i32))
    .bind(event.dwell_ms.map(|v| v as i32))
    .bind(&event.metadata_json)
    .fetch_one(&mut **tx)
    .await
    .map_err(Into::into)
}

async fn retry_context_installed(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>) -> Result<bool> {
    Ok(sqlx::query_scalar(
        "select to_regclass('public.feed_recommendation_retry_contexts') is not null",
    )
    .fetch_one(&mut **tx)
    .await?)
}

#[async_trait]
impl TimelineFanoutRepositoryPort for PgFeedRecommendationRepository {
    async fn enqueue_post(
        &self,
        event_id: Uuid,
        post_id: Uuid,
        author_id: Uuid,
        post_created_at: DateTime<Utc>,
        follower_count: i64,
        high_fanout_threshold: i64,
        now: DateTime<Utc>,
    ) -> Result<AuthorDeliveryMode> {
        let mut tx = self.pool.begin().await?;
        lock_post_subjects(&mut tx, post_id, &[author_id]).await?;
        let mode = enqueue_projection(
            &mut tx,
            event_id,
            post_id,
            author_id,
            post_created_at,
            follower_count,
            high_fanout_threshold,
            now,
        )
        .await?;
        tx.commit().await?;
        Ok(mode)
    }

    async fn claim_jobs(
        &self,
        request: &TimelineFanoutClaimRequestDto,
    ) -> Result<Vec<TimelineFanoutClaimDto>> {
        anyhow::ensure!(
            request.excluded_job_ids.len() <= 100,
            "oversized fanout claim exclusion set"
        );
        let lease_ms = i64::try_from(request.lease_timeout.as_millis())?;
        anyhow::ensure!(
            (1..=100).contains(&request.max_page_attempts),
            "invalid fanout page budget"
        );
        let retry_base_ms = i64::try_from(request.retry_base.as_millis())?;
        anyhow::ensure!(
            (100..=3_600_000).contains(&retry_base_ms),
            "invalid fanout base retry"
        );
        anyhow::ensure!(
            (1..=86_400_000).contains(&lease_ms),
            "invalid fanout lease duration"
        );
        // Expiry maintenance is bounded and independent of source visibility,
        // retry due time and worker cutoff. It never revokes a live final lease.
        sqlx::query(fanout_budget_sql::EXHAUSTED_MAINTENANCE_SQL)
            .execute(&self.pool)
            .await?;
        // Below-budget expiry consumes the already allocated attempt and
        // releases ownership once. The persisted delay cannot be extended by
        // subsequent maintenance ticks and does not depend on new config.
        sqlx::query(fanout_budget_sql::EXPIRED_MAINTENANCE_SQL)
            .execute(&self.pool)
            .await?;
        sqlx::query(fanout_budget_sql::SOURCE_RECHECK_SQL)
            .execute(&self.pool)
            .await?;
        // Lock a finite pending window before source lookup. Inactive sources
        // move to their own durable rotation queue; they spend no claim budget.
        let rows = sqlx::query(fanout_budget_sql::CLAIM_SQL)
            .bind(lease_ms)
            .bind(request.limit.clamp(1, 100))
            .bind(request.max_page_attempts)
            .bind(retry_base_ms)
            .bind(&request.excluded_job_ids)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .map(|row| TimelineFanoutClaimDto {
                job: TimelineFanoutJob {
                    job_id: row.get("job_id"),
                    event_id: row.get("event_id"),
                    post_id: row.get("post_id"),
                    author_id: row.get("author_id"),
                    post_created_at: row.get("post_created_at"),
                    cursor: row.get("cursor"),
                    attempt_count: row.get("attempt_count"),
                },
                lease_token: row.get("lease_token"),
                lease_expires_at: row.get("lease_expires_at"),
                page_attempt_count: row.get("page_attempt_count"),
                max_page_attempts: row.get("max_page_attempts"),
            })
            .collect())
    }

    async fn apply_page(
        &self,
        claim: &TimelineFanoutClaimDto,
        page: &TimelineFanoutPageDto,
    ) -> Result<TimelineFanoutOutcomeDto> {
        self.apply_fanout_page(claim, page, None, &mut false).await
    }

    async fn record_job_failure(
        &self,
        claim: &TimelineFanoutClaimDto,
        failure: &TimelineFanoutFailureDto,
    ) -> Result<TimelineFanoutOutcomeDto> {
        let mut tx = self.pool.begin().await?;
        if !lock_fanout_claim(&mut tx, claim).await? {
            tx.rollback().await?;
            return Ok(TimelineFanoutOutcomeDto::Lost);
        }
        let changed=sqlx::query(r#"
            update feed_fanout_jobs set status=case when page_attempt_count >= max_page_attempts then 'DEAD_LETTERED' else 'PENDING' end,
                dead_lettered_at=case when page_attempt_count >= max_page_attempts then clock_timestamp() else null end,
                last_error=left($4,2048),locked_at=null,lease_token=null,lease_expires_at=null,
                next_attempt_at=clock_timestamp()+(least(3600000::bigint,retry_base_ms*(1::bigint << least(greatest(page_attempt_count-1,0),20)))*interval '1 millisecond'),updated_at=clock_timestamp()
            where job_id=$1 and lease_token=$2 and attempt_count=$3 and status='PROCESSING'
              and lease_expires_at>clock_timestamp()
        "#).bind(claim.job.job_id).bind(claim.lease_token).bind(claim.job.attempt_count)
            .bind(&failure.message).execute(&mut *tx).await?.rows_affected();
        tx.commit().await?;
        Ok(if changed == 1 {
            TimelineFanoutOutcomeDto::Applied
        } else {
            TimelineFanoutOutcomeDto::Lost
        })
    }

    async fn remove_post(&self, post_id: Uuid) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        lock_post_subjects(&mut tx, post_id, &[]).await?;
        sqlx::query("select id from posts where id=$1 for update")
            .bind(post_id)
            .fetch_optional(&mut *tx)
            .await?;
        // Revoke job authority before cleaning materialized rows, under the
        // same post-before-job order as page application.
        remove_projection(&mut tx, post_id).await?;
        tx.commit().await?;
        Ok(())
    }
}

#[async_trait]
impl FeedFeatureRepositoryPort for PgFeedRecommendationRepository {
    async fn record_engagement(
        &self,
        event_id: Uuid,
        post_id: Uuid,
        actor_id: Uuid,
        author_id: Uuid,
        action: FeedEngagementAction,
        now: DateTime<Utc>,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        lock_post_subjects(&mut tx, post_id, &[actor_id, author_id]).await?;
        // The inbox row is written in the same transaction as the projection and
        // before it. Delivery is at-least-once, so a replay of an event that was
        // already applied must not apply its delta a second time: the counters
        // and affinity scores below are deltas, and applying one twice inflates
        // the ranking features instead of leaving them unchanged.
        //
        // Ordering matters inside the transaction. A projection that fails rolls
        // back this insert with it, which is the honest outcome: the event has
        // not been applied, so it must be retryable. Committing the inbox row on
        // its own would mark an event applied that never was, losing it rather
        // than duplicating it.
        engage_projection(&mut tx, event_id, post_id, actor_id, author_id, action, now).await?;
        tx.commit().await?;
        Ok(())
    }

    async fn cleanup_user(&self, user_id: Uuid) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("select post_lock_subject_ids($1::uuid[])")
            .bind(vec![user_id])
            .execute(&mut *tx)
            .await?;
        forget_affinity_sources(&mut tx, user_id).await?;
        for query in [
            "delete from feed_fanout_jobs where author_id = $1",
            "delete from feed_author_affinities where viewer_id = $1 or author_id = $1",
            "delete from feed_recommendation_events where viewer_id = $1 or actor_id = $1",
            "delete from feed_recommendation_impressions where viewer_id = $1 or author_id = $1",
            "delete from feed_recommendation_snapshots where viewer_id = $1",
            "delete from feed_experiment_assignments where viewer_id = $1",
            "delete from feed_recent_actions where viewer_id = $1",
            "delete from feed_embeddings where subject_id = $1",
            "delete from home_timeline_entries where viewer_id = $1 or author_id = $1",
            "delete from high_fanout_posts where author_id = $1",
        ] {
            sqlx::query(query).bind(user_id).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }
}

#[async_trait]
impl FeedRecommendationMaintenancePort for PgFeedRecommendationRepository {
    async fn purge_expired(
        &self,
        now: DateTime<Utc>,
        event_cutoff: DateTime<Utc>,
        limit: i32,
    ) -> Result<FeedRecommendationRetentionResult> {
        let limit = i64::from(limit.clamp(1, 5_000));
        let mut tx = self.pool.begin().await?;
        let timeline_entries = sqlx::query(
            r#"with doomed as (
                select ctid from home_timeline_entries
                where expires_at <= $1 order by expires_at, viewer_id, post_id
                for update skip locked
                limit $2
            ) delete from home_timeline_entries
              where ctid in (select ctid from doomed)"#,
        )
        .bind(now)
        .bind(limit)
        .execute(&mut *tx)
        .await?
        .rows_affected() as i64;
        let high_fanout_posts = sqlx::query(
            r#"with doomed as (
                select ctid from high_fanout_posts
                where expires_at <= $1 order by expires_at, post_id
                for update skip locked
                limit $2
            ) delete from high_fanout_posts
              where ctid in (select ctid from doomed)"#,
        )
        .bind(now)
        .bind(limit)
        .execute(&mut *tx)
        .await?
        .rows_affected() as i64;
        let impressions = sqlx::query(
            r#"with doomed as (
                select i.ctid
                from feed_recommendation_impressions i
                join feed_recommendation_snapshots s on s.snapshot_id = i.snapshot_id
                where s.expires_at <= $1
                order by i.snapshot_id, i.impression_id
                for update of i skip locked
                limit $2
            ) delete from feed_recommendation_impressions
              where ctid in (select ctid from doomed)"#,
        )
        .bind(now)
        .bind(limit)
        .execute(&mut *tx)
        .await?
        .rows_affected() as i64;
        let snapshot_items = sqlx::query(
            r#"with doomed as (
                select i.ctid
                from feed_recommendation_snapshot_items i
                join feed_recommendation_snapshots s on s.snapshot_id = i.snapshot_id
                where s.expires_at <= $1
                order by i.snapshot_id, i.position
                for update of i skip locked
                limit $2
            ) delete from feed_recommendation_snapshot_items
              where ctid in (select ctid from doomed)"#,
        )
        .bind(now)
        .bind(limit)
        .execute(&mut *tx)
        .await?
        .rows_affected() as i64;
        let snapshots = sqlx::query(
            r#"with doomed as (
                select ctid from feed_recommendation_snapshots
                where expires_at <= $1
                  and not exists (
                      select 1 from feed_recommendation_impressions
                      where snapshot_id = feed_recommendation_snapshots.snapshot_id
                  )
                  and not exists (
                      select 1 from feed_recommendation_snapshot_items
                      where snapshot_id = feed_recommendation_snapshots.snapshot_id
                  )
                order by expires_at, snapshot_id
                for update skip locked
                limit $2
            ) delete from feed_recommendation_snapshots
              where ctid in (select ctid from doomed)"#,
        )
        .bind(now)
        .bind(limit)
        .execute(&mut *tx)
        .await?
        .rows_affected() as i64;
        let events = sqlx::query(
            r#"with doomed as (
                select ctid from feed_recommendation_events
                where occurred_at <= $1 order by occurred_at, event_id
                for update skip locked
                limit $2
            ) delete from feed_recommendation_events
              where ctid in (select ctid from doomed)"#,
        )
        .bind(event_cutoff)
        .bind(limit)
        .execute(&mut *tx)
        .await?
        .rows_affected() as i64;
        let experiment_assignments = sqlx::query(
            r#"with doomed as (
                select ctid from feed_experiment_assignments
                where expires_at <= $1 order by expires_at, viewer_id
                for update skip locked
                limit $2
            ) delete from feed_experiment_assignments
              where ctid in (select ctid from doomed)"#,
        )
        .bind(now)
        .bind(limit)
        .execute(&mut *tx)
        .await?
        .rows_affected() as i64;
        let embeddings = sqlx::query(
            r#"with doomed as (
                select e.ctid from feed_embeddings e
                where e.updated_at <= $1
                  and exists (
                      select 1 from feed_recommendation_models m
                      where m.model_version=e.model_version
                        and m.status in ('RETIRED','REVOKED')
                  )
                order by e.updated_at, e.subject_id
                for update skip locked
                limit $2
            ) delete from feed_embeddings
              where ctid in (select ctid from doomed)"#,
        )
        .bind(event_cutoff)
        .bind(limit)
        .execute(&mut *tx)
        .await?
        .rows_affected() as i64;
        let fanout_jobs = sqlx::query(
            r#"with doomed as (
                select ctid from feed_fanout_jobs
                where status in ('COMPLETED','DEAD_LETTERED') and updated_at <= $1
                order by updated_at, job_id
                for update skip locked
                limit $2
            ) delete from feed_fanout_jobs
              where ctid in (select ctid from doomed)"#,
        )
        .bind(event_cutoff)
        .bind(limit)
        .execute(&mut *tx)
        .await?
        .rows_affected() as i64;
        tx.commit().await?;
        Ok(FeedRecommendationRetentionResult {
            timeline_entries,
            high_fanout_posts,
            snapshot_items,
            impressions,
            snapshots,
            events,
            experiment_assignments,
            embeddings,
            fanout_jobs,
        })
    }
}

fn feed_model_manifest_from_row_with_version(
    model_version: &str,
    row: &sqlx::postgres::PgRow,
) -> Result<ModelManifest> {
    Ok(ModelManifest {
        model_name: row
            .get::<Option<String>, _>("model_name")
            .ok_or_else(|| anyhow::anyhow!("Learned model name is missing"))?,
        model_version: model_version.to_owned(),
        feature_schema_version: row
            .get::<Option<String>, _>("feature_schema_version")
            .ok_or_else(|| anyhow::anyhow!("Learned feature schema is missing"))?,
        artifact_uri: row
            .get::<Option<String>, _>("artifact_uri")
            .ok_or_else(|| anyhow::anyhow!("Learned artifact URI is missing"))?,
        sha256: row
            .get::<Option<String>, _>("artifact_sha256")
            .ok_or_else(|| anyhow::anyhow!("Learned artifact digest is missing"))?,
        embedding_dimension: row
            .get::<Option<i32>, _>("embedding_dimension")
            .map(usize::try_from)
            .transpose()?,
        activated_at: row.get("activated_at"),
        fallback_version: row
            .get::<Option<String>, _>("fallback_version")
            .ok_or_else(|| anyhow::anyhow!("Learned fallback version is missing"))?,
        status: ModelStatus::parse(
            row.get::<Option<String>, _>("status")
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("Learned model status is missing"))?,
        )?,
    })
}

fn feed_model_manifest_from_row(row: &sqlx::postgres::PgRow) -> Result<ModelManifest> {
    Ok(ModelManifest {
        model_name: row.get("model_name"),
        model_version: row.get("model_version"),
        feature_schema_version: row.get("feature_schema_version"),
        artifact_uri: row
            .get::<Option<String>, _>("artifact_uri")
            .ok_or_else(|| anyhow::anyhow!("Learned feed model artifact URI is missing"))?,
        sha256: row
            .get::<Option<String>, _>("artifact_sha256")
            .ok_or_else(|| anyhow::anyhow!("Learned feed model artifact digest is missing"))?,
        embedding_dimension: row
            .get::<Option<i32>, _>("embedding_dimension")
            .map(usize::try_from)
            .transpose()?,
        activated_at: row.get("activated_at"),
        fallback_version: row
            .get::<Option<String>, _>("fallback_version")
            .ok_or_else(|| anyhow::anyhow!("Learned feed model fallback is missing"))?,
        status: ModelStatus::parse(row.get::<String, _>("status").as_str())?,
    })
}

fn map_recommendation_event(row: sqlx::postgres::PgRow) -> Result<RecommendationEvent> {
    let event = RecommendationEvent {
        event_id: row.get("event_id"),
        schema_version: u16::try_from(row.get::<i32, _>("schema_version"))?,
        occurred_at: row.get("occurred_at"),
        viewer_id: row.get("viewer_id"),
        subject_type: platform::recommendation::RecommendationSubjectType::parse(
            row.get::<String, _>("subject_type").as_str(),
        )?,
        subject_id: row.get("subject_id"),
        actor_id: row.get("actor_id"),
        surface: platform::recommendation::RecommendationSurface::parse(
            row.get::<String, _>("surface").as_str(),
        )?,
        position: row.get("position"),
        request_id: row.get("request_id"),
        session_id: row.get("session_id"),
        action: RecommendationAction::parse(row.get::<String, _>("action").as_str())?,
        source: row.get("source"),
        model_version: row.get("model_version"),
        metadata_json: row.get("metadata_json"),
        propensity: row
            .get::<Option<i32>, _>("propensity_micros")
            .map(|value| f64::from(value) / 1_000_000.0),
        impression_id: row.get("impression_id"),
        dwell_ms: row
            .get::<Option<i32>, _>("dwell_ms")
            .map(u32::try_from)
            .transpose()?,
    };
    event.validate()?;
    Ok(event)
}

fn map_candidate(row: sqlx::postgres::PgRow) -> Result<FeedCandidate> {
    Ok(FeedCandidate {
        post_id: row.get("post_id"),
        author_id: row.get("author_id"),
        source: FeedCandidateSource::parse(row.get::<String, _>("source").as_str())?,
        source_score: row.get("source_score"),
        author_affinity: row.get("author_affinity"),
        freshness: row.get("freshness"),
        social_proof: row.get("social_proof"),
        engagement_velocity: row.get("engagement_velocity"),
        topic_match: row.get("topic_match"),
        repeated_impression_penalty: row.get("repeated_impression_penalty"),
        author_saturation_penalty: row.get("author_saturation_penalty"),
        negative_feedback_penalty: row.get("negative_feedback_penalty"),
        generated_at: row.get("generated_at"),
        reason_code: row.get("reason_code"),
        in_network: row.get("in_network"),
        exploration: row.get("exploration"),
    })
}

fn map_snapshot_item(row: sqlx::postgres::PgRow) -> Result<FeedSnapshotItem> {
    Ok(FeedSnapshotItem {
        post_id: row.get("post_id"),
        author_id: row.get("author_id"),
        source: FeedCandidateSource::parse(row.get::<String, _>("source").as_str())?,
        reason_code: row.get("reason_code"),
        score_micros: row.get("score_micros"),
        position: u32::try_from(row.get::<i32, _>("position"))?,
        impression_id: row.get("impression_id"),
        propensity_micros: u32::try_from(row.get::<i32, _>("propensity_micros"))?,
    })
}

fn map_impression(row: sqlx::postgres::PgRow) -> Result<FeedRecommendationImpression> {
    Ok(FeedRecommendationImpression {
        impression_id: row.get("impression_id"),
        snapshot_id: row.get("snapshot_id"),
        viewer_id: row.get("viewer_id"),
        post_id: row.get("post_id"),
        author_id: row.get("author_id"),
        source: FeedCandidateSource::parse(row.get::<String, _>("source").as_str())?,
        reason_code: row.get("reason_code"),
        position: u32::try_from(row.get::<i32, _>("position"))?,
        rank_version: row.get("rank_version"),
        request_id: row.get("request_id"),
        session_id: row.get("session_id"),
        score_micros: row.get("score_micros"),
        propensity_micros: u32::try_from(row.get::<i32, _>("propensity_micros"))?,
        occurred_at: row.get("occurred_at"),
    })
}

fn affinity_delta(action: RecommendationAction, dwell_ms: Option<u32>) -> (f64, f64) {
    match action {
        RecommendationAction::Like => (0.20, 0.0),
        RecommendationAction::Comment => (0.30, 0.0),
        RecommendationAction::Save | RecommendationAction::Share => (0.35, 0.0),
        RecommendationAction::Follow | RecommendationAction::AcceptFollow => (0.50, 0.0),
        RecommendationAction::Click => (0.08, 0.0),
        RecommendationAction::Dwell => {
            let score = dwell_ms.unwrap_or_default().min(60_000) as f64 / 60_000.0 * 0.20;
            (score, 0.0)
        }
        RecommendationAction::FastScroll => (0.0, 0.15),
        RecommendationAction::Hide | RecommendationAction::NotInterested => (0.0, 0.40),
        RecommendationAction::Mute => (0.0, 0.70),
        RecommendationAction::Block | RecommendationAction::Report => (0.0, 1.0),
        RecommendationAction::Impression => (0.0, 0.0),
    }
}

async fn lock_fanout_claim(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    claim: &TimelineFanoutClaimDto,
) -> Result<bool> {
    // Lock first, then evaluate the current database clock. A predicate read
    // before waiting on a job lock cannot authorize a later expired completion.
    let found:Option<Uuid>=sqlx::query_scalar("select job_id from feed_fanout_jobs where job_id=$1 and event_id=$2 and post_id=$3 and author_id=$4 and lease_token=$5 and attempt_count=$6 and status='PROCESSING' for update")
        .bind(claim.job.job_id).bind(claim.job.event_id).bind(claim.job.post_id)
        .bind(claim.job.author_id).bind(claim.lease_token).bind(claim.job.attempt_count)
        .fetch_optional(&mut **tx).await?;
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

#[allow(clippy::too_many_arguments)]
pub(super) async fn enqueue_projection(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    event_id: Uuid,
    post_id: Uuid,
    author_id: Uuid,
    post_created_at: DateTime<Utc>,
    follower_count: i64,
    high_fanout_threshold: i64,
    now: DateTime<Utc>,
) -> Result<AuthorDeliveryMode> {
    sqlx::query(
        r#"
            insert into home_timeline_entries (
                viewer_id, post_id, author_id, source, source_score, inserted_at, expires_at
            ) values ($1, $2, $1, 'SELF_HISTORY', 1, $3, $4)
            on conflict (viewer_id, post_id, source) do nothing
            "#,
    )
    .bind(author_id)
    .bind(post_id)
    .bind(post_created_at)
    .bind(post_created_at + chrono::Duration::days(14))
    .execute(&mut **tx)
    .await?;

    let mode = if follower_count >= high_fanout_threshold {
        sqlx::query(
            r#"
                insert into high_fanout_posts (
                    post_id, author_id, follower_count, created_at, expires_at
                ) values ($1, $2, $3, $4, $5)
                on conflict (post_id) do nothing
                "#,
        )
        .bind(post_id)
        .bind(author_id)
        .bind(follower_count)
        .bind(post_created_at)
        .bind(post_created_at + chrono::Duration::days(14))
        .execute(&mut **tx)
        .await?
        .rows_affected();
        AuthorDeliveryMode::HighFanout
    } else {
        sqlx::query(
            r#"
                insert into feed_fanout_jobs (
                    job_id, event_id, post_id, author_id, post_created_at,
                    next_attempt_at, created_at, updated_at
                ) values ($1, $2, $3, $4, $5, statement_timestamp(), $6, $6)
                on conflict (event_id) do nothing
                "#,
        )
        .bind(Uuid::new_v4())
        .bind(event_id)
        .bind(post_id)
        .bind(author_id)
        .bind(post_created_at)
        .bind(now)
        .execute(&mut **tx)
        .await?
        .rows_affected();
        AuthorDeliveryMode::Normal
    };
    Ok(mode)
}

pub(super) async fn remove_projection(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    post_id: Uuid,
) -> Result<()> {
    sqlx::query("delete from feed_fanout_jobs where post_id=$1")
        .bind(post_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query("delete from home_timeline_entries where post_id = $1")
        .bind(post_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query("delete from high_fanout_posts where post_id = $1")
        .bind(post_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub(super) async fn engage_projection(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    event_id: Uuid,
    post_id: Uuid,
    actor_id: Uuid,
    author_id: Uuid,
    action: FeedEngagementAction,
    now: DateTime<Utc>,
) -> Result<()> {
    let applied = sqlx::query(
        r#"
            insert into feed_projection_inbox (consumer, event_id, applied_at)
            values ('feed-engagement', $1, $2)
            on conflict (consumer, event_id) do nothing
            "#,
    )
    .bind(event_id)
    .bind(now)
    .execute(&mut **tx)
    .await?
    .rows_affected();
    if applied == 0 {
        // Already applied. Returning here commits an empty transaction,
        // which records no new effect and keeps the caller's retry harmless.
        return Ok(());
    }
    let (like_delta, comment_delta) = action.velocity_delta();
    sqlx::query(
        r#"
            insert into post_engagement_velocity (
                post_id, like_events, comment_events, velocity_score, updated_at
            ) values (
                $1,
                greatest(0, $2),
                greatest(0, $3),
                greatest(
                    0::float8,
                    greatest(0, $2)::float8 + 2 * greatest(0, $3)::float8
                ),
                $4
            )
            on conflict (post_id) do update
            set like_events = greatest(0, post_engagement_velocity.like_events + $2),
                comment_events = greatest(0, post_engagement_velocity.comment_events + $3),
                velocity_score = greatest(
                    0::float8,
                    (greatest(0, post_engagement_velocity.like_events + $2))::float8
                    + 2 * (greatest(0, post_engagement_velocity.comment_events + $3))::float8
                ),
                updated_at = $4
            "#,
    )
    .bind(post_id)
    .bind(like_delta)
    .bind(comment_delta)
    .bind(now)
    .execute(&mut **tx)
    .await?;

    if actor_id != author_id {
        let delta = action.affinity_delta();
        let positive = delta.max(0.0);
        let negative = (-delta).max(0.0);
        if !super::affinity_provenance::apply_legacy_effect(
            tx,
            &super::affinity_provenance::LegacyAffinityMutationDto {
                namespace: "OUTBOX",
                source_id: event_id,
                viewer_id: actor_id,
                author_id,
                actor_id: Some(actor_id),
                post_id,
                action: action.as_str(),
                occurred_at: now,
                positive,
                negative,
            },
        )
        .await?
        {
            sqlx::query(
                r#"
                insert into feed_author_affinities (
                    viewer_id, author_id, affinity_score, negative_score,
                    last_interacted_at, feature_version
                ) values ($1, $2, $3, $4, $5, 'feed-affinity-v1')
                on conflict (viewer_id, author_id) do update
                set affinity_score = least(1::float8, greatest(0::float8,
                        feed_author_affinities.affinity_score * 0.95 + $3)),
                    negative_score = least(1::float8, greatest(0::float8,
                        feed_author_affinities.negative_score * 0.95 + $4)),
                    last_interacted_at = greatest(feed_author_affinities.last_interacted_at, $5),
                    feature_version = 'feed-affinity-v1'
                "#,
            )
            .bind(actor_id)
            .bind(author_id)
            .bind(positive)
            .bind(negative)
            .bind(now)
            .execute(&mut **tx)
            .await?;
        }
    }
    Ok(())
}
