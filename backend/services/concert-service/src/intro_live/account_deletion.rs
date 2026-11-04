//! BE42 INTRO_TEXT owner effect. The aggregate alone acknowledges PrivateData.
use crate::{
    account_private_data::{
        PrivateDataCategory, PrivateDataCategoryOwner, PrivateDataCategoryOwners,
        PrivateDataCategoryReceipt, CATEGORY_VERSION,
    },
    adapters::out::repository::intro::erase_account_page_in,
    world::account_deletion::{
        establish_fence_in, verify_source, DeletionAuthority, DeletionFailure,
        VerifiedDeletionSource, WorldDeletionCommand, WorldDeletionTarget,
    },
};
use async_trait::async_trait;
use sqlx::{PgConnection, PgPool, Row};
use std::sync::Arc;
use tokio::time::Instant;

pub const INTRO_DELETION_PAGE: i64 = 128;

/// A Social-owner read on the SAME transaction, never a synthetic BE15 edit.
/// Missing/nonterminal consent withdrawal is retryable and cannot certify erasure.
#[async_trait]
pub trait IntroConsentDeletionEvidence: Send + Sync {
    async fn withdrawn_in(
        &self,
        connection: &mut PgConnection,
        command: &WorldDeletionCommand,
        source: &VerifiedDeletionSource,
        deadline: Instant,
    ) -> Result<bool, DeletionFailure>;
}

/// Frozen read contract supplied by the Sharing owner (4592a705). Its terminal
/// receipt includes actual consent withdrawal AND audience publication. This
/// read grants no new consent and does not mutate another owner's state.
pub struct PgIntroConsentDeletionEvidence;
#[async_trait]
impl IntroConsentDeletionEvidence for PgIntroConsentDeletionEvidence {
    async fn withdrawn_in(
        &self,
        c: &mut PgConnection,
        command: &WorldDeletionCommand,
        source: &VerifiedDeletionSource,
        deadline: Instant,
    ) -> Result<bool, DeletionFailure> {
        source.check(command, deadline)?;
        let mut sharing = command.clone();
        sharing.target = WorldDeletionTarget::Sharing;
        let row = sqlx::query("select d.* from public.concert_social_account_deletions d join public.concert_world_account_deletion_fences f using(user_id,saga_id) where d.saga_id=$1 and d.user_id=$2 and f.identity_binding=$3 and f.fence_revision=1 for share of d,f")
            .bind(command.saga_id).bind(command.user_id).bind(command.identity_binding()?.as_slice()).fetch_optional(&mut *c).await.map_err(db)?;
        let Some(row) = row else {
            return Ok(false);
        };
        let sharing_source: Vec<u8> = row.try_get("source_binding").map_err(db)?;
        if row.try_get::<uuid::Uuid, _>("actor_id").map_err(db)? != command.actor_id
            || row.try_get::<String, _>("target").map_err(db)? != "SHARING"
            || row.try_get::<i64, _>("retention_seconds").map_err(db)?
                != command.retention_until.timestamp()
            || row.try_get::<i32, _>("retention_nanos").map_err(db)?
                != command.retention_until.timestamp_subsec_nanos() as i32
            || row.try_get::<Vec<u8>, _>("identity_binding").map_err(db)?
                != sharing.identity_binding()?
            || row.try_get::<Vec<u8>, _>("command_binding").map_err(db)? != sharing.binding()?
            || row.try_get::<i64, _>("fence_revision").map_err(db)? != 1
            || sharing_source.len() != 32
            || sharing_source.iter().all(|b| *b == 0)
            || row
                .try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("completed_at")
                .map_err(db)?
                .is_none()
        {
            return Ok(false);
        }
        let withdrawn: bool = sqlx::query_scalar("select not exists(select 1 from public.concert_social_deletion_audience where saga_id=$1 and published_at is null) and not exists(select 1 from public.concert_social_requests where (requester_id=$2 or recipient_id=$2) and state in ('PENDING','APPROVED')) and not exists(select 1 from public.concert_social_grants g join public.concert_social_requests r using(event_id,request_id) where (r.requester_id=$2 or r.recipient_id=$2) and g.state='ACTIVE')")
            .bind(command.saga_id).bind(command.user_id).fetch_one(&mut *c).await.map_err(db)?;
        source.check(command, deadline)?;
        Ok(withdrawn)
    }
}

pub struct IntroAccountDeletion {
    pool: PgPool,
    authority: Arc<dyn DeletionAuthority>,
    consent: Arc<dyn IntroConsentDeletionEvidence>,
}
impl IntroAccountDeletion {
    /// Composition MUST supply the actual canonical verifier and Social owner.
    pub fn new(
        pool: PgPool,
        authority: Arc<dyn DeletionAuthority>,
        consent: Arc<dyn IntroConsentDeletionEvidence>,
    ) -> Self {
        Self {
            pool,
            authority,
            consent,
        }
    }
    pub fn category_owners(self) -> PrivateDataCategoryOwners {
        PrivateDataCategoryOwners::new([(
            PrivateDataCategory::IntroText,
            Arc::new(self) as Arc<dyn PrivateDataCategoryOwner>,
        )])
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
            return Err(DeletionFailure::InvalidCommand);
        }
        source.check(command, deadline)
    }
    async fn page(
        &self,
        command: &WorldDeletionCommand,
        source: &VerifiedDeletionSource,
        deadline: Instant,
    ) -> Result<PrivateDataCategoryReceipt, DeletionFailure> {
        self.recheck(command, source, deadline).await?;
        let mut tx = self.pool.begin().await.map_err(db)?;
        // This acquires the canonical user lock BEFORE receipt/event/Intro rows.
        let fence = establish_fence_in(&mut tx, command, source).await?;
        sqlx::query("insert into public.concert_intro_account_deletions(user_id,saga_id,actor_id,retention_seconds,retention_nanos,identity_binding,command_binding,source_binding,fence_revision) values($1,$2,$3,$4,$5,$6,$7,$8,$9) on conflict(user_id) do nothing")
            .bind(command.user_id).bind(command.saga_id).bind(command.actor_id)
            .bind(command.retention_until.timestamp()).bind(command.retention_until.timestamp_subsec_nanos() as i32)
            .bind(command.identity_binding()?.as_slice()).bind(command.binding()?.as_slice())
            .bind(source.source_binding().as_slice()).bind(fence.revision as i64)
            .execute(&mut *tx).await.map_err(db)?;
        let row = sqlx::query(
            "select * from public.concert_intro_account_deletions where user_id=$1 for update",
        )
        .bind(command.user_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(db)?;
        if row.try_get::<uuid::Uuid, _>("saga_id").map_err(db)? != command.saga_id
            || row.try_get::<uuid::Uuid, _>("actor_id").map_err(db)? != command.actor_id
            || row.try_get::<i64, _>("retention_seconds").map_err(db)?
                != command.retention_until.timestamp()
            || row.try_get::<i32, _>("retention_nanos").map_err(db)?
                != command.retention_until.timestamp_subsec_nanos() as i32
            || row.try_get::<Vec<u8>, _>("command_binding").map_err(db)? != command.binding()?
            || row.try_get::<Vec<u8>, _>("identity_binding").map_err(db)?
                != command.identity_binding()?
            || row.try_get::<Vec<u8>, _>("source_binding").map_err(db)? != *source.source_binding()
            || row.try_get::<i64, _>("fence_revision").map_err(db)? != fence.revision as i64
            || row.try_get::<i32, _>("category_version").map_err(db)? != CATEGORY_VERSION
        {
            return Err(DeletionFailure::InvalidCommand);
        }
        let mut revision = row.try_get::<i64, _>("progress_revision").map_err(db)?;
        let mut complete = row
            .try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("completed_at")
            .map_err(db)?
            .is_some();
        if !complete {
            // Conservative mapping of the ORIGINAL monotonic remainder to DB time.
            let sent = Instant::now();
            let now: chrono::DateTime<chrono::Utc> = sqlx::query_scalar("select clock_timestamp()")
                .fetch_one(&mut *tx)
                .await
                .map_err(db)?;
            let received = Instant::now();
            let remaining = deadline
                .saturating_duration_since(received)
                .checked_sub(received.saturating_duration_since(sent))
                .ok_or(DeletionFailure::Unavailable)?;
            let until = now + chrono::Duration::microseconds(remaining.as_micros() as i64);
            sqlx::query("update public.concert_intro_account_deletions set active_xid=pg_current_xact_id()::text,authority_until=$2 where user_id=$1")
                .bind(command.user_id).bind(until).execute(&mut *tx).await.map_err(db)?;
        }
        if !self
            .consent
            .withdrawn_in(&mut tx, command, source, deadline)
            .await?
        {
            self.recheck(command, source, deadline).await?;
            tx.commit().await.map_err(db)?;
            source.check(command, deadline)?;
            return Ok(category_receipt(command, source, 0, false));
        }
        if !complete {
            let last: Option<uuid::Uuid> = row.try_get("last_event").map_err(db)?;
            let (next, removed) = erase_account_page_in(
                &mut tx,
                command,
                source,
                deadline,
                last,
                INTRO_DELETION_PAGE,
            )
            .await?;
            complete = !sqlx::query_scalar::<_, bool>(
                "select exists(select 1 from public.concert_intros where owner_id=$1)",
            )
            .bind(command.user_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(db)?;
            revision = revision
                .checked_add(1)
                .ok_or(DeletionFailure::Unavailable)?;
            sqlx::query("update public.concert_intro_account_deletions set last_event=coalesce($2,last_event),deleted_intros=deleted_intros+$3,progress_revision=$4,completed_at=case when $5 then clock_timestamp() else null end where user_id=$1")
                .bind(command.user_id).bind(next).bind(removed).bind(revision).bind(complete)
                .execute(&mut *tx).await.map_err(db)?;
        }
        // Terminal replay never renews or mutates its immutable effect receipt.
        if complete
            && (revision <= 0
                || sqlx::query_scalar::<_, bool>(
                    "select exists(select 1 from public.concert_intros where owner_id=$1)",
                )
                .bind(command.user_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(db)?)
        {
            return Err(DeletionFailure::Unavailable);
        }
        // Recheck even on terminal replay. Missing/rebound Sharing evidence is
        // retryable, never a positive category acknowledgement or Social write.
        if !self
            .consent
            .withdrawn_in(&mut tx, command, source, deadline)
            .await?
        {
            tx.rollback().await.map_err(db)?;
            self.recheck(command, source, deadline).await?;
            return Ok(category_receipt(command, source, 0, false));
        }
        self.recheck(command, source, deadline).await?;
        tx.commit().await.map_err(db)?;
        source.check(command, deadline)?;
        Ok(category_receipt(
            command,
            source,
            if complete { revision as u64 } else { 0 },
            complete,
        ))
    }
}
#[async_trait]
impl PrivateDataCategoryOwner for IntroAccountDeletion {
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
        tokio::time::timeout_at(deadline, self.page(command, source, deadline))
            .await
            .map_err(|_| DeletionFailure::Unavailable)?
    }
}
fn db(_: sqlx::Error) -> DeletionFailure {
    DeletionFailure::Unavailable
}
fn category_receipt(
    command: &WorldDeletionCommand,
    source: &VerifiedDeletionSource,
    revision: u64,
    complete: bool,
) -> PrivateDataCategoryReceipt {
    PrivateDataCategoryReceipt {
        command: command.clone(),
        category: PrivateDataCategory::IntroText,
        version: CATEGORY_VERSION,
        source_binding: *source.source_binding(),
        progress_revision: revision,
        complete,
    }
}
