//! Concert-owned Age erasure. No external verifier or reconstructed owner scope.
use crate::{
    account_private_data::{
        PrivateDataCategory, PrivateDataCategoryOwner, PrivateDataCategoryOwners,
        PrivateDataCategoryReceipt, CATEGORY_VERSION,
    },
    world::account_deletion::{
        bound_transaction_in, check_deadline, establish_fence_in, lock_user_in, verify_source,
        DeletionAuthority, DeletionFailure, VerifiedDeletionSource, WorldDeletionCommand,
        WorldDeletionTarget,
    },
};
use async_trait::async_trait;
use sqlx::{PgConnection, PgPool, Row};
use std::sync::Arc;
use tokio::time::Instant;
use uuid::Uuid;

/// At most this many verification/claim pairs are erased in one transaction.
pub const AGE_DELETION_PAGE: i64 = 64;

pub struct AgeAccountDeletion {
    pool: PgPool,
    authority: Arc<dyn DeletionAuthority>,
}

impl AgeAccountDeletion {
    pub fn new(pool: PgPool, authority: Arc<dyn DeletionAuthority>) -> Self {
        Self { pool, authority }
    }

    /// Root composes this disjoint registry with the other six real owners.
    pub fn category_owners(
        pool: PgPool,
        authority: Arc<dyn DeletionAuthority>,
    ) -> PrivateDataCategoryOwners {
        PrivateDataCategoryOwners::new([(
            PrivateDataCategory::AgeClaims,
            Arc::new(Self::new(pool, authority)) as Arc<dyn PrivateDataCategoryOwner>,
        )])
    }

    async fn apply_in(
        &self,
        command: &WorldDeletionCommand,
        source: &VerifiedDeletionSource,
        deadline: Instant,
    ) -> Result<PrivateDataCategoryReceipt, DeletionFailure> {
        // A coordinator capability is necessary but never replaces a current
        // authoritative check at the actual owner before mutation and COMMIT.
        if verify_source(self.authority.as_ref(), command, deadline).await? != *source {
            return Err(DeletionFailure::InvalidCommand);
        }
        let mut tx = self.pool.begin().await.map_err(db)?;
        establish_fence_in(&mut tx, command, source).await?;
        sqlx::query("insert into public.concert_age_deletion_progress(user_id,saga_id,command_binding,source_binding) values($1,$2,$3,$4) on conflict do nothing")
            .bind(command.user_id).bind(command.saga_id).bind(command.binding()?.as_slice())
            .bind(source.source_binding().as_slice()).execute(&mut *tx).await.map_err(db)?;
        let row = sqlx::query("select saga_id,command_binding,source_binding,progress_revision,completed_at is not null as complete from public.concert_age_deletion_progress where user_id=$1 for update")
            .bind(command.user_id).fetch_one(&mut *tx).await.map_err(db)?;
        if row.try_get::<Uuid, _>("saga_id").map_err(db)? != command.saga_id
            || row.try_get::<Vec<u8>, _>("command_binding").map_err(db)? != command.binding()?
            || row.try_get::<Vec<u8>, _>("source_binding").map_err(db)? != *source.source_binding()
        {
            return Err(DeletionFailure::InvalidCommand);
        }
        let mut complete: bool = row.try_get("complete").map_err(db)?;
        let mut revision: i64 = row.try_get("progress_revision").map_err(db)?;
        if !complete {
            bound_transaction_in(&mut tx, deadline).await?;
            let ids: Vec<Uuid> = sqlx::query_scalar("select verification_id from public.concert_age_verifications where user_id=$1 order by verification_id limit $2 for update")
                .bind(command.user_id).bind(AGE_DELETION_PAGE).fetch_all(&mut *tx).await.map_err(db)?;
            // Claims reference verification rows; erase children first. Neither
            // command receipts nor immutable native witnesses are touched.
            let claims = sqlx::query(
                "delete from public.concert_age_claims where user_id=$1 and claim_id=any($2)",
            )
            .bind(command.user_id)
            .bind(&ids)
            .execute(&mut *tx)
            .await
            .map_err(db)?
            .rows_affected();
            let verifications = sqlx::query("delete from public.concert_age_verifications where user_id=$1 and verification_id=any($2)")
                .bind(command.user_id).bind(&ids).execute(&mut *tx).await.map_err(db)?.rows_affected();
            complete = !sqlx::query_scalar::<_,bool>("select exists(select 1 from public.concert_age_verifications where user_id=$1) or exists(select 1 from public.concert_age_claims where user_id=$1)")
                .bind(command.user_id).fetch_one(&mut *tx).await.map_err(db)?;
            if complete {
                sqlx::query("delete from public.concert_age_owners where user_id=$1")
                    .bind(command.user_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(db)?;
            }
            revision = sqlx::query_scalar("update public.concert_age_deletion_progress set erased_claims=erased_claims+$2,erased_verifications=erased_verifications+$3,progress_revision=progress_revision+1,completed_at=case when $4 then clock_timestamp() else null end where user_id=$1 returning progress_revision")
                .bind(command.user_id).bind(claims as i64).bind(verifications as i64).bind(complete)
                .fetch_one(&mut *tx).await.map_err(db)?;
        }
        if verify_source(self.authority.as_ref(), command, deadline).await? != *source {
            return Err(DeletionFailure::Unavailable);
        }
        source.check(command, deadline)?;
        tx.commit().await.map_err(db)?;
        check_deadline(deadline)?;
        Ok(PrivateDataCategoryReceipt {
            command: command.clone(),
            category: PrivateDataCategory::AgeClaims,
            version: CATEGORY_VERSION,
            source_binding: *source.source_binding(),
            progress_revision: if complete { revision as u64 } else { 0 },
            complete,
        })
    }
}

#[async_trait]
impl PrivateDataCategoryOwner for AgeAccountDeletion {
    async fn apply_category(
        &self,
        command: &WorldDeletionCommand,
        source: &VerifiedDeletionSource,
        deadline: Instant,
    ) -> Result<PrivateDataCategoryReceipt, DeletionFailure> {
        source.check(command, deadline)?;
        if command.target != WorldDeletionTarget::PrivateData {
            return Err(DeletionFailure::InvalidCommand);
        }
        tokio::time::timeout_at(deadline, self.apply_in(command, source, deadline))
            .await
            .map_err(|_| DeletionFailure::Unavailable)?
    }
}

/// Normal Age operations take the SAME canonical user lock before event/claim
/// locks, retain it through their native commit and deny permanently fenced users.
pub(super) async fn allow_age_in(c: &mut PgConnection, user: Uuid) -> Result<(), DeletionFailure> {
    lock_user_in(c, user).await.map_err(db)?;
    let fenced: bool = sqlx::query_scalar("select exists(select 1 from public.concert_world_account_deletion_fences where user_id=$1)")
        .bind(user).fetch_one(&mut *c).await.map_err(db)?;
    if fenced {
        return Err(DeletionFailure::InvalidCommand);
    }
    Ok(())
}

fn db(_: sqlx::Error) -> DeletionFailure {
    DeletionFailure::Unavailable
}
