//! Actual Concert-owned beacon expiry and reviewed private retention. The caller
//! establishes current lifecycle/relationship authority. This leaf only performs
//! bounded native SQL and checks migration 0056's same-transaction witnesses.
use super::{db, finish, load, LocalOwner};
use async_trait::async_trait;
use concert_application::{commands::CommandError, event_cleanup::*};
use concert_domain::event_lifecycle::{Job, JobState, Retention, Step};
use sqlx::{postgres::PgTransactionManager, PgConnection, Row, TransactionManager};
use uuid::Uuid;

pub struct BeaconExpiryOwner;
pub struct PrivateRetentionOwner;

#[async_trait]
impl LocalOwner for BeaconExpiryOwner {
    fn owner(&self) -> Owner {
        Owner::Beacons
    }
    async fn apply_in(
        &self,
        c: &mut PgConnection,
        r: &BatchRequest,
    ) -> Result<Outcome, CommandError> {
        apply(c, r, Owner::Beacons, Step::ExpireBeacons).await
    }
    async fn verify_in(
        &self,
        c: &mut PgConnection,
        r: &BatchRequest,
        receipt: Receipt,
    ) -> Result<(), CommandError> {
        verify(c, r, receipt, Owner::Beacons, Step::ExpireBeacons).await
    }
}
#[async_trait]
impl LocalOwner for PrivateRetentionOwner {
    fn owner(&self) -> Owner {
        Owner::PrivateRetention
    }
    async fn apply_in(
        &self,
        c: &mut PgConnection,
        r: &BatchRequest,
    ) -> Result<Outcome, CommandError> {
        apply(c, r, Owner::PrivateRetention, Step::Cleanup).await
    }
    async fn verify_in(
        &self,
        c: &mut PgConnection,
        r: &BatchRequest,
        receipt: Receipt,
    ) -> Result<(), CommandError> {
        verify(c, r, receipt, Owner::PrivateRetention, Step::Cleanup).await
    }
}

async fn scope(
    c: &mut PgConnection,
    r: &BatchRequest,
    owner: Owner,
    step: Step,
) -> Result<bool, CommandError> {
    let s = r.scope();
    if PgTransactionManager::get_transaction_depth(c) == 0
        || s.owner != owner
        || s.step != step
        || r.cursor()
            .is_some_and(|v| v.bytes() != r.ordinal().to_be_bytes())
    {
        return Err(CommandError::InvalidInput);
    }
    let phase: Option<String> =
        sqlx::query_scalar("select phase from public.concert_events where event_id=$1 for update")
            .bind(s.event)
            .fetch_optional(&mut *c)
            .await
            .map_err(db)?;
    if phase.as_deref() != Some("READ_ONLY") {
        return Err(CommandError::InvalidState);
    }
    let row = sqlx::query(
        "select * from public.concert_closure_jobs where event_id=$1 and job_id=$2 for update",
    )
    .bind(s.event)
    .bind(s.job)
    .fetch_optional(&mut *c)
    .await
    .map_err(db)?
    .ok_or(CommandError::Denied)?;
    let job: Job = serde_json::from_value(row.try_get("progress").map_err(db)?)
        .map_err(|_| CommandError::InvalidState)?;
    job.validate().map_err(|_| CommandError::InvalidState)?;
    let retention = Retention {
        policy_revision: row.try_get("retention_revision").map_err(db)?,
        closed_at: row.try_get("closed_at").map_err(db)?,
        cleanup_after: row.try_get("cleanup_after").map_err(db)?,
    };
    if job.event_id != s.event
        || job.job_id != s.job
        || job.next() != Some(step)
        || !matches!(job.state, JobState::Pending | JobState::Running)
        || job.revision != row.try_get::<i64, _>("revision").map_err(db)?
        || row.try_get::<Uuid, _>("actor_id").map_err(db)? != s.actor
        || retention != r.retention()
    {
        return Err(CommandError::InvalidState);
    }
    let head = load(c, s, retention)
        .await?
        .ok_or(CommandError::Unavailable)?;
    if head.request().binding() != r.binding() {
        return Err(CommandError::InvalidState);
    }
    finish(c).await?;
    let valid:bool=sqlx::query_scalar("select coalesce(clock_timestamp()<nullif(current_setting('mcr.relationship_authority_until',true),'')::timestamptz,false) and ($1<>'CLEANUP' or clock_timestamp()>=$2)")
        .bind(step.name()).bind(retention.cleanup_after).fetch_one(c).await.map_err(db)?;
    if !valid {
        return Err(CommandError::Unavailable);
    }
    Ok(head.state() == State::Complete)
}

async fn apply(
    c: &mut PgConnection,
    r: &BatchRequest,
    owner: Owner,
    step: Step,
) -> Result<Outcome, CommandError> {
    if scope(c, r, owner, step).await? {
        return Err(CommandError::InvalidState);
    }
    // Correlation only: SQL validates the actual locked job/head and derives the
    // request binding independently. No source/role authority is created here.
    sqlx::query("insert into public.concert_event_cleanup_owner_pages(operation_id,job_id,event_id,actor_id,step,owner,ordinal,batch_limit,cursor_in,binding) values($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)")
        .bind(r.operation()).bind(r.scope().job).bind(r.scope().event).bind(r.scope().actor)
        .bind(step.name()).bind(owner.name()).bind(r.ordinal() as i64).bind(r.limit() as i16)
        .bind(r.cursor().map(Cursor::bytes)).bind(r.binding().to_vec()).execute(&mut *c).await.map_err(db)?;
    let previous: String =
        sqlx::query_scalar("select coalesce(current_setting('mcr.cleanup_operation_id',true),'')")
            .fetch_one(&mut *c)
            .await
            .map_err(db)?;
    sqlx::query("select set_config('mcr.cleanup_operation_id',$1,true)")
        .bind(r.operation().to_string())
        .execute(&mut *c)
        .await
        .map_err(db)?;
    if owner == Owner::Beacons {
        let ids:Vec<Uuid>=sqlx::query_scalar("select beacon_id from public.concert_meeting_beacons where event_id=$1 and state='ACTIVE' order by beacon_id limit $2 for update")
            .bind(r.scope().event).bind(i64::from(r.limit())).fetch_all(&mut *c).await.map_err(db)?;
        for id in ids {
            sqlx::query("update public.concert_meeting_beacons set state='REVOKED',revision=revision+1 where event_id=$1 and beacon_id=$2 and state='ACTIVE'")
                .bind(r.scope().event).bind(id).execute(&mut *c).await.map_err(db)?;
        }
    } else {
        // The canonical set is removed leaf first; SQL captures every actual
        // row change, including cascades, and rejects a page above its row cap.
        let keys:Vec<Vec<u8>>=sqlx::query_scalar("select resource_key from public.concert_event_cleanup_private_candidates($1) order by resource_key limit $2")
            .bind(r.scope().event).bind(i64::from(r.limit())).fetch_all(&mut *c).await.map_err(db)?;
        for key in keys {
            sqlx::query("select public.concert_event_cleanup_private_apply($1,$2)")
                .bind(r.scope().event)
                .bind(key)
                .execute(&mut *c)
                .await
                .map_err(db)?;
        }
    }
    sqlx::query("select set_config('mcr.cleanup_operation_id',$1,true)")
        .bind(previous)
        .execute(&mut *c)
        .await
        .map_err(db)?;
    sqlx::query("select public.concert_event_cleanup_owner_seal($1)")
        .bind(r.operation())
        .execute(&mut *c)
        .await
        .map_err(db)?;
    let (receipt, next) = witness(c, r).await?;
    verify(c, r, receipt, owner, step).await?;
    match next {
        Some(next) => Ok(Outcome::Page { receipt, next }),
        None => Ok(Outcome::Converged { receipt }),
    }
}

async fn witness(
    c: &mut PgConnection,
    r: &BatchRequest,
) -> Result<(Receipt, Option<Cursor>), CommandError> {
    // Reconstructs and checks automatic native effects/current postimages, not
    // merely a UUID/count or a caller-authored witness row.
    sqlx::query("select public.concert_event_cleanup_owner_verify($1)")
        .bind(r.operation())
        .execute(&mut *c)
        .await
        .map_err(db)?;
    let row=sqlx::query("select binding,receipt_id,processed,cursor_out from public.concert_event_cleanup_owner_pages where operation_id=$1 and state='SEALED' for share")
        .bind(r.operation()).fetch_optional(c).await.map_err(db)?.ok_or(CommandError::Unavailable)?;
    if row.try_get::<Vec<u8>, _>("binding").map_err(db)? != r.binding() {
        return Err(CommandError::InvalidState);
    }
    let receipt = Receipt {
        binding: r.binding(),
        operation: r.operation(),
        id: row.try_get("receipt_id").map_err(db)?,
        revision: 1,
        processed: row.try_get::<i16, _>("processed").map_err(db)? as u16,
    };
    let next = row
        .try_get::<Option<Vec<u8>>, _>("cursor_out")
        .map_err(db)?
        .map(|v| Cursor::new(v).map_err(|_| CommandError::InvalidState))
        .transpose()?;
    Ok((receipt, next))
}
async fn verify(
    c: &mut PgConnection,
    r: &BatchRequest,
    receipt: Receipt,
    owner: Owner,
    step: Step,
) -> Result<(), CommandError> {
    let complete = scope(c, r, owner, step).await?;
    let (actual, next) = witness(c, r).await?;
    if receipt != actual || (complete && next.is_some()) {
        return Err(CommandError::InvalidState);
    }
    if complete {
        let matches:bool=sqlx::query_scalar("select exists(select 1 from public.concert_event_cleanup_batches where operation_id=$1 and receipt_id=$2 and binding=$3 and owner_revision=1 and processed=$4 and outcome='CONVERGED')")
            .bind(r.operation()).bind(receipt.id).bind(receipt.binding.to_vec()).bind(receipt.processed as i16).fetch_one(&mut *c).await.map_err(db)?;
        if !matches {
            return Err(CommandError::InvalidState);
        }
    }
    finish(c).await
}
