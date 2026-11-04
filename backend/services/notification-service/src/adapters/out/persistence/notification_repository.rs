use crate::application::dto::repository_dto::{
    NotificationAuditRecordDto as NotificationAuditEvent,
    NotificationPageRecordDto as NotificationPage, NotificationRecordDto as Notification,
};
use crate::application::errors::{NotificationError, NotificationInvalidInput};
use crate::application::notification_contracts::{
    NotificationCreateOutcome, NotificationCursor, NotificationPosition, NotificationQueryPage,
    NotificationSuppression,
};
use crate::application::notification_creation::{
    NotificationAdmittedCreatePort, NotificationWriteDeadlineDto,
};
use crate::application::notification_cursor;
use crate::application::ports::NotificationRepositoryPort;
use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::{PgPool, Row};
use uuid::Uuid;

const LEGACY_PURGE_BATCH_LIMIT: i32 = 500;

struct WriteDeadline {
    expires: DateTime<Utc>,
    stop: tokio::time::Instant,
}
impl WriteDeadline {
    fn new(dto: NotificationWriteDeadlineDto) -> Result<Self> {
        let remaining = dto
            .expires_at
            .signed_duration_since(Utc::now())
            .to_std()
            .map_err(|_| NotificationError::AdmissionExpired)?;
        let stop = tokio::time::Instant::now()
            .checked_add(remaining)
            .ok_or(NotificationError::AdmissionExpired)?;
        Ok(Self {
            expires: dto.expires_at,
            stop,
        })
    }
    fn check(&self) -> Result<()> {
        if Utc::now() >= self.expires || tokio::time::Instant::now() >= self.stop {
            return Err(NotificationError::AdmissionExpired.into());
        }
        Ok(())
    }
    async fn arm(&self, tx: &mut sqlx::Transaction<'_, sqlx::Postgres>) -> Result<()> {
        self.check()?;
        let wall = self
            .expires
            .signed_duration_since(Utc::now())
            .num_milliseconds();
        let mono = self
            .stop
            .saturating_duration_since(tokio::time::Instant::now())
            .as_millis();
        let millis = i128::from(wall).min(mono as i128);
        // PostgreSQL timeout0 disables its bound; never round a short deadline
        // into an unlimited query. This setting is transaction-local.
        if millis <= 0 {
            return Err(NotificationError::AdmissionExpired.into());
        }
        sqlx::query("select set_config('statement_timeout',$1,true)")
            .bind(format!("{millis}ms"))
            .execute(&mut **tx)
            .await?;
        self.check()
    }
}

#[derive(Clone)]
pub struct PgNotificationRepository {
    pool: PgPool,
}

impl PgNotificationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn create_with_deadline(
        &self,
        notification: &Notification,
        deadline: Option<NotificationWriteDeadlineDto>,
    ) -> crate::application::errors::NotificationResult<NotificationCreateOutcome> {
        let result: anyhow::Result<_> = async {
            let bound = deadline.map(WriteDeadline::new).transpose()?;
            let mut tx = if let Some(bound) = &bound {
                tokio::time::timeout_at(bound.stop, self.pool.begin())
                    .await
                    .map_err(|_| NotificationError::AdmissionExpired)??
            } else {
                self.pool.begin().await?
            };
            let work = async {
                if let Some(bound) = &bound {
                    bound.arm(&mut tx).await?;
                }
                let mut subjects = vec![notification.user_id];
                if let Some(actor_id) = notification.actor_id {
                    subjects.push(actor_id);
                }
                Self::lock_subjects(&mut tx, &subjects).await?;
                if let Some(bound) = &bound {
                    bound.check()?;
                    bound.arm(&mut tx).await?;
                }
                if let Some(reason) =
                    Self::suppression_for(&mut tx, notification.user_id, notification.actor_id)
                        .await?
                {
                    return Ok(NotificationCreateOutcome::Suppressed(reason));
                }

                if let Some(bound) = &bound {
                    bound.check()?;
                    bound.arm(&mut tx).await?;
                }
                let existing = sqlx::query(
                    r#"
            select id, user_id, type, payload_json, created_at, read_at,
                   event_id, actor_id, occurred_at
            from notifications
            where user_id = $1 and event_id = $2 and type = $3
            "#,
                )
                .bind(notification.user_id)
                .bind(notification.event_id)
                .bind(&notification.notification_type)
                .fetch_optional(&mut *tx)
                .await?;
                if let Some(bound) = &bound {
                    bound.check()?;
                    bound.arm(&mut tx).await?;
                }
                if let Some(row) = existing {
                    return Ok(NotificationCreateOutcome::Existing(Self::map_notification(
                        row,
                    )));
                }

                let inserted = sqlx::query(
                    r#"
            insert into notifications (
                id, user_id, type, payload_json, created_at, read_at,
                event_id, actor_id, occurred_at
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            on conflict (user_id, event_id, type) do nothing
            returning id, user_id, type, payload_json, created_at, read_at,
                      event_id, actor_id, occurred_at
            "#,
                )
                .bind(notification.id)
                .bind(notification.user_id)
                .bind(&notification.notification_type)
                .bind(&notification.payload_json)
                .bind(notification.created_at)
                .bind(notification.read_at)
                .bind(notification.event_id)
                .bind(notification.actor_id)
                .bind(notification.occurred_at)
                .fetch_optional(&mut *tx)
                .await?;
                if let Some(bound) = &bound {
                    bound.check()?;
                    bound.arm(&mut tx).await?;
                }
                let outcome = if let Some(row) = inserted {
                    NotificationCreateOutcome::Created(Self::map_notification(row))
                } else {
                    let row = sqlx::query(
                        r#"
                select id, user_id, type, payload_json, created_at, read_at,
                       event_id, actor_id, occurred_at
                from notifications
                where user_id = $1 and event_id = $2 and type = $3
                "#,
                    )
                    .bind(notification.user_id)
                    .bind(notification.event_id)
                    .bind(&notification.notification_type)
                    .fetch_one(&mut *tx)
                    .await?;
                    if let Some(bound) = &bound {
                        bound.check()?;
                    }
                    NotificationCreateOutcome::Existing(Self::map_notification(row))
                };
                Ok(outcome)
            };
            let result: anyhow::Result<NotificationCreateOutcome> = if let Some(bound) = &bound {
                tokio::time::timeout_at(bound.stop, work)
                    .await
                    .map_err(|_| NotificationError::AdmissionExpired)?
            } else {
                work.await
            };
            let outcome = result.map_err(|error| {
                if bound.is_some()
                    && error
                        .downcast_ref::<sqlx::Error>()
                        .and_then(|e| e.as_database_error())
                        .and_then(|e| e.code())
                        .is_some_and(|code| code == "57014")
                {
                    anyhow::Error::new(NotificationError::AdmissionExpired)
                } else {
                    error
                }
            })?;
            if let Some(bound) = &bound {
                tokio::time::timeout_at(bound.stop, bound.arm(&mut tx))
                    .await
                    .map_err(|_| NotificationError::AdmissionExpired)??;
                bound.check()?;
                // COMMIT has now been sent: an acknowledgement timeout cannot
                // establish rollback and must remain an uncertain dependency failure.
                tokio::time::timeout_at(bound.stop, tx.commit())
                    .await
                    .map_err(|_| NotificationError::DependencyUnavailable)??;
            } else {
                tx.commit().await?;
            }
            Ok(outcome)
        }
        .await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    fn map_notification(row: sqlx::postgres::PgRow) -> Notification {
        Notification {
            id: row.get("id"),
            user_id: row.get("user_id"),
            notification_type: row.get("type"),
            payload_json: row.get("payload_json"),
            created_at: row.get("created_at"),
            read_at: row.get("read_at"),
            event_id: row.get("event_id"),
            actor_id: row.get("actor_id"),
            occurred_at: row.get("occurred_at"),
        }
    }

    /// Serialize all identities affected by one notification in UUID order.
    ///
    /// The state row exists before suppression, so creation and deletion
    /// synchronize even when a subject has never been deleted. The UUID order
    /// prevents reversed recipient/actor notifications from creating a lock cycle.
    async fn lock_subjects(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        subjects: &[Uuid],
    ) -> Result<Vec<Uuid>> {
        let mut ordered = subjects.to_vec();
        ordered.sort_unstable();
        ordered.dedup();
        for subject_id in &ordered {
            sqlx::query(
                r#"
                insert into notification_subject_states (subject_id, created_at)
                values ($1, now())
                on conflict (subject_id) do nothing
                "#,
            )
            .bind(subject_id)
            .execute(&mut **tx)
            .await?;
        }
        for subject_id in &ordered {
            sqlx::query_scalar::<_, Uuid>(
                "select subject_id from notification_subject_states where subject_id = $1 for update",
            )
            .bind(subject_id)
            .fetch_one(&mut **tx)
            .await?;
        }
        Ok(ordered)
    }

    async fn suppression_for(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        recipient_id: Uuid,
        actor_id: Option<Uuid>,
    ) -> Result<Option<NotificationSuppression>> {
        let recipient_suppressed: bool = sqlx::query_scalar(
            "select suppress_as_recipient from notification_subject_states where subject_id = $1",
        )
        .bind(recipient_id)
        .fetch_one(&mut **tx)
        .await?;
        if recipient_suppressed {
            return Ok(Some(NotificationSuppression::Recipient));
        }
        if let Some(actor_id) = actor_id {
            let actor_suppressed: bool = sqlx::query_scalar(
                "select suppress_as_actor from notification_subject_states where subject_id = $1",
            )
            .bind(actor_id)
            .fetch_one(&mut **tx)
            .await?;
            if actor_suppressed {
                return Ok(Some(NotificationSuppression::Actor));
            }
        }
        Ok(None)
    }

    async fn resolve_position(
        &self,
        recipient_id: Uuid,
        cursor: Option<NotificationCursor>,
    ) -> Result<Option<NotificationPosition>> {
        match cursor {
            None => Ok(None),
            Some(NotificationCursor::V2 {
                recipient_id: bound_recipient,
                position,
            }) => {
                if bound_recipient != recipient_id {
                    return Err(NotificationError::InvalidInput(
                        NotificationInvalidInput::CursorRecipientMismatch,
                    )
                    .into());
                }
                Ok(Some(position))
            }
            Some(NotificationCursor::Legacy { epoch_millis, id }) => {
                // The old token discarded sub-millisecond precision. Recover the
                // exact database value from its recipient-scoped anchor instead
                // of multiplying milliseconds and silently skipping rows. Soft-
                // deleted anchors remain resolvable; a purged/mismatched anchor
                // is an explicit expired legacy cursor.
                let created_at = sqlx::query_scalar::<_, DateTime<Utc>>(
                    "select created_at from notifications where user_id = $1 and id = $2",
                )
                .bind(recipient_id)
                .bind(id)
                .fetch_optional(&self.pool)
                .await?
                .ok_or(NotificationError::InvalidInput(
                    NotificationInvalidInput::LegacyCursorUnavailable,
                ))?;
                if created_at.timestamp_millis() != epoch_millis {
                    return Err(NotificationError::InvalidInput(
                        NotificationInvalidInput::LegacyCursorMismatch,
                    )
                    .into());
                }
                Ok(Some(NotificationPosition { created_at, id }))
            }
        }
    }

    async fn list_exact_inner(
        &self,
        recipient_id: Uuid,
        cursor: Option<NotificationCursor>,
        limit: i32,
    ) -> Result<NotificationQueryPage> {
        let limit = limit.clamp(1, 100);
        let position = self.resolve_position(recipient_id, cursor).await?;
        let cursor_time = position.map(|value| value.created_at);
        let cursor_id = position.map(|value| value.id);
        let rows = sqlx::query(
            r#"
            select id, user_id, type, payload_json, created_at, read_at,
                   event_id, actor_id, occurred_at
            from notifications
            where user_id = $1
              and deleted_at is null
              and (
                $2::timestamptz is null
                or created_at < $2
                or (created_at = $2 and id < $3)
              )
            order by created_at desc, id desc
            limit $4
            "#,
        )
        .bind(recipient_id)
        .bind(cursor_time)
        .bind(cursor_id)
        .bind(i64::from(limit) + 1)
        .fetch_all(&self.pool)
        .await?;
        let mut items: Vec<Notification> = rows.into_iter().map(Self::map_notification).collect();
        let next_position = if items.len() > limit as usize {
            let anchor = &items[limit as usize - 1];
            let position = NotificationPosition {
                created_at: anchor.created_at,
                id: anchor.id,
            };
            items.truncate(limit as usize);
            Some(position)
        } else {
            None
        };
        Ok(NotificationQueryPage {
            items,
            next_position,
            legacy_next_cursor: None,
        })
    }

    async fn purge_expired_batch_inner(
        &self,
        cutoff: DateTime<Utc>,
        actor_id: Uuid,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
        dry_run: bool,
        limit: i32,
    ) -> Result<Vec<Uuid>> {
        let limit = limit.clamp(1, 1_000);
        let mut tx = self.pool.begin().await?;
        let rows = sqlx::query(
            r#"
            select id, user_id
            from notifications
            where deleted_at is not null
              and retention_until is not null
              and retention_until <= $1
            order by retention_until asc, id asc
            for update skip locked
            limit $2
            "#,
        )
        .bind(cutoff)
        .bind(i64::from(limit))
        .fetch_all(&mut *tx)
        .await?;
        let notifications: Vec<(Uuid, Uuid)> = rows
            .into_iter()
            .map(|row| (row.get("id"), row.get("user_id")))
            .collect();

        if notifications.is_empty() {
            tx.commit().await?;
            return Ok(Vec::new());
        }

        let outcome = if dry_run { "DRY_RUN" } else { "SUCCESS" };
        for (notification_id, user_id) in &notifications {
            insert_audit_event(
                &mut tx,
                &NotificationAuditEvent {
                    id: Uuid::new_v4(),
                    actor_id: Some(actor_id),
                    notification_id: Some(*notification_id),
                    user_id: Some(*user_id),
                    action: "NOTIFICATION_PURGED".into(),
                    outcome: outcome.into(),
                    request_id: request_id.clone(),
                    ip_address: ip_address.clone(),
                    user_agent: user_agent.clone(),
                    metadata_json: json!({
                        "cutoff": cutoff,
                        "dryRun": dry_run,
                        "batchLimit": limit
                    })
                    .to_string(),
                    occurred_at: Utc::now(),
                },
            )
            .await?;
        }

        let notification_ids: Vec<Uuid> = notifications
            .iter()
            .map(|(notification_id, _)| *notification_id)
            .collect();
        if !dry_run {
            sqlx::query("delete from notifications where id = any($1)")
                .bind(&notification_ids)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(notification_ids)
    }
}

#[async_trait]
impl NotificationAdmittedCreatePort for PgNotificationRepository {
    async fn create_admitted(
        &self,
        notification: &Notification,
        deadline: NotificationWriteDeadlineDto,
    ) -> crate::application::errors::NotificationResult<NotificationCreateOutcome> {
        self.create_with_deadline(notification, Some(deadline))
            .await
    }
}

#[async_trait]
impl NotificationRepositoryPort for PgNotificationRepository {
    async fn is_recipient_suppressed(
        &self,
        user_id: Uuid,
    ) -> crate::application::errors::NotificationResult<bool> {
        let result: anyhow::Result<_> = async {

        Ok(sqlx::query_scalar::<_, bool>(
            "select exists(select 1 from notification_subject_states where subject_id = $1 and suppress_as_recipient)",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?)

        }.await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    async fn is_actor_suppressed(
        &self,
        user_id: Uuid,
    ) -> crate::application::errors::NotificationResult<bool> {
        let result: anyhow::Result<_> = async {

        Ok(sqlx::query_scalar::<_, bool>(
            "select exists(select 1 from notification_subject_states where subject_id = $1 and suppress_as_actor)",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?)

        }.await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    async fn create_consistent(
        &self,
        notification: &Notification,
    ) -> crate::application::errors::NotificationResult<NotificationCreateOutcome> {
        self.create_with_deadline(notification, None).await
    }

    async fn list_consistent(
        &self,
        recipient_id: Uuid,
        cursor: Option<NotificationCursor>,
        limit: i32,
    ) -> crate::application::errors::NotificationResult<NotificationQueryPage> {
        let result: anyhow::Result<_> =
            async { self.list_exact_inner(recipient_id, cursor, limit).await }.await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    async fn purge_expired_batch_consistent(
        &self,
        cutoff: DateTime<Utc>,
        actor_id: Uuid,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
        dry_run: bool,
        limit: i32,
    ) -> crate::application::errors::NotificationResult<Vec<Uuid>> {
        let result: anyhow::Result<_> = async {
            self.purge_expired_batch_inner(
                cutoff, actor_id, request_id, ip_address, user_agent, dry_run, limit,
            )
            .await
        }
        .await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    /// Compatibility-only raw save. Production creation uses
    /// NotificationCreatePort::create_unless_suppressed. Migration 0004 also
    /// guards the table boundary so a rolling legacy writer cannot bypass the
    /// recipient/actor state locks after its separate suppression precheck.
    async fn save(
        &self,
        notification: &Notification,
    ) -> crate::application::errors::NotificationResult<Notification> {
        let result: anyhow::Result<_> = async {

        let row = sqlx::query(
            r#"
            insert into notifications (
                id, user_id, type, payload_json, created_at, read_at, event_id, actor_id, occurred_at
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            on conflict (user_id, event_id, type) do update
            set id = notifications.id
            returning id, user_id, type, payload_json, created_at, read_at, event_id, actor_id, occurred_at
            "#,
        )
        .bind(notification.id)
        .bind(notification.user_id)
        .bind(&notification.notification_type)
        .bind(&notification.payload_json)
        .bind(notification.created_at)
        .bind(notification.read_at)
        .bind(notification.event_id)
        .bind(notification.actor_id)
        .bind(notification.occurred_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(Self::map_notification(row))

        }.await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    async fn find_by_event_id_and_type(
        &self,
        user_id: Uuid,
        event_id: Uuid,
        notification_type: &str,
    ) -> crate::application::errors::NotificationResult<Option<Notification>> {
        let result: anyhow::Result<_> = async {

        let row = sqlx::query(
            r#"
            select id, user_id, type, payload_json, created_at, read_at, event_id, actor_id, occurred_at
            from notifications
            where user_id = $1 and event_id = $2 and type = $3
            "#,
        )
        .bind(user_id)
        .bind(event_id)
        .bind(notification_type)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(Self::map_notification))

        }.await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    async fn find_by_id(
        &self,
        user_id: Uuid,
        notification_id: Uuid,
    ) -> crate::application::errors::NotificationResult<Option<Notification>> {
        let result: anyhow::Result<_> = async {

        let row = sqlx::query(
            r#"
            select id, user_id, type, payload_json, created_at, read_at, event_id, actor_id, occurred_at
            from notifications
            where user_id = $1 and id = $2 and deleted_at is null
            "#,
        )
        .bind(user_id)
        .bind(notification_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(Self::map_notification))

        }.await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    async fn list(
        &self,
        user_id: Uuid,
        cursor: Option<(i64, Uuid)>,
        limit: i32,
    ) -> crate::application::errors::NotificationResult<NotificationPage> {
        let result: anyhow::Result<_> = async {
            let cursor =
                cursor.map(|(epoch_millis, id)| NotificationCursor::Legacy { epoch_millis, id });
            let page = self.list_exact_inner(user_id, cursor, limit).await?;
            Ok(NotificationPage {
                items: page.items,
                next_cursor: page
                    .next_position
                    .map(|position| notification_cursor::encode(user_id, position)),
            })
        }
        .await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    async fn mark_read(
        &self,
        user_id: Uuid,
        notification_id: Uuid,
        read_at: DateTime<Utc>,
    ) -> crate::application::errors::NotificationResult<Option<(Notification, bool)>> {
        let result: anyhow::Result<_> = async {
            let row = sqlx::query(
                r#"
            with target as (
                select id, read_at is null as updated
                from notifications
                where user_id = $1 and id = $2 and deleted_at is null
                for update
            ), changed as (
                update notifications as notification
                set read_at = coalesce(notification.read_at, $3)
                from target
                where notification.id = target.id
                returning notification.id, notification.user_id, notification.type,
                    notification.payload_json, notification.created_at, notification.read_at,
                    notification.event_id, notification.actor_id, notification.occurred_at,
                    target.updated
            )
            select id, user_id, type, payload_json, created_at, read_at,
                event_id, actor_id, occurred_at, updated
            from changed
            "#,
            )
            .bind(user_id)
            .bind(notification_id)
            .bind(read_at)
            .fetch_optional(&self.pool)
            .await?;
            Ok(row.map(|row| {
                let updated = row.get::<bool, _>("updated");
                (Self::map_notification(row), updated)
            }))
        }
        .await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    async fn mark_all_read(
        &self,
        user_id: Uuid,
        read_at: DateTime<Utc>,
    ) -> crate::application::errors::NotificationResult<i64> {
        let result: anyhow::Result<_> = async {

        let result = sqlx::query(
            "update notifications set read_at = $2 where user_id = $1 and read_at is null and deleted_at is null",
        )
        .bind(user_id)
        .bind(read_at)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() as i64)

        }.await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    async fn soft_delete_with_audit(
        &self,
        user_id: Uuid,
        notification_id: Uuid,
        actor_id: Uuid,
        reason: &str,
        retention_until: DateTime<Utc>,
        audit_event: &NotificationAuditEvent,
    ) -> crate::application::errors::NotificationResult<bool> {
        let result: anyhow::Result<_> = async {
            let mut tx = self.pool.begin().await?;
            let result = sqlx::query(
                r#"
            update notifications
            set deleted_at = now(), deleted_by = $3, delete_reason = $4, retention_until = $5
            where user_id = $1 and id = $2 and deleted_at is null
            "#,
            )
            .bind(user_id)
            .bind(notification_id)
            .bind(actor_id)
            .bind(reason)
            .bind(retention_until)
            .execute(&mut *tx)
            .await?;
            if result.rows_affected() == 0 {
                tx.rollback().await?;
                return Ok(false);
            }
            insert_audit_event(&mut tx, audit_event).await?;
            tx.commit().await?;
            Ok(true)
        }
        .await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    async fn record_audit_event(
        &self,
        event: &NotificationAuditEvent,
    ) -> crate::application::errors::NotificationResult<()> {
        let result: anyhow::Result<_> = async {
            let mut tx = self.pool.begin().await?;
            insert_audit_event(&mut tx, event).await?;
            tx.commit().await?;
            Ok(())
        }
        .await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    async fn purge_expired_notifications(
        &self,
        cutoff: DateTime<Utc>,
        actor_id: Uuid,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
        dry_run: bool,
    ) -> crate::application::errors::NotificationResult<Vec<Uuid>> {
        let result: anyhow::Result<_> = async {
            self.purge_expired_batch_inner(
                cutoff,
                actor_id,
                request_id,
                ip_address,
                user_agent,
                dry_run,
                LEGACY_PURGE_BATCH_LIMIT,
            )
            .await
        }
        .await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    async fn apply_user_deletion(
        &self,
        saga_id: Uuid,
        user_id: Uuid,
        actor_id: Uuid,
        retention_until: DateTime<Utc>,
    ) -> crate::application::errors::NotificationResult<()> {
        let command = crate::concert::deletion_owner::DeletionCommand::new(
            saga_id, user_id, actor_id, retention_until,
        )?;
        self.apply_user_deletion_receipt(command).await.map(|_| ())
    }

    async fn apply_user_deletion_with_receipt(
        &self,
        saga_id: Uuid,
        user_id: Uuid,
        actor_id: Uuid,
        retention_until: DateTime<Utc>,
    ) -> crate::application::errors::NotificationResult<
        crate::application::dto::notification_dto::NotificationDeletionReceiptDto,
    > {
        let command = crate::concert::deletion_owner::DeletionCommand::new(
            saga_id,
            user_id,
            actor_id,
            retention_until,
        )?;
        let receipt = self.apply_user_deletion_receipt(command).await?;
        let command = receipt.command();
        Ok(
            crate::application::dto::notification_dto::NotificationDeletionReceiptDto {
                version: u32::from(
                    crate::concert::deletion_owner::CommittedDeletionReceipt::VERSION,
                ),
                saga_id: command.saga_id(),
                user_id: command.user_id(),
                actor_id: command.actor_id(),
                retention_until: command.retention_until(),
                fence_revision: receipt.fence_revision(),
                effect_xid: receipt.effect_xid().to_owned(),
                recorded_at: receipt.recorded_at(),
            },
        )
    }
}

impl PgNotificationRepository {
    /// Internal Notification-owner proof for the existing authenticated Auth
    /// saga. Transport must retain its Auth-only policy; this grants no public
    /// caller capability and is not the complete BE42 acknowledgement contract.
    pub async fn apply_user_deletion_receipt(
        &self,
        command: crate::concert::deletion_owner::DeletionCommand,
    ) -> crate::application::errors::NotificationResult<crate::concert::deletion_owner::CommittedDeletionReceipt> {
        let result: anyhow::Result<_> = async {
            let mut tx = self.pool.begin().await?;
            // Keep the exact shared absent-row/create/delete serialization and
            // UUID order. No independent receipt transaction may acknowledge it.
            Self::lock_subjects(&mut tx, &[command.user_id()]).await?;
            let pending = crate::concert::deletion_owner::apply_locked(&mut tx, &command).await?;
            // Failure/uncertainty here returns no typed committed receipt.
            // A retry uses the same command and recovers the durable proof.
            tx.commit().await?;
            Ok(pending.after_commit())
        }.await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }
}

async fn insert_audit_event(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    event: &NotificationAuditEvent,
) -> Result<()> {
    sqlx::query(
        r#"
        insert into notification_audit_events (
            id, actor_id, notification_id, user_id, action, outcome,
            request_id, ip_address, user_agent, metadata_json, occurred_at
        ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        "#,
    )
    .bind(event.id)
    .bind(event.actor_id)
    .bind(event.notification_id)
    .bind(event.user_id)
    .bind(&event.action)
    .bind(&event.outcome)
    .bind(&event.request_id)
    .bind(&event.ip_address)
    .bind(&event.user_agent)
    .bind(&event.metadata_json)
    .bind(event.occurred_at)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
