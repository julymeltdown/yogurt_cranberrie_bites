use crate::application::ports::EventOutboxPort;
use crate::domain::post::{
    DeadLetterReplayError, DeadLetterReplayOutcome, DeadLetterReplayResolution, DeliveryAttempt,
    EventReplayFilter, OutboxMetricsSnapshot, StoredEvent,
};
use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, QueryBuilder, Row};
use tracing::Instrument;
use uuid::Uuid;

#[derive(Clone)]
pub struct PgEventOutbox {
    pool: PgPool,
}

macro_rules! post_database_span {
    ($name:literal, $operation:literal) => {{
        let span = tracing::info_span!(
            target: "post_service::adapters::out::persistence::event_outbox",
            $name,
            db_system = "postgresql",
            db_operation = $operation,
            trace_id = tracing::field::Empty,
            span_id = tracing::field::Empty,
            parent_span_id = tracing::field::Empty,
            sampled = tracing::field::Empty,
            outcome = tracing::field::Empty,
        );
        platform::trace_context::bind_current_child_to_span(&span);
        span
    }};
}

fn record_database_outcome<T, E>(span: &tracing::Span, result: &Result<T, E>) {
    span.record("outcome", if result.is_ok() { "success" } else { "error" });
}

impl PgEventOutbox {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn event_origin_traceparent(event: &StoredEvent) -> Option<String> {
        event
            .origin_traceparent
            .clone()
            .or_else(|| platform::trace_context::current().map(|context| context.traceparent()))
    }

    pub async fn record_publish_failure(
        &self,
        event_id: Uuid,
        error: &str,
        max_attempts: i32,
    ) -> Result<()> {
        let span = post_database_span!(
            "post_outbox_publish_failure",
            "post.outbox.record_publish_failure"
        );
        let result: Result<()> = async {
            sqlx::query(
                r#"
                update post_event_outbox
                set attempt_count = attempt_count + 1,
                    last_error = $2,
                    locked_at = null,
                    dead_lettered_at = case
                        when attempt_count + 1 >= $3 then coalesce(dead_lettered_at, now())
                        else dead_lettered_at
                    end
                where event_id = $1
                "#,
            )
            .bind(event_id)
            .bind(error)
            .bind(max_attempts.max(1))
            .execute(&self.pool)
            .await?;
            Ok(())
        }
        .instrument(span.clone())
        .await;
        record_database_outcome(&span, &result);
        result
    }

    pub(super) fn map_event(row: sqlx::postgres::PgRow) -> StoredEvent {
        StoredEvent {
            event_id: row.get("event_id"),
            event_type: row.get("event_type"),
            aggregate_id: row.get("aggregate_id"),
            aggregate_version: row.get("aggregate_version"),
            origin_traceparent: row.get("origin_traceparent"),
            actor_id: row.get("actor_id"),
            attributes: row.get("attributes"),
            payload: row.get("payload"),
            occurred_at: row.get("occurred_at"),
            created_at: row.get("created_at"),
            published: row.get("published"),
            published_at: row.get("published_at"),
            replay_resolution_id: row.get("replay_resolution_id"),
        }
    }
}

async fn find_resolution<'a>(
    tx: &mut sqlx::Transaction<'a, Postgres>,
    resolution_id: Uuid,
) -> Result<Option<sqlx::postgres::PgRow>> {
    Ok(sqlx::query(
        r#"
        select event_id, actor_id, reason, evidence_reference
        from post_event_dead_letter_resolutions
        where resolution_id = $1
        "#,
    )
    .bind(resolution_id)
    .fetch_optional(&mut **tx)
    .await?)
}

fn resolution_matches(
    row: &sqlx::postgres::PgRow,
    resolution: &DeadLetterReplayResolution,
) -> bool {
    row.get::<Uuid, _>("event_id") == resolution.event_id
        && row.get::<Uuid, _>("actor_id") == resolution.actor_id
        && row.get::<String, _>("reason") == resolution.reason
        && row.get::<String, _>("evidence_reference") == resolution.evidence_reference
}

#[async_trait]
impl EventOutboxPort for PgEventOutbox {
    async fn save(&self, event: &StoredEvent) -> Result<StoredEvent> {
        let span = post_database_span!("post_outbox_insert_event", "post.outbox.insert_event");
        let result: Result<StoredEvent> = async {
            let row = sqlx::query(
                r#"
                insert into post_event_outbox (
                    event_id, event_type, aggregate_id, actor_id, attributes, payload,
                    occurred_at, created_at, published, published_at, replay_resolution_id,
                    origin_traceparent
                )
                values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                returning event_id, event_type, aggregate_id, actor_id, attributes, payload,
                    occurred_at, created_at, published, published_at, aggregate_version,
                    replay_resolution_id, origin_traceparent
                "#,
            )
            .bind(event.event_id)
            .bind(&event.event_type)
            .bind(event.aggregate_id)
            .bind(event.actor_id)
            .bind(&event.attributes)
            .bind(&event.payload)
            .bind(event.occurred_at)
            .bind(event.created_at)
            .bind(event.published)
            .bind(event.published_at)
            .bind(event.replay_resolution_id)
            .bind(Self::event_origin_traceparent(event))
            .fetch_one(&self.pool)
            .await?;
            Ok(Self::map_event(row))
        }
        .instrument(span.clone())
        .await;
        record_database_outcome(&span, &result);
        result
    }

    async fn find_events(
        &self,
        filter: &EventReplayFilter,
        limit: i32,
    ) -> Result<Vec<StoredEvent>> {
        let mut qb = QueryBuilder::<Postgres>::new(
            "select event_id, event_type, aggregate_id, aggregate_version, actor_id, attributes, payload, occurred_at, created_at, published, published_at, replay_resolution_id, origin_traceparent from post_event_outbox where 1=1",
        );
        if let Some(actor_id) = filter.actor_id {
            qb.push(" and actor_id = ");
            qb.push_bind(actor_id);
        }
        if let Some(event_type) = &filter.event_type {
            qb.push(" and event_type = ");
            qb.push_bind(event_type);
        }
        if let Some(attribute) = &filter.attribute {
            qb.push(" and ");
            qb.push_bind(attribute);
            qb.push(" = any(attributes)");
        }
        if let Some(from) = filter.from {
            qb.push(" and occurred_at >= ");
            qb.push_bind(from);
        }
        if let Some(to) = filter.to {
            qb.push(" and occurred_at <= ");
            qb.push_bind(to);
        }
        if let Some(published) = filter.published {
            qb.push(" and published = ");
            qb.push_bind(published);
        }
        qb.push(" order by created_at asc limit ");
        qb.push_bind(i64::from(limit.max(1)));
        let rows = qb.build().fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(Self::map_event).collect())
    }

    async fn find_unpublished(
        &self,
        older_than: DateTime<Utc>,
        limit: i32,
    ) -> Result<Vec<StoredEvent>> {
        let span = post_database_span!("post_outbox_claim", "post.outbox.claim_unpublished");

        let result: Result<Vec<StoredEvent>> = async {
            let rows = sqlx::query(
                r#"
                with candidates as (
                    select event_id
                    from post_event_outbox as event
                    where published = false
                      and created_at <= $1
                      and dead_lettered_at is null
                      and (
                        locked_at is null
                        or locked_at <= now() - interval '5 minutes'
                      )
                      and not exists (
                        select 1
                        from post_event_outbox as earlier
                        where earlier.aggregate_id = event.aggregate_id
                          and earlier.published = false
                          and (
                            (
                              event.aggregate_version is null
                              and earlier.aggregate_version is null
                              and (earlier.created_at, earlier.event_id) < (event.created_at, event.event_id)
                            )
                            or (
                              event.aggregate_version is not null
                              and earlier.aggregate_version is null
                            )
                            or (
                              event.aggregate_version is not null
                              and earlier.aggregate_version is not null
                              and earlier.aggregate_version < event.aggregate_version
                            )
                          )
                      )
                    order by created_at asc, event_id asc
                    for update skip locked
                    limit $2
                )
                update post_event_outbox as event
                set locked_at = now()
                from candidates
                where event.event_id = candidates.event_id
                returning event.event_id, event.event_type, event.aggregate_id, event.actor_id,
                    event.attributes, event.payload, event.occurred_at, event.created_at,
                    event.published, event.published_at, event.aggregate_version,
                    event.replay_resolution_id, event.origin_traceparent
                "#,
            )
            .bind(older_than)
            .bind(i64::from(limit.max(1)))
            .fetch_all(&self.pool)
            .await?;
            Ok(rows.into_iter().map(Self::map_event).collect())
        }
        .instrument(span.clone())
        .await;

        record_database_outcome(&span, &result);
        result
    }

    async fn resolve_dead_letter(
        &self,
        resolution: &DeadLetterReplayResolution,
    ) -> Result<DeadLetterReplayOutcome> {
        let mut tx = self.pool.begin().await?;
        let event = sqlx::query(
            r#"
            select published, dead_lettered_at, locked_at, delivery_required_targets
            from post_event_outbox
            where event_id = $1
            for update
            "#,
        )
        .bind(resolution.event_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(DeadLetterReplayError::EventNotFound)?;

        if let Some(existing) = find_resolution(&mut tx, resolution.resolution_id).await? {
            if resolution_matches(&existing, resolution) {
                tx.commit().await?;
                return Ok(DeadLetterReplayOutcome::Existing);
            }
            return Err(DeadLetterReplayError::Conflict.into());
        }

        let published: bool = event.get("published");
        let dead_lettered_at: Option<DateTime<Utc>> = event.get("dead_lettered_at");
        let locked_at: Option<DateTime<Utc>> = event.get("locked_at");
        let required_targets: Option<Vec<String>> = event.get("delivery_required_targets");
        let legacy = required_targets.is_none();
        // In target mode, an expired final lease is authoritative even before
        // a worker maintenance tick has refreshed the parent dead-letter summary.
        if published || (legacy && dead_lettered_at.is_none()) {
            return Err(DeadLetterReplayError::NotDeadLettered.into());
        }
        if locked_at.is_some() {
            return Err(DeadLetterReplayError::Conflict.into());
        }

        // Use the same parent-before-job lock order as claims/completions. An
        // operator decision cannot cancel a live target lease. Lock before
        // evaluating database-clock expiry, including waits on another holder.
        let jobs = sqlx::query("select target from post_event_delivery_jobs where event_id=$1 order by target for update")
            .bind(resolution.event_id).fetch_all(&mut *tx).await?;
        if jobs.len() != 4 {
            return Err(DeadLetterReplayError::Conflict.into());
        }
        let live: bool = sqlx::query_scalar("select exists(select 1 from post_event_delivery_jobs where event_id=$1 and state='LEASED' and lease_expires_at>clock_timestamp())")
            .bind(resolution.event_id).fetch_one(&mut *tx).await?;
        if live {
            return Err(DeadLetterReplayError::Conflict.into());
        }
        super::delivery_schedule::credit_timeline_progress(&mut tx, resolution.event_id).await?;
        let eligible: bool = sqlx::query_scalar(
            "select exists(select 1 from post_event_delivery_jobs job where event_id=$1 and ($2 or job.target=any($3::text[])) and not exists(select 1 from event_publications p where p.event_id=job.event_id and p.target=job.target) and (state='DEAD' or (state='LEASED' and lease_expires_at<=clock_timestamp() and attempt_count-progress_credits>=max_attempts) or ($2 and state in ('PENDING','RETRY'))))"
        ).bind(resolution.event_id).bind(legacy).bind(&required_targets).fetch_one(&mut *tx).await?;
        if !eligible {
            return Err(DeadLetterReplayError::NotDeadLettered.into());
        }

        let inserted = sqlx::query_scalar::<_, Uuid>(
            r#"
            insert into post_event_dead_letter_resolutions (
                resolution_id, event_id, actor_id, reason, evidence_reference, created_at
            )
            values ($1, $2, $3, $4, $5, $6)
            on conflict (resolution_id) do nothing
            returning resolution_id
            "#,
        )
        .bind(resolution.resolution_id)
        .bind(resolution.event_id)
        .bind(resolution.actor_id)
        .bind(&resolution.reason)
        .bind(&resolution.evidence_reference)
        .bind(resolution.created_at)
        .fetch_optional(&mut *tx)
        .await?;

        if inserted.is_none() {
            if let Some(existing) = find_resolution(&mut tx, resolution.resolution_id).await? {
                if resolution_matches(&existing, resolution) {
                    tx.commit().await?;
                    return Ok(DeadLetterReplayOutcome::Existing);
                }
            }
            return Err(DeadLetterReplayError::Conflict.into());
        }

        // Established target scheduling releases only exhausted jobs. A null
        // mask explicitly identifies a legacy event-wide budget: its unfinished
        // targets participated in the same old budget and may be released.
        // Actual checkpoints and healthy target retries remain untouched.
        let released = sqlx::query(
            "update post_event_delivery_jobs job set state='PENDING',attempt_count=0,progress_credits=0,progress_checked_attempt_id=null,next_attempt_at=clock_timestamp(),lease_token=null,attempt_id=null,lease_expires_at=null,lease_kind='AUTO',replay_restore_state=null,release_resolution_id=$2 where event_id=$1 and ($3 or job.target=any($4::text[])) and not exists(select 1 from event_publications p where p.event_id=job.event_id and p.target=job.target) and (state='DEAD' or (state='LEASED' and lease_expires_at<=clock_timestamp() and attempt_count-progress_credits>=max_attempts) or ($3 and state in ('PENDING','RETRY')))"
        ).bind(resolution.event_id).bind(resolution.resolution_id).bind(legacy).bind(&required_targets).execute(&mut *tx).await?;
        if released.rows_affected() == 0 {
            return Err(DeadLetterReplayError::Conflict.into());
        }

        let updated = sqlx::query(
            r#"
            update post_event_outbox
            set attempt_count = (select coalesce(max(job.attempt_count),0) from post_event_delivery_jobs job where job.event_id=post_event_outbox.event_id),
                last_error = null,
                dead_lettered_at = null,
                locked_at = null,
                replay_resolution_id = $2
            where event_id = $1
              and published = false
              and locked_at is null
            "#,
        )
        .bind(resolution.event_id)
        .bind(resolution.resolution_id)
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() != 1 {
            return Err(DeadLetterReplayError::Conflict.into());
        }

        tx.commit().await?;
        Ok(DeadLetterReplayOutcome::Created)
    }

    async fn append_delivery_attempt(&self, attempt: &DeliveryAttempt) -> Result<()> {
        let span = post_database_span!(
            "post_outbox_append_delivery_attempt",
            "post.outbox.append_delivery_attempt"
        );
        let result: Result<()> = async {
            sqlx::query(
                r#"
                insert into post_event_delivery_attempts (
                    attempt_id, event_id, target, source, phase, occurred_at, resolution_id
                )
                values ($1, $2, $3, $4, $5, $6, $7)
                "#,
            )
            .bind(attempt.attempt_id)
            .bind(attempt.event_id)
            .bind(attempt.target.as_str())
            .bind(attempt.source.as_str())
            .bind(attempt.phase.as_str())
            .bind(attempt.occurred_at)
            .bind(attempt.resolution_id)
            .execute(&self.pool)
            .await?;
            Ok(())
        }
        .instrument(span.clone())
        .await;
        record_database_outcome(&span, &result);
        result
    }

    async fn delivery_completed(&self, event_id: Uuid, target: &str) -> Result<bool> {
        let span = post_database_span!(
            "post_outbox_delivery_check",
            "post.outbox.delivery_completed"
        );
        let result: Result<bool> = async {
            let completed: bool = sqlx::query_scalar(
                "select exists(select 1 from event_publications where event_id = $1 and target = $2)",
            )
            .bind(event_id)
            .bind(target)
            .fetch_one(&self.pool)
            .await?;
            Ok(completed)
        }
        .instrument(span.clone())
        .await;
        record_database_outcome(&span, &result);
        result
    }

    async fn mark_delivery_completed(
        &self,
        event_id: Uuid,
        target: &str,
        payload: &str,
        completed_at: DateTime<Utc>,
    ) -> Result<()> {
        let span = post_database_span!(
            "post_outbox_mark_delivery_completed",
            "post.outbox.mark_delivery_completed"
        );
        let result: Result<()> = async {
            sqlx::query(
                r#"
                insert into event_publications (event_id, target, published_at, payload)
                values ($1, $2, $3, $4)
                on conflict (event_id, target) do nothing
                "#,
            )
            .bind(event_id)
            .bind(target)
            .bind(completed_at)
            .bind(payload)
            .execute(&self.pool)
            .await?;
            Ok(())
        }
        .instrument(span.clone())
        .await;
        record_database_outcome(&span, &result);
        result
    }

    async fn mark_published(&self, event_id: Uuid, published_at: DateTime<Utc>) -> Result<()> {
        let span = post_database_span!("post_outbox_mark_published", "post.outbox.mark_published");
        let result: Result<()> = async {
            sqlx::query(
                r#"
                update post_event_outbox
                set published = true,
                    published_at = $2,
                    locked_at = null,
                    last_error = null
                where event_id = $1
                "#,
            )
            .bind(event_id)
            .bind(published_at)
            .execute(&self.pool)
            .await?;
            Ok(())
        }
        .instrument(span.clone())
        .await;
        record_database_outcome(&span, &result);
        result
    }

    async fn record_publish_failure(
        &self,
        event_id: Uuid,
        error: &str,
        max_attempts: i32,
    ) -> Result<()> {
        PgEventOutbox::record_publish_failure(self, event_id, error, max_attempts).await
    }

    async fn metrics_snapshot(&self, now: DateTime<Utc>) -> Result<OutboxMetricsSnapshot> {
        let unpublished_total: i64 = sqlx::query_scalar(
            r#"
            select count(*)
            from post_event_outbox
            where published = false
              and dead_lettered_at is null
            "#,
        )
        .fetch_one(&self.pool)
        .await?;
        let oldest_unpublished: Option<DateTime<Utc>> = sqlx::query_scalar(
            r#"
            select min(created_at)
            from post_event_outbox
            where published = false
              and dead_lettered_at is null
            "#,
        )
        .fetch_one(&self.pool)
        .await?;
        let dead_lettered_total: i64 = sqlx::query_scalar(
            "select count(*) from post_event_outbox where dead_lettered_at is not null",
        )
        .fetch_one(&self.pool)
        .await?;
        let oldest_unpublished_age_seconds = oldest_unpublished
            .map(|created_at| (now - created_at).num_seconds().max(0))
            .unwrap_or(0);
        Ok(OutboxMetricsSnapshot {
            unpublished_total,
            oldest_unpublished_age_seconds,
            dead_lettered_total,
        })
    }
}

#[async_trait]
impl crate::application::ports::DeliverySchedulePort for PgEventOutbox {
    async fn lease_status(
        &self,
        claim: &crate::application::dto::delivery_schedule_dto::TargetDeliveryClaimDto,
    ) -> Result<crate::application::dto::delivery_schedule_dto::DeliveryLeaseStatusDto> {
        super::delivery_schedule::PgDeliverySchedule::new(self.pool.clone())
            .lease_status(claim)
            .await
    }
    async fn claim_target(
        &self,
        target: crate::domain::post::DeliveryTarget,
        older_than: DateTime<Utc>,
        limit: i32,
        config: &crate::application::dto::delivery_schedule_dto::DeliveryScheduleConfigDto,
    ) -> Result<crate::application::dto::delivery_schedule_dto::TargetDeliveryBatchDto> {
        super::delivery_schedule::PgDeliverySchedule::new(self.pool.clone())
            .claim_target(target, older_than, limit, config)
            .await
    }
    async fn complete_target(
        &self,
        claim: &crate::application::dto::delivery_schedule_dto::TargetDeliveryClaimDto,
    ) -> Result<crate::application::dto::delivery_schedule_dto::DeliveryLeaseOutcomeDto> {
        super::delivery_schedule::PgDeliverySchedule::new(self.pool.clone())
            .complete_target(claim)
            .await
    }
    async fn fail_target(
        &self,
        claim: &crate::application::dto::delivery_schedule_dto::TargetDeliveryClaimDto,
    ) -> Result<crate::application::dto::delivery_schedule_dto::DeliveryLeaseOutcomeDto> {
        super::delivery_schedule::PgDeliverySchedule::new(self.pool.clone())
            .fail_target(claim)
            .await
    }
}
