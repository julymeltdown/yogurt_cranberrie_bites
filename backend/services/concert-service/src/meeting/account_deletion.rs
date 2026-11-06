//! Native Party/Meeting owner journals and account-scoped guidance withdrawal.
//! Source-only provider ABI dependency: acee2fac. No browser authority is minted.
use crate::{
    account_private_data::{
        PrivateDataCategory, PrivateDataCategoryOwner, PrivateDataCategoryReceipt, CATEGORY_VERSION,
    },
    world::account_deletion::{
        establish_fence_in, verify_source, DeletionAuthority, DeletionFailure,
        VerifiedDeletionSource, WorldDeletionCommand, WorldDeletionTarget,
    },
};
use async_trait::async_trait;
use sqlx::{
    postgres::PgTransactionManager, PgConnection, PgPool, Postgres, Row, Transaction,
    TransactionManager,
};
use std::sync::Arc;
use tokio::time::Instant;
use uuid::Uuid;

pub(crate) fn db(_: impl std::fmt::Debug) -> DeletionFailure {
    DeletionFailure::Unavailable
}

/// A producer takes every known affected user lock, sorted, before event/root
/// locks. A permanent fence is a refusal, never a new authority source.
pub(crate) async fn writer_users(
    c: &mut PgConnection,
    users: &[Uuid],
) -> Result<(), DeletionFailure> {
    if PgTransactionManager::get_transaction_depth(c) == 0 {
        return Err(DeletionFailure::InvalidCommand);
    }
    let mut users = users.to_vec();
    users.sort_unstable();
    users.dedup();
    for user in users {
        if user.is_nil() {
            return Err(DeletionFailure::InvalidCommand);
        }
        crate::world::account_deletion::lock_user_in(c, user)
            .await
            .map_err(db)?;
        let fenced:bool=sqlx::query_scalar("select exists(select 1 from public.concert_world_account_deletion_fences where user_id=$1)")
            .bind(user).fetch_one(&mut *c).await.map_err(db)?;
        if fenced {
            return Err(DeletionFailure::InvalidCommand);
        }
    }
    Ok(())
}

async fn recheck(
    authority: &dyn DeletionAuthority,
    command: &WorldDeletionCommand,
    source: &VerifiedDeletionSource,
    deadline: Instant,
) -> Result<(), DeletionFailure> {
    source.check(command, deadline)?;
    let current = verify_source(authority, command, deadline).await?;
    if current.source_binding() != source.source_binding() {
        return Err(DeletionFailure::InvalidCommand);
    }
    source.check(command, deadline)
}

pub(crate) async fn open_owner(
    pool: &PgPool,
    authority: &dyn DeletionAuthority,
    command: &WorldDeletionCommand,
    source: &VerifiedDeletionSource,
    deadline: Instant,
    kind: &str,
) -> Result<(Transaction<'static, Postgres>, bool), DeletionFailure> {
    if command.target != WorldDeletionTarget::PrivateData || !matches!(kind, "PARTY" | "MEETING") {
        return Err(DeletionFailure::InvalidCommand);
    }
    recheck(authority, command, source, deadline).await?;
    let mut tx = pool.begin().await.map_err(db)?;
    establish_fence_in(&mut tx, command, source).await?;
    // Anchor to a database timestamp sampled BEFORE computing the remaining
    // monotonic duration. Adding a duration computed before an SQL round trip
    // to clock_timestamp() inside that call would extend the original limit.
    let database_now: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("select clock_timestamp()")
            .fetch_one(&mut *tx)
            .await
            .map_err(db)?;
    let micros = deadline
        .saturating_duration_since(Instant::now())
        .as_micros();
    if micros == 0 {
        return Err(DeletionFailure::Unavailable);
    }
    let until = database_now
        .checked_add_signed(chrono::Duration::microseconds(
            i64::try_from(micros).map_err(db)?,
        ))
        .ok_or(DeletionFailure::Unavailable)?;
    sqlx::query("select set_config('mcr.meeting_deletion_deadline',$1,true)")
        .bind(until.to_rfc3339())
        .execute(&mut *tx)
        .await
        .map_err(db)?;
    sqlx::query("insert into public.concert_meeting_account_deletion(user_id,owner_kind,saga_id,actor_id,retention_until,command_binding,identity_binding,source_binding) values($1,$2,$3,$4,$5,$6,$7,$8) on conflict do nothing")
        .bind(command.user_id).bind(kind).bind(command.saga_id).bind(command.actor_id).bind(command.retention_until)
        .bind(command.binding()?.as_slice()).bind(command.identity_binding()?.as_slice()).bind(source.source_binding().as_slice())
        .execute(&mut *tx).await.map_err(db)?;
    let row=sqlx::query("select saga_id,actor_id,command_binding,identity_binding,source_binding,complete,owner_revision from public.concert_meeting_account_deletion where user_id=$1 and owner_kind=$2 for update")
        .bind(command.user_id).bind(kind).fetch_one(&mut *tx).await.map_err(db)?;
    if row.try_get::<Uuid, _>("saga_id").map_err(db)? != command.saga_id
        || row.try_get::<Uuid, _>("actor_id").map_err(db)? != command.actor_id
        || row.try_get::<Vec<u8>, _>("command_binding").map_err(db)? != command.binding()?
        || row.try_get::<Vec<u8>, _>("identity_binding").map_err(db)?
            != command.identity_binding()?
        || row.try_get::<Vec<u8>, _>("source_binding").map_err(db)? != *source.source_binding()
    {
        return Err(DeletionFailure::InvalidCommand);
    }
    let complete: bool = row.try_get("complete").map_err(db)?;
    let revision: i64 = row.try_get("owner_revision").map_err(db)?;
    if revision != if complete { 1 } else { 0 } {
        return Err(DeletionFailure::Unavailable);
    }
    Ok((tx, complete))
}

pub(crate) async fn finish_owner(
    mut tx: Transaction<'_, Postgres>,
    authority: &dyn DeletionAuthority,
    command: &WorldDeletionCommand,
    source: &VerifiedDeletionSource,
    deadline: Instant,
    kind: &str,
    was_complete: bool,
    complete: bool,
) -> Result<(), DeletionFailure> {
    if was_complete && !complete {
        return Err(DeletionFailure::Unavailable);
    }
    if !was_complete {
        sqlx::query("update public.concert_meeting_account_deletion set complete=$3,owner_revision=case when $3 then 1 else 0 end,native_xid=pg_current_xact_id() where user_id=$1 and owner_kind=$2")
            .bind(command.user_id).bind(kind).bind(complete).execute(&mut *tx).await.map_err(db)?;
    }
    recheck(authority, command, source, deadline).await?;
    tx.commit().await.map_err(db)?;
    source.check(command, deadline)
}

pub struct MeetingPrivateInputDeletion {
    pool: PgPool,
    authority: Arc<dyn DeletionAuthority>,
}
impl MeetingPrivateInputDeletion {
    pub fn new(pool: PgPool, authority: Arc<dyn DeletionAuthority>) -> Self {
        Self { pool, authority }
    }
}
#[async_trait]
impl PrivateDataCategoryOwner for MeetingPrivateInputDeletion {
    async fn apply_category(
        &self,
        command: &WorldDeletionCommand,
        source: &VerifiedDeletionSource,
        deadline: Instant,
    ) -> Result<PrivateDataCategoryReceipt, DeletionFailure> {
        tokio::time::timeout_at(deadline,async {
            let (mut tx,was_complete)=open_owner(&self.pool,self.authority.as_ref(),command,source,deadline,"MEETING").await?;
            if !was_complete {
                // Deleting a private beacon cascades only its photo association;
                // media bytes stay with the separate actual media owner category.
                sqlx::query("with page as (select beacon_id from public.concert_meeting_beacons where owner_id=$1 or (audience->>'kind'='EXPLICIT_USERS' and audience->'user_ids' @> to_jsonb(array[$1::uuid])) order by event_id,beacon_id limit 32 for update) delete from public.concert_meeting_beacons b using page where b.beacon_id=page.beacon_id")
                    .bind(command.user_id).execute(&mut *tx).await.map_err(db)?;
                sqlx::query("with page as (select event_id,party_id,user_id from public.concert_meeting_rally_states where user_id=$1 order by event_id,party_id limit 32 for update) delete from public.concert_meeting_rally_states r using page p where (r.event_id,r.party_id,r.user_id)=(p.event_id,p.party_id,p.user_id)")
                    .bind(command.user_id).execute(&mut *tx).await.map_err(db)?;
                sqlx::query("with page as (select beacon_id from public.concert_meeting_beacon_photos where owner_id=$1 order by beacon_id limit 32 for update) delete from public.concert_meeting_beacon_photos p using page where p.beacon_id=page.beacon_id")
                    .bind(command.user_id).execute(&mut *tx).await.map_err(db)?;
            }
            let remaining:bool=sqlx::query_scalar("select public.concert_meeting_has_private_input($1)")
                .bind(command.user_id).fetch_one(&mut *tx).await.map_err(db)?;
            let complete=!remaining;
            finish_owner(tx,self.authority.as_ref(),command,source,deadline,"MEETING",was_complete,complete).await?;
            Ok(PrivateDataCategoryReceipt {command:command.clone(),category:PrivateDataCategory::BeaconsAndRally,
                version:CATEGORY_VERSION,source_binding:*source.source_binding(),progress_revision:if complete{1}else{0},complete})
        }).await.map_err(|_|DeletionFailure::Unavailable)?
    }
}
