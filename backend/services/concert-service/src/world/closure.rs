//! Transactional World component of the existing closure START operation.
//! Event-exclusive START drains qualified publication leases: they hold event
//! SHARE and disable native handoff under its mutex BEFORE their SQL rollback.
use concert_application::{commands::CommandError, world_ticket::WorldError};
use sqlx::{postgres::PgTransactionManager, PgConnection, Row, TransactionManager};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WorldReadOnlyReceipt {
    pub(crate) event: Uuid,
    pub(crate) job: Uuid,
    pub(crate) operation: Uuid,
    pub(crate) receipt_id: Uuid,
    pub(crate) revision: i64,
}
/// Every World authority-creating transaction takes this lock FIRST, before
/// ticket/advisory/room/member/owner locks. Caller retains it through commit.
pub(crate) async fn lock_event_open_in(
    c: &mut PgConnection,
    event: Uuid,
) -> Result<(), WorldError> {
    if event.is_nil() || PgTransactionManager::get_transaction_depth(c) == 0 {
        return Err(WorldError::Invalid);
    }
    let phase: Option<String> =
        sqlx::query_scalar("select phase from public.concert_events where event_id=$1 for share")
            .bind(event)
            .fetch_optional(c)
            .await
            .map_err(|_| WorldError::Unavailable)?;
    if !phase.is_some_and(|p| matches!(p.as_str(), "OPEN" | "LIVE")) {
        return Err(WorldError::Denied);
    }
    Ok(())
}
/// Never begins or commits a transaction and refuses an autocommit connection.
/// Caller is the retained lifecycle START transaction, already event-exclusive.
/// Replays return the exact original component receipt, without repeating effects.
pub(crate) async fn fence_event_read_only_in(
    c: &mut PgConnection,
    event: Uuid,
    job: Uuid,
    operation: Uuid,
) -> Result<WorldReadOnlyReceipt, CommandError> {
    use crate::event_lifecycle::EffectKey;
    use concert_domain::event_lifecycle::Step;
    if [event, job, operation].iter().any(Uuid::is_nil)
        || PgTransactionManager::get_transaction_depth(c) == 0
        || operation
            != (EffectKey {
                event,
                job,
                step: Step::ReadOnly,
            })
            .operation_id()
    {
        return Err(CommandError::InvalidInput);
    }
    let phase: Option<String> =
        sqlx::query_scalar("select phase from public.concert_events where event_id=$1 for update")
            .bind(event)
            .fetch_optional(&mut *c)
            .await
            .map_err(|_| CommandError::Unavailable)?;
    if !phase.is_some_and(|p| p == "READ_ONLY") {
        return Err(CommandError::InvalidState);
    }
    let valid: bool = sqlx::query_scalar(
        "select exists(select 1 from public.concert_closure_jobs where event_id=$1 and job_id=$2)",
    )
    .bind(event)
    .bind(job)
    .fetch_one(&mut *c)
    .await
    .map_err(|_| CommandError::Unavailable)?;
    if !valid {
        return Err(CommandError::InvalidState);
    }
    if let Some(row)=sqlx::query("select operation_id,receipt_id,revision from public.concert_world_read_only_receipts where event_id=$1 and job_id=$2")
        .bind(event).bind(job).fetch_optional(&mut *c).await.map_err(|_|CommandError::Unavailable)? {
        if row.try_get::<Uuid,_>("operation_id").map_err(|_|CommandError::InvalidState)?!=operation {return Err(CommandError::Conflict)}
        return Ok(WorldReadOnlyReceipt{event,job,operation,receipt_id:row.try_get("receipt_id").map_err(|_|CommandError::InvalidState)?,revision:row.try_get("revision").map_err(|_|CommandError::InvalidState)?});
    }
    // Capture the original owner identities before legally retiring their rows.
    let owners=sqlx::query("select room_id,owner_process_id,owner_epoch from public.concert_world_owner_leases where event_id=$1 order by room_id for update")
        .bind(event).fetch_all(&mut *c).await.map_err(|_|CommandError::Unavailable)?;
    if owners.len() > 16 {
        return Err(CommandError::InvalidState);
    }
    let owners:Vec<_>=owners.into_iter().map(|r|serde_json::json!({"room":r.get::<Uuid,_>("room_id"),"owner":r.get::<String,_>("owner_process_id"),"epoch":r.get::<Uuid,_>("owner_epoch")})).collect();
    let rows=sqlx::query("select user_id,room_id,session_id,connection_id,owner_process_id,owner_epoch from public.concert_world_memberships where event_id=$1 and not disconnected and lease_until>clock_timestamp() and access_expires_at>clock_timestamp() order by user_id for update")
        .bind(event).fetch_all(&mut *c).await.map_err(|_|CommandError::Unavailable)?;
    if rows.len() > 800 {
        return Err(CommandError::InvalidState);
    }
    let victims:Vec<_>=rows.into_iter().map(|r|serde_json::json!({"user":r.get::<Uuid,_>("user_id"),"room":r.get::<Uuid,_>("room_id"),"family":r.get::<Uuid,_>("session_id"),"connection":r.get::<Uuid,_>("connection_id"),"owner":r.get::<String,_>("owner_process_id"),"epoch":r.get::<Uuid,_>("owner_epoch")})).collect();
    let tickets = sqlx::query("delete from public.concert_world_tickets where event_id=$1")
        .bind(event)
        .execute(&mut *c)
        .await
        .map_err(|_| CommandError::Unavailable)?
        .rows_affected();
    let memberships=sqlx::query("update public.concert_world_memberships set disconnected=true where event_id=$1 and not disconnected").bind(event).execute(&mut *c).await.map_err(|_|CommandError::Unavailable)?.rows_affected();
    let rooms=sqlx::query("update public.concert_world_rooms set availability='UNAVAILABLE' where event_id=$1 and availability<>'UNAVAILABLE'").bind(event).execute(&mut *c).await.map_err(|_|CommandError::Unavailable)?.rows_affected();
    // DELETE is legal; setting lease_until=now could violate lease_until>leased_at.
    let retired = sqlx::query("delete from public.concert_world_owner_leases where event_id=$1")
        .bind(event)
        .execute(&mut *c)
        .await
        .map_err(|_| CommandError::Unavailable)?
        .rows_affected();
    let receipt_id = Uuid::new_v4();
    sqlx::query("insert into public.concert_world_read_only_receipts(event_id,job_id,operation_id,receipt_id,revision,owners,victims,tickets_removed,memberships_revoked,rooms_disabled,owners_retired) values($1,$2,$3,$4,1,$5,$6,$7,$8,$9,$10)")
        .bind(event).bind(job).bind(operation).bind(receipt_id).bind(serde_json::json!(owners)).bind(serde_json::json!(victims)).bind(tickets as i64).bind(memberships as i64).bind(rooms as i64).bind(retired as i64).execute(c).await.map_err(|_|CommandError::Unavailable)?;
    Ok(WorldReadOnlyReceipt {
        event,
        job,
        operation,
        receipt_id,
        revision: 1,
    })
}
