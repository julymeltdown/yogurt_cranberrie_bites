//! BE42 Media owner. A category receipt is not a whole PrivateData acknowledgement.
use crate::{
    account_private_data::{
        PrivateDataCategory, PrivateDataCategoryOwner, PrivateDataCategoryReceipt, CATEGORY_VERSION,
    },
    world::account_deletion::{
        establish_fence_in, lock_user_in, verify_source, DeletionAuthority, DeletionFailure,
        VerifiedDeletionSource, WorldDeletionCommand, WorldDeletionTarget,
    },
};
use async_trait::async_trait;
use sqlx::{PgConnection, PgPool, Row};
use std::sync::Arc;
use tokio::time::Instant;
use uuid::Uuid;

pub const PAGE_SIZE: i64 = 128;

/// Normal writer/read admission only, never account-erasure authorization.
/// Call BEFORE event/target/media locks; public readers pass actor and actual
/// media owner together in the first call so cross-user locks are sorted.
pub async fn lock_live_media_users_in(
    c: &mut PgConnection,
    users: &[Uuid],
) -> Result<(), concert_application::commands::CommandError> {
    use concert_application::commands::CommandError;
    if users.is_empty() || users.len() > 2 || users.iter().any(Uuid::is_nil) {
        return Err(CommandError::Denied);
    }
    require_current_snapshot_in(c)
        .await
        .map_err(|_| CommandError::Unavailable)?;
    let mut users = users.to_vec();
    users.sort_unstable();
    users.dedup();
    for user in users {
        lock_user_in(c, user)
            .await
            .map_err(|_| CommandError::Unavailable)?;
        let deleted: bool = sqlx::query_scalar(
            "select exists(select 1 from public.concert_world_account_deletion_fences where user_id=$1)",
        )
        .bind(user)
        .fetch_one(&mut *c)
        .await
        .map_err(|_| CommandError::Unavailable)?;
        if deleted {
            return Err(CommandError::Denied);
        }
    }
    Ok(())
}

/// Actual Auth deletion-source authority is mandatory; there is no successful
/// default, caller-controlled bypass flag or interpretation of retention dates.
pub struct MediaPrivateDataOwner {
    pool: PgPool,
    authority: Arc<dyn DeletionAuthority>,
}
impl MediaPrivateDataOwner {
    pub fn new(pool: PgPool, authority: Arc<dyn DeletionAuthority>) -> Self {
        Self { pool, authority }
    }

    async fn round(
        &self,
        command: &WorldDeletionCommand,
        source: &VerifiedDeletionSource,
        deadline: Instant,
    ) -> Result<PrivateDataCategoryReceipt, DeletionFailure> {
        let fresh = verify_source(self.authority.as_ref(), command, deadline).await?;
        if fresh.source_binding() != source.source_binding() {
            return Err(DeletionFailure::InvalidCommand);
        }
        let mut tx = self.pool.begin().await.map_err(db)?;
        require_current_snapshot_in(&mut tx).await?;
        // Common canonical user lock/fence precedes progress, associations and
        // media. The common identity cannot be rebound to another saga/actor.
        let fence = establish_fence_in(&mut tx, command, source).await?;
        sqlx::query("insert into public.concert_media_account_deletion_progress(user_id,saga_id,actor_id,retention_until,command_binding,source_binding,category_version,fence_revision) values($1,$2,$3,$4,$5,$6,$7,$8) on conflict(user_id) do nothing")
            .bind(command.user_id).bind(command.saga_id).bind(command.actor_id)
            .bind(command.retention_until).bind(command.binding()?.as_slice())
            .bind(source.source_binding().as_slice()).bind(CATEGORY_VERSION)
            .bind(i64::try_from(fence.revision).map_err(|_|DeletionFailure::Unavailable)?)
            .execute(&mut *tx).await.map_err(db)?;
        let row = sqlx::query("select saga_id,actor_id,command_binding,source_binding,category_version,fence_revision,cursor_media_id,progress_revision,completed_at from public.concert_media_account_deletion_progress where user_id=$1 for update")
            .bind(command.user_id).fetch_one(&mut *tx).await.map_err(db)?;
        if row.try_get::<Uuid, _>("saga_id").map_err(db)? != command.saga_id
            || row.try_get::<Uuid, _>("actor_id").map_err(db)? != command.actor_id
            || row.try_get::<Vec<u8>, _>("command_binding").map_err(db)? != command.binding()?
            || row.try_get::<Vec<u8>, _>("source_binding").map_err(db)? != *source.source_binding()
            || row.try_get::<i32, _>("category_version").map_err(db)? != CATEGORY_VERSION
            || row.try_get::<i64, _>("fence_revision").map_err(db)? as u64 != fence.revision
        {
            return Err(DeletionFailure::InvalidCommand);
        }
        let terminal = row
            .try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("completed_at")
            .map_err(db)?
            .is_some();
        let mut revision = row.try_get::<i64, _>("progress_revision").map_err(db)?;
        if terminal {
            if revision <= 0 || remaining_in(&mut tx, command.user_id).await? {
                return Err(DeletionFailure::InvalidCommand);
            }
        } else {
            let cursor = row
                .try_get::<Option<Uuid>, _>("cursor_media_id")
                .map_err(db)?;
            // Include a private photo association even if its referenced media
            // is already tombstoned. Never scrub another user's media by hint.
            let ids: Vec<Uuid> = sqlx::query_scalar("select media_id from (select media_id from public.concert_media where owner_id=$1 and (state<>'DELETED' or staged is not null or sanitized is not null or width is not null or height is not null or target_id is not null) union select media_id from public.concert_meeting_beacon_photos where owner_id=$1) owned where ($2::uuid is null or media_id>$2) order by media_id limit $3")
                .bind(command.user_id).bind(cursor).bind(PAGE_SIZE)
                .fetch_all(&mut *tx).await.map_err(db)?;
            let associations = sqlx::query("delete from public.concert_meeting_beacon_photos where owner_id=$1 and media_id=any($2::uuid[])")
                .bind(command.user_id).bind(&ids).execute(&mut *tx).await.map_err(db)?.rows_affected();
            // Lock the whole bounded set in stable order before effects. No
            // event-phase, old audience, place or provider proof gates erasure.
            sqlx::query("select media_id from public.concert_media where owner_id=$1 and media_id=any($2::uuid[]) order by media_id for update")
                .bind(command.user_id).bind(&ids).fetch_all(&mut *tx).await.map_err(db)?;
            let erased = sqlx::query("update public.concert_media set state='DELETED',staged=null,sanitized=null,width=null,height=null,target_id=null,revision=revision+1 where owner_id=$1 and media_id=any($2::uuid[]) and state<>'DELETED'")
                .bind(command.user_id).bind(&ids).execute(&mut *tx).await.map_err(db)?.rows_affected();
            let complete = !remaining_in(&mut tx, command.user_id).await?;
            if !complete && ids.is_empty() {
                // A behind-cursor recreation or inconsistent store never earns
                // completion. Keep the prior durable cursor/receipt unchanged.
                return Err(DeletionFailure::Unavailable);
            }
            let next = ids.last().copied().or(cursor);
            revision = sqlx::query_scalar("update public.concert_media_account_deletion_progress set cursor_media_id=$2,processed_media=processed_media+$3,erased_media=erased_media+$4,removed_associations=removed_associations+$5,progress_revision=progress_revision+1,completed_at=case when $6 then clock_timestamp() else null end,updated_at=clock_timestamp() where user_id=$1 returning progress_revision")
                .bind(command.user_id).bind(next).bind(ids.len() as i64)
                .bind(i64::try_from(erased).map_err(|_|DeletionFailure::Unavailable)?)
                .bind(i64::try_from(associations).map_err(|_|DeletionFailure::Unavailable)?)
                .bind(complete).fetch_one(&mut *tx).await.map_err(db)?;
        }
        let complete = terminal || !remaining_in(&mut tx, command.user_id).await?;
        // A response is issued only after a fresh exact source check and commit.
        // Lost/unknown COMMIT is retried using the same durable operation key.
        let final_source = verify_source(self.authority.as_ref(), command, deadline).await?;
        if final_source.source_binding() != source.source_binding() {
            return Err(DeletionFailure::InvalidCommand);
        }
        source.check(command, deadline)?;
        tx.commit().await.map_err(db)?;
        source.check(command, deadline)?;
        Ok(PrivateDataCategoryReceipt {
            command: command.clone(),
            category: PrivateDataCategory::MediaAndMeetingPhotos,
            version: CATEGORY_VERSION,
            source_binding: *source.source_binding(),
            progress_revision: if complete {
                u64::try_from(revision)
                    .ok()
                    .filter(|r| *r > 0)
                    .ok_or(DeletionFailure::Unavailable)?
            } else {
                0
            },
            complete,
        })
    }
}

#[async_trait]
impl PrivateDataCategoryOwner for MediaPrivateDataOwner {
    async fn apply_category(
        &self,
        command: &WorldDeletionCommand,
        source: &VerifiedDeletionSource,
        deadline: Instant,
    ) -> Result<PrivateDataCategoryReceipt, DeletionFailure> {
        command.validate()?;
        if command.target != WorldDeletionTarget::PrivateData {
            return Err(DeletionFailure::InvalidCommand);
        }
        source.check(command, deadline)?;
        tokio::time::timeout_at(deadline, self.round(command, source, deadline))
            .await
            .map_err(|_| DeletionFailure::Unavailable)?
    }
}

async fn remaining_in(c: &mut PgConnection, user: Uuid) -> Result<bool, DeletionFailure> {
    sqlx::query_scalar("select exists(select 1 from public.concert_media where owner_id=$1 and (state<>'DELETED' or staged is not null or sanitized is not null or width is not null or height is not null or target_id is not null)) or exists(select 1 from public.concert_meeting_beacon_photos where owner_id=$1)")
        .bind(user).fetch_one(c).await.map_err(db)
}
// A repeatable snapshot taken before the user lock wait could miss a committed
// fence or producer row. Do not silently downgrade caller isolation or certify
// absence from such a snapshot. Every post-lock query must see current commits.
async fn require_current_snapshot_in(c: &mut PgConnection) -> Result<(), DeletionFailure> {
    let isolation: String = sqlx::query_scalar("show transaction_isolation")
        .fetch_one(c)
        .await
        .map_err(db)?;
    if isolation != "read committed" {
        return Err(DeletionFailure::Unavailable);
    }
    Ok(())
}
fn db(_: sqlx::Error) -> DeletionFailure {
    DeletionFailure::Unavailable
}
