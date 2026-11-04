//! BE42-WORLD-TICKETS-01. World-owned ticket cleanup, not an Auth transport.
//!
//! The command mirrors auth-application::concert_deletion at 89f26e6e. A
//! configured authority must authenticate its canonical Auth deletion source;
//! raw command identity is never sufficient. No positive default is provided.
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Row, Transaction};
use std::sync::Arc;
use tokio::time::Instant;
use uuid::Uuid;

pub const MAX_DISPATCH: std::time::Duration = std::time::Duration::from_secs(5);
pub const PAGE_SIZE: i64 = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorldDeletionTarget {
    Tickets,
    Sessions,
    Sharing,
    PrivateData,
}
#[derive(Clone, PartialEq, Eq)]
pub struct WorldDeletionCommand {
    pub saga_id: Uuid,
    pub user_id: Uuid,
    pub target: WorldDeletionTarget,
    pub actor_id: Uuid,
    pub retention_until: DateTime<Utc>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeletionFailure {
    InvalidCommand,
    Unavailable,
    Incomplete,
}
impl WorldDeletionCommand {
    pub fn validate(&self) -> Result<(), DeletionFailure> {
        if self.saga_id.is_nil() || self.user_id.is_nil() || self.actor_id.is_nil() {
            return Err(DeletionFailure::InvalidCommand);
        }
        Ok(())
    }
    pub fn identity_binding(&self) -> Result<[u8; 32], DeletionFailure> {
        self.validate()?;
        let mut h = Sha256::new();
        h.update(b"MCR-AUTH-CONCERT-DELETION-1\0");
        for value in [self.saga_id, self.user_id, self.actor_id] {
            h.update(value.as_bytes());
        }
        h.update(self.retention_until.timestamp().to_be_bytes());
        h.update(self.retention_until.timestamp_subsec_nanos().to_be_bytes());
        Ok(h.finalize().into())
    }
    pub fn binding(&self) -> Result<[u8; 32], DeletionFailure> {
        let mut h = Sha256::new();
        h.update(self.identity_binding()?);
        h.update([match self.target {
            WorldDeletionTarget::Tickets => 1,
            WorldDeletionTarget::Sessions => 2,
            WorldDeletionTarget::Sharing => 3,
            WorldDeletionTarget::PrivateData => 4,
        }]);
        Ok(h.finalize().into())
    }
}
impl std::fmt::Debug for WorldDeletionCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("WorldDeletionCommand([REDACTED])")
    }
}

/// Issued only by the configured trusted adapter after checking actual Auth
/// repository/transport provenance. This constructor does not authenticate raw
/// requests; no HTTP/RPC entry point or default authority is mounted here.
#[derive(Clone, PartialEq, Eq)]
pub struct VerifiedDeletionSource {
    command_binding: [u8; 32],
    source_binding: [u8; 32],
    original_deadline: Instant,
}
impl VerifiedDeletionSource {
    pub fn from_verified_auth_source(
        command: &WorldDeletionCommand,
        original_deadline: Instant,
        source_binding: [u8; 32],
    ) -> Result<Self, DeletionFailure> {
        if source_binding == [0; 32] {
            return Err(DeletionFailure::Unavailable);
        }
        check_deadline(original_deadline)?;
        Ok(Self {
            command_binding: command.binding()?,
            source_binding,
            original_deadline,
        })
    }
}
impl std::fmt::Debug for VerifiedDeletionSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("VerifiedDeletionSource([REDACTED])")
    }
}
#[async_trait]
pub trait DeletionAuthority: Send + Sync {
    /// Authenticate exact stable Auth saga/subject/target/actor/retention and the
    /// ORIGINAL monotonic dispatch deadline. Never renew it after an await.
    async fn verify(
        &self,
        command: &WorldDeletionCommand,
        original_deadline: Instant,
    ) -> Result<VerifiedDeletionSource, DeletionFailure>;
}
#[derive(Clone, PartialEq, Eq)]
pub struct DeletionAcknowledgement {
    pub command: WorldDeletionCommand,
    pub fence_revision: u64,
}
impl std::fmt::Debug for DeletionAcknowledgement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DeletionAcknowledgement([REDACTED])")
    }
}
pub fn check_deadline(deadline: Instant) -> Result<(), DeletionFailure> {
    let now = Instant::now();
    if deadline <= now || deadline.duration_since(now) > MAX_DISPATCH {
        return Err(DeletionFailure::Unavailable);
    }
    Ok(())
}

pub struct TicketDeletion {
    pool: PgPool,
    authority: Arc<dyn DeletionAuthority>,
}
impl TicketDeletion {
    pub fn new(pool: PgPool, authority: Arc<dyn DeletionAuthority>) -> Self {
        Self { pool, authority }
    }
}

/// This lock exists even before any fence row does. Issue, consume and deletion
/// must take it FIRST; it covers one Auth user across every event. Hash collisions
/// only serialize unrelated users and cannot allow a missed deletion fence.
pub async fn lock_user(tx: &mut Transaction<'_, Postgres>, user: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("select pg_advisory_xact_lock(hashtextextended('mcr-world-account-deletion:' || $1::uuid::text,0))")
        .bind(user).execute(&mut **tx).await?;
    Ok(())
}

/// One immutable identity shared by Tickets, Sessions, Sharing and PrivateData.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct FenceReceipt {
    pub user_id: Uuid,
    pub saga_id: Uuid,
    pub revision: u64,
}
impl std::fmt::Debug for FenceReceipt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FenceReceipt([REDACTED])")
    }
}
impl VerifiedDeletionSource {
    pub fn check(
        &self,
        command: &WorldDeletionCommand,
        deadline: Instant,
    ) -> Result<(), DeletionFailure> {
        check_deadline(deadline)?;
        if self.original_deadline != deadline
            || self.command_binding != command.binding()?
            || self.source_binding == [0; 32]
        {
            return Err(DeletionFailure::InvalidCommand);
        }
        Ok(())
    }
    pub fn source_binding(&self) -> &[u8; 32] {
        &self.source_binding
    }
    pub fn original_deadline(&self) -> Instant {
        self.original_deadline
    }
}
/// Shared adapter boundary. The configured authority owns verification of actual
/// Auth repository provenance; this module has no synthetic positive authority.
pub async fn verify_source(
    authority: &dyn DeletionAuthority,
    command: &WorldDeletionCommand,
    original_deadline: Instant,
) -> Result<VerifiedDeletionSource, DeletionFailure> {
    command.validate()?;
    check_deadline(original_deadline)?;
    let verified = tokio::time::timeout_at(
        original_deadline,
        authority.verify(command, original_deadline),
    )
    .await
    .map_err(|_| DeletionFailure::Unavailable)??;
    verified.check(command, original_deadline)?;
    Ok(verified)
}
/// Call after lock_user when checking a normal producer path. Absence permits
/// ordinary work only; it never certifies Auth deletion provenance.
pub async fn is_fenced_in(
    tx: &mut Transaction<'_, Postgres>,
    user: Uuid,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("select exists(select 1 from public.concert_world_account_deletion_fences where user_id=$1)")
        .bind(user).fetch_one(&mut **tx).await
}
fn db_error(_: sqlx::Error) -> DeletionFailure {
    DeletionFailure::Unavailable
}
/// Sets bounded transaction-local PostgreSQL waits from the ORIGINAL deadline.
/// Callers must ALSO bound the entire attempt with timeout_at(original_deadline).
pub async fn bound_transaction_in(
    tx: &mut Transaction<'_, Postgres>,
    deadline: Instant,
) -> Result<(), DeletionFailure> {
    check_deadline(deadline)?;
    let millis = deadline
        .saturating_duration_since(Instant::now())
        .as_millis()
        .max(1)
        .min(5000);
    let value = format!("{millis}ms");
    sqlx::query(
        "select set_config('statement_timeout',$1,true),set_config('lock_timeout',$1,true)",
    )
    .bind(value)
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;
    Ok(())
}
/// Acquire the shared user advisory lock before ANY event, row or digest lock.
/// The fence and the leaf effects/receipt must commit in this SAME transaction.
/// This function never acknowledges a target: each leaf owns completion.
pub async fn establish_fence_in(
    tx: &mut Transaction<'_, Postgres>,
    command: &WorldDeletionCommand,
    verified: &VerifiedDeletionSource,
) -> Result<FenceReceipt, DeletionFailure> {
    verified.check(command, verified.original_deadline)?;
    bound_transaction_in(tx, verified.original_deadline).await?;
    lock_user(tx, command.user_id).await.map_err(db_error)?;
    sqlx::query("insert into public.concert_world_account_deletion_fences(user_id,saga_id,identity_binding,fence_revision) values($1,$2,$3,1) on conflict do nothing")
        .bind(command.user_id).bind(command.saga_id).bind(command.identity_binding()?.as_slice())
        .execute(&mut **tx).await.map_err(db_error)?;
    let row = sqlx::query("select saga_id,identity_binding,fence_revision from public.concert_world_account_deletion_fences where user_id=$1")
        .bind(command.user_id).fetch_optional(&mut **tx).await.map_err(db_error)?
        .ok_or(DeletionFailure::InvalidCommand)?;
    if row.try_get::<Uuid, _>("saga_id").map_err(db_error)? != command.saga_id
        || row
            .try_get::<Vec<u8>, _>("identity_binding")
            .map_err(db_error)?
            != command.identity_binding()?
    {
        return Err(DeletionFailure::InvalidCommand);
    }
    let revision = row.try_get::<i64, _>("fence_revision").map_err(db_error)?;
    if revision <= 0 {
        return Err(DeletionFailure::Unavailable);
    }
    verified.check(command, verified.original_deadline)?;
    Ok(FenceReceipt {
        user_id: command.user_id,
        saga_id: command.saga_id,
        revision: revision as u64,
    })
}
impl TicketDeletion {
    pub async fn apply(
        &self,
        command: WorldDeletionCommand,
        original_deadline: Instant,
    ) -> Result<DeletionAcknowledgement, DeletionFailure> {
        command.validate()?;
        if command.target != WorldDeletionTarget::Tickets {
            return Err(DeletionFailure::InvalidCommand);
        }
        check_deadline(original_deadline)?;
        tokio::time::timeout_at(
            original_deadline,
            self.apply_in_deadline(command, original_deadline),
        )
        .await
        .map_err(|_| DeletionFailure::Unavailable)?
    }
    async fn apply_in_deadline(
        &self,
        command: WorldDeletionCommand,
        deadline: Instant,
    ) -> Result<DeletionAcknowledgement, DeletionFailure> {
        let verified = verify_source(self.authority.as_ref(), &command, deadline).await?;
        let mut tx = self.pool.begin().await.map_err(db_error)?;
        let fence = establish_fence_in(&mut tx, &command, &verified).await?;
        sqlx::query("insert into public.concert_world_ticket_deletion_progress(user_id,saga_id,command_binding,source_binding) values($1,$2,$3,$4) on conflict do nothing")
            .bind(command.user_id).bind(command.saga_id).bind(command.binding()?.as_slice())
            .bind(verified.source_binding().as_slice()).execute(&mut *tx).await.map_err(db_error)?;
        let row = sqlx::query("select saga_id,command_binding,source_binding,completed_at from public.concert_world_ticket_deletion_progress where user_id=$1 for update")
            .bind(command.user_id).fetch_one(&mut *tx).await.map_err(db_error)?;
        if row.try_get::<Uuid, _>("saga_id").map_err(db_error)? != command.saga_id
            || row
                .try_get::<Vec<u8>, _>("command_binding")
                .map_err(db_error)?
                != command.binding()?
            || row
                .try_get::<Vec<u8>, _>("source_binding")
                .map_err(db_error)?
                != *verified.source_binding()
        {
            return Err(DeletionFailure::InvalidCommand);
        }
        let mut complete = row
            .try_get::<Option<DateTime<Utc>>, _>("completed_at")
            .map_err(db_error)?
            .is_some();
        if !complete {
            bound_transaction_in(&mut tx, deadline).await?;
            let progress = sqlx::query("with batch as (select ticket_digest from public.concert_world_tickets where user_id=$1 and consumed_at is null order by ticket_digest limit $2 for update), removed as (delete from public.concert_world_tickets t using batch b where t.ticket_digest=b.ticket_digest returning t.ticket_digest) select count(*)::bigint as removed,(array_agg(ticket_digest order by ticket_digest desc))[1] as cursor from removed")
                .bind(command.user_id).bind(PAGE_SIZE).fetch_one(&mut *tx).await.map_err(db_error)?;
            let removed: i64 = progress.try_get("removed").map_err(db_error)?;
            let cursor: Option<Vec<u8>> = progress.try_get("cursor").map_err(db_error)?;
            let pending: bool = sqlx::query_scalar("select exists(select 1 from public.concert_world_tickets where user_id=$1 and consumed_at is null)")
                .bind(command.user_id).fetch_one(&mut *tx).await.map_err(db_error)?;
            complete = !pending;
            sqlx::query("update public.concert_world_ticket_deletion_progress set revoked_count=revoked_count+$2,cursor_digest=greatest(cursor_digest,$3),progress_revision=progress_revision+1,completed_at=case when $4 then clock_timestamp() else null end where user_id=$1")
                .bind(command.user_id).bind(removed).bind(cursor).bind(complete).execute(&mut *tx).await.map_err(db_error)?;
        }
        // A stale/rebound authority discovered before commit rolls effects back.
        let again = verify_source(self.authority.as_ref(), &command, deadline).await?;
        if again != verified {
            return Err(DeletionFailure::Unavailable);
        }
        check_deadline(deadline)?;
        tx.commit().await.map_err(db_error)?;
        check_deadline(deadline)?;
        if complete {
            Ok(DeletionAcknowledgement {
                command,
                fence_revision: fence.revision,
            })
        } else {
            Err(DeletionFailure::Incomplete)
        }
    }
}
