//! BE42 PrivateData aggregate coordinator.
//!
//! The core leaf is one category, never a whole PrivateData acknowledgement.
//! This coordinator binds a closed set of Concert-owned categories to the one
//! canonical deletion command and records only exact, verified owner progress.

use super::{PrivateDataCoreAcknowledgement, PrivateDataCoreDeletion};
use crate::world::account_deletion::{
    bound_transaction_in, check_deadline, establish_fence_in, verify_source, DeletionAuthority,
    DeletionFailure, VerifiedDeletionSource, WorldDeletionCommand, WorldDeletionTarget,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Row, Transaction};
use std::{collections::HashMap, sync::Arc};
use tokio::time::Instant;
use uuid::Uuid;

pub const CATEGORY_PAGE: usize = 32;
pub const CATEGORY_VERSION: i32 = 1;

/// Every Concert-owned category required before a whole PrivateData receipt may
/// be issued. External Auth/Profile/Post/Friend/Notification/Sharing owners are
/// separate saga targets and are deliberately not represented as local success.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PrivateDataCategory {
    CorePreferencesAndAvatar,
    IntroText,
    AgeClaims,
    MediaAndMeetingPhotos,
    BeaconsAndRally,
    PartyAndGiftInput,
    MuralPrivateMetadata,
}

impl PrivateDataCategory {
    pub const ALL: [Self; 7] = [
        Self::CorePreferencesAndAvatar,
        Self::IntroText,
        Self::AgeClaims,
        Self::MediaAndMeetingPhotos,
        Self::BeaconsAndRally,
        Self::PartyAndGiftInput,
        Self::MuralPrivateMetadata,
    ];
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CorePreferencesAndAvatar => "CORE_PREFERENCES_AND_AVATAR",
            Self::IntroText => "INTRO_TEXT",
            Self::AgeClaims => "AGE_CLAIMS",
            Self::MediaAndMeetingPhotos => "MEDIA_AND_MEETING_PHOTOS",
            Self::BeaconsAndRally => "BEACONS_AND_RALLY",
            Self::PartyAndGiftInput => "PARTY_AND_GIFT_INPUT",
            Self::MuralPrivateMetadata => "MURAL_PRIVATE_METADATA",
        }
    }
}

impl std::fmt::Display for PrivateDataCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Owner result for one category. A category may advance through bounded
/// progress, but only an exact receipt may certify that category complete.
#[derive(Clone, PartialEq, Eq)]
pub struct PrivateDataCategoryReceipt {
    pub command: WorldDeletionCommand,
    pub category: PrivateDataCategory,
    pub version: i32,
    pub source_binding: [u8; 32],
    pub progress_revision: u64,
    pub complete: bool,
}

impl std::fmt::Debug for PrivateDataCategoryReceipt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PrivateDataCategoryReceipt([REDACTED])")
    }
}

impl PrivateDataCategoryReceipt {
    /// Progress and terminal receipts share the exact command, category and
    /// source binding. Only `COMPLETE` requires a positive owner revision.
    pub fn verifies(&self, command: &WorldDeletionCommand, source: &[u8; 32]) -> bool {
        self.command == *command
            && self.version == CATEGORY_VERSION
            && self.source_binding == *source
            && (self.complete && self.progress_revision > 0
                || !self.complete && self.progress_revision == 0)
    }
}

#[async_trait]
pub trait PrivateDataCategoryOwner: Send + Sync {
    async fn apply_category(
        &self,
        command: &WorldDeletionCommand,
        source: &VerifiedDeletionSource,
        deadline: Instant,
    ) -> Result<PrivateDataCategoryReceipt, DeletionFailure>;
}

/// One injected owner per closed Concert category. Missing entries fail closed;
/// this type never falls back to an empty-success implementation.
#[derive(Clone, Default)]
pub struct PrivateDataCategoryOwners {
    owners: HashMap<PrivateDataCategory, Arc<dyn PrivateDataCategoryOwner>>,
}

/// Test-only typed protocol fixture. It never performs a category effect and
/// must not be composed into a production registry.
#[derive(Clone)]
pub struct FixtureCategoryOwner {
    category: PrivateDataCategory,
}

impl FixtureCategoryOwner {
    pub fn new(category: PrivateDataCategory) -> Self {
        Self { category }
    }
}

#[async_trait]
impl PrivateDataCategoryOwner for FixtureCategoryOwner {
    async fn apply_category(
        &self,
        command: &WorldDeletionCommand,
        source: &VerifiedDeletionSource,
        deadline: Instant,
    ) -> Result<PrivateDataCategoryReceipt, DeletionFailure> {
        source.check(command, deadline)?;
        Ok(PrivateDataCategoryReceipt {
            command: command.clone(),
            category: self.category,
            version: CATEGORY_VERSION,
            source_binding: *source.source_binding(),
            progress_revision: 1,
            complete: true,
        })
    }
}

impl PrivateDataCategoryOwners {
    pub fn new(
        owners: impl IntoIterator<Item = (PrivateDataCategory, Arc<dyn PrivateDataCategoryOwner>)>,
    ) -> Self {
        Self {
            owners: owners.into_iter().collect(),
        }
    }
    pub fn core_only(core: Arc<dyn PrivateDataCategoryOwner>) -> Self {
        Self::new([(PrivateDataCategory::CorePreferencesAndAvatar, core)])
    }
    /// The empty registry used by protocol fixtures and by explicit production
    /// composition before an owner is available.
    pub fn empty() -> Self {
        Self::default()
    }
    /// Extend a registry with explicit owners. Duplicate category keys are
    /// rejected so neither a test nor a future composition can silently replace
    /// the actual core owner or claim a category twice.
    pub fn add_owners(
        mut self,
        owners: impl IntoIterator<Item = (PrivateDataCategory, Arc<dyn PrivateDataCategoryOwner>)>,
    ) -> Result<Self, DeletionFailure> {
        for (category, owner) in owners {
            if self.owners.insert(category, owner).is_some() {
                return Err(DeletionFailure::InvalidCommand);
            }
        }
        Ok(self)
    }
    pub fn get(&self, category: PrivateDataCategory) -> Option<&Arc<dyn PrivateDataCategoryOwner>> {
        self.owners.get(&category)
    }
    pub fn configured(&self) -> usize {
        self.owners.len()
    }
    /// Explicit production composition: the positive owner is only the actual
    /// core adapter. Other owners must be supplied once implemented; absence is
    /// represented by durable INCOMPLETE receipts rather than a local success.
    pub fn production(pool: PgPool, authority: Arc<dyn DeletionAuthority>) -> Self {
        Self::core_only(Arc::new(CorePrivateDataOwner::new(
            PrivateDataCoreDeletion::new(pool, authority),
        )))
    }
}

/// Production wrapper around the already-qualified core adapter. It still
/// consumes the canonical proof and original deadline; no authority is rebuilt.
pub struct CorePrivateDataOwner {
    core: PrivateDataCoreDeletion,
}
impl CorePrivateDataOwner {
    pub fn new(core: PrivateDataCoreDeletion) -> Self {
        Self { core }
    }
}

#[async_trait]
impl PrivateDataCategoryOwner for CorePrivateDataOwner {
    async fn apply_category(
        &self,
        command: &WorldDeletionCommand,
        source: &VerifiedDeletionSource,
        deadline: Instant,
    ) -> Result<PrivateDataCategoryReceipt, DeletionFailure> {
        source.check(command, deadline)?;
        let ack: PrivateDataCoreAcknowledgement =
            self.core.apply(command.clone(), deadline).await?;
        if !ack.verifies(command) {
            return Err(DeletionFailure::InvalidCommand);
        }
        Ok(PrivateDataCategoryReceipt {
            command: command.clone(),
            category: PrivateDataCategory::CorePreferencesAndAvatar,
            version: CATEGORY_VERSION,
            source_binding: *source.source_binding(),
            progress_revision: 1,
            complete: true,
        })
    }
}

/// Positive receipt available only when all categories are durable complete.
#[derive(Clone, PartialEq, Eq)]
pub struct PrivateDataAggregateAcknowledgement {
    pub command: WorldDeletionCommand,
    pub fence_revision: u64,
    pub completed_categories: usize,
}
impl std::fmt::Debug for PrivateDataAggregateAcknowledgement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PrivateDataAggregateAcknowledgement([REDACTED])")
    }
}
impl PrivateDataAggregateAcknowledgement {
    pub fn verifies(&self, command: &WorldDeletionCommand) -> bool {
        self.fence_revision > 0
            && &self.command == command
            && self.completed_categories == PrivateDataCategory::ALL.len()
    }
}

/// Durable, retryable state retained after a missing or partial owner round.
#[derive(Clone, PartialEq, Eq)]
pub struct PrivateDataAggregateProgress {
    pub command: WorldDeletionCommand,
    pub fence_revision: u64,
    pub completed: Vec<PrivateDataCategory>,
    pub missing: Vec<PrivateDataCategory>,
    pub cursor: usize,
}
impl std::fmt::Debug for PrivateDataAggregateProgress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PrivateDataAggregateProgress([REDACTED])")
    }
}

pub struct PrivateDataAggregateCoordinator {
    pool: PgPool,
    authority: Arc<dyn DeletionAuthority>,
    owners: PrivateDataCategoryOwners,
}
impl PrivateDataAggregateCoordinator {
    pub fn new(
        pool: PgPool,
        authority: Arc<dyn DeletionAuthority>,
        owners: PrivateDataCategoryOwners,
    ) -> Self {
        Self {
            pool,
            authority,
            owners,
        }
    }

    /// One bounded round. The returned status is only `Ok` for a full target
    /// receipt. A missing category leaves a durable `INCOMPLETE` receipt and
    /// returns `Incomplete`; callers use the status read for precise progress.
    pub async fn apply(
        &self,
        command: WorldDeletionCommand,
        original_deadline: Instant,
    ) -> Result<PrivateDataAggregateAcknowledgement, DeletionFailure> {
        command.validate()?;
        if command.target != WorldDeletionTarget::PrivateData {
            return Err(DeletionFailure::InvalidCommand);
        }
        check_deadline(original_deadline)?;
        tokio::time::timeout_at(original_deadline, self.round(command, original_deadline))
            .await
            .map_err(|_| DeletionFailure::Unavailable)?
    }

    async fn round(
        &self,
        command: WorldDeletionCommand,
        deadline: Instant,
    ) -> Result<PrivateDataAggregateAcknowledgement, DeletionFailure> {
        let source = verify_source(self.authority.as_ref(), &command, deadline).await?;
        // PrivateData is the first owner for this target in a fresh saga. Its
        // aggregate establishes the single common fence through the shared
        // helper; the core leaf then verifies the already-established identity.
        let mut fence_tx = self.pool.begin().await.map_err(db)?;
        let fence = establish_fence_in(&mut fence_tx, &command, &source).await?;
        fence_tx.commit().await.map_err(db)?;
        let mut tx = self.pool.begin().await.map_err(db)?;
        let persisted_fence = user_fence(&mut tx, &command, &source).await?;
        if persisted_fence != fence.revision {
            return Err(DeletionFailure::InvalidCommand);
        }
        insert_aggregate(&mut tx, &command, &source, fence.revision).await?;
        let row = exact_aggregate(&mut tx, &command, &source).await?;
        let stored_fence = row.get::<i64, _>("fence_revision");
        if stored_fence as u64 != fence.revision {
            return Err(DeletionFailure::InvalidCommand);
        }
        if !category_integrity(&mut tx, &command, &source).await? {
            return Err(DeletionFailure::InvalidCommand);
        }
        if !category_pair_integrity(&mut tx, &command, &source).await? {
            return Err(DeletionFailure::InvalidCommand);
        }
        if row
            .get::<Option<DateTime<Utc>>, _>("completed_at")
            .is_some()
        {
            if category_count(&mut tx, &command, &source).await?
                != PrivateDataCategory::ALL.len() as i64
            {
                return Err(DeletionFailure::InvalidCommand);
            }
            // A stored completion is not reusable authority. Reauthenticate
            // the exact command and source before replaying the whole ACK.
            let current = verify_source(self.authority.as_ref(), &command, deadline).await?;
            if current.source_binding() != source.source_binding() {
                return Err(DeletionFailure::Unavailable);
            }
            source.check(&command, deadline)?;
            tx.commit().await.map_err(db)?;
            return Ok(PrivateDataAggregateAcknowledgement {
                command,
                fence_revision: checked_u64(stored_fence)?,
                completed_categories: PrivateDataCategory::ALL.len(),
            });
        }
        let completed = completed_categories(&mut tx, &command, &source).await?;
        let categories = PrivateDataCategory::ALL
            .iter()
            .copied()
            // The cursor records bounded forward progress, but it is never a
            // tombstone for a missing owner. Every incomplete category remains
            // eligible on a later round so a real owner configured after the
            // first attempt can supply its one terminal receipt.
            .filter(|category| !completed.contains(category))
            .take(CATEGORY_PAGE)
            .collect::<Vec<_>>();
        // Do not hold a receipt lock across owner calls or a shared core lock.
        tx.commit().await.map_err(db)?;
        let mut missing_core = false;
        for category in categories {
            source.check(&command, deadline)?;
            let Some(owner) = self.owners.get(category) else {
                self.record_missing(&command, &source, category, ordinal(category), deadline)
                    .await?;
                self.advance_cursor(&command, &source, category, deadline)
                    .await?;
                if category == PrivateDataCategory::CorePreferencesAndAvatar {
                    missing_core = true;
                }
                continue;
            };
            let receipt = owner.apply_category(&command, &source, deadline).await?;
            if receipt.category != category || !receipt.verifies(&command, source.source_binding())
            {
                return Err(DeletionFailure::InvalidCommand);
            }
            self.record_receipt(&command, &source, &receipt, ordinal(category), deadline)
                .await?;
            self.advance_cursor(&command, &source, category, deadline)
                .await?;
        }
        source.check(&command, deadline)?;
        let mut tx = self.pool.begin().await.map_err(db)?;
        let fence = user_fence(&mut tx, &command, &source).await?;
        exact_aggregate(&mut tx, &command, &source).await?;
        let complete = category_count(&mut tx, &command, &source).await?
            == PrivateDataCategory::ALL.len() as i64;
        if complete {
            sqlx::query("update public.concert_private_data_owner_progress set next_category=7,completed_at=clock_timestamp(),progress_revision=progress_revision+1,updated_at=clock_timestamp() where user_id=$1")
                .bind(command.user_id).execute(&mut *tx).await.map_err(db)?;
        }
        source.check(&command, deadline)?;
        let current = verify_source(self.authority.as_ref(), &command, deadline).await?;
        if current.source_binding() != source.source_binding() {
            return Err(DeletionFailure::Unavailable);
        }
        source.check(&command, deadline)?;
        tx.commit().await.map_err(db)?;
        if complete {
            Ok(PrivateDataAggregateAcknowledgement {
                command,
                fence_revision: checked_u64(stored_fence)?,
                completed_categories: PrivateDataCategory::ALL.len(),
            })
        } else {
            Err(if missing_core {
                DeletionFailure::Unavailable
            } else {
                DeletionFailure::Incomplete
            })
        }
    }

    async fn advance_cursor(
        &self,
        command: &WorldDeletionCommand,
        source: &VerifiedDeletionSource,
        category: PrivateDataCategory,
        deadline: Instant,
    ) -> Result<(), DeletionFailure> {
        let mut tx = self.pool.begin().await.map_err(db)?;
        user_fence(&mut tx, command, source).await?;
        exact_aggregate(&mut tx, command, source).await?;
        source.check(command, deadline)?;
        sqlx::query("update public.concert_private_data_owner_progress set next_category=greatest(next_category,$2),progress_revision=progress_revision+1,updated_at=clock_timestamp() where user_id=$1")
            .bind(command.user_id).bind(ordinal(category)+1)
            .execute(&mut *tx).await.map_err(db)?;
        let current = verify_source(self.authority.as_ref(), command, deadline).await?;
        if current.source_binding() != source.source_binding() {
            return Err(DeletionFailure::Unavailable);
        }
        source.check(command, deadline)?;
        tx.commit().await.map_err(db)
    }

    async fn record_missing(
        &self,
        command: &WorldDeletionCommand,
        source: &VerifiedDeletionSource,
        category: PrivateDataCategory,
        ordinal: i32,
        deadline: Instant,
    ) -> Result<(), DeletionFailure> {
        let mut tx = self.pool.begin().await.map_err(db)?;
        let fence = user_fence(&mut tx, command, source).await?;
        exact_aggregate(&mut tx, command, source).await?;
        let inserted = sqlx::query("insert into public.concert_private_data_category_progress(user_id,saga_id,category,category_version,command_binding,source_binding,fence_revision,ordinal,state) values($1,$2,$3,$4,$5,$6,$7,$8,'INCOMPLETE') on conflict do nothing")
            .bind(command.user_id).bind(command.saga_id).bind(category.as_str())
            .bind(CATEGORY_VERSION).bind(command.binding()?.as_slice())
            .bind(source.source_binding().as_slice()).bind(fence as i64).bind(ordinal)
            .execute(&mut *tx).await.map_err(db)?;
        if inserted.rows_affected() == 0 {
            exact_category(&mut tx, command, source, category).await?;
        }
        source.check(command, deadline)?;
        let current = verify_source(self.authority.as_ref(), command, deadline).await?;
        if current.source_binding() != source.source_binding() {
            return Err(DeletionFailure::Unavailable);
        }
        source.check(command, deadline)?;
        tx.commit().await.map_err(db)
    }

    async fn record_receipt(
        &self,
        command: &WorldDeletionCommand,
        source: &VerifiedDeletionSource,
        receipt: &PrivateDataCategoryReceipt,
        ordinal: i32,
        deadline: Instant,
    ) -> Result<(), DeletionFailure> {
        let mut tx = self.pool.begin().await.map_err(db)?;
        let fence = user_fence(&mut tx, command, source).await?;
        exact_aggregate(&mut tx, command, source).await?;
        // An owner result is a terminal receipt. Intermediate owner progress is
        // represented by a durable INCOMPLETE row with no owner revision; a
        // non-terminal result must never be inserted as a live new row.
        let state = if receipt.complete {
            "COMPLETE"
        } else {
            "INCOMPLETE"
        };
        // An incomplete return is indistinguishable from a retryable missing
        // owner: persist it as INCOMPLETE but do not certify an owner revision.
        let inserted = sqlx::query("insert into public.concert_private_data_category_progress(user_id,saga_id,category,category_version,command_binding,source_binding,fence_revision,ordinal,owner_revision,state,completed_at) values($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,case when $10='COMPLETE' then clock_timestamp() else null end) on conflict do nothing")
            .bind(command.user_id).bind(command.saga_id).bind(receipt.category.as_str())
            .bind(receipt.version).bind(command.binding()?.as_slice())
            .bind(source.source_binding().as_slice()).bind(fence as i64)
            .bind(ordinal)
            .bind(receipt.complete.then_some(receipt.progress_revision as i64))
            .bind(state)
            .execute(&mut *tx).await.map_err(db)?;
        if inserted.rows_affected() == 0 {
            let existing = exact_category(&mut tx, command, source, receipt.category).await?;
            if existing.get::<i64, _>("ordinal") != ordinal as i64 {
                return Err(DeletionFailure::InvalidCommand);
            }
            if existing.get::<String, _>("state") == "INCOMPLETE" {
                if !receipt.complete {
                    // A repeated exact nonterminal owner result is progress,
                    // not a forged terminal receipt. Keep the durable row
                    // retryable without inventing an owner revision.
                    if existing.get::<Option<i64>, _>("owner_revision").is_some() {
                        return Err(DeletionFailure::InvalidCommand);
                    }
                } else {
                    // The durable missing row is the only incomplete row that can
                    // become complete. Its owner revision must advance exactly once
                    // from NULL to a verified positive owner receipt. All bindings,
                    // ordinal, version and complete-row identity are immutable.
                    let changed = sqlx::query("update public.concert_private_data_category_progress set owner_revision=$5,state='COMPLETE',completed_at=clock_timestamp() where user_id=$1 and saga_id=$2 and category=$3 and category_version=$4 and command_binding=$6 and source_binding=$7 and fence_revision=$8 and ordinal=$9 and owner_revision is null and state='INCOMPLETE'")
                        .bind(command.user_id).bind(command.saga_id).bind(receipt.category.as_str())
                        .bind(receipt.version).bind(receipt.progress_revision as i64)
                        .bind(command.binding()?.as_slice()).bind(source.source_binding().as_slice())
                        .bind(existing.get::<i64, _>("fence_revision")).bind(ordinal)
                        .execute(&mut *tx).await.map_err(db)?;
                    if changed.rows_affected() != 1 {
                        return Err(DeletionFailure::InvalidCommand);
                    }
                }
            } else if existing.get::<Option<i64>, _>("owner_revision")
                != Some(receipt.progress_revision as i64)
                || existing.get::<String, _>("state") != "COMPLETE"
            {
                return Err(DeletionFailure::InvalidCommand);
            }
        }
        source.check(command, deadline)?;
        let current = verify_source(self.authority.as_ref(), command, deadline).await?;
        if current.source_binding() != source.source_binding() {
            return Err(DeletionFailure::Unavailable);
        }
        source.check(command, deadline)?;
        tx.commit().await.map_err(db)
    }

    pub async fn progress(
        &self,
        user_id: Uuid,
    ) -> Result<Option<PrivateDataAggregateProgress>, DeletionFailure> {
        let Some(row) = sqlx::query("select saga_id,actor_id,retention_seconds,retention_nanos,fence_revision,command_binding,source_binding,identity_binding,completed_at from public.concert_private_data_owner_progress where user_id=$1")
            .bind(user_id).fetch_optional(&self.pool).await.map_err(db)? else { return Ok(None) };
        let saga_id: Uuid = row.get("saga_id");
        let actor_id: Uuid = row.get("actor_id");
        let command_binding: Vec<u8> = row.get("command_binding");
        let source_binding: Vec<u8> = row.get("source_binding");
        let _identity_binding: Vec<u8> = row.get("identity_binding");
        let retention_until = DateTime::<Utc>::from_timestamp(
            row.get("retention_seconds"),
            row.get::<i32, _>("retention_nanos") as u32,
        )
        .ok_or(DeletionFailure::InvalidCommand)?;
        let completed_names: Vec<String> = sqlx::query_scalar("select category from public.concert_private_data_category_progress where user_id=$1 and saga_id=$2 and command_binding=$3 and source_binding=$4 and state='COMPLETE' order by ordinal")
            .bind(user_id).bind(saga_id).bind(command_binding).bind(source_binding).fetch_all(&self.pool).await.map_err(db)?;
        let completed = completed_names
            .iter()
            .filter_map(|name| {
                PrivateDataCategory::ALL
                    .into_iter()
                    .find(|c| c.as_str() == name)
            })
            .collect::<Vec<_>>();
        let missing = PrivateDataCategory::ALL
            .into_iter()
            .filter(|c| !completed.contains(c))
            .collect();
        let cursor: i32 = sqlx::query_scalar(
            "select next_category from public.concert_private_data_owner_progress where user_id=$1",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await
        .map_err(db)?;
        let completed_at = row.get::<Option<DateTime<Utc>>, _>("completed_at");
        if completed_at.is_some() && completed.len() != PrivateDataCategory::ALL.len() {
            return Err(DeletionFailure::InvalidCommand);
        }
        let command = WorldDeletionCommand {
            saga_id,
            user_id,
            target: WorldDeletionTarget::PrivateData,
            actor_id,
            retention_until,
        };
        // Stored metadata never creates authority. A rebound or unavailable
        // canonical source cannot be observed as retryable progress. This read
        // has no caller-supplied deadline, so its verification gets the same
        // bounded window used by one owner dispatch.
        let deadline = Instant::now() + std::time::Duration::from_secs(4);
        let current = verify_source(self.authority.as_ref(), &command, deadline).await?;
        if current.source_binding() != source_binding.as_slice() {
            return Err(DeletionFailure::InvalidCommand);
        }
        Ok(Some(PrivateDataAggregateProgress {
            command,
            fence_revision: checked_u64(row.get::<i64, _>("fence_revision"))?,
            completed,
            missing,
            cursor: cursor as usize,
        }))
    }
}

async fn insert_aggregate(
    tx: &mut Transaction<'_, Postgres>,
    c: &WorldDeletionCommand,
    source: &VerifiedDeletionSource,
    revision: u64,
) -> Result<(), DeletionFailure> {
    bound_transaction_in(tx, source.original_deadline()).await?;
    sqlx::query("insert into public.concert_private_data_owner_progress(user_id,saga_id,actor_id,target,retention_seconds,retention_nanos,identity_binding,command_binding,source_binding,fence_revision) values($1,$2,$3,'PRIVATE_DATA',$4,$5,$6,$7,$8,$9) on conflict do nothing")
        .bind(c.user_id).bind(c.saga_id).bind(c.actor_id).bind(c.retention_until.timestamp())
        .bind(c.retention_until.timestamp_subsec_nanos() as i32)
        .bind(c.identity_binding()?.as_slice()).bind(c.binding()?.as_slice())
        .bind(source.source_binding().as_slice()).bind(revision as i64)
        .execute(&mut **tx).await.map_err(db)?;
    Ok(())
}
/// Take the shared user-first advisory lock and verify the already-established
/// common fence. Fence creation is a separate committed step so the long owner
/// call never holds this aggregate transaction open.
async fn user_fence(
    tx: &mut Transaction<'_, Postgres>,
    c: &WorldDeletionCommand,
    source: &VerifiedDeletionSource,
) -> Result<u64, DeletionFailure> {
    source.check(c, source.original_deadline())?;
    bound_transaction_in(tx, source.original_deadline()).await?;
    crate::world::account_deletion::lock_user(tx, c.user_id)
        .await
        .map_err(db)?;
    let revision: Option<i64> = sqlx::query_scalar(
        "select fence_revision from public.concert_world_account_deletion_fences where user_id=$1 and saga_id=$2 and identity_binding=$3",
    )
    .bind(c.user_id)
    .bind(c.saga_id)
    .bind(c.identity_binding()?.as_slice())
    .fetch_optional(&mut **tx)
    .await
    .map_err(db)?;
    let revision = revision.ok_or(DeletionFailure::InvalidCommand)?;
    if revision <= 0 {
        return Err(DeletionFailure::InvalidCommand);
    }
    Ok(revision as u64)
}
async fn exact_aggregate(
    tx: &mut Transaction<'_, Postgres>,
    c: &WorldDeletionCommand,
    source: &VerifiedDeletionSource,
) -> Result<sqlx::postgres::PgRow, DeletionFailure> {
    source.check(c, source.original_deadline())?;
    let row = sqlx::query(
        "select * from public.concert_private_data_owner_progress where user_id=$1 for update",
    )
    .bind(c.user_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db)?
    .ok_or(DeletionFailure::InvalidCommand)?;
    let common_fence: i64 = sqlx::query_scalar(
        "select fence_revision from public.concert_world_account_deletion_fences where user_id=$1 and saga_id=$2",
    )
    .bind(c.user_id)
    .bind(c.saga_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(db)?;
    if row.get::<Uuid, _>("saga_id") != c.saga_id
        || row.get::<Uuid, _>("actor_id") != c.actor_id
        || row.get::<String, _>("target") != "PRIVATE_DATA"
        || row.get::<i64, _>("retention_seconds") != c.retention_until.timestamp()
        || row.get::<i32, _>("retention_nanos") != c.retention_until.timestamp_subsec_nanos() as i32
        || row.get::<Vec<u8>, _>("identity_binding") != c.identity_binding()?
        || row.get::<Vec<u8>, _>("command_binding") != c.binding()?
        || row.get::<Vec<u8>, _>("source_binding") != source.source_binding().as_slice()
        || row.get::<i64, _>("fence_revision") != common_fence
        || common_fence <= 0
    {
        return Err(DeletionFailure::InvalidCommand);
    }
    Ok(row)
}
async fn category_count(
    tx: &mut Transaction<'_, Postgres>,
    c: &WorldDeletionCommand,
    source: &VerifiedDeletionSource,
) -> Result<i64, DeletionFailure> {
    sqlx::query_scalar("select count(*) from public.concert_private_data_category_progress where user_id=$1 and saga_id=$2 and command_binding=$3 and source_binding=$4 and state='COMPLETE'")
        .bind(c.user_id).bind(c.saga_id).bind(c.binding()?.as_slice()).bind(source.source_binding().as_slice())
        .fetch_one(&mut **tx).await.map_err(db)
}
/// Every persisted category row must retain the closed category-to-ordinal
/// mapping for this exact aggregate. A committed malformed row must never be
/// treated as progress, even if all seven state values say COMPLETE.
async fn category_integrity(
    tx: &mut Transaction<'_, Postgres>,
    c: &WorldDeletionCommand,
    source: &VerifiedDeletionSource,
) -> Result<bool, DeletionFailure> {
    let rows: i64 = sqlx::query_scalar("select count(*) from public.concert_private_data_category_progress where user_id=$1 and saga_id=$2 and command_binding=$3 and source_binding=$4 and category_version=$5 and ((category='CORE_PREFERENCES_AND_AVATAR' and ordinal=0) or (category='INTRO_TEXT' and ordinal=1) or (category='AGE_CLAIMS' and ordinal=2) or (category='MEDIA_AND_MEETING_PHOTOS' and ordinal=3) or (category='BEACONS_AND_RALLY' and ordinal=4) or (category='PARTY_AND_GIFT_INPUT' and ordinal=5) or (category='MURAL_PRIVATE_METADATA' and ordinal=6))")
        .bind(c.user_id).bind(c.saga_id).bind(c.binding()?.as_slice())
        .bind(source.source_binding().as_slice()).bind(CATEGORY_VERSION)
        .fetch_one(&mut **tx).await.map_err(db)?;
    let total: i64 = sqlx::query_scalar("select count(*) from public.concert_private_data_category_progress where user_id=$1 and saga_id=$2 and command_binding=$3 and source_binding=$4")
        .bind(c.user_id).bind(c.saga_id).bind(c.binding()?.as_slice())
        .bind(source.source_binding().as_slice())
        .fetch_one(&mut **tx).await.map_err(db)?;
    Ok(rows == total && rows <= PrivateDataCategory::ALL.len() as i64)
}
async fn category_pair_integrity(
    tx: &mut Transaction<'_, Postgres>,
    c: &WorldDeletionCommand,
    source: &VerifiedDeletionSource,
) -> Result<bool, DeletionFailure> {
    let pairs: Vec<(String, i32)> = sqlx::query_as("select category,ordinal from public.concert_private_data_category_progress where user_id=$1 and saga_id=$2 and command_binding=$3 and source_binding=$4")
        .bind(c.user_id).bind(c.saga_id).bind(c.binding()?.as_slice())
        .bind(source.source_binding().as_slice())
        .fetch_all(&mut **tx).await.map_err(db)?;
    if pairs.len() > PrivateDataCategory::ALL.len() {
        return Ok(false);
    }
    let mut seen = std::collections::HashSet::new();
    for (name, stored_ordinal) in pairs {
        let Some(category) = PrivateDataCategory::ALL
            .into_iter()
            .find(|category| category.as_str() == name)
        else {
            return Ok(false);
        };
        if stored_ordinal != ordinal(category) || !seen.insert(category) {
            return Ok(false);
        }
    }
    Ok(true)
}
/// The common fence revision is stored only as a positive i64. Expose it as
/// u64 through one checked conversion so a malformed persisted value cannot
/// reach a positive acknowledgement.
fn checked_u64(value: i64) -> Result<u64, DeletionFailure> {
    if value <= 0 {
        Err(DeletionFailure::InvalidCommand)
    } else {
        u64::try_from(value).map_err(|_| DeletionFailure::InvalidCommand)
    }
}
async fn completed_categories(
    tx: &mut Transaction<'_, Postgres>,
    c: &WorldDeletionCommand,
    source: &VerifiedDeletionSource,
) -> Result<Vec<PrivateDataCategory>, DeletionFailure> {
    let names: Vec<String> = sqlx::query_scalar("select category from public.concert_private_data_category_progress where user_id=$1 and saga_id=$2 and command_binding=$3 and source_binding=$4 and state='COMPLETE'")
        .bind(c.user_id).bind(c.saga_id).bind(c.binding()?.as_slice()).bind(source.source_binding().as_slice())
        .fetch_all(&mut **tx).await.map_err(db)?;
    Ok(names
        .iter()
        .filter_map(|name| {
            PrivateDataCategory::ALL
                .into_iter()
                .find(|c| c.as_str() == name)
        })
        .collect())
}
const fn ordinal(category: PrivateDataCategory) -> i32 {
    match category {
        PrivateDataCategory::CorePreferencesAndAvatar => 0,
        PrivateDataCategory::IntroText => 1,
        PrivateDataCategory::AgeClaims => 2,
        PrivateDataCategory::MediaAndMeetingPhotos => 3,
        PrivateDataCategory::BeaconsAndRally => 4,
        PrivateDataCategory::PartyAndGiftInput => 5,
        PrivateDataCategory::MuralPrivateMetadata => 6,
    }
}
async fn exact_category(
    tx: &mut Transaction<'_, Postgres>,
    c: &WorldDeletionCommand,
    source: &VerifiedDeletionSource,
    category: PrivateDataCategory,
) -> Result<sqlx::postgres::PgRow, DeletionFailure> {
    let row = sqlx::query("select * from public.concert_private_data_category_progress where user_id=$1 and category=$2 for update")
        .bind(c.user_id).bind(category.as_str()).fetch_optional(&mut **tx).await.map_err(db)?
        .ok_or(DeletionFailure::InvalidCommand)?;
    let expected_fence: i64 = sqlx::query_scalar(
        "select fence_revision from public.concert_private_data_owner_progress where user_id=$1",
    )
    .bind(c.user_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(db)?;
    if row.get::<Uuid, _>("saga_id") != c.saga_id
        || row.get::<Vec<u8>, _>("command_binding") != c.binding()?
        || row.get::<Vec<u8>, _>("source_binding") != source.source_binding().as_slice()
        || row.get::<String, _>("category") != category.as_str()
        || row.get::<i32, _>("category_version") != CATEGORY_VERSION
        || row.get::<i32, _>("ordinal") != ordinal(category)
        || row.get::<i64, _>("fence_revision") != expected_fence
        || expected_fence <= 0
    {
        return Err(DeletionFailure::InvalidCommand);
    }
    Ok(row)
}
fn db(_: sqlx::Error) -> DeletionFailure {
    DeletionFailure::Unavailable
}
