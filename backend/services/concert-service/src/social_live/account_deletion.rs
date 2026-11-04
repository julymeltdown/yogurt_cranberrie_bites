//! Sharing leaf of canonical Auth deletion. No transport/default authority.
//! Common identity/fence belongs exclusively to world::account_deletion (0052).
use super::invalidation::SocialInvalidationSink;
use crate::world::account_deletion::{
    bound_transaction_in, check_deadline, establish_fence_in, verify_source,
    DeletionAcknowledgement, DeletionAuthority, DeletionFailure, VerifiedDeletionSource,
    WorldDeletionCommand, WorldDeletionTarget,
};
use chrono::Utc;
use sqlx::{PgPool, Postgres, Row, Transaction};
use std::sync::Arc;
use tokio::time::Instant;
use uuid::Uuid;

pub const REQUEST_PAGE: i64 = 32;
const PUBLICATION_PAGE: i64 = 32;

/// The caller MUST inject the real publisher and canonical Auth-source verifier.
/// A retryable publisher cannot be replaced by an empty local registry here.
pub struct SharingDeletion {
    pool: PgPool,
    authority: Arc<dyn DeletionAuthority>,
    invalidation: SocialInvalidationSink,
}
impl SharingDeletion {
    pub fn new(
        pool: PgPool,
        authority: Arc<dyn DeletionAuthority>,
        invalidation: SocialInvalidationSink,
    ) -> Self {
        Self {
            pool,
            authority,
            invalidation,
        }
    }
    /// A bounded round over ALL events. Incomplete is durable progress, never ACK.
    /// Retries use the same exact saga/user/target/actor/nanosecond retention key.
    pub async fn apply(
        &self,
        command: WorldDeletionCommand,
        original_deadline: Instant,
    ) -> Result<DeletionAcknowledgement, DeletionFailure> {
        command.validate()?;
        if command.target != WorldDeletionTarget::Sharing {
            return Err(DeletionFailure::InvalidCommand);
        }
        check_deadline(original_deadline)?;
        tokio::time::timeout_at(original_deadline, self.round(command, original_deadline))
            .await
            .map_err(|_| DeletionFailure::Unavailable)?
    }
    async fn round(
        &self,
        command: WorldDeletionCommand,
        deadline: Instant,
    ) -> Result<DeletionAcknowledgement, DeletionFailure> {
        let source = verify_source(self.authority.as_ref(), &command, deadline).await?;
        let mut tx = self.pool.begin().await.map_err(db)?;
        // This common lock precedes receipt, event, generation and request locks.
        let fence = establish_fence_in(&mut tx, &command, &source).await?;
        sqlx::query("insert into public.concert_social_account_deletions(saga_id,user_id,actor_id,retention_seconds,retention_nanos,identity_binding,command_binding,source_binding,fence_revision) values($1,$2,$3,$4,$5,$6,$7,$8,$9) on conflict do nothing")
            .bind(command.saga_id).bind(command.user_id).bind(command.actor_id)
            .bind(command.retention_until.timestamp()).bind(command.retention_until.timestamp_subsec_nanos() as i32)
            .bind(command.identity_binding()?.as_slice()).bind(command.binding()?.as_slice())
            .bind(source.source_binding().as_slice()).bind(fence.revision as i64)
            .execute(&mut *tx).await.map_err(db)?;
        let row = exact_receipt(&mut tx, &command, &source).await?;
        if row
            .get::<Option<chrono::DateTime<Utc>>, _>("completed_at")
            .is_some()
        {
            self.recheck(&command, &source, deadline).await?;
            tx.commit().await.map_err(db)?;
            return Ok(DeletionAcknowledgement {
                command,
                fence_revision: fence.revision,
            });
        }
        deletion_context(&mut tx, &command, &source).await?;
        let active: Option<Uuid> = row.get("active_event");
        let last: Option<Uuid> = row.get("last_event");
        let after: Option<Uuid> = row.get("last_request");
        let event = match active {
            Some(event) => Some(event),
            None => next_event(&mut tx, command.user_id, last).await?,
        };
        if let Some(event) = event {
            sqlx::query("select event_id from public.concert_events where event_id=$1 for update")
                .bind(event)
                .fetch_optional(&mut *tx)
                .await
                .map_err(db)?
                .ok_or(DeletionFailure::Incomplete)?;
            audience(&mut tx, &command, &source, event, command.user_id).await?;
            // Keep handle revision/tombstone and all immutable consent bindings.
            sqlx::query("update public.concert_social_handles set key_id=null,nonce=null,ciphertext=null,revision=revision+1 where event_id=$1 and user_id=$2 and ciphertext is not null")
                .bind(event).bind(command.user_id).execute(&mut *tx).await.map_err(db)?;
            let requests = sqlx::query("select request_id,requester_id,recipient_id from public.concert_social_requests where event_id=$1 and (requester_id=$2 or recipient_id=$2) and ($3::uuid is null or request_id>$3) order by request_id limit $4 for update")
                .bind(event).bind(command.user_id).bind(after).bind(REQUEST_PAGE)
                .fetch_all(&mut *tx).await.map_err(db)?;
            let mut cursor = after;
            for request in &requests {
                source.check(&command, deadline)?;
                let id: Uuid = request.get("request_id");
                for actor in [
                    request.get::<Uuid, _>("requester_id"),
                    request.get::<Uuid, _>("recipient_id"),
                ] {
                    audience(&mut tx, &command, &source, event, actor).await?;
                }
                sqlx::query("update public.concert_social_grants set state='REVOKED',revision=revision+1 where event_id=$1 and request_id=$2 and state='ACTIVE'")
                    .bind(event).bind(id).execute(&mut *tx).await.map_err(db)?;
                sqlx::query("update public.concert_social_requests set state='REVOKED',revision=revision+1 where event_id=$1 and request_id=$2 and state in('PENDING','APPROVED')")
                    .bind(event).bind(id).execute(&mut *tx).await.map_err(db)?;
                cursor = Some(id);
            }
            let end = requests.len() < REQUEST_PAGE as usize;
            sqlx::query("update public.concert_social_account_deletions set last_event=$2,active_event=$3,last_request=$4,progress_revision=progress_revision+1 where saga_id=$1")
                .bind(command.saga_id).bind(if end {Some(event)} else {last})
                .bind(if end {None} else {Some(event)}).bind(if end {None} else {cursor})
                .execute(&mut *tx).await.map_err(db)?;
        }
        source.check(&command, deadline)?;
        // Forces deferred removal-context checks before the same source check.
        sqlx::query("set constraints all immediate")
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        self.recheck(&command, &source, deadline).await?;
        tx.commit().await.map_err(db)?;

        self.invalidation
            .drain_account_deletion(command.saga_id, command.user_id, PUBLICATION_PAGE)
            .await
            .map_err(|_| DeletionFailure::Incomplete)?;
        source.check(&command, deadline)?;
        let mut tx = self.pool.begin().await.map_err(db)?;
        let fence = establish_fence_in(&mut tx, &command, &source).await?;
        let row = exact_receipt(&mut tx, &command, &source).await?;
        let active: Option<Uuid> = row.get("active_event");
        let last: Option<Uuid> = row.get("last_event");
        if active.is_some() || next_event(&mut tx, command.user_id, last).await?.is_some() {
            self.recheck(&command, &source, deadline).await?;
            tx.commit().await.map_err(db)?;
            return Err(DeletionFailure::Incomplete);
        }
        let pending: bool = sqlx::query_scalar("select exists(select 1 from public.concert_social_deletion_audience where saga_id=$1 and published_at is null)")
            .bind(command.saga_id).fetch_one(&mut *tx).await.map_err(db)?;
        if pending {
            self.recheck(&command, &source, deadline).await?;
            tx.commit().await.map_err(db)?;
            return Err(DeletionFailure::Incomplete);
        }
        if row
            .get::<Option<chrono::DateTime<Utc>>, _>("completed_at")
            .is_none()
        {
            deletion_context(&mut tx, &command, &source).await?;
            sqlx::query("update public.concert_social_account_deletions set completed_at=clock_timestamp(),progress_revision=progress_revision+1 where saga_id=$1")
                .bind(command.saga_id).execute(&mut *tx).await.map_err(db)?;
        }
        self.recheck(&command, &source, deadline).await?;
        tx.commit().await.map_err(db)?;
        Ok(DeletionAcknowledgement {
            command,
            fence_revision: fence.revision,
        })
    }

    async fn recheck(
        &self,
        command: &WorldDeletionCommand,
        source: &VerifiedDeletionSource,
        deadline: Instant,
    ) -> Result<(), DeletionFailure> {
        source.check(command, deadline)?;
        let current = verify_source(self.authority.as_ref(), command, deadline).await?;
        if current.source_binding() != source.source_binding() {
            return Err(DeletionFailure::Unavailable);
        }
        source.check(command, deadline)
    }
}

async fn exact_receipt(
    tx: &mut Transaction<'_, Postgres>,
    c: &WorldDeletionCommand,
    source: &VerifiedDeletionSource,
) -> Result<sqlx::postgres::PgRow, DeletionFailure> {
    source.check(c, source.original_deadline())?;
    let row = sqlx::query(
        "select * from public.concert_social_account_deletions where saga_id=$1 for update",
    )
    .bind(c.saga_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db)?
    .ok_or(DeletionFailure::InvalidCommand)?;
    if row.get::<Uuid, _>("user_id") != c.user_id
        || row.get::<Uuid, _>("actor_id") != c.actor_id
        || row.get::<String, _>("target") != "SHARING"
        || row.get::<i64, _>("retention_seconds") != c.retention_until.timestamp()
        || row.get::<i32, _>("retention_nanos") != c.retention_until.timestamp_subsec_nanos() as i32
        || row.get::<Vec<u8>, _>("identity_binding") != c.identity_binding()?
        || row.get::<Vec<u8>, _>("command_binding") != c.binding()?
        || row.get::<Vec<u8>, _>("source_binding") != source.source_binding().as_slice()
        || row.get::<i64, _>("fence_revision") != 1
    {
        return Err(DeletionFailure::InvalidCommand);
    }
    Ok(row)
}
async fn deletion_context(
    tx: &mut Transaction<'_, Postgres>,
    command: &WorldDeletionCommand,
    source: &VerifiedDeletionSource,
) -> Result<(), DeletionFailure> {
    source.check(command, source.original_deadline())?;
    bound_transaction_in(tx, source.original_deadline()).await?;
    // Translate only the ORIGINAL monotonic remainder to a DB-time cutoff.
    // It is never installed as an ordinary Participant/relationship authority.
    let sent = Instant::now();
    let server_now: chrono::DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
        .fetch_one(&mut **tx)
        .await
        .map_err(db)?;
    let received = Instant::now();
    let remaining = source
        .original_deadline()
        .saturating_duration_since(received)
        .checked_sub(received.saturating_duration_since(sent))
        .ok_or(DeletionFailure::Unavailable)?;
    let until = server_now + chrono::Duration::microseconds(remaining.as_micros() as i64);
    let source_hex: String = source
        .source_binding()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    sqlx::query("select set_config('mcr.social_deletion_saga',$1,true),set_config('mcr.social_deletion_source',$2,true),set_config('mcr.social_deletion_until',$3,true)")
        .bind(command.saga_id.to_string()).bind(source_hex).bind(until.to_rfc3339())
        .execute(&mut **tx).await.map_err(db)?;
    Ok(())
}
async fn next_event(
    tx: &mut Transaction<'_, Postgres>,
    user: Uuid,
    after: Option<Uuid>,
) -> Result<Option<Uuid>, DeletionFailure> {
    sqlx::query_scalar("select event_id from (select event_id from public.concert_social_handles where user_id=$1 union select event_id from public.concert_social_requests where requester_id=$1 union select event_id from public.concert_social_requests where recipient_id=$1) events where ($2::uuid is null or event_id>$2) order by event_id limit 1")
        .bind(user).bind(after).fetch_optional(&mut **tx).await.map_err(db)
}
async fn audience(
    tx: &mut Transaction<'_, Postgres>,
    c: &WorldDeletionCommand,
    source: &VerifiedDeletionSource,
    event: Uuid,
    actor: Uuid,
) -> Result<(), DeletionFailure> {
    let exists: bool = sqlx::query_scalar("select exists(select 1 from public.concert_social_deletion_audience where saga_id=$1 and event_id=$2 and actor_id=$3)")
        .bind(c.saga_id).bind(event).bind(actor).fetch_one(&mut **tx).await.map_err(db)?;
    if exists {
        return Ok(());
    }
    let revision: i64 = sqlx::query_scalar("insert into public.concert_social_policy_generations as g(event_id,actor_id,revision) values($1,$2,2) on conflict(event_id,actor_id) do update set revision=g.revision+1 returning revision")
        .bind(event).bind(actor).fetch_one(&mut **tx).await.map_err(db)?;
    sqlx::query("insert into public.concert_social_deletion_audience(saga_id,event_id,actor_id,policy_revision) values($1,$2,$3,$4)")
        .bind(c.saga_id).bind(event).bind(actor).bind(revision).execute(&mut **tx).await.map_err(db)?;
    sqlx::query("insert into public.concert_social_policy_signals(signal_id,event_id,actor_id,policy_revision,reason,deletion_saga,deletion_user,deletion_command_binding,deletion_source_binding,deletion_fence_revision) values($1,$2,$3,$4,'CANONICAL_POLICY_CHANGED',$5,$6,$7,$8,1)")
        .bind(Uuid::new_v4()).bind(event).bind(actor).bind(revision).bind(c.saga_id).bind(c.user_id)
        .bind(c.binding()?.as_slice()).bind(source.source_binding().as_slice())
        .execute(&mut **tx).await.map_err(db)?;
    Ok(())
}
fn db(_: sqlx::Error) -> DeletionFailure {
    DeletionFailure::Unavailable
}
