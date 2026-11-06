//! BE42 Wall category only. Uses the actual Auth source and common deletion
//! fence; never acknowledges the aggregate or removes generic transport audit.
use crate::{
    account_private_data::{
        PrivateDataCategory, PrivateDataCategoryOwner, PrivateDataCategoryReceipt, CATEGORY_VERSION,
    },
    world::account_deletion::{
        bound_transaction_in, establish_fence_in, verify_source, DeletionAuthority,
        DeletionFailure, VerifiedDeletionSource, WorldDeletionCommand, WorldDeletionTarget,
    },
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgConnection, PgPool, Row};
use std::sync::Arc;
use tokio::time::Instant;
use uuid::Uuid;

pub const DELETION_PAGE_ROWS: i64 = 128;
pub struct MuralPrivateDataOwner {
    pool: PgPool,
    authority: Arc<dyn DeletionAuthority>,
}
impl MuralPrivateDataOwner {
    pub fn new(pool: PgPool, authority: Arc<dyn DeletionAuthority>) -> Self {
        Self { pool, authority }
    }
}
fn db(_: sqlx::Error) -> DeletionFailure {
    DeletionFailure::Unavailable
}

/// Normal Wall readers/writers acquire this before event/root/receipt locks.
/// A common immutable fence denies recreation for every deletion target; this
/// is a negative admission check, never a source of deletion authority.
pub(crate) async fn fence_writer_in(c: &mut PgConnection, user: Uuid) -> Result<bool, sqlx::Error> {
    crate::world::account_deletion::lock_user_in(c, user).await?;
    let fenced:bool=sqlx::query_scalar("select exists(select 1 from public.concert_world_account_deletion_fences where user_id=$1)")
        .bind(user).fetch_one(&mut *c).await?;
    Ok(!fenced)
}

const REMAINING: &str =
    "select exists(select 1 from public.concert_mural_operations where actor_id=$1)
 or exists(select 1 from public.concert_mural_limits where actor_id=$1)
 or exists(select 1 from public.concert_mural_tool_actions where actor_id=$1)
 or exists(select 1 from public.concert_mural_tool_correlations where actor_id=$1)
 or exists(select 1 from public.concert_mural_tool_charges where actor_id=$1)";

#[async_trait]
impl PrivateDataCategoryOwner for MuralPrivateDataOwner {
    async fn apply_category(
        &self,
        command: &WorldDeletionCommand,
        source: &VerifiedDeletionSource,
        deadline: Instant,
    ) -> Result<PrivateDataCategoryReceipt, DeletionFailure> {
        command.validate()?;
        source.check(command, deadline)?;
        if command.target != WorldDeletionTarget::PrivateData {
            return Err(DeletionFailure::InvalidCommand);
        }
        tokio::time::timeout_at(deadline, self.page(command, source, deadline))
            .await
            .map_err(|_| DeletionFailure::Unavailable)?
    }
}
impl MuralPrivateDataOwner {
    async fn page(
        &self,
        command: &WorldDeletionCommand,
        source: &VerifiedDeletionSource,
        deadline: Instant,
    ) -> Result<PrivateDataCategoryReceipt, DeletionFailure> {
        let current = verify_source(self.authority.as_ref(), command, deadline).await?;
        if current.source_binding() != source.source_binding() {
            return Err(DeletionFailure::InvalidCommand);
        }
        source.check(command, deadline)?;
        let mut tx = self.pool.begin().await.map_err(db)?;
        bound_transaction_in(&mut tx, deadline).await?;
        // The common user-first lock and exact immutable saga identity precede
        // all owner progress and per-event rows, just as normal Wall writers do.
        let fence = establish_fence_in(&mut tx, command, source).await?;
        sqlx::query("insert into public.concert_mural_deletion_progress(user_id,saga_id,actor_id,retention_until,command_binding,source_binding) values($1,$2,$3,$4,$5,$6) on conflict(user_id) do nothing")
            .bind(command.user_id).bind(command.saga_id).bind(command.actor_id).bind(command.retention_until)
            .bind(command.binding()?.as_slice()).bind(source.source_binding().as_slice()).execute(&mut *tx).await.map_err(db)?;
        let row = sqlx::query(
            "select * from public.concert_mural_deletion_progress where user_id=$1 for update",
        )
        .bind(command.user_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(db)?;
        if row.get::<Uuid, _>("saga_id") != command.saga_id
            || row.get::<Uuid, _>("actor_id") != command.actor_id
            || row.get::<DateTime<Utc>, _>("retention_until") != command.retention_until
            || row.get::<Vec<u8>, _>("command_binding") != command.binding()?
            || row.get::<Vec<u8>, _>("source_binding") != *source.source_binding()
            || row.get::<String, _>("category") != "MURAL_PRIVATE_METADATA"
            || row.get::<i32, _>("category_version") != CATEGORY_VERSION
        {
            return Err(DeletionFailure::InvalidCommand);
        }
        let was_complete = row
            .get::<Option<DateTime<Utc>>, _>("completed_at")
            .is_some();
        let mut progress = row.get::<i64, _>("progress_revision");
        let mut processed = 0i64;
        if !was_complete {
            let millis = deadline
                .saturating_duration_since(Instant::now())
                .as_millis()
                .min(5000) as i64;
            if millis <= 0 {
                return Err(DeletionFailure::Unavailable);
            }
            sqlx::query("select set_config('mcr.mural_private_deletion_user',$1,true),set_config('mcr.mural_private_deletion_until',(clock_timestamp()+$2*interval '1 millisecond')::text,true)")
                .bind(command.user_id.to_string()).bind(millis).execute(&mut *tx).await.map_err(db)?;
            // The actor-indexed remaining set is the durable cursor: rows only
            // disappear or lose both private fields, and the permanent common
            // fence prohibits re-creation. At most 128 total rows per dispatch.
            let statements=[
                "with page as (select event_id,revision from public.concert_mural_operations where actor_id=$1 order by event_id,revision limit $2 for update) update public.concert_mural_operations o set actor_id=null,next_allowed_at=null from page p where (o.event_id,o.revision)=(p.event_id,p.revision)",
                "with page as (select event_id,actor_id from public.concert_mural_limits where actor_id=$1 order by event_id limit $2 for update) delete from public.concert_mural_limits o using page p where (o.event_id,o.actor_id)=(p.event_id,p.actor_id)",
                // Undo children have later revisions: remove those first and
                // retain all existing FK/audit guards, including across pages.
                "with page as (select action_id from public.concert_mural_tool_actions where actor_id=$1 order by event_id,revision desc limit $2 for update) delete from public.concert_mural_tool_actions o using page p where o.action_id=p.action_id",
                "with page as (select event_id,actor_id,idempotency_key from public.concert_mural_tool_correlations c where actor_id=$1 and not exists(select 1 from public.concert_mural_tool_actions a where (a.event_id,a.actor_id,a.idempotency_key)=(c.event_id,c.actor_id,c.idempotency_key)) order by event_id,idempotency_key limit $2 for update) delete from public.concert_mural_tool_correlations o using page p where (o.event_id,o.actor_id,o.idempotency_key)=(p.event_id,p.actor_id,p.idempotency_key)",
                // No UPDATE and no live-window DELETE: 0058's actual immutable
                // charge trigger remains unchanged and independently enforced.
                "with page as (select action_id from public.concert_mural_tool_charges where actor_id=$1 and committed_at<=clock_timestamp()-interval '600 seconds' order by committed_at,event_id limit $2 for update) delete from public.concert_mural_tool_charges o using page p where o.action_id=p.action_id",
            ];
            for statement in statements {
                source.check(command, deadline)?;
                if processed >= DELETION_PAGE_ROWS {
                    break;
                }
                let changed = sqlx::query(statement)
                    .bind(command.user_id)
                    .bind(DELETION_PAGE_ROWS - processed)
                    .execute(&mut *tx)
                    .await
                    .map_err(db)?
                    .rows_affected();
                processed += i64::try_from(changed).map_err(|_| DeletionFailure::Unavailable)?;
            }
        }
        let remaining: bool = sqlx::query_scalar(REMAINING)
            .bind(command.user_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(db)?;
        if was_complete && remaining {
            return Err(DeletionFailure::InvalidCommand);
        }
        if !was_complete {
            let wait_until:Option<DateTime<Utc>>=sqlx::query_scalar("select min(committed_at+interval '600 seconds') from public.concert_mural_tool_charges where actor_id=$1 and committed_at>clock_timestamp()-interval '600 seconds'")
                .bind(command.user_id).fetch_one(&mut *tx).await.map_err(db)?;
            sqlx::query("update public.concert_mural_deletion_progress set processed_rows=processed_rows+$2,progress_revision=progress_revision+1,wait_until=case when $3 then $4 else null end,completed_at=case when $3 then null else clock_timestamp() end where user_id=$1")
                .bind(command.user_id).bind(processed).bind(remaining).bind(wait_until).execute(&mut *tx).await.map_err(db)?;
            progress += 1;
        }
        source.check(command, deadline)?;
        let current = verify_source(self.authority.as_ref(), command, deadline).await?;
        if current.source_binding() != source.source_binding() {
            return Err(DeletionFailure::Unavailable);
        }
        source.check(command, deadline)?;
        tx.commit().await.map_err(db)?;
        Ok(PrivateDataCategoryReceipt {
            command: command.clone(),
            category: PrivateDataCategory::MuralPrivateMetadata,
            version: CATEGORY_VERSION,
            source_binding: *source.source_binding(),
            progress_revision: if remaining {
                0
            } else {
                u64::try_from(progress)
                    .map_err(|_| DeletionFailure::Unavailable)?
                    .max(fence.revision)
            },
            complete: !remaining,
        })
    }
}
