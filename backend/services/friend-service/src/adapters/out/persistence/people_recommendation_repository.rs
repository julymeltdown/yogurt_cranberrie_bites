use crate::application::dto::mutation_dto::MutationError;
use crate::application::ports::{
    PeopleRecommendationMaintenancePort, PeopleRecommendationRepositoryPort,
    RecommendationEventRepositoryPort, RecommendationRefreshClaim, RecommendationRollout,
};
use crate::domain::error::FriendError;
use crate::domain::recommendation::{
    PeopleCandidate, PeopleCandidateFeatures, PeopleCandidateSource,
    PeopleRecommendationImpression, PeopleRecommendationRetentionResult, PeopleSnapshot,
    PeopleSnapshotItem,
};
use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use platform::model_manifest::{ModelManifest, ModelStatus};
use platform::recommendation::{
    RecentActionLimit, RecommendationAction, RecommendationCursor, RecommendationEvent,
    RecommendationSubjectType, RecommendationSurface, RolloutMode,
};
use sqlx::{PgPool, Row};
use std::collections::HashSet;
use uuid::Uuid;

const ENQUEUE_REFRESH_SQL: &str = r#"insert into people_recommendation_refresh_jobs (viewer_id,status,next_attempt_at,updated_at)
    select viewer_id,'PENDING',$2,$2 from unnest($1::uuid[]) viewer_id
    on conflict (viewer_id) do update
    set status='PENDING',next_attempt_at=least(people_recommendation_refresh_jobs.next_attempt_at,excluded.next_attempt_at),updated_at=excluded.updated_at"#;

fn recommendation_admission_error(error: sqlx::Error) -> anyhow::Error {
    match error.as_database_error().and_then(|db| db.constraint()) {
        Some("friend_recommendation_event_identity") => MutationError::Conflict.into(),
        Some("friend_recommendation_retry_unavailable") => MutationError::Unavailable.into(),
        Some("friend_recommendation_metadata_admission") => FriendError::InvalidInput(
            "Recommendation metadata must be a bounded JSON object".into(),
        )
        .into(),
        Some("friend_recommendation_impression_admission") => FriendError::InvalidInput(
            "Recommendation impression must belong to viewer and candidate".into(),
        )
        .into(),
        Some("friend_user_deletion_fence") => {
            FriendError::PermissionDenied("Account is deleted".into()).into()
        }
        _ => error.into(),
    }
}

async fn enqueue_refresh_with_executor<'e, E>(
    executor: E,
    viewer_ids: &[Uuid],
    now: DateTime<Utc>,
) -> Result<()>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    if viewer_ids.is_empty() {
        return Ok(());
    }
    sqlx::query(ENQUEUE_REFRESH_SQL)
        .bind(viewer_ids)
        .bind(now)
        .execute(executor)
        .await?;
    Ok(())
}

#[derive(Clone)]
pub struct PgPeopleRecommendationRepository {
    pool: PgPool,
}

impl PgPeopleRecommendationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn map_candidate(row: sqlx::postgres::PgRow) -> Result<PeopleCandidate> {
        let features = serde_json::from_str::<PeopleCandidateFeatures>(
            &row.get::<String, _>("feature_snapshot_json"),
        )?;
        Ok(PeopleCandidate {
            viewer_id: row.get("viewer_id"),
            candidate_user_id: row.get("candidate_user_id"),
            source: PeopleCandidateSource::parse(&row.get::<String, _>("source"))?,
            source_score: row.get("source_score"),
            features,
            mutual_cluster: row.get("mutual_cluster"),
            generated_at: row.get("generated_at"),
            expires_at: row.get("expires_at"),
            model_version: row.get("model_version"),
        })
    }

    fn map_snapshot_item(row: sqlx::postgres::PgRow) -> Result<PeopleSnapshotItem> {
        Ok(PeopleSnapshotItem {
            snapshot_id: row.get("snapshot_id"),
            candidate_user_id: row.get("candidate_user_id"),
            source: PeopleCandidateSource::parse(&row.get::<String, _>("source"))?,
            reason_code: row.get("reason_code"),
            score_micros: row.get("score_micros"),
            rank_version: row.get("rank_version"),
            feature_snapshot_json: row.get("feature_snapshot_json"),
            mutual_cluster: row.get("mutual_cluster"),
            generated_at: row.get("generated_at"),
        })
    }
}

#[async_trait]
impl PeopleRecommendationRepositoryPort for PgPeopleRecommendationRepository {
    async fn refresh_candidates(
        &self,
        viewer_id: Uuid,
        generated_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
        model_version: &str,
        max_candidates: i32,
    ) -> Result<i64> {
        let max_candidates = max_candidates.clamp(1, 2_000);
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "delete from people_recommendation_candidates where viewer_id=$1 or expires_at <= $2",
        )
        .bind(viewer_id)
        .bind(generated_at)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            with viewer_following as (
                select followee_id
                from follows
                where follower_id=$1 and deleted_at is null
            ),
            intermediate_degree as (
                select follower_id, count(*)::double precision as degree
                from follows
                where deleted_at is null
                group by follower_id
            ),
            mutual as (
                select second.followee_id as candidate_user_id,
                       count(distinct first.followee_id)::integer as mutual_count,
                       least(1.0, count(distinct first.followee_id)::double precision / 10.0) as mutual_score,
                       least(1.0, sum(1.0 / ln(greatest(2.0, coalesce(d.degree, 1.0) + 1.0))) / 5.0) as adamic_adar,
                       -- PostgreSQL has UUID ordering but no built-in min(uuid).
                       -- Canonical UUID text under C collation preserves byte order
                       -- without collecting every bridge into an aggregate array.
                       min(first.followee_id::text collate "C")::uuid as mutual_cluster
                from follows first
                join follows second on second.follower_id=first.followee_id and second.deleted_at is null
                left join intermediate_degree d on d.follower_id=first.followee_id
                where first.follower_id=$1 and first.deleted_at is null
                  and second.followee_id <> $1
                  and not exists (
                      select 1 from follows direct
                      where direct.follower_id=$1 and direct.followee_id=second.followee_id
                        and direct.deleted_at is null
                  )
                group by second.followee_id
                order by mutual_count desc, second.followee_id
                limit $5
            )
            insert into people_recommendation_candidates (
                viewer_id, candidate_user_id, source, source_score,
                feature_snapshot_json, mutual_cluster, generated_at, expires_at, model_version
            )
            select $1, m.candidate_user_id, 'MUTUAL_FOLLOW', m.mutual_score,
                   json_build_object(
                     'mutual_count', m.mutual_count,
                     'mutual_score', m.mutual_score,
                     'adamic_adar', m.adamic_adar,
                     'jaccard', least(1.0, m.mutual_count::double precision /
                         greatest(1.0,
                           (select count(*) from viewer_following)::double precision
                           + (select count(*) from follows cf where cf.follower_id=m.candidate_user_id and cf.deleted_at is null)::double precision
                           - m.mutual_count::double precision)),
                     'profile_overlap', 0.0,
                     'interaction_affinity', 0.0,
                     'freshness', 1.0,
                     'candidate_quality', 0.5,
                     'impression_penalty', least(1.0, coalesce((select count(*) from people_recommendation_impressions pi where pi.viewer_id=$1 and pi.candidate_user_id=m.candidate_user_id and pi.occurred_at > $2 - interval '30 days'),0)::double precision / 10.0),
                     'dismissal_penalty', case when exists(select 1 from people_recommendation_events pe where pe.viewer_id=$1 and pe.subject_id=m.candidate_user_id and pe.action in ('NOT_INTERESTED','HIDE','BLOCK','MUTE','REPORT') and pe.occurred_at > $2 - interval '30 days') then 1.0 else 0.0 end
                   )::text,
                   m.mutual_cluster, $2, $3, $4
            from mutual m
            on conflict (viewer_id, candidate_user_id, source) do update
            set source_score=excluded.source_score,
                feature_snapshot_json=excluded.feature_snapshot_json,
                mutual_cluster=excluded.mutual_cluster,
                generated_at=excluded.generated_at,
                expires_at=excluded.expires_at,
                model_version=excluded.model_version
            "#,
        )
        .bind(viewer_id)
        .bind(generated_at)
        .bind(expires_at)
        .bind(model_version)
        .bind(max_candidates)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            with viewer_targets as (
              select followee_id from follows where follower_id=$1 and deleted_at is null
            ), overlap_candidates as (
              select candidate.follower_id as candidate_user_id,
                     count(distinct candidate.followee_id)::integer as overlap_count
              from follows candidate
              join viewer_targets vt on vt.followee_id=candidate.followee_id
              where candidate.deleted_at is null and candidate.follower_id <> $1
                and not exists(select 1 from follows d where d.follower_id=$1 and d.followee_id=candidate.follower_id and d.deleted_at is null)
              group by candidate.follower_id
              order by overlap_count desc, candidate.follower_id
              limit greatest(1, $5 / 2)
            )
            insert into people_recommendation_candidates (
              viewer_id,candidate_user_id,source,source_score,feature_snapshot_json,
              mutual_cluster,generated_at,expires_at,model_version
            )
            select $1,candidate_user_id,'FOLLOWER_OVERLAP',least(1.0,overlap_count::double precision/10.0),
                   json_build_object(
                     'mutual_count', overlap_count,
                     'mutual_score', 0.0,
                     'adamic_adar', 0.0,
                     'jaccard', least(1.0,overlap_count::double precision/10.0),
                     'profile_overlap', 0.0,
                     'interaction_affinity', 0.0,
                     'freshness', 0.8,
                     'candidate_quality', 0.5,
                     'impression_penalty', 0.0,
                     'dismissal_penalty', 0.0
                   )::text,
                   null,$2,$3,$4
            from overlap_candidates
            on conflict (viewer_id,candidate_user_id,source) do update
            set source_score=excluded.source_score,
                feature_snapshot_json=excluded.feature_snapshot_json,
                generated_at=excluded.generated_at,
                expires_at=excluded.expires_at,
                model_version=excluded.model_version
            "#,
        )
        .bind(viewer_id)
        .bind(generated_at)
        .bind(expires_at)
        .bind(model_version)
        .bind(max_candidates)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            with graph_users as (
              select follower_id as user_id from follows where deleted_at is null
              union
              select followee_id as user_id from follows where deleted_at is null
            ), exploration as (
              select user_id
              from graph_users
              where user_id <> $1
                and not exists(select 1 from follows d where d.follower_id=$1 and d.followee_id=user_id and d.deleted_at is null)
              order by md5($1::text || user_id::text || $2::date::text)
              limit greatest(1, least(20, $5 / 10))
            )
            insert into people_recommendation_candidates (
              viewer_id,candidate_user_id,source,source_score,feature_snapshot_json,
              mutual_cluster,generated_at,expires_at,model_version
            )
            select $1,user_id,'EXPLORATION',0.1,
                   '{"mutual_count":0,"mutual_score":0.0,"adamic_adar":0.0,"jaccard":0.0,"profile_overlap":0.0,"interaction_affinity":0.0,"freshness":0.6,"candidate_quality":0.5,"impression_penalty":0.0,"dismissal_penalty":0.0}',
                   null,$2,$3,$4
            from exploration
            on conflict (viewer_id,candidate_user_id,source) do update
            set generated_at=excluded.generated_at,expires_at=excluded.expires_at,model_version=excluded.model_version
            "#,
        )
        .bind(viewer_id)
        .bind(generated_at)
        .bind(expires_at)
        .bind(model_version)
        .bind(max_candidates)
        .execute(&mut *tx)
        .await?;

        let count: i64 = sqlx::query_scalar(
            "select count(*)::bigint from people_recommendation_candidates where viewer_id=$1 and expires_at>$2",
        )
        .bind(viewer_id)
        .bind(generated_at)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(count)
    }

    async fn list_candidates(
        &self,
        viewer_id: Uuid,
        now: DateTime<Utc>,
        limit: i32,
    ) -> Result<Vec<PeopleCandidate>> {
        let rows = sqlx::query(
            r#"
            select viewer_id,candidate_user_id,source,source_score,feature_snapshot_json,
                   mutual_cluster,generated_at,expires_at,model_version
            from people_recommendation_candidates
            where viewer_id=$1 and expires_at>$2
            order by source_score desc, generated_at desc, candidate_user_id
            limit $3
            "#,
        )
        .bind(viewer_id)
        .bind(now)
        .bind(i64::from(limit.clamp(1, 2_000)))
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(Self::map_candidate).collect()
    }

    async fn ineligible_candidate_ids(
        &self,
        viewer_id: Uuid,
        candidate_ids: &[Uuid],
    ) -> Result<HashSet<Uuid>> {
        if candidate_ids.is_empty() {
            return Ok(HashSet::new());
        }
        let rows = sqlx::query(
            r#"
            select candidate_id
            from unnest($2::uuid[]) candidate_id
            where candidate_id=$1
               or exists(select 1 from follows f where f.follower_id=$1 and f.followee_id=candidate_id and f.deleted_at is null)
               or exists(select 1 from follow_requests fr where fr.follower_id=$1 and fr.followee_id=candidate_id and fr.status='PENDING' and fr.deleted_at is null)
               or exists(select 1 from user_blocks b where b.deleted_at is null and ((b.blocker_id=$1 and b.blocked_id=candidate_id) or (b.blocker_id=candidate_id and b.blocked_id=$1)))
               or exists(select 1 from user_mutes m where m.deleted_at is null and m.muter_id=$1 and m.muted_id=candidate_id)
               or exists(select 1 from people_recommendation_events e where e.viewer_id=$1 and e.subject_id=candidate_id and e.action in ('NOT_INTERESTED','HIDE','BLOCK','MUTE','REPORT') and e.occurred_at > now()-interval '30 days')
            "#,
        )
        .bind(viewer_id)
        .bind(candidate_ids)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| row.get("candidate_id"))
            .collect())
    }

    async fn create_snapshot(
        &self,
        snapshot: &PeopleSnapshot,
        items: &[PeopleSnapshotItem],
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "insert into people_recommendation_snapshots (id,viewer_id,rank_version,created_at,expires_at) values ($1,$2,$3,$4,$5)",
        )
        .bind(snapshot.id)
        .bind(snapshot.viewer_id)
        .bind(&snapshot.rank_version)
        .bind(snapshot.created_at)
        .bind(snapshot.expires_at)
        .execute(&mut *tx)
        .await?;
        for item in items {
            sqlx::query(
                r#"insert into people_recommendation_snapshot_items (
                  snapshot_id,candidate_user_id,source,reason_code,score_micros,rank_version,
                  feature_snapshot_json,mutual_cluster,generated_at
                ) values ($1,$2,$3,$4,$5,$6,$7,$8,$9)"#,
            )
            .bind(item.snapshot_id)
            .bind(item.candidate_user_id)
            .bind(item.source.as_str())
            .bind(&item.reason_code)
            .bind(item.score_micros)
            .bind(&item.rank_version)
            .bind(&item.feature_snapshot_json)
            .bind(item.mutual_cluster)
            .bind(item.generated_at)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn find_snapshot(
        &self,
        viewer_id: Uuid,
        snapshot_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<Option<PeopleSnapshot>> {
        let row = sqlx::query(
            "select id,viewer_id,rank_version,created_at,expires_at from people_recommendation_snapshots where id=$1 and viewer_id=$2 and expires_at>$3",
        )
        .bind(snapshot_id)
        .bind(viewer_id)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|row| PeopleSnapshot {
            id: row.get("id"),
            viewer_id: row.get("viewer_id"),
            rank_version: row.get("rank_version"),
            created_at: row.get("created_at"),
            expires_at: row.get("expires_at"),
        }))
    }

    async fn load_snapshot_page(
        &self,
        snapshot_id: Uuid,
        cursor: Option<&RecommendationCursor>,
        limit: i32,
    ) -> Result<Vec<PeopleSnapshotItem>> {
        let rows = sqlx::query(
            r#"
            select snapshot_id,candidate_user_id,source,reason_code,score_micros,rank_version,
                   feature_snapshot_json,mutual_cluster,generated_at
            from people_recommendation_snapshot_items
            where snapshot_id=$1
              and ($2::bigint is null or (score_micros,candidate_user_id) < ($2,$3))
            order by score_micros desc,candidate_user_id desc
            limit $4
            "#,
        )
        .bind(snapshot_id)
        .bind(cursor.map(|value| value.score_micros))
        .bind(cursor.map(|value| value.tie_breaker_id))
        .bind(i64::from(limit.clamp(1, 100)) + 1)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(Self::map_snapshot_item).collect()
    }

    async fn record_impressions(
        &self,
        viewer_id: Uuid,
        snapshot_id: Uuid,
        impressions: &[PeopleRecommendationImpression],
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        for impression in impressions.iter().take(100) {
            if impression.viewer_id != viewer_id || impression.snapshot_id != snapshot_id {
                anyhow::bail!("Recommendation impression context mismatch");
            }
            sqlx::query(
                r#"insert into people_recommendation_impressions (
                  impression_id,snapshot_id,viewer_id,candidate_user_id,source,reason_code,
                  rank_version,position,score_micros,request_id,session_id,occurred_at,propensity_micros
                ) values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)
                on conflict (impression_id) do nothing"#,
            )
            .bind(impression.impression_id)
            .bind(snapshot_id)
            .bind(viewer_id)
            .bind(impression.candidate_user_id)
            .bind(impression.source.as_str())
            .bind(&impression.reason_code)
            .bind(&impression.rank_version)
            .bind(impression.position)
            .bind(impression.score_micros)
            .bind(&impression.request_id)
            .bind(impression.session_id)
            .bind(impression.occurred_at)
            .bind(i32::try_from(impression.propensity_micros).unwrap_or(i32::MAX))
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn enqueue_refresh(&self, viewer_ids: &[Uuid], now: DateTime<Utc>) -> Result<()> {
        enqueue_refresh_with_executor(&self.pool, viewer_ids, now).await
    }

    async fn claim_refresh_batch(
        &self,
        now: DateTime<Utc>,
        lease_timeout: Duration,
        limit: i32,
    ) -> Result<Vec<RecommendationRefreshClaim>> {
        let rows = sqlx::query(
            r#"with claimed as (
              select viewer_id from people_recommendation_refresh_jobs
              where next_attempt_at <= $1
                and (status='PENDING' or (status='CLAIMED' and claimed_at <= $1-$2::interval))
              order by next_attempt_at,viewer_id
              for update skip locked
              limit $3
            )
            update people_recommendation_refresh_jobs job
            set status='CLAIMED',claimed_at=$1,updated_at=$1
            from claimed where job.viewer_id=claimed.viewer_id
            returning job.viewer_id,job.claimed_at"#,
        )
        .bind(now)
        .bind(format!(
            "{} milliseconds",
            lease_timeout.num_milliseconds().max(100)
        ))
        .bind(i64::from(limit.clamp(1, 500)))
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| RecommendationRefreshClaim {
                viewer_id: row.get("viewer_id"),
                claimed_at: row.get("claimed_at"),
            })
            .collect())
    }

    async fn complete_refresh(&self, claim: RecommendationRefreshClaim) -> Result<()> {
        sqlx::query(
            "delete from people_recommendation_refresh_jobs where viewer_id=$1 and status='CLAIMED' and claimed_at=$2",
        )
            .bind(claim.viewer_id)
            .bind(claim.claimed_at)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn fail_refresh(
        &self,
        claim: RecommendationRefreshClaim,
        error: &str,
        next_attempt_at: DateTime<Utc>,
        max_attempts: i32,
    ) -> Result<()> {
        sqlx::query(
            r#"update people_recommendation_refresh_jobs
               set attempt_count=attempt_count+1,
                   status=case when attempt_count+1 >= $4 then 'DEAD_LETTERED' else 'PENDING' end,
                   next_attempt_at=$2,claimed_at=null,last_error=left($3,1000),updated_at=now()
               where viewer_id=$1 and status='CLAIMED' and claimed_at=$5"#,
        )
        .bind(claim.viewer_id)
        .bind(next_attempt_at)
        .bind(error)
        .bind(max_attempts.clamp(1, 20))
        .bind(claim.claimed_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn rollout(&self, surface: &str) -> Result<RecommendationRollout> {
        let row = sqlx::query(
            r#"
            select r.mode, r.deterministic_version, r.learned_version, r.canary_percent,
                   m.model_name, m.model_kind, m.status, m.artifact_uri,
                   m.artifact_sha256, m.feature_schema_version, m.fallback_version,
                   m.embedding_dimension, m.enabled, m.activated_at,
                   coalesce(readiness.ready, false) as data_ready
            from people_recommendation_rollouts r
            left join people_recommendation_models m
              on m.model_version = r.learned_version
            left join people_recommendation_model_readiness readiness
              on readiness.model_version = r.learned_version
            where r.surface = $1
            "#,
        )
        .bind(surface)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(RecommendationRollout::deterministic("people-linear-v1"));
        };
        let learned_version = row.get::<Option<String>, _>("learned_version");
        let learned_ready = learned_version.as_ref().is_some_and(|version| {
            learned_manifest_from_row(version, &row)
                .map(|manifest| manifest.validate(true).is_ok() && manifest.status.can_serve())
                .unwrap_or(false)
                && row.get::<Option<bool>, _>("enabled").unwrap_or(false)
                && row.get::<bool, _>("data_ready")
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
            from people_recommendation_events
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

#[async_trait]
impl RecommendationEventRepositoryPort for PgPeopleRecommendationRepository {
    async fn record_event(&self, event: &RecommendationEvent) -> Result<bool> {
        event.validate()?;
        let mut tx = self.pool.begin().await?;
        let inserted = sqlx::query(
            r#"insert into people_recommendation_events (
              event_id,schema_version,occurred_at,viewer_id,subject_type,subject_id,actor_id,
              surface,position,request_id,session_id,action,source,model_version,impression_id,
              propensity_micros,dwell_ms,metadata_json
            ) values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18)
            on conflict (event_id) do nothing"#,
        )
        .bind(event.event_id)
        .bind(i32::from(event.schema_version))
        .bind(event.occurred_at)
        .bind(event.viewer_id)
        .bind(event.subject_type.as_str())
        .bind(event.subject_id)
        .bind(event.actor_id)
        .bind(event.surface.as_str())
        .bind(
            event
                .position
                .map(|value| i32::try_from(value).unwrap_or(i32::MAX)),
        )
        .bind(&event.request_id)
        .bind(event.session_id)
        .bind(event.action.as_str())
        .bind(&event.source)
        .bind(&event.model_version)
        .bind(event.impression_id)
        .bind(
            event
                .propensity_micros
                .map(|value| i32::try_from(value).unwrap_or(i32::MAX)),
        )
        .bind(
            event
                .dwell_ms
                .map(|value| i32::try_from(value).unwrap_or(i32::MAX)),
        )
        .bind(&event.metadata_json)
        .execute(&mut *tx)
        .await
        .map_err(recommendation_admission_error)?
        .rows_affected()
            == 1;
        if inserted && event.action.is_negative() {
            enqueue_refresh_with_executor(&mut *tx, &[event.viewer_id], event.occurred_at).await?;
        }
        tx.commit().await?;
        Ok(inserted)
    }
}

#[async_trait]
impl PeopleRecommendationMaintenancePort for PgPeopleRecommendationRepository {
    async fn purge_expired(
        &self,
        now: DateTime<Utc>,
        event_cutoff: DateTime<Utc>,
        limit: i32,
    ) -> Result<PeopleRecommendationRetentionResult> {
        let limit = i64::from(limit.clamp(1, 5_000));
        let mut tx = self.pool.begin().await?;
        let candidates = sqlx::query(
            r#"with doomed as (
                select ctid from people_recommendation_candidates
                where expires_at <= $1 order by expires_at, viewer_id, candidate_user_id
                for update skip locked
                limit $2
            ) delete from people_recommendation_candidates
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
                from people_recommendation_impressions i
                join people_recommendation_snapshots s on s.id = i.snapshot_id
                where s.expires_at <= $1
                order by i.snapshot_id, i.impression_id
                for update of i skip locked
                limit $2
            ) delete from people_recommendation_impressions
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
                from people_recommendation_snapshot_items i
                join people_recommendation_snapshots s on s.id = i.snapshot_id
                where s.expires_at <= $1
                order by i.snapshot_id, i.candidate_user_id
                for update of i skip locked
                limit $2
            ) delete from people_recommendation_snapshot_items
              where ctid in (select ctid from doomed)"#,
        )
        .bind(now)
        .bind(limit)
        .execute(&mut *tx)
        .await?
        .rows_affected() as i64;
        let snapshots = sqlx::query(
            r#"with doomed as (
                select ctid from people_recommendation_snapshots
                where expires_at <= $1
                  and not exists (
                      select 1 from people_recommendation_impressions
                      where snapshot_id = people_recommendation_snapshots.id
                  )
                  and not exists (
                      select 1 from people_recommendation_snapshot_items
                      where snapshot_id = people_recommendation_snapshots.id
                  )
                order by expires_at, id
                for update skip locked
                limit $2
            ) delete from people_recommendation_snapshots
              where ctid in (select ctid from doomed)"#,
        )
        .bind(now)
        .bind(limit)
        .execute(&mut *tx)
        .await?
        .rows_affected() as i64;
        let events = sqlx::query(
            r#"with doomed as (
                select ctid from people_recommendation_events
                where occurred_at <= $1 order by occurred_at, event_id
                for update skip locked
                limit $2
            ) delete from people_recommendation_events
              where ctid in (select ctid from doomed)"#,
        )
        .bind(event_cutoff)
        .bind(limit)
        .execute(&mut *tx)
        .await?
        .rows_affected() as i64;
        let experiment_assignments = sqlx::query(
            r#"with doomed as (
                select ctid from people_experiment_assignments
                where expires_at <= $1 order by expires_at, viewer_id
                for update skip locked
                limit $2
            ) delete from people_experiment_assignments
              where ctid in (select ctid from doomed)"#,
        )
        .bind(now)
        .bind(limit)
        .execute(&mut *tx)
        .await?
        .rows_affected() as i64;
        let embeddings = sqlx::query(
            r#"with doomed as (
                select e.ctid from people_embeddings e
                where e.updated_at <= $1
                  and exists (
                      select 1 from people_recommendation_models m
                      where m.model_version=e.model_version
                        and m.status in ('RETIRED','REVOKED')
                  )
                order by e.updated_at, e.subject_id
                for update skip locked
                limit $2
            ) delete from people_embeddings
              where ctid in (select ctid from doomed)"#,
        )
        .bind(event_cutoff)
        .bind(limit)
        .execute(&mut *tx)
        .await?
        .rows_affected() as i64;
        let refresh_jobs = sqlx::query(
            r#"with doomed as (
                select ctid from people_recommendation_refresh_jobs
                where status='DEAD_LETTERED' and updated_at <= $1
                order by updated_at, viewer_id
                for update skip locked
                limit $2
            ) delete from people_recommendation_refresh_jobs
              where ctid in (select ctid from doomed)"#,
        )
        .bind(event_cutoff)
        .bind(limit)
        .execute(&mut *tx)
        .await?
        .rows_affected() as i64;
        tx.commit().await?;
        Ok(PeopleRecommendationRetentionResult {
            candidates,
            snapshot_items,
            impressions,
            snapshots,
            events,
            experiment_assignments,
            embeddings,
            refresh_jobs,
        })
    }
}

fn learned_manifest_from_row(
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

fn map_recommendation_event(row: sqlx::postgres::PgRow) -> Result<RecommendationEvent> {
    Ok(RecommendationEvent {
        event_id: row.get("event_id"),
        schema_version: u16::try_from(row.get::<i32, _>("schema_version"))?,
        occurred_at: row.get("occurred_at"),
        viewer_id: row.get("viewer_id"),
        subject_type: RecommendationSubjectType::parse(
            row.get::<String, _>("subject_type").as_str(),
        )?,
        subject_id: row.get("subject_id"),
        actor_id: row.get("actor_id"),
        surface: RecommendationSurface::parse(row.get::<String, _>("surface").as_str())?,
        position: row
            .get::<Option<i32>, _>("position")
            .map(u32::try_from)
            .transpose()?,
        request_id: row.get("request_id"),
        session_id: row.get("session_id"),
        action: RecommendationAction::parse(row.get::<String, _>("action").as_str())?,
        source: row.get("source"),
        model_version: row.get("model_version"),
        impression_id: row.get("impression_id"),
        propensity_micros: row
            .get::<Option<i32>, _>("propensity_micros")
            .map(u32::try_from)
            .transpose()?,
        dwell_ms: row
            .get::<Option<i32>, _>("dwell_ms")
            .map(u32::try_from)
            .transpose()?,
        metadata_json: row.get("metadata_json"),
    })
}
