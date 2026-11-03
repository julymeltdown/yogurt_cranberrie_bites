//! Real READ_ONLY composition and pretransaction native-publication barrier.
//! World owns its SQL implementation/schema; no healthy or boolean owner fallback.
use super::*;
use crate::{
    community::closure,
    world::{
        admission_bridge::AdmissionBridge,
        invalidation::{Removal, Scope as RemovalScope},
    },
};
use std::sync::{Arc, Mutex};

/// Actual owner fences must already be retained by this connection's caller.
/// Lock event BEFORE job, never an async native/admission gate after either SQL lock.
pub(crate) async fn require_scope_in(
    c: &mut PgConnection,
    key: EffectKey,
) -> Result<(), CommandError> {
    if key.step != Step::ReadOnly || key.event.is_nil() || key.job.is_nil() {
        return Err(CommandError::InvalidInput);
    }
    let phase: Option<String> =
        sqlx::query_scalar("select phase from concert_events where event_id=$1 for update")
            .bind(key.event)
            .fetch_optional(&mut *c)
            .await
            .map_err(unavailable)?;
    if !matches!(phase.as_deref(), Some("READ_ONLY" | "ARCHIVED")) {
        return Err(CommandError::InvalidState);
    }
    let job: Option<Uuid> = sqlx::query_scalar(
        "select job_id from concert_closure_jobs where event_id=$1 and job_id=$2 for update",
    )
    .bind(key.event)
    .bind(key.job)
    .fetch_optional(&mut *c)
    .await
    .map_err(unavailable)?;
    if job != Some(key.job) {
        return Err(CommandError::Denied);
    }
    finish(c).await
}
pub(crate) async fn require_new_barrier_in(
    c: &mut PgConnection,
    key: EffectKey,
) -> Result<(), CommandError> {
    let initial: bool = sqlx::query_scalar("select exists(select 1 from concert_closure_jobs j join concert_events e using(event_id) where j.event_id=$1 and j.job_id=$2 and e.phase='READ_ONLY' and j.revision=1 and j.progress->>'state'='PENDING')")
        .bind(key.event).bind(key.job).fetch_one(c).await.map_err(unavailable)?;
    if initial {
        Ok(())
    } else {
        Err(CommandError::InvalidState)
    }
}
fn validate_world(
    world: &crate::world::closure::WorldReadOnlyReceipt,
    key: EffectKey,
) -> Result<(), CommandError> {
    if world.event != key.event
        || world.job != key.job
        || world.operation != key.operation_id()
        || world.receipt_id.is_nil()
        || !(1..=concert_domain::MAX_REVISION).contains(&world.revision)
    {
        return Err(CommandError::InvalidState);
    }
    Ok(())
}
fn decode(row: &sqlx::postgres::PgRow, key: EffectKey) -> Result<Applied, CommandError> {
    if row.try_get::<Uuid, _>("event_id").map_err(unavailable)? != key.event
        || row.try_get::<Uuid, _>("job_id").map_err(unavailable)? != key.job
        || row
            .try_get::<Uuid, _>("operation_id")
            .map_err(unavailable)?
            != key.operation_id()
    {
        return Err(CommandError::InvalidState);
    }
    Applied {
        key,
        receipt: row.try_get("receipt_id").map_err(unavailable)?,
        revision: row.try_get("revision").map_err(unavailable)?,
    }
    .validate(key)
}
/// Actual World and community mutations use this exact connection. The caller
/// must roll back on any error; this function cannot independently commit.
/// This low-level SQL component does not claim local native-cache removal.
pub async fn apply_read_only_in(
    c: &mut PgConnection,
    key: EffectKey,
) -> Result<Applied, CommandError> {
    require_scope_in(c, key).await?;
    if let Some(row) =
        sqlx::query("select * from concert_closure_read_only_barriers where job_id=$1")
            .bind(key.job)
            .fetch_optional(&mut *c)
            .await
            .map_err(unavailable)?
    {
        return decode(&row, key);
    }
    require_new_barrier_in(c, key).await?;
    let world =
        crate::world::closure::fence_event_read_only_in(c, key.event, key.job, key.operation_id())
            .await?;
    validate_world(&world, key)?;
    let community = closure::fence_event_read_only_in(c, key).await?;
    if community.event != key.event
        || community.job != key.job
        || community.operation != key.operation_id()
        || community.receipt_id.is_nil()
        || community.revision != 1
    {
        return Err(CommandError::InvalidState);
    }
    finish(c).await?;
    let row = sqlx::query("insert into concert_closure_read_only_barriers(event_id,job_id,operation_id,receipt_id,revision,world_receipt_id,world_revision,community_receipt_id,community_revision) values($1,$2,$3,$4,1,$5,$6,$7,$8) returning *")
        .bind(key.event).bind(key.job).bind(key.operation_id()).bind(Uuid::new_v4())
        .bind(world.receipt_id).bind(world.revision).bind(community.receipt_id).bind(community.revision)
        .fetch_one(c).await.map_err(unavailable)?;
    decode(&row, key)
}

/// No public constructor: production READ_ONLY ownership is prepared only by the
/// native driver. All later steps still require an explicit actual owning adapter.
pub struct ReadOnlyOwners<N> {
    later: N,
    prepared: Arc<Mutex<Option<EffectKey>>>,
}
#[async_trait]
impl<N: LifecycleOwners> LifecycleOwners for ReadOnlyOwners<N> {
    async fn apply_in(
        &self,
        c: &mut PgConnection,
        key: EffectKey,
        retention: Retention,
    ) -> Result<Applied, CommandError> {
        if key.step != Step::ReadOnly {
            return self.later.apply_in(c, key, retention).await;
        }
        if *self
            .prepared
            .lock()
            .map_err(|_| CommandError::Unavailable)?
            != Some(key)
        {
            return Err(CommandError::Unavailable);
        }
        apply_read_only_in(c, key).await
    }
}
struct Prepared {
    state: Arc<Mutex<Option<EffectKey>>>,
    key: EffectKey,
}
impl Prepared {
    fn install(state: Arc<Mutex<Option<EffectKey>>>, key: EffectKey) -> Result<Self, CommandError> {
        let mut value = state.lock().map_err(|_| CommandError::Unavailable)?;
        if value.is_some() {
            return Err(CommandError::Unavailable);
        }
        *value = Some(key);
        drop(value);
        Ok(Self { state, key })
    }
}
impl Drop for Prepared {
    fn drop(&mut self) {
        // SQL preparation permission is ephemeral. Native removal is intentionally
        // NOT finished here: cancellation/failure must retain the pending fence.
        if let Ok(mut state) = self.state.lock() {
            if *state == Some(self.key) {
                *state = None;
            }
        }
    }
}
/// Construct from the actual bridge's pool/event, preventing a detached SQL pool
/// from certifying a different runtime. No HTTP/root or permissive authority setup.
pub struct ReadOnlyCommandDriver<A, N> {
    service: PgEventLifecycle<A, ReadOnlyOwners<N>>,
    bridge: Arc<AdmissionBridge>,
    event: Uuid,
    prepared: Arc<Mutex<Option<EffectKey>>>,
}
impl<A: FencedLifecycleAuthority, N: LifecycleOwners> ReadOnlyCommandDriver<A, N> {
    pub fn new(authority: A, later: N, bridge: Arc<AdmissionBridge>) -> Result<Self, CommandError> {
        let (pool, _, _, event) = bridge.invalidation_context();
        if event.is_nil() {
            return Err(CommandError::InvalidState);
        }
        let prepared = Arc::new(Mutex::new(None));
        Ok(Self {
            service: PgEventLifecycle::new(
                pool,
                authority,
                ReadOnlyOwners {
                    later,
                    prepared: prepared.clone(),
                },
            ),
            bridge,
            event,
            prepared,
        })
    }
    /// Preflight cannot authorize durable effects. It only prevents an unauthorized
    /// or already-stale caller from installing a process-wide removal fence.
    /// The BE15 producer repeats current authorization/revision checks under SQL.
    async fn needs_preparation(
        &self,
        actor: Actor,
        key: Uuid,
        command: &CloseCommand,
    ) -> Result<bool, CommandError> {
        let CloseCommand::Start {
            expected_revision, ..
        } = command
        else {
            return Ok(false);
        };
        let target = start_job_id(actor, key);
        let mut tx = self.service.pool.begin().await.map_err(unavailable)?;
        let (phase, revision) = authorize(
            &mut tx,
            &self.service.authority,
            AuthorityQuery {
                actor,
                job: target,
                access: Access::Start,
            },
            false,
        )
        .await?;
        let exists: bool = sqlx::query_scalar("select exists(select 1 from concert_command_receipts where actor_id=$1 and event_id=$2 and operation_id='closeConcertEvent' and idempotency_key=$3)")
            .bind(actor.actor).bind(actor.event).bind(key).fetch_one(&mut *tx).await.map_err(unavailable)?;
        finish(&mut tx).await?;
        tx.commit().await.map_err(unavailable)?;
        Ok(!exists && matches!(phase.as_str(), "OPEN" | "LIVE") && revision == *expected_revision)
    }
    pub async fn command(
        &self,
        actor: Actor,
        key: Uuid,
        command: CloseCommand,
        keys: &FingerprintKeys,
    ) -> Result<SafeReceipt, CommandError> {
        command.request(actor, key)?;
        if actor.event != self.event {
            return Err(CommandError::Denied);
        }
        if !self.needs_preparation(actor, key, &command).await? {
            // Exact replay/conflicts and RESUME retain canonical BE15 behavior.
            // A new START racing back to OPEN without preparation fails closed.
            return self.service.command(actor, key, command, keys).await;
        }
        let effect = EffectKey {
            event: actor.event,
            job: start_job_id(actor, key),
            step: Step::ReadOnly,
        };
        let operation = effect.operation_id();
        // Order shared with World invalidation. Absolutely no SQL transaction is
        // held while entering these gates, and no native mutex spans an await.
        let _delivery = self.bridge.invalidation_delivery_gate().lock().await;
        self.bridge
            .begin_invalidation(operation, Removal::Scope(RemovalScope::Event(actor.event)))
            .map_err(|_| CommandError::Unavailable)?;
        let _admission = self.bridge.invalidation_admission_gate().lock().await;
        self.bridge
            .pause_invalidation()
            .await
            .map_err(|_| CommandError::Unavailable)?;
        let _prepared = Prepared::install(self.prepared.clone(), effect)?;
        let result = self.service.command(actor, key, command, keys).await;
        if result
            .as_ref()
            .is_ok_and(|r| r.state == ReceiptState::Succeeded)
        {
            // Durable barrier committed. Removing the pending marker cannot
            // restore views. Failure here leaves native publication fail-closed;
            // it must never turn a committed admission into a claimed rollback.
            let _ = self.bridge.finish_invalidation(operation);
        }
        // Failure/cancellation retains the native marker. Fresh qualified World
        // recovery must re-read committed phase/authority before clearing it.
        result
    }
    pub async fn job(&self, actor: Actor, job: Uuid) -> Result<Job, CommandError> {
        if actor.event != self.event {
            return Err(CommandError::Denied);
        }
        self.service.job(actor, job).await
    }
    pub async fn run_next(&self, actor: Actor, job: Uuid) -> Result<Job, CommandError> {
        if actor.event != self.event {
            return Err(CommandError::Denied);
        }
        self.service.run_next(actor, job).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn world_component_receipt_must_match_every_binding() {
        let key = EffectKey {
            event: Uuid::new_v4(),
            job: Uuid::new_v4(),
            step: Step::ReadOnly,
        };
        let make = || crate::world::closure::WorldReadOnlyReceipt {
            event: key.event,
            job: key.job,
            operation: key.operation_id(),
            receipt_id: Uuid::new_v4(),
            revision: 1,
        };
        assert!(validate_world(&make(), key).is_ok());
        let mut wrong = make();
        wrong.event = Uuid::new_v4();
        assert!(validate_world(&wrong, key).is_err());
        let mut wrong = make();
        wrong.job = Uuid::new_v4();
        assert!(validate_world(&wrong, key).is_err());
        let mut wrong = make();
        wrong.operation = Uuid::new_v4();
        assert!(validate_world(&wrong, key).is_err());
        let mut wrong = make();
        wrong.receipt_id = Uuid::nil();
        assert!(validate_world(&wrong, key).is_err());
        let mut wrong = make();
        wrong.revision = 0;
        assert!(validate_world(&wrong, key).is_err());
        let mut wrong = make();
        wrong.revision = concert_domain::MAX_REVISION + 1;
        assert!(validate_world(&wrong, key).is_err());
    }
    #[test]
    fn preparation_cancellation_removes_only_ephemeral_permission() {
        let state = Arc::new(Mutex::new(None));
        let key = EffectKey {
            event: Uuid::new_v4(),
            job: Uuid::new_v4(),
            step: Step::ReadOnly,
        };
        let prepared = Prepared::install(state.clone(), key).unwrap();
        assert_eq!(*state.lock().unwrap(), Some(key));
        assert!(Prepared::install(state.clone(), key).is_err());
        drop(prepared);
        assert_eq!(*state.lock().unwrap(), None);
    }
}
