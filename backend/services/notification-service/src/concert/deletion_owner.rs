//! Notification-local proof of an authenticated canonical Auth saga command.
//!
//! No transport/caller grant is introduced. The existing Auth-only RPC remains
//! the entry point. This is suppression/retention proof, never physical erasure
//! policy or a complete BE42 acknowledgement. Constructors disclose no data via
//! Debug; only the repository can return a receipt after its actual commit.
use crate::application::errors::{NotificationError, NotificationInvalidInput};
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::{PgConnection, Row};
use std::fmt;
use uuid::Uuid;

#[derive(Clone, PartialEq, Eq)]
pub struct DeletionCommand {
    saga_id: Uuid,
    user_id: Uuid,
    actor_id: Uuid,
    retention_until: DateTime<Utc>,
}
impl DeletionCommand {
    pub fn new(
        saga_id: Uuid,
        user_id: Uuid,
        actor_id: Uuid,
        retention_until: DateTime<Utc>,
    ) -> crate::application::errors::NotificationResult<Self> {
        for (id, error) in [
            (saga_id, NotificationInvalidInput::SagaIdRequired),
            (user_id, NotificationInvalidInput::UserIdRequired),
            (actor_id, NotificationInvalidInput::ActorIdRequired),
        ] {
            if id.is_nil() {
                return Err(NotificationError::InvalidInput(error));
            }
        }
        // PostgreSQL timestamps cannot represent leap-second nanoseconds.
        if retention_until.timestamp_subsec_nanos() >= 1_000_000_000 {
            return Err(NotificationError::InvalidInput(
                NotificationInvalidInput::RetentionDeadlineInvalid,
            ));
        }
        Ok(Self {
            saga_id,
            user_id,
            actor_id,
            retention_until,
        })
    }
    pub fn saga_id(&self) -> Uuid {
        self.saga_id
    }
    pub fn user_id(&self) -> Uuid {
        self.user_id
    }
    pub fn actor_id(&self) -> Uuid {
        self.actor_id
    }
    pub fn retention_until(&self) -> DateTime<Utc> {
        self.retention_until
    }
}
impl fmt::Debug for DeletionCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeletionCommand").finish_non_exhaustive()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct CommittedDeletionReceipt {
    command: DeletionCommand,
    fence_revision: i64,
    effect_xid: String,
    recorded_at: DateTime<Utc>,
}
impl CommittedDeletionReceipt {
    pub const VERSION: u16 = 1;
    pub fn command(&self) -> &DeletionCommand {
        &self.command
    }
    pub fn fence_revision(&self) -> i64 {
        self.fence_revision
    }
    /// Owner-native transaction identity, not a bearer capability or client proof.
    pub fn effect_xid(&self) -> &str {
        &self.effect_xid
    }
    /// Write observation time; not a fabricated PostgreSQL commit timestamp.
    pub fn recorded_at(&self) -> DateTime<Utc> {
        self.recorded_at
    }
}
impl fmt::Debug for CommittedDeletionReceipt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CommittedDeletionReceipt")
            .finish_non_exhaustive()
    }
}
pub(crate) struct PendingDeletionReceipt(CommittedDeletionReceipt);
impl PendingDeletionReceipt {
    /// Called only after the owning transaction's COMMIT acknowledgement.
    pub(crate) fn after_commit(self) -> CommittedDeletionReceipt {
        self.0
    }
}

fn rebound() -> anyhow::Error {
    NotificationError::InvalidInput(NotificationInvalidInput::DeletionSagaInputMismatch).into()
}
fn unavailable() -> anyhow::Error {
    NotificationError::DependencyUnavailable.into()
}

/// Caller holds the existing permanent subject row FOR UPDATE. This function
/// never opens a second connection/transaction or reverses creation's lock order.
/// Even an exact retry rechecks durable proof and permanent deny before return.
pub(crate) async fn apply_locked(
    c: &mut PgConnection,
    command: &DeletionCommand,
) -> Result<PendingDeletionReceipt> {
    if let Some(receipt) = load_locked(c, command).await? {
        return Ok(receipt);
    }
    let inserted = sqlx::query_scalar::<_, Uuid>(
        "insert into notification_user_deletion_receipts(saga_id,user_id,actor_id,retention_until,applied_at) values($1,$2,$3,$4,now()) on conflict(saga_id) do nothing returning saga_id",
    ).bind(command.saga_id).bind(command.user_id).bind(command.actor_id)
     .bind(command.retention_until).fetch_optional(&mut *c).await?;
    if inserted.is_none() {
        // A different-subject concurrent command can win the saga key while we
        // wait. A proved exact retry is fine; historical mutable rows are not.
        return load_locked(c, command).await?.ok_or_else(unavailable);
    }
    let audit_exists: bool =
        sqlx::query_scalar("select exists(select 1 from notification_audit_events where id=$1)")
            .bind(command.saga_id)
            .fetch_one(&mut *c)
            .await?;
    if audit_exists {
        return Err(unavailable());
    }

    // Retained behavior from PgNotificationRepository::apply_user_deletion at
    // 10bde5f3. No payload scrub or blanket purge is inferred from this deadline.
    sqlx::query("update notification_subject_states set suppress_as_recipient=true,suppress_as_actor=true,reason='account_deletion',suppressed_at=least(coalesce(suppressed_at,now()),now()),retention_until=$2 where subject_id=$1")
        .bind(command.user_id).bind(command.retention_until).execute(&mut *c).await?;
    sqlx::query("insert into notification_recipient_suppressions(user_id,reason,suppressed_at,retention_until) values($1,'account_deletion',now(),$2) on conflict(user_id) do update set reason=excluded.reason,suppressed_at=least(notification_recipient_suppressions.suppressed_at,excluded.suppressed_at),retention_until=excluded.retention_until")
        .bind(command.user_id).bind(command.retention_until).execute(&mut *c).await?;
    sqlx::query("update notifications set deleted_at=coalesce(deleted_at,now()),deleted_by=coalesce(deleted_by,$2),delete_reason=coalesce(delete_reason,'account_deletion'),retention_until=$3 where user_id=$1 or actor_id=$1")
        .bind(command.user_id).bind(command.actor_id).bind(command.retention_until).execute(&mut *c).await?;
    let existing_fence: Option<i64> = sqlx::query_scalar(
        "select fence_revision from notification_deletion_fences_v1 where subject_id=$1",
    )
    .bind(command.user_id)
    .fetch_optional(&mut *c)
    .await?;
    let revision = match existing_fence {
        Some(revision) => revision,
        None => sqlx::query_scalar("insert into notification_deletion_fences_v1(subject_id) values($1) returning fence_revision")
            .bind(command.user_id).fetch_one(&mut *c).await?,
    };
    let metadata = json!({
        "sagaId": command.saga_id,
        "retentionUntil": command.retention_until,
        "retentionSeconds": command.retention_until.timestamp(),
        "retentionNanos": command.retention_until.timestamp_subsec_nanos(),
        "fenceRevision": revision,
        "proofVersion": 1,
    });
    sqlx::query("insert into notification_audit_events(id,actor_id,notification_id,user_id,action,outcome,request_id,ip_address,user_agent,metadata_json,occurred_at) values($1,$2,null,$3,'USER_DELETION_APPLIED','SUCCESS',null,null,null,$4,now())")
        .bind(command.saga_id).bind(command.actor_id).bind(command.user_id).bind(metadata.to_string())
        .execute(&mut *c).await?;
    sqlx::query("insert into notification_deletion_receipts_v1(saga_id,user_id,actor_id,retention_until,retention_seconds,retention_nanos,fence_revision,audit_event_id) values($1,$2,$3,$4,$5,$6,$7,$1)")
        .bind(command.saga_id).bind(command.user_id).bind(command.actor_id).bind(command.retention_until)
        .bind(command.retention_until.timestamp()).bind(command.retention_until.timestamp_subsec_nanos() as i32)
        .bind(revision).execute(&mut *c).await?;
    load_locked(c, command).await?.ok_or_else(unavailable)
}

async fn load_locked(
    c: &mut PgConnection,
    command: &DeletionCommand,
) -> Result<Option<PendingDeletionReceipt>> {
    let row = sqlx::query("select *,effect_xid::text as native_xid from notification_deletion_receipts_v1 where saga_id=$1")
        .bind(command.saga_id).fetch_optional(&mut *c).await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let retained = DeletionCommand::new(
        row.try_get("saga_id")?,
        row.try_get("user_id")?,
        row.try_get("actor_id")?,
        DateTime::from_timestamp(
            row.try_get("retention_seconds")?,
            row.try_get::<i32, _>("retention_nanos")? as u32,
        )
        .ok_or_else(unavailable)?,
    )?;
    if retained != *command {
        return Err(rebound());
    }
    let revision: i64 = row.try_get("fence_revision")?;
    if revision <= 0 {
        return Err(unavailable());
    }
    let valid: bool = sqlx::query_scalar(
        "select exists(select 1 from notification_deletion_receipts_v1 r join notification_deletion_fences_v1 f on f.subject_id=r.user_id and f.fence_revision=r.fence_revision join notification_subject_states s on s.subject_id=r.user_id join notification_user_deletion_receipts l on l.saga_id=r.saga_id and l.user_id=r.user_id and l.actor_id=r.actor_id and l.retention_until=r.retention_until join notification_audit_events a on a.id=r.audit_event_id and a.user_id=r.user_id and a.actor_id=r.actor_id where r.saga_id=$1 and s.suppress_as_recipient and s.suppress_as_actor and a.action='USER_DELETION_APPLIED' and a.outcome='SUCCESS' and a.notification_id is null and (a.metadata_json::jsonb - 'retentionUntil')=jsonb_build_object('sagaId',r.saga_id,'retentionSeconds',r.retention_seconds,'retentionNanos',r.retention_nanos,'fenceRevision',r.fence_revision,'proofVersion',1) and not exists(select 1 from notifications n where (n.user_id=r.user_id or n.actor_id=r.user_id) and n.deleted_at is null))",
    ).bind(command.saga_id).fetch_one(&mut *c).await?;
    if !valid {
        return Err(unavailable());
    }
    Ok(Some(PendingDeletionReceipt(CommittedDeletionReceipt {
        command: retained,
        fence_revision: revision,
        effect_xid: row.try_get("native_xid")?,
        recorded_at: row.try_get("recorded_at")?,
    })))
}
