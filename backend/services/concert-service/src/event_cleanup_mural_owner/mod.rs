//! Actual local BE41 mural freeze. Compose through CleanupRuntime on its supplied
//! transaction; this adapter neither establishes privileged authority nor commits.
//! The immutable engine batch binds the verified snapshot to the exact request.
use crate::event_cleanup_live::LocalOwner;
use async_trait::async_trait;
use concert_application::{commands::CommandError, event_cleanup::*};
use concert_domain::event_lifecycle::{Job, Retention, Step};
use sqlx::{postgres::PgTransactionManager, PgConnection, Row, TransactionManager};
use uuid::Uuid;

/// Stateless owning adapter: no pool, network client, policy or enabled default.
pub struct MuralFreezeOwner;

#[async_trait]
impl LocalOwner for MuralFreezeOwner {
    fn owner(&self) -> Owner {
        Owner::Mural
    }

    async fn apply_in(
        &self,
        c: &mut PgConnection,
        request: &BatchRequest,
    ) -> Result<Outcome, CommandError> {
        scope_in(c, request).await?;
        let row = sqlx::query(
            "select state,revision from public.concert_mural_state where event_id=$1 for update",
        )
        .bind(request.scope().event)
        .fetch_optional(&mut *c)
        .await
        .map_err(db)?
        .ok_or(CommandError::Unavailable)?;
        let revision: i64 = row.try_get("revision").map_err(db)?;
        match row.try_get::<&str, _>("state").map_err(db)? {
            "FROZEN" => {} // Never invent or repair an absent recovery checkpoint.
            "EDITABLE" | "PAUSED" => {
                if revision >= concert_domain::MAX_REVISION {
                    return Err(CommandError::InvalidState);
                }
                // 0019 forbids reusing a checkpoint ID and changing a frozen row.
                // This nonpixel revision resets recovery without rewriting pixels,
                // palette, retention floor, locks, author history or cooldowns.
                sqlx::query("update public.concert_mural_state set state='FROZEN',revision=revision+1,reset_revision=revision+1,snapshot_id=$2,snapshot_created_at=clock_timestamp() where event_id=$1")
                    .bind(request.scope().event).bind(Uuid::new_v4())
                    .execute(&mut *c).await.map_err(db)?;
                sqlx::query("insert into public.concert_mural_snapshots(snapshot_id,event_id,revision,state,pixels,palette,created_at) select snapshot_id,event_id,revision,state,pixels,palette,snapshot_created_at from public.concert_mural_state where event_id=$1")
                    .bind(request.scope().event).execute(&mut *c).await.map_err(db)?;
            }
            _ => return Err(CommandError::InvalidState),
        }
        let receipt = current_in(c, request).await?;
        // Verify the effect before returning convergence. CleanupRuntime repeats
        // verification and appends its immutable batch in this same transaction.
        self.verify_in(c, request, receipt).await?;
        Ok(Outcome::Converged { receipt })
    }

    async fn verify_in(
        &self,
        c: &mut PgConnection,
        request: &BatchRequest,
        receipt: Receipt,
    ) -> Result<(), CommandError> {
        let complete = scope_in(c, request).await?;
        if receipt != current_in(c, request).await? {
            return Err(CommandError::InvalidState);
        }
        let prior = sqlx::query("select * from public.concert_event_cleanup_batches where job_id=$1 and step='FREEZE_MURAL' and owner='MURAL' and ordinal=0 for share")
            .bind(request.scope().job).fetch_optional(&mut *c).await.map_err(db)?;
        if let Some(row) = prior {
            let scope = request.scope();
            if !complete
                || row.try_get::<Uuid, _>("event_id").map_err(db)? != scope.event
                || row.try_get::<Uuid, _>("actor_id").map_err(db)? != scope.actor
                || row.try_get::<Uuid, _>("operation_id").map_err(db)? != receipt.operation
                || row.try_get::<Vec<u8>, _>("binding").map_err(db)? != receipt.binding
                || row.try_get::<Uuid, _>("receipt_id").map_err(db)? != receipt.id
                || row.try_get::<i64, _>("owner_revision").map_err(db)? != receipt.revision
                || row.try_get::<i16, _>("processed").map_err(db)? != 1
                || row.try_get::<i64, _>("cumulative").map_err(db)? != 1
                || row.try_get::<&str, _>("outcome").map_err(db)? != "CONVERGED"
                || row
                    .try_get::<Option<Vec<u8>>, _>("cursor_in")
                    .map_err(db)?
                    .is_some()
                || row
                    .try_get::<Option<Vec<u8>>, _>("cursor_out")
                    .map_err(db)?
                    .is_some()
            {
                return Err(CommandError::InvalidState);
            }
        } else if complete {
            return Err(CommandError::InvalidState);
        }
        deadline(c).await
    }
}

fn db(_: sqlx::Error) -> CommandError {
    CommandError::Unavailable
}

/// This checks the engine's retained scope, not a new source of authority. The
/// trusted engine caller must already have established actual current fences.
async fn scope_in(c: &mut PgConnection, r: &BatchRequest) -> Result<bool, CommandError> {
    let scope = r.scope();
    if PgTransactionManager::get_transaction_depth(c) == 0
        || scope.owner != Owner::Mural
        || scope.step != Step::FreezeMural
        || r.ordinal() != 0
        || r.cursor().is_some()
    {
        return Err(CommandError::InvalidInput);
    }
    let phase: Option<String> =
        sqlx::query_scalar("select phase from public.concert_events where event_id=$1 for update")
            .bind(scope.event)
            .fetch_optional(&mut *c)
            .await
            .map_err(db)?;
    if phase.as_deref() != Some("READ_ONLY") {
        return Err(CommandError::InvalidState);
    }
    let job = sqlx::query("select actor_id,retention_revision,closed_at,cleanup_after,progress from public.concert_closure_jobs where event_id=$1 and job_id=$2 for update")
        .bind(scope.event).bind(scope.job).fetch_optional(&mut *c).await.map_err(db)?
        .ok_or(CommandError::Denied)?;
    let projection: Job = serde_json::from_value(job.try_get("progress").map_err(db)?)
        .map_err(|_| CommandError::InvalidState)?;
    projection
        .validate()
        .map_err(|_| CommandError::InvalidState)?;
    let retention = Retention {
        policy_revision: job.try_get("retention_revision").map_err(db)?,
        closed_at: job.try_get("closed_at").map_err(db)?,
        cleanup_after: job.try_get("cleanup_after").map_err(db)?,
    };
    if job.try_get::<Uuid, _>("actor_id").map_err(db)? != scope.actor
        || projection.job_id != scope.job
        || projection.event_id != scope.event
        || projection.next() != Some(Step::FreezeMural)
        || retention != r.retention()
    {
        return Err(CommandError::InvalidState);
    }
    let head = sqlx::query("select batch_limit,ordinal,cursor,processed,state from public.concert_event_cleanup_progress where job_id=$1 and event_id=$2 and actor_id=$3 and step='FREEZE_MURAL' and owner='MURAL' for update")
        .bind(scope.job).bind(scope.event).bind(scope.actor).fetch_optional(&mut *c).await.map_err(db)?
        .ok_or(CommandError::Unavailable)?;
    let state: &str = head.try_get("state").map_err(db)?;
    let complete = state == "COMPLETE";
    if head.try_get::<i16, _>("batch_limit").map_err(db)? != r.limit() as i16
        || head.try_get::<i64, _>("ordinal").map_err(db)? != 0
        || head
            .try_get::<Option<Vec<u8>>, _>("cursor")
            .map_err(db)?
            .is_some()
        || head.try_get::<i64, _>("processed").map_err(db)? != i64::from(complete)
        || !matches!(state, "PENDING" | "PARTIAL_FAILURE" | "COMPLETE")
    {
        return Err(CommandError::InvalidState);
    }
    deadline(c).await?;
    Ok(complete)
}

async fn deadline(c: &mut PgConnection) -> Result<(), CommandError> {
    let valid: bool = sqlx::query_scalar("select coalesce(clock_timestamp()<nullif(current_setting('mcr.lifecycle_authority_until',true),'')::timestamptz,false)")
        .fetch_one(c).await.map_err(db)?;
    if valid {
        Ok(())
    } else {
        Err(CommandError::Unavailable)
    }
}

async fn current_in(c: &mut PgConnection, r: &BatchRequest) -> Result<Receipt, CommandError> {
    // Exact row equality includes creation time and current aggregate snapshot ID.
    // A matching older checkpoint or merely FROZEN state is insufficient.
    let row = sqlx::query("select m.snapshot_id,m.revision from public.concert_mural_state m join public.concert_mural_snapshots s on s.snapshot_id=m.snapshot_id where m.event_id=$1 and m.state='FROZEN' and m.reset_revision=m.revision and (s.event_id,s.revision,s.state,s.pixels,s.palette,s.created_at)=(m.event_id,m.revision,m.state,m.pixels,m.palette,m.snapshot_created_at) for share of m,s")
        .bind(r.scope().event).fetch_optional(c).await.map_err(db)?
        .ok_or(CommandError::Unavailable)?;
    let receipt = Receipt {
        binding: r.binding(),
        operation: r.operation(),
        id: row.try_get("snapshot_id").map_err(db)?,
        revision: row.try_get("revision").map_err(db)?,
        // Count the one verified resource, including an existing frozen mural.
        processed: 1,
    };
    if receipt.id.is_nil() || !(1..=concert_domain::MAX_REVISION).contains(&receipt.revision) {
        return Err(CommandError::InvalidState);
    }
    Ok(receipt)
}
