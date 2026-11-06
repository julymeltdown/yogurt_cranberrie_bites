//! Actual Gift owning-store child. Party alone composes the combined category.
use crate::world::account_deletion::{
    bound_transaction_in, lock_user, verify_source, DeletionAuthority, DeletionFailure,
    VerifiedDeletionSource, WorldDeletionCommand, WorldDeletionTarget,
};
use sqlx::{PgPool, Row};
use std::sync::Arc;
use tokio::time::Instant;

#[derive(Clone, PartialEq, Eq)]
pub struct GiftPrivateInputReceipt {
    pub command: WorldDeletionCommand,
    pub source_binding: [u8; 32],
    pub owner_revision: u64,
    pub complete: bool,
}
impl GiftPrivateInputReceipt {
    pub fn verifies(&self, command: &WorldDeletionCommand, source: &[u8; 32]) -> bool {
        self.command == *command
            && self.source_binding == *source
            && ((self.complete && self.owner_revision > 0)
                || (!self.complete && self.owner_revision == 0))
    }
}
pub struct GiftPrivateInputDeletion {
    pool: PgPool,
    authority: Arc<dyn DeletionAuthority>,
}
impl GiftPrivateInputDeletion {
    pub fn new(pool: PgPool, authority: Arc<dyn DeletionAuthority>) -> Self {
        Self { pool, authority }
    }
    pub async fn apply(
        &self,
        command: &WorldDeletionCommand,
        source: &VerifiedDeletionSource,
        deadline: Instant,
    ) -> Result<GiftPrivateInputReceipt, DeletionFailure> {
        source.check(command, deadline)?;
        if command.target != WorldDeletionTarget::PrivateData {
            return Err(DeletionFailure::InvalidCommand);
        }
        tokio::time::timeout_at(deadline, self.round(command, source, deadline))
            .await
            .map_err(|_| DeletionFailure::Unavailable)?
    }
    async fn round(
        &self,
        command: &WorldDeletionCommand,
        source: &VerifiedDeletionSource,
        deadline: Instant,
    ) -> Result<GiftPrivateInputReceipt, DeletionFailure> {
        let current = verify_source(self.authority.as_ref(), command, deadline).await?;
        if current.source_binding() != source.source_binding() {
            return Err(DeletionFailure::InvalidCommand);
        }
        let mut tx = self.pool.begin().await.map_err(db)?;
        bound_transaction_in(&mut tx, deadline).await?;
        let remaining = deadline.saturating_duration_since(Instant::now());
        let until = chrono::Utc::now()
            .checked_add_signed(chrono::Duration::microseconds(
                remaining
                    .as_micros()
                    .try_into()
                    .map_err(|_| DeletionFailure::Unavailable)?,
            ))
            .ok_or(DeletionFailure::Unavailable)?;
        sqlx::query("select set_config('mcr.gift_deletion_until',$1,true),set_config('mcr.community_authority_until',$1,true)")
            .bind(until.to_rfc3339()).execute(&mut *tx).await.map_err(db)?;
        lock_user(&mut tx, command.user_id).await.map_err(db)?;
        let fence: Option<i64> = sqlx::query_scalar("select fence_revision from concert_world_account_deletion_fences where user_id=$1 and saga_id=$2 and identity_binding=$3")
            .bind(command.user_id).bind(command.saga_id).bind(command.identity_binding()?.as_slice())
            .fetch_optional(&mut *tx).await.map_err(db)?;
        let fence = fence
            .filter(|n| *n > 0)
            .ok_or(DeletionFailure::InvalidCommand)?;
        sqlx::query("insert into concert_gift_account_deletion_progress(user_id,saga_id,command_binding,source_binding,identity_binding,fence_revision,revision,complete) values($1,$2,$3,$4,$5,$6,1,false) on conflict do nothing")
            .bind(command.user_id).bind(command.saga_id).bind(command.binding()?.as_slice())
            .bind(source.source_binding().as_slice()).bind(command.identity_binding()?.as_slice()).bind(fence)
            .execute(&mut *tx).await.map_err(db)?;
        let row = sqlx::query(
            "select * from concert_gift_account_deletion_progress where user_id=$1 for update",
        )
        .bind(command.user_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(db)?;
        if row.try_get::<uuid::Uuid, _>("saga_id").map_err(db)? != command.saga_id
            || row.try_get::<Vec<u8>, _>("command_binding").map_err(db)? != command.binding()?
            || row.try_get::<Vec<u8>, _>("source_binding").map_err(db)?
                != source.source_binding().as_slice()
            || row.try_get::<Vec<u8>, _>("identity_binding").map_err(db)?
                != command.identity_binding()?
            || row.try_get::<i64, _>("fence_revision").map_err(db)? != fence
        {
            return Err(DeletionFailure::InvalidCommand);
        }
        let mut complete: bool = row.try_get("complete").map_err(db)?;
        let mut revision: i64 = row.try_get("revision").map_err(db)?;
        if !complete {
            complete = crate::community::giveaway::withdraw_private_input_in(
                &mut tx, command, source, deadline, 16,
            )
            .await
            .map_err(|_| DeletionFailure::Unavailable)?;
            revision = revision
                .checked_add(1)
                .ok_or(DeletionFailure::Unavailable)?;
            sqlx::query("update concert_gift_account_deletion_progress set revision=$2,complete=$3 where user_id=$1")
                .bind(command.user_id).bind(revision).bind(complete).execute(&mut *tx).await.map_err(db)?;
        }
        source.check(command, deadline)?;
        let current = verify_source(self.authority.as_ref(), command, deadline).await?;
        if current.source_binding() != source.source_binding() {
            return Err(DeletionFailure::InvalidCommand);
        }
        source.check(command, deadline)?;
        tx.commit().await.map_err(db)?;
        Ok(GiftPrivateInputReceipt {
            command: command.clone(),
            source_binding: *source.source_binding(),
            owner_revision: if complete { revision as u64 } else { 0 },
            complete,
        })
    }
}
fn db(_: sqlx::Error) -> DeletionFailure {
    DeletionFailure::Unavailable
}
