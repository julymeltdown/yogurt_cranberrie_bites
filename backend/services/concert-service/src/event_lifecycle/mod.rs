//! BE41 persistence adapter. Event lock -> current authority -> job -> BE15 budget.
//! No cross-service tables, network mutations, or request-provided authority claims.
use crate::adapters::out::repository::receipt::{CommandProducer, PgCommandRepository};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::{
    commands::*,
    event_lifecycle::{Actor, CloseCommand},
};
use concert_domain::event_lifecycle::{Job, JobState, Retention, Step};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool, Row};
use uuid::Uuid;

pub mod archive;
pub mod http;
pub const SCHEMA: &str = include_str!("schema.sql");
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Start,
    Resume,
    ReadJob,
    RunStep,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuthorityQuery {
    pub actor: Actor,
    pub job: Uuid,
    pub access: Access,
}
/// Trusted owner result. Implementor retains identity/role/policy revision fences
/// through the transaction; setting a deadline alone is never an authority proof.
#[derive(Clone, Copy, Debug)]
pub struct AuthorityFence {
    pub query: AuthorityQuery,
    pub auth_revision: i64,
    pub role_revision: i64,
    pub policy_revision: i64,
    pub checked_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}
#[async_trait]
pub trait FencedLifecycleAuthority: Send + Sync {
    /// Must reject deleted, suspended, expired and non-operator identities. Worker
    /// checks are also current initiating-actor checks, never cached START grants.
    async fn check_in(
        &self,
        c: &mut PgConnection,
        query: AuthorityQuery,
        now: DateTime<Utc>,
    ) -> Result<AuthorityFence, CommandError>;
    /// Return reviewed operator retention from authoritative configuration under lock.
    async fn retention_in(
        &self,
        c: &mut PgConnection,
        actor: Actor,
        closed_at: DateTime<Utc>,
    ) -> Result<Retention, CommandError>;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EffectKey {
    pub event: Uuid,
    pub job: Uuid,
    pub step: Step,
}
impl EffectKey {
    pub fn operation_id(self) -> Uuid {
        let mut hash = Sha256::new();
        hash.update(b"mcr-be41-effect-v1");
        hash.update(self.event.as_bytes());
        hash.update(self.job.as_bytes());
        hash.update(self.step.name());
        digest_uuid(hash.finalize().as_slice())
    }
}
/// Returned only after owning adapters actually applied every effect represented
/// by the named step on this connection, or verified the same durable operation.
#[derive(Clone, Copy, Debug)]
pub struct Applied {
    pub key: EffectKey,
    pub receipt: Uuid,
    pub revision: i64,
}
impl Applied {
    fn validate(self, key: EffectKey) -> Result<Self, CommandError> {
        if self.key != key
            || self.receipt.is_nil()
            || !(1..=concert_domain::MAX_REVISION).contains(&self.revision)
        {
            Err(CommandError::InvalidState)
        } else {
            Ok(self)
        }
    }
}
#[async_trait]
pub trait LifecycleOwners: Send + Sync {
    /// Local owner SQL only. Freeze must pin actual FROZEN mural; snapshot/export
    /// must require qualified public Post/media projections, not copy live bodies.
    /// EXPIRE_BEACONS also requires community reservations/party interactions and
    /// world tickets/leases fenced. CLEANUP honors exact retention and ownership.
    /// No remote destructive operation is allowed inside this local transaction.
    async fn apply_in(
        &self,
        c: &mut PgConnection,
        key: EffectKey,
        retention: Retention,
    ) -> Result<Applied, CommandError>;
}
fn unavailable(_: sqlx::Error) -> CommandError {
    CommandError::Unavailable
}
fn invalid(_: impl std::fmt::Display) -> CommandError {
    CommandError::InvalidState
}
async fn now(c: &mut PgConnection) -> Result<DateTime<Utc>, CommandError> {
    sqlx::query_scalar("select clock_timestamp()")
        .fetch_one(c)
        .await
        .map_err(unavailable)
}
async fn authorize<A: FencedLifecycleAuthority>(
    c: &mut PgConnection,
    a: &A,
    q: AuthorityQuery,
    write: bool,
) -> Result<(String, i64), CommandError> {
    if [q.actor.actor, q.actor.event, q.job]
        .iter()
        .any(Uuid::is_nil)
    {
        return Err(CommandError::Denied);
    }
    let row = sqlx::query(if write {
        "select phase,revision from concert_events where event_id=$1 for update"
    } else {
        "select phase,revision from concert_events where event_id=$1 for share"
    })
    .bind(q.actor.event)
    .fetch_optional(&mut *c)
    .await
    .map_err(unavailable)?
    .ok_or(CommandError::Denied)?;
    let started = now(c).await?;
    let fence = a.check_in(c, q, started).await?;
    let finished = now(c).await?;
    if fence.query != q
        || [
            fence.auth_revision,
            fence.role_revision,
            fence.policy_revision,
        ]
        .iter()
        .any(|r| !(1..=concert_domain::MAX_REVISION).contains(r))
        || fence.checked_at < started
        || fence.checked_at > finished
        || fence.expires_at <= finished
    {
        return Err(CommandError::Unavailable);
    }
    sqlx::query("select set_config('mcr.lifecycle_authority_until',least(coalesce(nullif(current_setting('mcr.lifecycle_authority_until',true),'')::timestamptz,'infinity'::timestamptz),$1::timestamptz)::text,true)")
        .bind(fence.expires_at).execute(&mut *c).await.map_err(unavailable)?;
    Ok((
        row.try_get("phase").map_err(invalid)?,
        row.try_get("revision").map_err(invalid)?,
    ))
}
async fn finish(c: &mut PgConnection) -> Result<(), CommandError> {
    let valid:bool=sqlx::query_scalar("select clock_timestamp()<nullif(current_setting('mcr.lifecycle_authority_until',true),'')::timestamptz").fetch_one(c).await.map_err(unavailable)?;
    if valid {
        Ok(())
    } else {
        Err(CommandError::Unavailable)
    }
}
fn project(row: &sqlx::postgres::PgRow) -> Result<Job, CommandError> {
    let job: Job =
        serde_json::from_value(row.try_get("progress").map_err(invalid)?).map_err(invalid)?;
    job.validate().map_err(invalid)?;
    if job.job_id != row.try_get::<Uuid, _>("job_id").map_err(invalid)?
        || job.event_id != row.try_get::<Uuid, _>("event_id").map_err(invalid)?
        || job.revision != row.try_get::<i64, _>("revision").map_err(invalid)?
    {
        return Err(CommandError::InvalidState);
    }
    Ok(job)
}
async fn load(
    c: &mut PgConnection,
    actor: Actor,
    job: Uuid,
    write: bool,
) -> Result<sqlx::postgres::PgRow, CommandError> {
    sqlx::query(if write {"select * from concert_closure_jobs where event_id=$1 and job_id=$2 and actor_id=$3 for update"}else{"select * from concert_closure_jobs where event_id=$1 and job_id=$2 and actor_id=$3 for share"})
        .bind(actor.event).bind(job).bind(actor.actor).fetch_optional(c).await.map_err(unavailable)?.ok_or(CommandError::Denied)
}
async fn save(c: &mut PgConnection, job: &Job) -> Result<(), CommandError> {
    job.validate().map_err(invalid)?;
    let n=sqlx::query("update concert_closure_jobs set revision=$3,progress=$4 where event_id=$1 and job_id=$2 and revision=$3-1")
        .bind(job.event_id).bind(job.job_id).bind(job.revision).bind(serde_json::to_value(job).map_err(invalid)?).execute(c).await.map_err(unavailable)?.rows_affected();
    if n != 1 {
        Err(CommandError::Conflict)
    } else {
        Ok(())
    }
}
async fn audit(
    c: &mut PgConnection,
    actor: Actor,
    job: &Job,
    key: Option<Uuid>,
    step: Option<Step>,
    outcome: &str,
) -> Result<(), CommandError> {
    sqlx::query("insert into concert_closure_audit(audit_id,event_id,job_id,actor_id,command_key,step,outcome,job_revision) values($1,$2,$3,$4,$5,$6,$7,$8)")
        .bind(Uuid::new_v4()).bind(actor.event).bind(job.job_id).bind(actor.actor).bind(key).bind(step.map(Step::name)).bind(outcome).bind(job.revision).execute(c).await.map_err(unavailable)?;
    Ok(())
}
fn digest_uuid(bytes: &[u8]) -> Uuid {
    let mut out = [0; 16];
    out.copy_from_slice(&bytes[..16]);
    out[6] = (out[6] & 15) | 0x50;
    out[8] = (out[8] & 63) | 0x80;
    Uuid::from_bytes(out)
}
pub fn start_job_id(actor: Actor, key: Uuid) -> Uuid {
    let mut h = Sha256::new();
    h.update(b"mcr-be41-start-v1");
    h.update(actor.event.as_bytes());
    h.update(actor.actor.as_bytes());
    h.update(key.as_bytes());
    digest_uuid(h.finalize().as_slice())
}
pub struct PgEventLifecycle<A, O> {
    pool: PgPool,
    authority: A,
    owners: O,
}
impl<A: FencedLifecycleAuthority, O: LifecycleOwners> PgEventLifecycle<A, O> {
    pub fn new(pool: PgPool, authority: A, owners: O) -> Self {
        Self {
            pool,
            authority,
            owners,
        }
    }
    pub async fn command(
        &self,
        actor: Actor,
        key: Uuid,
        command: CloseCommand,
        keys: &FingerprintKeys,
    ) -> Result<SafeReceipt, CommandError> {
        let target = match &command {
            CloseCommand::Start { .. } => start_job_id(actor, key),
            CloseCommand::Resume { job_id, .. } => *job_id,
        };
        let request = command.request(actor, key)?;
        PgCommandRepository::new(self.pool.clone())
            .execute(
                &request,
                target,
                keys,
                &CloseProducer {
                    service: self,
                    command,
                },
            )
            .await
    }
    /// Status is strictly a read. It never schedules/retries/executes a step.
    pub async fn job(&self, actor: Actor, job_id: Uuid) -> Result<Job, CommandError> {
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        authorize(
            &mut tx,
            &self.authority,
            AuthorityQuery {
                actor,
                job: job_id,
                access: Access::ReadJob,
            },
            false,
        )
        .await?;
        let row = load(&mut tx, actor, job_id, false).await?;
        let job = project(&row)?;
        finish(&mut tx).await?;
        tx.commit().await.map_err(unavailable)?;
        Ok(job)
    }
    /// Internal bounded scheduling projection. It grants no execution authority.
    pub async fn due_jobs(&self, limit: i64) -> Result<Vec<(Actor, Uuid)>, CommandError> {
        if !(1..=100).contains(&limit) {
            return Err(CommandError::InvalidInput);
        }
        let rows=sqlx::query("select actor_id,event_id,job_id from concert_closure_jobs where progress->>'state' in ('PENDING','RUNNING') and ((progress->'steps'->5->>'state')<>'SUCCEEDED' or cleanup_after<=clock_timestamp()) order by updated_at,job_id limit $1")
            .bind(limit).fetch_all(&self.pool).await.map_err(unavailable)?;
        rows.into_iter()
            .map(|r| {
                Ok((
                    Actor {
                        actor: r.try_get("actor_id").map_err(invalid)?,
                        event: r.try_get("event_id").map_err(invalid)?,
                    },
                    r.try_get("job_id").map_err(invalid)?,
                ))
            })
            .collect()
    }
    /// One durable step per transaction. Effects and evidence commit together.
    /// A process kill before commit rolls both back; after commit next() skips it.
    pub async fn run_next(&self, actor: Actor, job_id: Uuid) -> Result<Job, CommandError> {
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        let (phase, _) = authorize(
            &mut tx,
            &self.authority,
            AuthorityQuery {
                actor,
                job: job_id,
                access: Access::RunStep,
            },
            true,
        )
        .await?;
        let row = load(&mut tx, actor, job_id, true).await?;
        let mut job = project(&row)?;
        if job.state == JobState::Completed {
            finish(&mut tx).await?;
            tx.commit().await.map_err(unavailable)?;
            return Ok(job);
        }
        if phase != "READ_ONLY" {
            return Err(CommandError::InvalidState);
        }
        if job.state == JobState::PartialFailure {
            return Err(CommandError::InvalidState);
        }
        let step = job.next().ok_or(CommandError::InvalidState)?;
        let retention = Retention {
            policy_revision: row.try_get("retention_revision").map_err(invalid)?,
            closed_at: row.try_get("closed_at").map_err(invalid)?,
            cleanup_after: row.try_get("cleanup_after").map_err(invalid)?,
        }
        .validate()
        .map_err(invalid)?;
        if step == Step::Cleanup && now(&mut tx).await? < retention.cleanup_after {
            finish(&mut tx).await?;
            tx.commit().await.map_err(unavailable)?;
            return Ok(job);
        }
        let effect = EffectKey {
            event: actor.event,
            job: job_id,
            step,
        };
        sqlx::query("savepoint lifecycle_effect")
            .execute(&mut *tx)
            .await
            .map_err(unavailable)?;
        match self
            .owners
            .apply_in(&mut tx, effect, retention)
            .await
            .and_then(|v| v.validate(effect))
        {
            Ok(receipt) => {
                sqlx::query("insert into concert_closure_effects(event_id,job_id,step,owner_receipt,owner_revision) values($1,$2,$3,$4,$5)")
                    .bind(actor.event).bind(job_id).bind(step.name()).bind(receipt.receipt).bind(receipt.revision).execute(&mut *tx).await.map_err(unavailable)?;
                job.record(step, true).map_err(invalid)?;
                if job.state == JobState::Completed {
                    let changed=sqlx::query("update concert_events set phase='ARCHIVED',revision=revision+1 where event_id=$1 and phase='READ_ONLY'")
                        .bind(actor.event).execute(&mut *tx).await.map_err(unavailable)?.rows_affected();
                    if changed != 1 {
                        return Err(CommandError::InvalidState);
                    }
                }
                save(&mut tx, &job).await?;
                audit(&mut tx, actor, &job, None, Some(step), "SUCCEEDED").await?;
            }
            Err(_) => {
                sqlx::query("rollback to savepoint lifecycle_effect")
                    .execute(&mut *tx)
                    .await
                    .map_err(unavailable)?;
                job.record(step, false).map_err(invalid)?;
                save(&mut tx, &job).await?;
                audit(&mut tx, actor, &job, None, Some(step), "FAILED").await?;
            }
        }
        finish(&mut tx).await?;
        tx.commit().await.map_err(unavailable)?;
        Ok(job)
    }
}
struct CloseProducer<'a, A, O> {
    service: &'a PgEventLifecycle<A, O>,
    command: CloseCommand,
}
#[async_trait]
impl<A: FencedLifecycleAuthority, O: LifecycleOwners> CommandProducer for CloseProducer<'_, A, O> {
    async fn authorize(
        &self,
        c: &mut PgConnection,
        s: Scope,
        target: Uuid,
    ) -> Result<(), CommandError> {
        if s.operation.id() != "closeConcertEvent" {
            return Err(CommandError::Denied);
        }
        let actor = Actor {
            actor: s.actor,
            event: s.event,
        };
        let access = match self.command {
            CloseCommand::Start { .. } => Access::Start,
            CloseCommand::Resume { .. } => Access::Resume,
        };
        authorize(
            c,
            &self.service.authority,
            AuthorityQuery {
                actor,
                job: target,
                access,
            },
            true,
        )
        .await?;
        if matches!(self.command, CloseCommand::Resume { .. }) {
            load(c, actor, target, true).await?;
        }
        Ok(())
    }
    async fn apply(
        &self,
        c: &mut PgConnection,
        s: Scope,
        target: Uuid,
    ) -> Result<(Completion, Option<Invalidation>), CommandError> {
        let actor = Actor {
            actor: s.actor,
            event: s.event,
        };
        let job = match &self.command {
            CloseCommand::Start {
                expected_revision, ..
            } => {
                let event = sqlx::query(
                    "select phase,revision from concert_events where event_id=$1 for update",
                )
                .bind(s.event)
                .fetch_one(&mut *c)
                .await
                .map_err(unavailable)?;
                let exists: bool = sqlx::query_scalar(
                    "select exists(select 1 from concert_closure_jobs where event_id=$1)",
                )
                .bind(s.event)
                .fetch_one(&mut *c)
                .await
                .map_err(unavailable)?;
                if exists
                    || !matches!(
                        event
                            .try_get::<String, _>("phase")
                            .map_err(invalid)?
                            .as_str(),
                        "OPEN" | "LIVE"
                    )
                {
                    return Ok((Completion::Failed(SafeFailure::InvalidTransition), None));
                }
                if event.try_get::<i64, _>("revision").map_err(invalid)? != *expected_revision {
                    return Ok((Completion::Failed(SafeFailure::VersionConflict), None));
                }
                let at = now(c).await?;
                let retention = self
                    .service
                    .authority
                    .retention_in(c, actor, at)
                    .await?
                    .validate()
                    .map_err(invalid)?;
                if retention.closed_at != at {
                    return Err(CommandError::InvalidState);
                }
                let job = Job::new(target, s.event).map_err(invalid)?;
                sqlx::query("insert into concert_closure_jobs(job_id,event_id,actor_id,start_key,original_event_revision,retention_revision,closed_at,cleanup_after,progress) values($1,$2,$3,$4,$5,$6,$7,$8,$9)")
                    .bind(target).bind(s.event).bind(s.actor).bind(s.key).bind(expected_revision).bind(retention.policy_revision).bind(at).bind(retention.cleanup_after).bind(serde_json::to_value(&job).map_err(invalid)?).execute(&mut *c).await.map_err(unavailable)?;
                sqlx::query("update concert_events set phase='READ_ONLY',revision=revision+1 where event_id=$1").bind(s.event).execute(&mut *c).await.map_err(unavailable)?;
                // Phase alone cannot terminate already-admitted world/party flows.
                // The local owning adapters must fence them before START commits.
                let effect = EffectKey {
                    event: s.event,
                    job: target,
                    step: Step::ReadOnly,
                };
                let applied = self
                    .service
                    .owners
                    .apply_in(c, effect, retention)
                    .await?
                    .validate(effect)?;
                sqlx::query("insert into concert_closure_effects(event_id,job_id,step,owner_receipt,owner_revision) values($1,$2,'READ_ONLY',$3,$4)")
                    .bind(s.event).bind(target).bind(applied.receipt).bind(applied.revision).execute(&mut *c).await.map_err(unavailable)?;
                job
            }
            CloseCommand::Resume {
                job_id,
                expected_revision,
                ..
            } => {
                if *job_id != target {
                    return Err(CommandError::InvalidInput);
                }
                let row = load(c, actor, target, true).await?;
                if row.try_get::<Uuid, _>("start_key").map_err(invalid)? == s.key {
                    return Err(CommandError::Conflict);
                }
                let mut job = project(&row)?;
                let phase: String =
                    sqlx::query_scalar("select phase from concert_events where event_id=$1")
                        .bind(s.event)
                        .fetch_one(&mut *c)
                        .await
                        .map_err(unavailable)?;
                if phase != "READ_ONLY" || job.state != JobState::PartialFailure {
                    return Ok((Completion::Failed(SafeFailure::InvalidTransition), None));
                }
                if job.revision != *expected_revision {
                    return Ok((Completion::Failed(SafeFailure::VersionConflict), None));
                }
                job.resume(*expected_revision).map_err(invalid)?;
                save(c, &job).await?;
                job
            }
        };
        audit(c, actor, &job, Some(s.key), None, "ACCEPTED").await?;
        let reference = SafeReference {
            resource: job.job_id,
            revision: job.revision,
        };
        Ok((
            Completion::Succeeded {
                reference,
                status: 200,
            },
            Some(Invalidation { reference }),
        ))
    }
}
