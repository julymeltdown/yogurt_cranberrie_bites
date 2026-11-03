use crate::application::errors::AuthError;
use crate::application::ports::UserDeletionSagaPort;
use crate::domain::user_deletion::{
    UserDeletionSaga, UserDeletionSagaStatus, UserDeletionStep, UserDeletionStepStatus,
    UserDeletionTarget,
};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use sqlx::{PgConnection, PgPool, Row};
use std::collections::HashMap;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Clone)]
pub struct PgUserDeletionSagaRepository {
    pool: PgPool,
}

impl PgUserDeletionSagaRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn find_by_saga_id(&self, saga_id: Uuid) -> anyhow::Result<Option<UserDeletionSaga>> {
        let row = sqlx::query(
            r#"
            select id, user_id, actor_id, status, retention_until,
                   created_at, updated_at, completed_at
            from user_deletion_sagas
            where id = $1
            "#,
        )
        .bind(saga_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let steps = self.load_steps(saga_id).await?;
        Ok(Some(map_saga(row, steps)?))
    }

    async fn load_steps(&self, saga_id: Uuid) -> anyhow::Result<Vec<UserDeletionStep>> {
        let rows = sqlx::query(
            r#"
            select s.saga_id, g.user_id, g.actor_id, s.target, s.status,
                   s.attempt_count, s.next_attempt_at, s.locked_at,
                   s.last_error, s.completed_at, g.retention_until,
                   g.retry_generation, g.origin_traceparent
            from user_deletion_steps s
            join user_deletion_sagas g on g.id = s.saga_id
            where s.saga_id = $1
            order by s.target asc
            "#,
        )
        .bind(saga_id)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(map_step).collect()
    }

    async fn load_steps_for_sagas(
        connection: &mut PgConnection,
        saga_ids: &[Uuid],
    ) -> anyhow::Result<HashMap<Uuid, Vec<UserDeletionStep>>> {
        if saga_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let rows = sqlx::query(
            r#"
            select s.saga_id, g.user_id, g.actor_id, s.target, s.status,
                   s.attempt_count, s.next_attempt_at, s.locked_at,
                   s.last_error, s.completed_at, g.retention_until,
                   g.retry_generation, g.origin_traceparent
            from user_deletion_steps s
            join user_deletion_sagas g on g.id = s.saga_id
            where s.saga_id = any($1)
            order by s.saga_id asc, s.target asc
            "#,
        )
        .bind(saga_ids)
        .fetch_all(connection)
        .await?;
        let mut steps_by_saga = HashMap::<Uuid, Vec<UserDeletionStep>>::new();
        for row in rows {
            let saga_id: Uuid = row.get("saga_id");
            steps_by_saga
                .entry(saga_id)
                .or_default()
                .push(map_step(row)?);
        }
        Ok(steps_by_saga)
    }
}

#[async_trait]
impl UserDeletionSagaPort for PgUserDeletionSagaRepository {
    async fn begin(
        &self,
        request: crate::application::dto::deletion_dto::AuthBeginUserDeletionDto,
    ) -> crate::application::errors::AuthResult<
        Option<crate::application::dto::deletion_dto::AuthUserDeletionSagaRecordDto>,
    > {
        let request = crate::domain::user_deletion::BeginUserDeletion::from(request);

        let result: anyhow::Result<_> = async {
            if let Some(existing) = self
                .find_by_user_id(request.user_id)
                .await?
                .map(UserDeletionSaga::from)
            {
                return Ok(Some(existing));
            }

            let mut tx = self.pool.begin().await?;
            let user_exists = sqlx::query_scalar::<_, Uuid>(
                r#"
            update users
            set status = 'DELETED',
                deleted_at = coalesce(deleted_at, $5),
                deleted_by = $2,
                delete_reason = $3,
                retention_until = $4,
                updated_at = $5
            where id = $1
            returning id
            "#,
            )
            .bind(request.user_id)
            .bind(request.actor_id)
            .bind(&request.reason)
            .bind(request.retention_until)
            .bind(request.audit.occurred_at)
            .fetch_optional(&mut *tx)
            .await?;
            if user_exists.is_none() {
                tx.rollback().await?;
                return Ok(None);
            }

            sqlx::query(
                r#"
            update user_identities
            set deleted_at = coalesce(deleted_at, $2), retention_until = $3
            where user_id = $1
            "#,
            )
            .bind(request.user_id)
            .bind(request.audit.occurred_at)
            .bind(request.retention_until)
            .execute(&mut *tx)
            .await?;

            sqlx::query(
            "update refresh_tokens set revoked_at = coalesce(revoked_at, $2) where user_id = $1",
        )
        .bind(request.user_id)
        .bind(request.audit.occurred_at)
        .execute(&mut *tx)
        .await?;

            sqlx::query(
                r#"
            insert into auth_audit_events (
                id, actor_id, subject_user_id, action, outcome,
                request_id, ip_address, user_agent, metadata_json, occurred_at
            ) values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
            "#,
            )
            .bind(request.audit.id)
            .bind(request.audit.actor_id)
            .bind(request.audit.subject_user_id)
            .bind(&request.audit.action)
            .bind(&request.audit.outcome)
            .bind(&request.audit.request_id)
            .bind(&request.audit.ip_address)
            .bind(&request.audit.user_agent)
            .bind(&request.audit.metadata_json)
            .bind(request.audit.occurred_at)
            .execute(&mut *tx)
            .await?;

            let inserted = sqlx::query_scalar::<_, Uuid>(
                r#"
            insert into user_deletion_sagas (
                id, user_id, actor_id, status, retention_until, created_at, updated_at,
                origin_traceparent
            ) values ($1,$2,$3,'IN_PROGRESS',$4,$5,$5,$6)
            on conflict (user_id) do nothing
            returning id
            "#,
            )
            .bind(request.saga_id)
            .bind(request.user_id)
            .bind(request.actor_id)
            .bind(request.retention_until)
            .bind(request.audit.occurred_at)
            .bind(platform::trace_context::current().map(|context| context.traceparent()))
            .fetch_optional(&mut *tx)
            .await?;

            let saga_id = if let Some(saga_id) = inserted {
                for target in request.targets {
                    sqlx::query(
                        r#"
                    insert into user_deletion_steps (
                        saga_id, target, status, attempt_count, next_attempt_at
                    ) values ($1,$2,'PENDING',0,$3)
                    on conflict (saga_id, target) do nothing
                    "#,
                    )
                    .bind(saga_id)
                    .bind(target.as_str())
                    .bind(request.audit.occurred_at)
                    .execute(&mut *tx)
                    .await?;
                }
                saga_id
            } else {
                sqlx::query_scalar::<_, Uuid>(
                    "select id from user_deletion_sagas where user_id = $1",
                )
                .bind(request.user_id)
                .fetch_one(&mut *tx)
                .await?
            };

            tx.commit().await?;
            self.find_by_saga_id(saga_id).await
        }
        .await;
        result
            .map_err(crate::adapters::out::errors::boundary_error)
            .map(|value| value.map(Into::into))
    }

    async fn find_by_user_id(
        &self,
        user_id: Uuid,
    ) -> crate::application::errors::AuthResult<
        Option<crate::application::dto::deletion_dto::AuthUserDeletionSagaRecordDto>,
    > {
        let result: anyhow::Result<_> = async {
            let saga_id = sqlx::query_scalar::<_, Uuid>(
                "select id from user_deletion_sagas where user_id = $1",
            )
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?;
            match saga_id {
                Some(saga_id) => self.find_by_saga_id(saga_id).await,
                None => Ok(None),
            }
        }
        .await;
        result
            .map_err(crate::adapters::out::errors::boundary_error)
            .map(|value| value.map(Into::into))
    }

    async fn find_by_saga_id(
        &self,
        saga_id: Uuid,
    ) -> crate::application::errors::AuthResult<
        Option<crate::application::dto::deletion_dto::AuthUserDeletionSagaRecordDto>,
    > {
        let result: anyhow::Result<_> =
            async { PgUserDeletionSagaRepository::find_by_saga_id(self, saga_id).await }.await;
        result
            .map_err(crate::adapters::out::errors::boundary_error)
            .map(|value| value.map(Into::into))
    }

    async fn list_dead_lettered(
        &self,
        after: Option<(DateTime<Utc>, Uuid)>,
        limit: i32,
    ) -> crate::application::errors::AuthResult<
        Vec<crate::application::dto::deletion_dto::AuthUserDeletionSagaRecordDto>,
    > {
        let result: anyhow::Result<_> = async {
            let limit = i64::from(limit.clamp(1, 101));
            let mut tx = self.pool.begin().await?;
            sqlx::query("set transaction isolation level repeatable read read only")
                .execute(&mut *tx)
                .await?;
            let rows = if let Some((updated_at, saga_id)) = after {
                sqlx::query(
                    r#"
                select id, user_id, actor_id, status, retention_until,
                       created_at, updated_at, completed_at
                from user_deletion_sagas
                where status = 'DEAD_LETTERED' and (updated_at, id) > ($1, $2)
                order by updated_at asc, id asc
                limit $3
                "#,
                )
                .bind(updated_at)
                .bind(saga_id)
                .bind(limit)
                .fetch_all(&mut *tx)
                .await?
            } else {
                sqlx::query(
                    r#"
                select id, user_id, actor_id, status, retention_until,
                       created_at, updated_at, completed_at
                from user_deletion_sagas
                where status = 'DEAD_LETTERED'
                order by updated_at asc, id asc
                limit $1
                "#,
                )
                .bind(limit)
                .fetch_all(&mut *tx)
                .await?
            };
            let saga_ids = rows
                .iter()
                .take(limit as usize)
                .map(|row| row.get::<Uuid, _>("id"))
                .collect::<Vec<_>>();
            let mut steps_by_saga = Self::load_steps_for_sagas(&mut tx, &saga_ids).await?;
            let sagas = rows
                .into_iter()
                .take(limit as usize)
                .map(|row| {
                    let saga_id: Uuid = row.get("id");
                    map_saga(row, steps_by_saga.remove(&saga_id).unwrap_or_default())
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            tx.commit().await?;
            Ok(sagas)
        }
        .await;
        result
            .map_err(crate::adapters::out::errors::boundary_error)
            .map(|value| value.into_iter().map(Into::into).collect())
    }

    async fn retry_dead_lettered(
        &self,
        request: crate::application::dto::deletion_dto::AuthRetryUserDeletionDto,
    ) -> crate::application::errors::AuthResult<
        Option<crate::application::dto::deletion_dto::AuthUserDeletionSagaRecordDto>,
    > {
        let request = crate::domain::user_deletion::RetryUserDeletion::from(request);

        let result: anyhow::Result<_> = async {
            let mut tx = self.pool.begin().await?;
            let saga = sqlx::query(
                r#"
            select id, user_id, status, retry_generation
            from user_deletion_sagas
            where id = $1
            for update
            "#,
            )
            .bind(request.saga_id)
            .fetch_optional(&mut *tx)
            .await?;
            let Some(saga) = saga else {
                tx.rollback().await?;
                return Ok(None);
            };

            let user_id: Uuid = saga.get("user_id");
            let current_generation: i32 = saga.get("retry_generation");
            let current_status: String = saga.get("status");

            if let Some(existing) = sqlx::query(
                r#"
            select saga_id, actor_id, reason, evidence_reference
            from user_deletion_retry_resolutions
            where resolution_id = $1
            "#,
            )
            .bind(request.resolution_id)
            .fetch_optional(&mut *tx)
            .await?
            {
                let identical = existing.get::<Uuid, _>("saga_id") == request.saga_id
                    && existing.get::<Uuid, _>("actor_id") == request.actor_id
                    && existing.get::<String, _>("reason") == request.reason
                    && existing.get::<String, _>("evidence_reference")
                        == request.evidence_reference;
                tx.commit().await?;
                if !identical {
                    return Err(
                        AuthError::Conflict("Resolution ID conflicts with a prior retry").into(),
                    );
                }
                return self.find_by_saga_id(request.saga_id).await;
            }

            if current_status != UserDeletionSagaStatus::DeadLettered.as_str() {
                tx.rollback().await?;
                return Err(AuthError::Conflict(
                    "Only a dead-lettered account deletion can be retried",
                )
                .into());
            }

            let retry_generation = current_generation
                .checked_add(1)
                .ok_or_else(|| anyhow::anyhow!("User deletion retry generation exhausted"))?;
            let inserted = sqlx::query_scalar::<_, Uuid>(
                r#"
            insert into user_deletion_retry_resolutions (
                resolution_id, saga_id, actor_id, reason, evidence_reference,
                retry_generation, created_at
            ) values ($1, $2, $3, $4, $5, $6, $7)
            on conflict (resolution_id) do nothing
            returning resolution_id
            "#,
            )
            .bind(request.resolution_id)
            .bind(request.saga_id)
            .bind(request.actor_id)
            .bind(&request.reason)
            .bind(&request.evidence_reference)
            .bind(retry_generation)
            .bind(request.occurred_at)
            .fetch_optional(&mut *tx)
            .await?;
            if inserted.is_none() {
                let existing = sqlx::query(
                    r#"
                select saga_id, actor_id, reason, evidence_reference
                from user_deletion_retry_resolutions
                where resolution_id = $1
                "#,
                )
                .bind(request.resolution_id)
                .fetch_one(&mut *tx)
                .await?;
                let identical = existing.get::<Uuid, _>("saga_id") == request.saga_id
                    && existing.get::<Uuid, _>("actor_id") == request.actor_id
                    && existing.get::<String, _>("reason") == request.reason
                    && existing.get::<String, _>("evidence_reference")
                        == request.evidence_reference;
                tx.commit().await?;
                if !identical {
                    return Err(
                        AuthError::Conflict("Resolution ID conflicts with a prior retry").into(),
                    );
                }
                return self.find_by_saga_id(request.saga_id).await;
            }

            let requeued = sqlx::query(
                r#"
            update user_deletion_steps
            set status = 'PENDING', attempt_count = 0, next_attempt_at = $2,
                locked_at = null, completed_at = null
            where saga_id = $1 and status <> 'SUCCEEDED'
            "#,
            )
            .bind(request.saga_id)
            .bind(request.occurred_at)
            .execute(&mut *tx)
            .await?
            .rows_affected();
            if requeued == 0 {
                tx.rollback().await?;
                return Err(AuthError::Conflict(
                    "Account deletion has no incomplete steps to retry",
                )
                .into());
            }

            sqlx::query(
                r#"
            update user_deletion_sagas
            set status = 'IN_PROGRESS', retry_generation = $2,
                completed_at = null, updated_at = $3
            where id = $1
            "#,
            )
            .bind(request.saga_id)
            .bind(retry_generation)
            .bind(request.occurred_at)
            .execute(&mut *tx)
            .await?;

            let metadata = serde_json::json!({
                "resolutionId": request.resolution_id,
                "reason": request.reason,
                "evidenceReference": request.evidence_reference,
                "retryGeneration": retry_generation,
            })
            .to_string();
            sqlx::query(
                r#"
            insert into auth_audit_events (
                id, actor_id, subject_user_id, action, outcome,
                metadata_json, occurred_at
            ) values ($1, $2, $3, 'USER_DELETION_RETRY_AUTHORIZED', 'SUCCESS', $4, $5)
            "#,
            )
            .bind(Uuid::new_v4())
            .bind(request.actor_id)
            .bind(user_id)
            .bind(metadata)
            .bind(request.occurred_at)
            .execute(&mut *tx)
            .await?;

            tx.commit().await?;
            self.find_by_saga_id(request.saga_id).await
        }
        .await;
        result
            .map_err(crate::adapters::out::errors::boundary_error)
            .map(|value| value.map(Into::into))
    }

    async fn claim_due(
        &self,
        now: DateTime<Utc>,
        lease_timeout: Duration,
        limit: i32,
    ) -> crate::application::errors::AuthResult<
        Vec<crate::application::dto::deletion_dto::AuthUserDeletionStepRecordDto>,
    > {
        let result: anyhow::Result<Vec<UserDeletionStep>> = async {
            let stale_before = now - lease_timeout;
            let rows = sqlx::query(
                r#"
            with candidates as (
                select s.saga_id, s.target
                from user_deletion_steps s
                join user_deletion_sagas g on g.id = s.saga_id
                where g.status = 'IN_PROGRESS'
                  and (
                    (s.status in ('PENDING','RETRY') and s.next_attempt_at <= $1)
                    or (s.status = 'IN_PROGRESS' and s.locked_at <= $2)
                  )
                order by s.next_attempt_at asc, s.saga_id asc, s.target asc
                for update of s skip locked
                limit $3
            ), updated as (
                update user_deletion_steps s
                set status = 'IN_PROGRESS',
                    locked_at = $1,
                    attempt_count = s.attempt_count + 1,
                    last_error = null
                from candidates c
                where s.saga_id = c.saga_id and s.target = c.target
                returning s.saga_id, s.target, s.status, s.attempt_count,
                          s.next_attempt_at, s.locked_at, s.last_error, s.completed_at
            )
            select u.saga_id, g.user_id, g.actor_id, u.target, u.status,
                   u.attempt_count, u.next_attempt_at, u.locked_at,
                   u.last_error, u.completed_at, g.retention_until,
                   g.retry_generation, g.origin_traceparent
            from updated u
            join user_deletion_sagas g on g.id = u.saga_id
            order by u.saga_id asc, u.target asc
            "#,
            )
            .bind(now)
            .bind(stale_before)
            .bind(i64::from(limit.clamp(1, 100)))
            .fetch_all(&self.pool)
            .await?;
            rows.into_iter().map(map_step).collect()
        }
        .await;
        result
            .map_err(crate::adapters::out::errors::boundary_error)
            .map(|value| value.into_iter().map(Into::into).collect())
    }

    async fn mark_step_succeeded(
        &self,
        step: &crate::application::dto::deletion_dto::AuthUserDeletionStepRecordDto,
        now: DateTime<Utc>,
    ) -> crate::application::errors::AuthResult<
        crate::application::dto::deletion_dto::AuthUserDeletionSagaRecordDto,
    > {
        let step = &crate::domain::user_deletion::UserDeletionStep::from(step.clone());

        let result: anyhow::Result<_> = async {
            let mut tx = self.pool.begin().await?;
            let retry_generation = sqlx::query_scalar::<_, i32>(
                "select retry_generation from user_deletion_sagas where id = $1 for update",
            )
            .bind(step.saga_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| anyhow::anyhow!("User deletion saga not found"))?;
            if retry_generation != step.retry_generation {
                tx.commit().await?;
                return self
                    .find_by_saga_id(step.saga_id)
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("User deletion saga not found"));
            }
            let updated = sqlx::query(
                r#"
            update user_deletion_steps
            set status='SUCCEEDED', locked_at=null, last_error=null, completed_at=$4
            where saga_id=$1 and target=$2
              and status='IN_PROGRESS' and attempt_count=$3
            "#,
            )
            .bind(step.saga_id)
            .bind(step.target.as_str())
            .bind(step.attempt_count)
            .bind(now)
            .execute(&mut *tx)
            .await?
            .rows_affected();
            if updated == 0 {
                tx.commit().await?;
                return self
                    .find_by_saga_id(step.saga_id)
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("User deletion saga not found"));
            }
            sqlx::query(
                r#"
            update user_deletion_sagas g
            set status = case when not exists (
                    select 1 from user_deletion_steps s
                    where s.saga_id = g.id and s.status <> 'SUCCEEDED'
                ) then 'COMPLETED' else g.status end,
                completed_at = case when not exists (
                    select 1 from user_deletion_steps s
                    where s.saga_id = g.id and s.status <> 'SUCCEEDED'
                ) then $2 else g.completed_at end,
                updated_at = $2
            where g.id = $1
            "#,
            )
            .bind(step.saga_id)
            .bind(now)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            self.find_by_saga_id(step.saga_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("User deletion saga not found"))
        }
        .await;
        result
            .map_err(crate::adapters::out::errors::boundary_error)
            .map(|value| value.into())
    }

    async fn mark_step_failed(
        &self,
        step: &crate::application::dto::deletion_dto::AuthUserDeletionStepRecordDto,
        error: &str,
        next_attempt_at: DateTime<Utc>,
        dead_lettered: bool,
        now: DateTime<Utc>,
    ) -> crate::application::errors::AuthResult<
        crate::application::dto::deletion_dto::AuthUserDeletionSagaRecordDto,
    > {
        let step = &crate::domain::user_deletion::UserDeletionStep::from(step.clone());

        let result: anyhow::Result<_> = async {
            let mut tx = self.pool.begin().await?;
            let retry_generation = sqlx::query_scalar::<_, i32>(
                "select retry_generation from user_deletion_sagas where id = $1 for update",
            )
            .bind(step.saga_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| anyhow::anyhow!("User deletion saga not found"))?;
            if retry_generation != step.retry_generation {
                tx.commit().await?;
                return self
                    .find_by_saga_id(step.saga_id)
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("User deletion saga not found"));
            }
            let status = if dead_lettered {
                "DEAD_LETTERED"
            } else {
                "RETRY"
            };
            let updated = sqlx::query(
                r#"
            update user_deletion_steps
            set status=$3, locked_at=null, last_error=$4,
                next_attempt_at=$5, completed_at=case when $3='DEAD_LETTERED' then $6 else null end
            where saga_id=$1 and target=$2
              and status='IN_PROGRESS' and attempt_count=$7
            "#,
            )
            .bind(step.saga_id)
            .bind(step.target.as_str())
            .bind(status)
            .bind(error)
            .bind(next_attempt_at)
            .bind(now)
            .bind(step.attempt_count)
            .execute(&mut *tx)
            .await?
            .rows_affected();
            if updated == 0 {
                tx.commit().await?;
                return self
                    .find_by_saga_id(step.saga_id)
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("User deletion saga not found"));
            }
            sqlx::query(
                r#"
            update user_deletion_sagas
            set status = case when $2 then 'DEAD_LETTERED' else status end,
                updated_at = $3
            where id = $1
            "#,
            )
            .bind(step.saga_id)
            .bind(dead_lettered)
            .bind(now)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            self.find_by_saga_id(step.saga_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("User deletion saga not found"))
        }
        .await;
        result
            .map_err(crate::adapters::out::errors::boundary_error)
            .map(|value| value.into())
    }
}

fn map_saga(
    row: sqlx::postgres::PgRow,
    steps: Vec<UserDeletionStep>,
) -> anyhow::Result<UserDeletionSaga> {
    Ok(UserDeletionSaga {
        id: row.get("id"),
        user_id: row.get("user_id"),
        actor_id: row.get("actor_id"),
        status: UserDeletionSagaStatus::from_str(&row.get::<String, _>("status"))?,
        retention_until: row.get("retention_until"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        completed_at: row.get("completed_at"),
        steps,
    })
}

fn map_step(row: sqlx::postgres::PgRow) -> anyhow::Result<UserDeletionStep> {
    Ok(UserDeletionStep {
        saga_id: row.get("saga_id"),
        user_id: row.get("user_id"),
        actor_id: row.get("actor_id"),
        target: UserDeletionTarget::from_str(&row.get::<String, _>("target"))?,
        status: UserDeletionStepStatus::from_str(&row.get::<String, _>("status"))?,
        attempt_count: row.get("attempt_count"),
        retry_generation: row.get("retry_generation"),
        next_attempt_at: row.get("next_attempt_at"),
        locked_at: row.get("locked_at"),
        last_error: row.get("last_error"),
        completed_at: row.get("completed_at"),
        retention_until: row.get("retention_until"),
        origin_traceparent: row.get("origin_traceparent"),
    })
}
