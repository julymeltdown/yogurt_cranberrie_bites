//! Durable local owner batches. This module supplies no operator/system authority.
//! Missing owners remain pending; remote mutations must use their owning service's
//! durable protocol outside this local transaction. READ_ONLY uses the retained
//! native ReadOnlyCommandDriver, never a receipt invented by this batch engine.
use crate::event_lifecycle::{
    Access, Applied, AuthorityQuery, EffectKey, FencedLifecycleAuthority, LifecycleOwners,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::{commands::CommandError, event_cleanup::*, event_lifecycle::Actor};
use concert_domain::event_lifecycle::{Job, JobState, Retention, Step};
use sqlx::{postgres::PgRow, PgConnection, PgPool, Row, TransactionManager};
use std::{collections::BTreeMap, sync::Arc};
use uuid::Uuid;

pub const MIGRATION: &str = include_str!("../../migrations/0046_event_cleanup_runtime.sql");

/// Implementations are trusted local owning adapters, not request data. They must
/// preserve actual current source/role/revocation fences on the supplied native
/// transaction. No pool acquisition, commit, detached scope or remote mutation.
#[async_trait]
pub trait LocalOwner: Send + Sync {
    fn owner(&self) -> Owner;
    async fn apply_in(
        &self,
        c: &mut PgConnection,
        request: &BatchRequest,
    ) -> Result<Outcome, CommandError>;
    /// Verify an actual durable owner receipt and current convergence/source fences.
    /// A structurally valid UUID, affected count or batch row alone is insufficient.
    async fn verify_in(
        &self,
        c: &mut PgConnection,
        request: &BatchRequest,
        receipt: Receipt,
    ) -> Result<(), CommandError>;
}
#[derive(Clone, Default)]
pub struct Owners(BTreeMap<Owner, Arc<dyn LocalOwner>>);
impl Owners {
    pub fn new(owners: Vec<Arc<dyn LocalOwner>>) -> Result<Self, CommandError> {
        let mut result = Self::default();
        for owner in owners {
            if result.0.insert(owner.owner(), owner).is_some() {
                return Err(CommandError::InvalidInput);
            }
        }
        Ok(result)
    }
    pub fn missing(&self, step: Step) -> Vec<Owner> {
        required_owners(step)
            .iter()
            .copied()
            .filter(|o| !self.0.contains_key(o))
            .collect()
    }
}
/// Metadata only. Never exposes cursors, raw owner output or private data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    pub owner: Owner,
    pub state: State,
    pub processed: u64,
}

pub struct CleanupRuntime {
    pool: PgPool,
    authority: Option<Arc<dyn FencedLifecycleAuthority>>,
    owners: Owners,
    limit: u16,
}
impl CleanupRuntime {
    pub fn new(
        pool: PgPool,
        authority: Option<Arc<dyn FencedLifecycleAuthority>>,
        owners: Owners,
        limit: u16,
    ) -> Result<Self, CommandError> {
        if !(1..=MAX_BATCH).contains(&limit) {
            return Err(CommandError::InvalidInput);
        }
        Ok(Self {
            pool,
            authority,
            owners,
            limit,
        })
    }
    /// One bounded page, independent of the retained lifecycle savepoint. Successful
    /// page effects and their cursor commit together; the parent step stays pending.
    /// Retries after process loss load the last committed cursor under the event lock.
    pub async fn run_owner(
        &self,
        actor: Actor,
        job: Uuid,
        owner: Owner,
    ) -> Result<Progress, CommandError> {
        let authority = self.authority.as_ref().ok_or(CommandError::Unavailable)?;
        let mut tx = self.pool.begin().await.map_err(db)?;
        let (step, retention) = authorize(&mut tx, authority.as_ref(), actor, job).await?;
        let scope = Scope {
            actor: actor.actor,
            event: actor.event,
            job,
            step,
            owner,
        }
        .validate()
        .map_err(invalid)?;
        if step == Step::Cleanup && now(&mut tx).await? < retention.cleanup_after {
            let processed = load(&mut tx, scope, retention)
                .await?
                .map_or(0, |p| p.processed());
            finish(&mut tx).await?;
            tx.commit().await.map_err(db)?;
            return Ok(Progress {
                owner,
                state: State::Pending,
                processed,
            });
        }
        sqlx::query("insert into concert_event_cleanup_progress(job_id,event_id,actor_id,step,owner,batch_limit) values($1,$2,$3,$4,$5,$6) on conflict(job_id,step,owner) do nothing")
            .bind(job).bind(actor.event).bind(actor.actor).bind(step.name()).bind(owner.name()).bind(self.limit as i16).execute(&mut *tx).await.map_err(db)?;
        let mut checkpoint = load(&mut tx, scope, retention)
            .await?
            .ok_or(CommandError::InvalidState)?;
        if checkpoint.request().limit() != self.limit {
            return Err(CommandError::InvalidState);
        }
        if checkpoint.state() != State::Complete {
            if let Some(adapter) = self.owners.0.get(&owner) {
                sqlx::query("savepoint cleanup_page")
                    .execute(&mut *tx)
                    .await
                    .map_err(db)?;
                let result = async {
                    let outcome = adapter.apply_in(&mut tx, checkpoint.request()).await?;
                    let receipt = match &outcome {
                        Outcome::Pending => None,
                        Outcome::Page { receipt, .. } | Outcome::Converged { receipt } => {
                            Some(*receipt)
                        }
                    };
                    let input = checkpoint.request().clone();
                    if let Some(receipt) = receipt {
                        adapter.verify_in(&mut tx, &input, receipt).await?;
                    }
                    checkpoint.apply(outcome).map_err(invalid)?;
                    if let Some(receipt) = receipt {
                        append(&mut tx, &input, &checkpoint, receipt).await?;
                    } else {
                        // An unconfirmed local outcome cannot commit speculative effects.
                        sqlx::query("rollback to savepoint cleanup_page")
                            .execute(&mut *tx)
                            .await
                            .map_err(db)?;
                        mark(&mut tx, scope, State::Pending, None).await?;
                    }
                    Ok::<(), CommandError>(())
                }
                .await;
                if let Err(error) = result {
                    sqlx::query("rollback to savepoint cleanup_page")
                        .execute(&mut *tx)
                        .await
                        .map_err(db)?;
                    // Discard speculative in-memory cursor as well as owner SQL.
                    checkpoint = load(&mut tx, scope, retention)
                        .await?
                        .ok_or(CommandError::InvalidState)?;
                    let failure = match error {
                        CommandError::Denied => Failure::OwnerDenied,
                        CommandError::InvalidState => Failure::SourceChanged,
                        _ => Failure::OwnerUnavailable,
                    };
                    checkpoint.failed(failure).map_err(invalid)?;
                    mark(&mut tx, scope, State::PartialFailure, Some(failure)).await?;
                }
                sqlx::query("release savepoint cleanup_page")
                    .execute(&mut *tx)
                    .await
                    .map_err(db)?;
            } else {
                // Unknown downstream status is NOT an applied or successful effect.
                checkpoint.apply(Outcome::Pending).map_err(invalid)?;
                mark(&mut tx, scope, State::Pending, None).await?;
            }
        } else {
            // Terminal replay still needs current authority and owner confirmation.
            let adapter = self.owners.0.get(&owner).ok_or(CommandError::Unavailable)?;
            adapter
                .verify_in(
                    &mut tx,
                    checkpoint.request(),
                    checkpoint
                        .last_receipt()
                        .ok_or(CommandError::InvalidState)?,
                )
                .await?;
        }
        finish(&mut tx).await?;
        let progress = Progress {
            owner,
            state: checkpoint.state(),
            processed: checkpoint.processed(),
        };
        tx.commit().await.map_err(db)?;
        Ok(progress)
    }
    /// Safe scheduling entrypoint for Root's actual retained driver composition.
    /// The driver and batches must use the same event store, real authority and
    /// registered owners. Completed jobs are read-only replays. A parent failure
    /// requires the retained explicit RESUME command before scheduling more work.
    pub async fn tick<A: FencedLifecycleAuthority>(
        &self,
        driver: &crate::event_lifecycle::read_only::ReadOnlyCommandDriver<A, ConvergedOwners>,
        actor: Actor,
        job: Uuid,
    ) -> Result<Job, CommandError> {
        let current = driver.job(actor, job).await?;
        if matches!(
            current.state,
            JobState::Completed | JobState::PartialFailure
        ) {
            return Ok(current);
        }
        let step = current.next().ok_or(CommandError::InvalidState)?;
        let required = required_owners(step);
        if required.is_empty() {
            return Err(CommandError::InvalidState);
        }
        for owner in required {
            let progress = self.run_owner(actor, job, *owner).await?;
            if progress.state != State::Complete {
                return driver.job(actor, job).await;
            }
        }
        if self.ready(actor, job).await? {
            driver.run_next(actor, job).await
        } else {
            driver.job(actor, job).await
        }
    }

    /// A retained step may be attempted only after every required owner converged.
    /// The retained adapter below rechecks receipts in that step's actual transaction.
    pub async fn ready(&self, actor: Actor, job: Uuid) -> Result<bool, CommandError> {
        let authority = self.authority.as_ref().ok_or(CommandError::Unavailable)?;
        let mut tx = self.pool.begin().await.map_err(db)?;
        let (step, retention) = authorize(&mut tx, authority.as_ref(), actor, job).await?;
        if step == Step::Cleanup && now(&mut tx).await? < retention.cleanup_after {
            finish(&mut tx).await?;
            tx.commit().await.map_err(db)?;
            return Ok(false);
        }
        let result = verify_all(&mut tx, &self.owners, actor, job, step, retention).await?;
        finish(&mut tx).await?;
        tx.commit().await.map_err(db)?;
        Ok(result)
    }
}
/// Supply to retained ReadOnlyCommandDriver as its later-step owners. No detached
/// pool or transaction is accepted: every final receipt is verified on caller c.
#[derive(Clone)]
pub struct ConvergedOwners {
    owners: Owners,
}
impl ConvergedOwners {
    pub fn new(owners: Owners) -> Self {
        Self { owners }
    }
}
#[async_trait]
impl LifecycleOwners for ConvergedOwners {
    async fn apply_in(
        &self,
        c: &mut PgConnection,
        key: EffectKey,
        retention: Retention,
    ) -> Result<Applied, CommandError> {
        if sqlx::postgres::PgTransactionManager::get_transaction_depth(c) == 0 {
            return Err(CommandError::InvalidInput);
        }
        finish(c).await?;
        if key.step == Step::ReadOnly {
            return Err(CommandError::Unavailable);
        }
        let actor: Uuid = sqlx::query_scalar(
            "select actor_id from concert_closure_jobs where event_id=$1 and job_id=$2 for share",
        )
        .bind(key.event)
        .bind(key.job)
        .fetch_one(&mut *c)
        .await
        .map_err(db)?;
        if !verify_all(
            c,
            &self.owners,
            Actor {
                actor,
                event: key.event,
            },
            key.job,
            key.step,
            retention,
        )
        .await?
        {
            return Err(CommandError::Unavailable);
        }
        // Correlation only after actual durable owner receipts/current fences above.
        Ok(Applied {
            key,
            receipt: key.operation_id(),
            revision: 1,
        })
    }
}
async fn verify_all(
    c: &mut PgConnection,
    owners: &Owners,
    actor: Actor,
    job: Uuid,
    step: Step,
    retention: Retention,
) -> Result<bool, CommandError> {
    let required = required_owners(step);
    if required.is_empty() {
        return Ok(false);
    }
    for owner in required {
        let Some(adapter) = owners.0.get(owner) else {
            return Ok(false);
        };
        let scope = Scope {
            actor: actor.actor,
            event: actor.event,
            job,
            step,
            owner: *owner,
        };
        let Some(checkpoint) = load(c, scope, retention).await? else {
            return Ok(false);
        };
        if checkpoint.state() != State::Complete {
            return Ok(false);
        }
        adapter
            .verify_in(
                c,
                checkpoint.request(),
                checkpoint
                    .last_receipt()
                    .ok_or(CommandError::InvalidState)?,
            )
            .await?;
    }
    Ok(true)
}
fn db(_: sqlx::Error) -> CommandError {
    CommandError::Unavailable
}
fn invalid(_: impl std::fmt::Display) -> CommandError {
    CommandError::InvalidState
}
async fn now(c: &mut PgConnection) -> Result<DateTime<Utc>, CommandError> {
    sqlx::query_scalar("select clock_timestamp()")
        .fetch_one(c)
        .await
        .map_err(db)
}
async fn finish(c: &mut PgConnection) -> Result<(), CommandError> {
    let valid:bool=sqlx::query_scalar("select coalesce(clock_timestamp()<nullif(current_setting('mcr.lifecycle_authority_until',true),'')::timestamptz,false)").fetch_one(c).await.map_err(db)?;
    if valid {
        Ok(())
    } else {
        Err(CommandError::Unavailable)
    }
}
async fn authorize(
    c: &mut PgConnection,
    authority: &dyn FencedLifecycleAuthority,
    actor: Actor,
    job: Uuid,
) -> Result<(Step, Retention), CommandError> {
    if [actor.actor, actor.event, job].iter().any(Uuid::is_nil) {
        return Err(CommandError::Denied);
    }
    let phase: String =
        sqlx::query_scalar("select phase from concert_events where event_id=$1 for update")
            .bind(actor.event)
            .fetch_optional(&mut *c)
            .await
            .map_err(db)?
            .ok_or(CommandError::Denied)?;
    let query = AuthorityQuery {
        actor,
        job,
        access: Access::RunStep,
    };
    let started = now(c).await?;
    let fence = authority.check_in(c, query, started).await?;
    let checked = now(c).await?;
    if fence.query != query
        || [
            fence.auth_revision,
            fence.role_revision,
            fence.policy_revision,
        ]
        .iter()
        .any(|r| !(1..=concert_domain::MAX_REVISION).contains(r))
        || fence.checked_at < started
        || fence.checked_at > checked
        || fence.expires_at <= checked
    {
        return Err(CommandError::Unavailable);
    }
    sqlx::query("select set_config('mcr.lifecycle_authority_until',least(coalesce(nullif(current_setting('mcr.lifecycle_authority_until',true),'')::timestamptz,'infinity'::timestamptz),$1::timestamptz)::text,true)").bind(fence.expires_at).execute(&mut *c).await.map_err(db)?;
    let row = sqlx::query(
        "select * from concert_closure_jobs where event_id=$1 and job_id=$2 for update",
    )
    .bind(actor.event)
    .bind(job)
    .fetch_optional(&mut *c)
    .await
    .map_err(db)?
    .ok_or(CommandError::Denied)?;
    let projection: Job =
        serde_json::from_value(row.try_get("progress").map_err(db)?).map_err(invalid)?;
    projection.validate().map_err(invalid)?;
    if phase != "READ_ONLY"
        || projection.job_id != job
        || projection.event_id != actor.event
        || projection.revision != row.try_get::<i64, _>("revision").map_err(db)?
        || row.try_get::<Uuid, _>("actor_id").map_err(db)? != actor.actor
        || !matches!(projection.state, JobState::Pending | JobState::Running)
    {
        return Err(CommandError::InvalidState);
    }
    let retention = Retention {
        policy_revision: row.try_get("retention_revision").map_err(db)?,
        closed_at: row.try_get("closed_at").map_err(db)?,
        cleanup_after: row.try_get("cleanup_after").map_err(db)?,
    };
    retention.validate().map_err(invalid)?;
    let current = authority
        .retention_in(c, actor, retention.closed_at)
        .await?;
    if current != retention {
        return Err(CommandError::Unavailable);
    }
    let step = projection.next().ok_or(CommandError::InvalidState)?;
    if step == Step::ReadOnly {
        return Err(CommandError::InvalidState);
    }
    Ok((step, retention))
}
fn failure_name(value: Failure) -> &'static str {
    match value {
        Failure::AuthorityUnavailable => "AUTHORITY_UNAVAILABLE",
        Failure::OwnerUnavailable => "OWNER_UNAVAILABLE",
        Failure::OwnerDenied => "OWNER_DENIED",
        Failure::SourceChanged => "SOURCE_CHANGED",
    }
}
fn state_name(value: State) -> &'static str {
    match value {
        State::Pending => "PENDING",
        State::PartialFailure => "PARTIAL_FAILURE",
        State::Complete => "COMPLETE",
    }
}
async fn mark(
    c: &mut PgConnection,
    scope: Scope,
    state: State,
    failure: Option<Failure>,
) -> Result<(), CommandError> {
    sqlx::query("update concert_event_cleanup_progress set state=$4,failure=$5 where job_id=$1 and step=$2 and owner=$3")
        .bind(scope.job).bind(scope.step.name()).bind(scope.owner.name()).bind(state_name(state)).bind(failure.map(failure_name)).execute(c).await.map_err(db)?;
    Ok(())
}
fn cursor(row: &PgRow, column: &str) -> Result<Option<Cursor>, CommandError> {
    row.try_get::<Option<Vec<u8>>, _>(column)
        .map_err(db)?
        .map(|bytes| Cursor::new(bytes).map_err(invalid))
        .transpose()
}
fn unsigned(value: i64) -> Result<u64, CommandError> {
    value.try_into().map_err(invalid)
}
async fn load(
    c: &mut PgConnection,
    scope: Scope,
    retention: Retention,
) -> Result<Option<Checkpoint>, CommandError> {
    let Some(head)=sqlx::query("select * from concert_event_cleanup_progress where job_id=$1 and step=$2 and owner=$3 for update")
        .bind(scope.job).bind(scope.step.name()).bind(scope.owner.name()).fetch_optional(&mut *c).await.map_err(db)? else{return Ok(None)};
    if head.try_get::<Uuid, _>("event_id").map_err(db)? != scope.event
        || head.try_get::<Uuid, _>("actor_id").map_err(db)? != scope.actor
    {
        return Err(CommandError::InvalidState);
    }
    let state = match head.try_get::<&str, _>("state").map_err(db)? {
        "PENDING" => State::Pending,
        "PARTIAL_FAILURE" => State::PartialFailure,
        "COMPLETE" => State::Complete,
        _ => return Err(CommandError::InvalidState),
    };
    let failure = match head.try_get::<Option<&str>, _>("failure").map_err(db)? {
        None => None,
        Some("AUTHORITY_UNAVAILABLE") => Some(Failure::AuthorityUnavailable),
        Some("OWNER_UNAVAILABLE") => Some(Failure::OwnerUnavailable),
        Some("OWNER_DENIED") => Some(Failure::OwnerDenied),
        Some("SOURCE_CHANGED") => Some(Failure::SourceChanged),
        _ => return Err(CommandError::InvalidState),
    };
    let ordinal = unsigned(head.try_get("ordinal").map_err(db)?)?;
    let processed = unsigned(head.try_get("processed").map_err(db)?)?;
    let limit: u16 = head
        .try_get::<i16, _>("batch_limit")
        .map_err(db)?
        .try_into()
        .map_err(invalid)?;
    let request = BatchRequest::new(scope, retention, ordinal, cursor(&head, "cursor")?, limit)
        .map_err(invalid)?;
    let receipt = if ordinal > 0 || state == State::Complete {
        let index = if state == State::Complete {
            ordinal
        } else {
            ordinal - 1
        };
        let row=sqlx::query("select * from concert_event_cleanup_batches where job_id=$1 and step=$2 and owner=$3 and ordinal=$4")
            .bind(scope.job).bind(scope.step.name()).bind(scope.owner.name()).bind(index as i64).fetch_one(c).await.map_err(db)?;
        let prior = BatchRequest::new(scope, retention, index, cursor(&row, "cursor_in")?, limit)
            .map_err(invalid)?;
        let binding: [u8; 32] = row
            .try_get::<Vec<u8>, _>("binding")
            .map_err(db)?
            .try_into()
            .map_err(|_| CommandError::InvalidState)?;
        let receipt = Receipt {
            binding,
            operation: row.try_get("operation_id").map_err(db)?,
            id: row.try_get("receipt_id").map_err(db)?,
            revision: row.try_get("owner_revision").map_err(db)?,
            processed: row
                .try_get::<i16, _>("processed")
                .map_err(db)?
                .try_into()
                .map_err(invalid)?,
        };
        let out = cursor(&row, "cursor_out")?;
        if receipt.binding != prior.binding()
            || receipt.operation != prior.operation()
            || processed != unsigned(row.try_get("cumulative").map_err(db)?)?
            || row.try_get::<Uuid, _>("event_id").map_err(db)? != scope.event
            || row.try_get::<Uuid, _>("actor_id").map_err(db)? != scope.actor
            || (state == State::Complete
                && (row.try_get::<&str, _>("outcome").map_err(db)? != "CONVERGED"
                    || out.is_some()
                    || prior.cursor() != request.cursor()))
            || (state != State::Complete
                && (row.try_get::<&str, _>("outcome").map_err(db)? != "PAGE"
                    || out.as_ref() != request.cursor()))
        {
            return Err(CommandError::InvalidState);
        }
        Some(receipt)
    } else {
        None
    };
    Checkpoint::restore(request, state, processed, receipt, failure)
        .map(Some)
        .map_err(invalid)
}
async fn append(
    c: &mut PgConnection,
    input: &BatchRequest,
    checkpoint: &Checkpoint,
    receipt: Receipt,
) -> Result<(), CommandError> {
    let scope = input.scope();
    let (outcome, next) = if checkpoint.state() == State::Complete {
        ("CONVERGED", None)
    } else {
        ("PAGE", checkpoint.request().cursor().map(Cursor::bytes))
    };
    sqlx::query("insert into concert_event_cleanup_batches(operation_id,job_id,event_id,actor_id,step,owner,ordinal,binding,cursor_in,cursor_out,outcome,receipt_id,owner_revision,processed,cumulative) values($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)")
        .bind(input.operation()).bind(scope.job).bind(scope.event).bind(scope.actor).bind(scope.step.name()).bind(scope.owner.name())
        .bind(input.ordinal() as i64).bind(input.binding().to_vec()).bind(input.cursor().map(Cursor::bytes)).bind(next).bind(outcome)
        .bind(receipt.id).bind(receipt.revision).bind(receipt.processed as i16).bind(checkpoint.processed() as i64).execute(c).await.map_err(db)?;
    Ok(())
}
/// Exact additive source pin; a delivery receipt separately binds retained sources,
/// manifests, lockfile, toolchain and the frozen executable SHA-256.
pub fn owned_source_pin() -> String {
    use sha2::{Digest, Sha256};
    let mut digest = Sha256::new();
    for (path, source) in [
        (
            "application",
            include_str!("../../../../crates/concert-application/src/event_cleanup/mod.rs"),
        ),
        ("runtime", include_str!("mod.rs")),
        ("migration", MIGRATION),
        ("config", include_str!("../config/event_cleanup.rs")),
        (
            "application_tests",
            include_str!("../../../../crates/concert-application/tests/event_cleanup.rs"),
        ),
        (
            "runtime_tests",
            include_str!("../../tests/event_cleanup_runtime.rs"),
        ),
        (
            "test_support",
            include_str!("../../tests/event_cleanup_support/mod.rs"),
        ),
    ] {
        digest.update(path);
        digest.update([0]);
        digest.update(source);
        digest.update([0]);
    }
    format!("{:x}", digest.finalize())
}
