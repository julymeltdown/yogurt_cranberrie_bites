//! BE34/35 PostgreSQL adapter. Public routing and production policy wiring are separate.
use super::receipt::{CommandProducer, PgCommandRepository};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::{commands::*, mural::*};
use concert_domain::{mural::*, MAX_REVISION};
use sqlx::{postgres::PgRow, PgConnection, PgPool, Row};
use std::sync::Mutex;
use uuid::Uuid;

/// Trusted adapter: resolve canonical Auth/session/event identity, member eligibility,
/// suspension and BE37 relationship policy. Lock local policy fences through commit;
/// external assertions must be current and fenced. No browser-supplied authority.
#[async_trait]
pub trait MuralAuthority: Send + Sync {
    async fn authorize_in(&self, c: &mut PgConnection, actor: MuralActor)
        -> Result<(), MuralError>;
}
pub struct UnavailableMuralAuthority;
#[async_trait]
impl MuralAuthority for UnavailableMuralAuthority {
    async fn authorize_in(&self, _: &mut PgConnection, _: MuralActor) -> Result<(), MuralError> {
        Err(MuralError::PolicyUnavailable)
    }
}
/// Closed own-result adapter envelope, bound to one verified durable receipt.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct OwnPixelResult {
    pub receipt_id: Uuid,
    pub idempotency_key: Uuid,
    pub result: PixelOperation,
}
pub struct PgMuralRepository<A> {
    pool: PgPool,
    authority: A,
}
impl<A: MuralAuthority> PgMuralRepository<A> {
    pub fn new(pool: PgPool, authority: A) -> Self {
        Self { pool, authority }
    }
    /// One call represents an explicit confirmed pixel, not a preview/drag event.
    pub async fn place(
        &self,
        actor: MuralActor,
        key: Uuid,
        input: PlacePixel,
        keys: &FingerprintKeys,
    ) -> Result<PixelOperation, MuralError> {
        if !actor.valid() || key.is_nil() || !input.valid() {
            return Err(MuralError::InvalidInput);
        }
        let scope = scope(actor, key)?;
        let body = CanonicalValue::Object(vec![
            (
                "expected_revision".into(),
                CanonicalValue::Integer(input.expected_revision),
            ),
            ("x".into(), CanonicalValue::Integer(input.x.into())),
            ("y".into(), CanonicalValue::Integer(input.y.into())),
            (
                "color_index".into(),
                CanonicalValue::Integer(input.color_index.into()),
            ),
        ]);
        let event = actor.event.to_string();
        let request =
            CanonicalRequest::json(scope, &[("eventId", event.as_str())], &[], Some(&body))?;
        let producer = Producer {
            authority: &self.authority,
            actor,
            input: Some(input),
            error: Mutex::new(None),
        };
        let receipt = PgCommandRepository::new(self.pool.clone())
            .execute(&request, actor.event, keys, &producer)
            .await;
        let receipt = match receipt {
            Ok(value) => value,
            Err(error) => {
                if let Some(domain) = producer
                    .error
                    .lock()
                    .map_err(|_| MuralError::InvalidState)?
                    .take()
                {
                    return Err(domain);
                }
                return Err(error.into());
            }
        };
        if let Some(failure) = receipt.failure {
            let original = producer
                .error
                .lock()
                .map_err(|_| MuralError::InvalidState)?
                .take();
            return Err(original.unwrap_or_else(|| from_failure(failure)));
        }
        if receipt.state != ReceiptState::Succeeded {
            return Err(MuralError::InvalidState);
        }
        Ok(self.recover(actor, key).await?.result)
    }
    /// Read-only uncertain-outcome recovery. Missing, expired, retired, pending,
    /// failed and inconsistent records are indistinguishable ResourceUnavailable.
    /// One fresh authorized transaction binds the exact receipt/key to the original
    /// actor/event/revision operation; neither status nor pixel equality is proof.
    pub async fn recover(
        &self,
        actor: MuralActor,
        key: Uuid,
    ) -> Result<OwnPixelResult, MuralError> {
        if !actor.valid() || key.is_nil() {
            return Err(MuralError::InvalidInput);
        }
        let mut tx = self.pool.begin().await.map_err(storage)?;
        gate(&self.authority, &mut tx, actor).await?;
        let present: Option<Uuid> = sqlx::query_scalar(
            "select event_id from public.concert_mural_state where event_id=$1 for share",
        )
        .bind(actor.event)
        .fetch_optional(&mut *tx)
        .await
        .map_err(storage)?;
        if present.is_none() {
            return Err(MuralError::Denied);
        }
        let row=sqlx::query("select r.receipt_id,r.idempotency_key,o.operation_id,o.event_id,o.revision,o.x,o.y,o.color_index,o.committed_at,o.next_allowed_at from public.concert_command_receipts r join public.concert_mural_operations o on o.event_id=r.event_id and o.revision=r.resource_revision and o.actor_id=r.actor_id where r.actor_id=$1 and r.event_id=$2 and r.idempotency_key=$3 and r.operation_id='placeConcertPixel' and r.state='SUCCEEDED' and r.expires_at>clock_timestamp() and r.resource_id=r.event_id and r.target_id=r.event_id and r.result_status=200 and r.failure_reason is null for share of r,o")
            .bind(actor.actor).bind(actor.event).bind(key).fetch_optional(&mut *tx).await.map_err(storage)?.ok_or(MuralError::Denied)?;
        let result = PixelOperation {
            public: public_operation(&row).map_err(|_| MuralError::Denied)?,
            next_allowed_at: row
                .try_get("next_allowed_at")
                .map_err(|_| MuralError::Denied)?,
        };
        if result.public.operation_id.is_nil()
            || result.public.event_id != actor.event
            || result.public.revision < 2
            || !(PlacePixel {
                expected_revision: result.public.revision,
                x: result.public.x,
                y: result.public.y,
                color_index: result.public.color_index,
            })
            .valid()
            || result.next_allowed_at < result.public.committed_at
        {
            return Err(MuralError::Denied);
        }
        let value = OwnPixelResult {
            receipt_id: row.try_get("receipt_id").map_err(|_| MuralError::Denied)?,
            idempotency_key: row
                .try_get("idempotency_key")
                .map_err(|_| MuralError::Denied)?,
            result,
        };
        if value.receipt_id.is_nil() || value.idempotency_key != key {
            return Err(MuralError::Denied);
        }
        tx.commit().await.map_err(storage)?;
        Ok(value)
    }
}
fn scope(actor: MuralActor, key: Uuid) -> Result<Scope, CommandError> {
    Ok(Scope {
        actor: actor.actor,
        event: actor.event,
        key,
        operation: RegisteredOperation::find("placeConcertPixel")?,
    })
}
struct Producer<'a, A> {
    authority: &'a A,
    actor: MuralActor,
    input: Option<PlacePixel>,
    error: Mutex<Option<MuralError>>,
}
#[async_trait]
impl<A: MuralAuthority> CommandProducer for Producer<'_, A> {
    async fn authorize(
        &self,
        c: &mut PgConnection,
        s: Scope,
        target: Uuid,
    ) -> Result<(), CommandError> {
        if s.actor != self.actor.actor
            || s.event != self.actor.event
            || target != self.actor.event
            || s.operation.id() != "placeConcertPixel"
        {
            return Err(CommandError::Denied);
        }
        if let Err(error) = gate(self.authority, c, self.actor).await {
            return self.fail(error);
        }
        // Serializes every writer before receipt admission; replay does NOT check writable
        // state/cooldown, so successful commands remain recoverable after pause/freeze.
        let present: Option<Uuid> = sqlx::query_scalar(
            "select event_id from public.concert_mural_state where event_id=$1 for update",
        )
        .bind(target)
        .fetch_optional(c)
        .await
        .map_err(|_| CommandError::Unavailable)?;
        if present.is_none() {
            return Err(CommandError::Denied);
        }
        Ok(())
    }
    async fn apply(
        &self,
        c: &mut PgConnection,
        _: Scope,
        target: Uuid,
    ) -> Result<(Completion, Option<Invalidation>), CommandError> {
        let Some(input) = self.input else {
            return Err(CommandError::InvalidState);
        };
        match mutate(c, self.actor, input).await {
            Ok(revision) => {
                let reference = SafeReference {
                    resource: target,
                    revision,
                };
                Ok((
                    Completion::Succeeded {
                        reference,
                        status: 200,
                    },
                    Some(Invalidation { reference }),
                ))
            }
            Err(error) => {
                let failure = match error {
                    MuralError::Paused => SafeFailure::MuralPaused,
                    MuralError::Frozen => SafeFailure::MuralFrozen,
                    MuralError::Cooldown(_) => SafeFailure::MuralCooldown,
                    MuralError::RegionLocked => SafeFailure::MuralRegionLocked,
                    MuralError::RevisionConflict => SafeFailure::VersionConflict,
                    MuralError::EventReadOnly => SafeFailure::EventReadOnly,
                    _ => return self.fail(error),
                };
                *self.error.lock().map_err(|_| CommandError::InvalidState)? = Some(error);
                Ok((Completion::Failed(failure), None))
            }
        }
    }
}
impl<A> Producer<'_, A> {
    fn fail<T>(&self, error: MuralError) -> Result<T, CommandError> {
        *self.error.lock().map_err(|_| CommandError::InvalidState)? = Some(error);
        Err(CommandError::Unavailable)
    }
}
async fn gate<A: MuralAuthority>(
    a: &A,
    c: &mut PgConnection,
    actor: MuralActor,
) -> Result<String, MuralError> {
    if !actor.valid() {
        return Err(MuralError::InvalidInput);
    }
    let phase: Option<String> =
        sqlx::query_scalar("select phase from public.concert_events where event_id=$1 for share")
            .bind(actor.event)
            .fetch_optional(&mut *c)
            .await
            .map_err(storage)?;
    let phase = phase.ok_or(MuralError::Denied)?;
    a.authorize_in(c, actor)
        .await
        .map_err(|error| match error {
            MuralError::Denied => error,
            _ => MuralError::PolicyUnavailable,
        })?;
    if !matches!(phase.as_str(), "OPEN" | "LIVE" | "READ_ONLY") {
        return Err(MuralError::Denied);
    }
    Ok(phase)
}
async fn mutate(
    c: &mut PgConnection,
    actor: MuralActor,
    input: PlacePixel,
) -> Result<i64, MuralError> {
    let phase: String =
        sqlx::query_scalar("select phase from public.concert_events where event_id=$1")
            .bind(actor.event)
            .fetch_one(&mut *c)
            .await
            .map_err(storage)?;
    if !matches!(phase.as_str(), "OPEN" | "LIVE") {
        return Err(MuralError::EventReadOnly);
    }
    let row = sqlx::query("select * from public.concert_mural_state where event_id=$1 for update")
        .bind(actor.event)
        .fetch_one(&mut *c)
        .await
        .map_err(storage)?;
    match row.try_get::<String, _>("state").map_err(invalid)?.as_str() {
        "EDITABLE" => {}
        "PAUSED" => return Err(MuralError::Paused),
        "FROZEN" => return Err(MuralError::Frozen),
        _ => return Err(MuralError::InvalidState),
    }
    let now: DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
        .fetch_one(&mut *c)
        .await
        .map_err(storage)?;
    let next: Option<DateTime<Utc>> = sqlx::query_scalar(
        "select next_allowed_at from public.concert_mural_limits where event_id=$1 and actor_id=$2",
    )
    .bind(actor.event)
    .bind(actor.actor)
    .fetch_optional(&mut *c)
    .await
    .map_err(storage)?;
    if let Some(next) = next {
        if now < next {
            return Err(MuralError::Cooldown(Some(next)));
        }
    }
    let revision: i64 = row.try_get("revision").map_err(invalid)?;
    if revision != input.expected_revision || revision >= MAX_REVISION {
        return Err(MuralError::RevisionConflict);
    }
    let locked:bool=sqlx::query_scalar("select exists(select 1 from public.concert_mural_locks where event_id=$1 and $2>=x_min and $2<x_max_exclusive and $3>=y_min and $3<y_max_exclusive)")
        .bind(actor.event).bind(input.x).bind(input.y).fetch_one(&mut *c).await.map_err(storage)?;
    if locked {
        return Err(MuralError::RegionLocked);
    }
    let cooldown: i64 = row.try_get("cooldown_ms").map_err(invalid)?;
    let next = now + chrono::Duration::milliseconds(cooldown);
    sqlx::query("update public.concert_mural_state set pixels=set_byte(pixels,$2,$3),revision=revision+1,snapshot_id=$4,snapshot_created_at=$5 where event_id=$1")
        .bind(actor.event).bind(input.y*128+input.x).bind(input.color_index).bind(Uuid::new_v4()).bind(now).execute(&mut *c).await.map_err(storage)?;
    sqlx::query("insert into public.concert_mural_operations(event_id,revision,operation_id,actor_id,x,y,color_index,committed_at,next_allowed_at) values($1,$2,$3,$4,$5,$6,$7,$8,$9)")
        .bind(actor.event).bind(revision+1).bind(Uuid::new_v4()).bind(actor.actor).bind(input.x).bind(input.y).bind(input.color_index).bind(now).bind(next).execute(&mut *c).await.map_err(storage)?;
    sqlx::query("insert into public.concert_mural_limits(event_id,actor_id,next_allowed_at) values($1,$2,$3) on conflict(event_id,actor_id) do update set next_allowed_at=excluded.next_allowed_at")
        .bind(actor.event).bind(actor.actor).bind(next).execute(&mut *c).await.map_err(storage)?;
    Ok(revision + 1)
}
#[async_trait]
impl<A: MuralAuthority> MuralReader for PgMuralRepository<A> {
    async fn authorize(&self, actor: MuralActor) -> Result<(), MuralError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        gate(&self.authority, &mut tx, actor).await?;
        tx.commit().await.map_err(storage)
    }
    async fn snapshot(&self, actor: MuralActor) -> Result<MuralSnapshot, MuralError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        gate(&self.authority, &mut tx, actor).await?;
        // Row SHARE lock holds grid+revision+own cooldown at a single write boundary.
        let row =
            sqlx::query("select * from public.concert_mural_state where event_id=$1 for share")
                .bind(actor.event)
                .fetch_optional(&mut *tx)
                .await
                .map_err(storage)?
                .ok_or(MuralError::Denied)?;
        let next=sqlx::query_scalar("select next_allowed_at from public.concert_mural_limits where event_id=$1 and actor_id=$2").bind(actor.event).bind(actor.actor).fetch_optional(&mut *tx).await.map_err(storage)?;
        let server_time = sqlx::query_scalar("select clock_timestamp()")
            .fetch_one(&mut *tx)
            .await
            .map_err(storage)?;
        sqlx::query("insert into public.concert_mural_snapshots(snapshot_id,event_id,revision,state,pixels,palette,created_at) select snapshot_id,event_id,revision,state,pixels,palette,snapshot_created_at from public.concert_mural_state where event_id=$1 on conflict(snapshot_id) do nothing")
            .bind(actor.event).execute(&mut *tx).await.map_err(storage)?;
        let checkpoint_matches:bool=sqlx::query_scalar("select exists(select 1 from public.concert_mural_snapshots s join public.concert_mural_state m on m.snapshot_id=s.snapshot_id where m.event_id=$1 and (s.event_id,s.revision,s.state,s.pixels,s.palette,s.created_at)=(m.event_id,m.revision,m.state,m.pixels,m.palette,m.snapshot_created_at))")
            .bind(actor.event).fetch_one(&mut *tx).await.map_err(storage)?;
        if !checkpoint_matches {
            return Err(MuralError::InvalidState);
        }
        let pixels: Vec<u8> = row.try_get("pixels").map_err(invalid)?;
        let palette: Vec<String> = row.try_get("palette").map_err(invalid)?;
        if pixels.len() != PIXELS || pixels.iter().any(|v| *v > 15) || palette.len() != 16 {
            return Err(MuralError::InvalidState);
        }
        let value = MuralSnapshot {
            snapshot_id: row.try_get("snapshot_id").map_err(invalid)?,
            event_id: actor.event,
            revision: row.try_get("revision").map_err(invalid)?,
            state: MuralState::parse(&row.try_get::<String, _>("state").map_err(invalid)?)
                .ok_or(MuralError::InvalidState)?,
            width: SIDE,
            height: SIDE,
            palette,
            pixels,
            created_at: row.try_get("snapshot_created_at").map_err(invalid)?,
            server_time,
            cooldown_ms: row.try_get("cooldown_ms").map_err(invalid)?,
            next_allowed_at: next,
        };
        if !value.valid() {
            return Err(MuralError::InvalidState);
        }
        tx.commit().await.map_err(storage)?;
        Ok(value)
    }
    async fn delta(&self, actor: MuralActor, after: i64) -> Result<MuralDelta, MuralError> {
        if !(1..=MAX_REVISION).contains(&after) {
            return Err(MuralError::InvalidInput);
        }
        let mut tx = self.pool.begin().await.map_err(storage)?;
        gate(&self.authority, &mut tx, actor).await?;
        let row=sqlx::query("select revision,retained_from_revision,reset_revision from public.concert_mural_state where event_id=$1 for share").bind(actor.event).fetch_optional(&mut *tx).await.map_err(storage)?.ok_or(MuralError::Denied)?;
        let current: i64 = row.try_get("revision").map_err(invalid)?;
        let floor: i64 = row.try_get("retained_from_revision").map_err(invalid)?;
        let reset: i64 = row.try_get("reset_revision").map_err(invalid)?;
        let mut value = MuralDelta::reset(actor.event, after, current);
        if after <= current
            && after >= floor
            && after >= reset
            && current - after <= MAX_DELTA as i64
        {
            let rows=sqlx::query("select operation_id,event_id,revision,x,y,color_index,committed_at from public.concert_mural_operations where event_id=$1 and revision>$2 and revision<=$3 order by revision limit 512")
                .bind(actor.event).bind(after).bind(current).fetch_all(&mut *tx).await.map_err(storage)?;
            value.operations = rows
                .iter()
                .map(public_operation)
                .collect::<Result<_, _>>()?;
            value.reset_required = false;
            if !value.contiguous() {
                value = MuralDelta::reset(actor.event, after, current);
            }
        }
        tx.commit().await.map_err(storage)?;
        Ok(value)
    }
}
fn public_operation(row: &PgRow) -> Result<PublicPixelOperation, MuralError> {
    Ok(PublicPixelOperation {
        operation_id: row.try_get("operation_id").map_err(invalid)?,
        event_id: row.try_get("event_id").map_err(invalid)?,
        revision: row.try_get("revision").map_err(invalid)?,
        x: row.try_get("x").map_err(invalid)?,
        y: row.try_get("y").map_err(invalid)?,
        color_index: row.try_get("color_index").map_err(invalid)?,
        committed_at: row.try_get("committed_at").map_err(invalid)?,
    })
}
fn storage(_: sqlx::Error) -> MuralError {
    MuralError::StorageUnavailable
}
fn invalid(_: impl std::fmt::Debug) -> MuralError {
    MuralError::InvalidState
}

fn from_failure(f: SafeFailure) -> MuralError {
    match f {
        SafeFailure::MuralPaused => MuralError::Paused,
        SafeFailure::MuralFrozen => MuralError::Frozen,
        SafeFailure::MuralCooldown => MuralError::Cooldown(None),
        SafeFailure::MuralRegionLocked => MuralError::RegionLocked,
        SafeFailure::VersionConflict => MuralError::RevisionConflict,
        SafeFailure::EventReadOnly => MuralError::EventReadOnly,
        _ => MuralError::InvalidState,
    }
}
