//! Operator-only Wall moderation on the incumbent aggregate. The public C04
//! admin family was mounted by the Gateway and frozen in the command registry but
//! had no Concert producer; this adapter supplies it with the same receipt/
//! outbox/native-scope boundary as Wall B member tools. Authorization is the
//! actual event OPERATOR role in PostgreSQL, never a browser role claim.
use super::protocol::WallOperation;
use crate::adapters::out::repository::receipt::{CommandProducer, PgCommandRepository};
use concert_application::commands::*;
use concert_application::mural::{tools_b::ToolError, MuralActor, MuralError};
use concert_domain::mural::{
    tools_b::{control_transition, AdminControl, AdminLock, AdminRestore, RegionBounds},
    MuralState,
};
use concert_domain::MAX_REVISION;
use serde::Serialize;
use serde_json::{json, Value};
use sqlx::{PgConnection, PgPool, Row};
use std::sync::{Arc, Mutex};
use tonic::Status;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub enum AdminRequest {
    Control(AdminControl),
    Locks(AdminLock),
    Restore(AdminRestore),
}

pub fn body(op: WallOperation, bytes: &[u8]) -> Result<AdminRequest, Status> {
    if bytes.len() > 8192 || !op.admin() || !op.mutation() {
        return Err(invalid());
    }
    let value = match op {
        WallOperation::AdminControl => {
            let input: AdminControl = serde_json::from_slice(bytes).map_err(|_| invalid())?;
            input.valid().then_some(AdminRequest::Control(input))
        }
        WallOperation::AdminLocks => {
            let input: AdminLock = serde_json::from_slice(bytes).map_err(|_| invalid())?;
            input.valid().then_some(AdminRequest::Locks(input))
        }
        WallOperation::AdminRestore => {
            let input: AdminRestore = serde_json::from_slice(bytes).map_err(|_| invalid())?;
            input.valid().then_some(AdminRequest::Restore(input))
        }
        _ => None,
    };
    value.ok_or_else(invalid)
}

fn invalid() -> Status {
    Status::invalid_argument("Invalid Wall moderation command")
}

fn canonical(request: &AdminRequest) -> CanonicalValue {
    let fields = match request {
        AdminRequest::Control(v) => vec![
            (
                "expected_revision".into(),
                CanonicalValue::Integer(v.expected_revision),
            ),
            (
                "action".into(),
                CanonicalValue::String(
                    match v.action {
                        concert_domain::mural::tools_b::AdminAction::Pause => "PAUSE",
                        concert_domain::mural::tools_b::AdminAction::Resume => "RESUME",
                        concert_domain::mural::tools_b::AdminAction::Freeze => "FREEZE",
                    }
                    .into(),
                ),
            ),
            ("reason".into(), CanonicalValue::String(v.reason.clone())),
        ],
        AdminRequest::Locks(v) => vec![
            (
                "expected_revision".into(),
                CanonicalValue::Integer(v.expected_revision),
            ),
            (
                "x_min".into(),
                CanonicalValue::Integer(v.region.x_min.into()),
            ),
            (
                "y_min".into(),
                CanonicalValue::Integer(v.region.y_min.into()),
            ),
            (
                "x_max_exclusive".into(),
                CanonicalValue::Integer(v.region.x_max_exclusive.into()),
            ),
            (
                "y_max_exclusive".into(),
                CanonicalValue::Integer(v.region.y_max_exclusive.into()),
            ),
            ("locked".into(), CanonicalValue::Bool(v.locked)),
            ("reason".into(), CanonicalValue::String(v.reason.clone())),
        ],
        AdminRequest::Restore(v) => vec![
            (
                "expected_revision".into(),
                CanonicalValue::Integer(v.expected_revision),
            ),
            (
                "snapshot_id".into(),
                CanonicalValue::String(v.snapshot_id.to_string()),
            ),
            ("reason".into(), CanonicalValue::String(v.reason.clone())),
        ],
    };
    CanonicalValue::Object(fields)
}

#[derive(Serialize)]
struct AdminState {
    event_id: Uuid,
    state: String,
    revision: i64,
    snapshot_id: Uuid,
    locks: Vec<RegionBounds>,
}

pub struct MuralAdmin {
    pool: PgPool,
    keys: Arc<FingerprintKeys>,
}

impl MuralAdmin {
    pub fn new(pool: PgPool, keys: Arc<FingerprintKeys>) -> Self {
        Self { pool, keys }
    }

    /// Authoritative admin projection. Reads are operator-fenced like writes so a
    /// member never learns moderation metadata through an inference oracle.
    pub async fn state(&self, actor: MuralActor) -> Result<Value, ToolError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        authorize_in(&mut tx, actor, false).await?;
        let value = state_in(&mut tx, actor.event, None).await?;
        tx.commit().await.map_err(storage)?;
        Ok(value)
    }

    pub async fn execute(
        &self,
        actor: MuralActor,
        op: WallOperation,
        key: Uuid,
        command: AdminRequest,
    ) -> Result<Value, ToolError> {
        if !actor.valid() || key.is_nil() {
            return Err(ToolError::Mural(MuralError::InvalidInput));
        }
        let scope = Scope {
            actor: actor.actor,
            event: actor.event,
            key,
            operation: RegisteredOperation::find(op.id())
                .map_err(|e| ToolError::Mural(e.into()))?,
        };
        let value = canonical(&command);
        let event = actor.event.to_string();
        let request = CanonicalRequest::json(scope, &[("eventId", &event)], &[], Some(&value))
            .map_err(|e| ToolError::Mural(e.into()))?;
        let producer = AdminProducer {
            actor,
            operation: op,
            key,
            scope,
            command,
            error: Mutex::new(None),
        };
        let result = PgCommandRepository::new(self.pool.clone())
            .execute(&request, actor.event, &self.keys, &producer)
            .await;
        let recorded = producer.error.lock().map_err(|_| invalid_state())?.take();
        let receipt = match result {
            Ok(receipt) => receipt,
            Err(error) => return Err(recorded.unwrap_or_else(|| ToolError::Mural(error.into()))),
        };
        if let Some(failure) = receipt.failure {
            return Err(recorded.unwrap_or_else(|| from_failure(failure)));
        }
        if receipt.state != ReceiptState::Succeeded {
            return Err(ToolError::Unresolved);
        }
        let revision = receipt
            .reference
            .map(|reference| reference.revision)
            .ok_or_else(invalid_state)?;
        let mut tx = self.pool.begin().await.map_err(storage)?;
        authorize_in(&mut tx, actor, false).await?;
        let value = state_in(&mut tx, actor.event, Some(revision)).await?;
        tx.commit().await.map_err(storage)?;
        Ok(value)
    }

    /// Original-key receipt recovery. It never applies effects or infers success
    /// from current wall equality.
    pub async fn status(
        &self,
        actor: MuralActor,
        operation: RegisteredOperation,
        key: Uuid,
    ) -> Result<Value, ToolError> {
        let scope = Scope {
            actor: actor.actor,
            event: actor.event,
            key,
            operation,
        };
        let producer = AdminStatusProducer { actor };
        let receipt = PgCommandRepository::new(self.pool.clone())
            .lookup(scope, actor.event, &producer)
            .await
            .map_err(|error| ToolError::Mural(error.into()))?
            .ok_or(ToolError::Mural(MuralError::Denied))?;
        Ok(json!({
            "receipt_id": receipt.id,
            "event_id": scope.event,
            "actor_id": scope.actor,
            "operation_id": operation.id(),
            "idempotency_key": key,
            "state": receipt.state.as_str(),
            "resource_id": receipt.reference.map(|r| r.resource),
            "resource_revision": receipt.reference.map(|r| r.revision),
            "failure_reason": receipt.failure.map(|f| f.as_str()),
            "created_at": receipt.created_at,
            "expires_at": receipt.expires_at,
            "result_status": receipt.result_status,
        }))
    }
}

struct AdminProducer {
    actor: MuralActor,
    operation: WallOperation,
    key: Uuid,
    scope: Scope,
    command: AdminRequest,
    error: Mutex<Option<ToolError>>,
}
impl AdminProducer {
    fn fail<T>(&self, error: ToolError) -> Result<T, CommandError> {
        *self.error.lock().map_err(|_| CommandError::InvalidState)? = Some(error);
        Err(CommandError::Unavailable)
    }
}

#[async_trait::async_trait]
impl CommandProducer for AdminProducer {
    async fn authorize(
        &self,
        c: &mut PgConnection,
        scope: Scope,
        target: Uuid,
    ) -> Result<(), CommandError> {
        if scope.actor != self.actor.actor
            || scope.event != self.actor.event
            || scope.key != self.key
            || scope.operation.id() != self.operation.id()
            || target != self.actor.event
            || self.scope != scope
        {
            return Err(CommandError::Denied);
        }
        match authorize_in(c, self.actor, true).await {
            Ok(()) => Ok(()),
            Err(error) => self.fail(error),
        }
    }

    async fn apply(
        &self,
        c: &mut PgConnection,
        _: Scope,
        target: Uuid,
    ) -> Result<(Completion, Option<Invalidation>), CommandError> {
        match apply_in(c, self.actor, &self.command, target).await {
            Ok(Some(revision)) => Ok((
                Completion::Succeeded {
                    reference: SafeReference {
                        resource: target,
                        revision,
                    },
                    status: 200,
                },
                Some(Invalidation {
                    reference: SafeReference {
                        resource: target,
                        revision,
                    },
                }),
            )),
            Ok(None) => Ok((Completion::Failed(SafeFailure::InvalidTransition), None)),
            Err(error) => self.fail(error),
        }
    }
}

struct AdminStatusProducer {
    actor: MuralActor,
}
#[async_trait::async_trait]
impl CommandProducer for AdminStatusProducer {
    async fn authorize(
        &self,
        c: &mut PgConnection,
        scope: Scope,
        target: Uuid,
    ) -> Result<(), CommandError> {
        if scope.actor != self.actor.actor
            || scope.event != self.actor.event
            || target != self.actor.event
            || !matches!(
                scope.operation.id(),
                "controlConcertMural" | "lockConcertMuralRegion" | "restoreConcertMural"
            )
        {
            return Err(CommandError::Denied);
        }
        authorize_in(c, self.actor, false)
            .await
            .map_err(|_| CommandError::Denied)
    }

    async fn apply(
        &self,
        _: &mut PgConnection,
        _: Scope,
        _: Uuid,
    ) -> Result<(Completion, Option<Invalidation>), CommandError> {
        Err(CommandError::Denied)
    }
}

async fn authorize_in(
    c: &mut PgConnection,
    actor: MuralActor,
    write: bool,
) -> Result<(), ToolError> {
    if !actor.valid() {
        return Err(ToolError::Mural(MuralError::InvalidInput));
    }
    if !super::deletion::fence_writer_in(c, actor.actor)
        .await
        .map_err(storage)?
    {
        return Err(ToolError::Forbidden);
    }
    let phase: Option<String> =
        sqlx::query_scalar("select phase from public.concert_events where event_id=$1 for share")
            .bind(actor.event)
            .fetch_optional(&mut *c)
            .await
            .map_err(storage)?;
    let phase = phase.ok_or(ToolError::Forbidden)?;
    if !matches!(phase.as_str(), "OPEN" | "LIVE" | "READ_ONLY") {
        return Err(ToolError::Forbidden);
    }
    let role: Option<i64> = sqlx::query_scalar(
        "select revision from public.concert_event_operator_roles where event_id=$1 and user_id=$2 and role='OPERATOR' and revoked_at is null for share",
    )
    .bind(actor.event)
    .bind(actor.actor)
    .fetch_optional(&mut *c)
    .await
    .map_err(storage)?;
    let role = role.ok_or(ToolError::Forbidden)?;
    if !(1..=MAX_REVISION).contains(&role) {
        return Err(ToolError::Forbidden);
    }
    let query = if write {
        "select event_id from public.concert_mural_state where event_id=$1 for update"
    } else {
        "select event_id from public.concert_mural_state where event_id=$1 for share"
    };
    sqlx::query_scalar::<_, Uuid>(query)
        .bind(actor.event)
        .fetch_optional(&mut *c)
        .await
        .map_err(storage)?
        .ok_or(ToolError::Mural(MuralError::Denied))?;
    Ok(())
}

/// Returns the new revision, or None for a deliberate frozen transition refusal.
async fn apply_in(
    c: &mut PgConnection,
    actor: MuralActor,
    command: &AdminRequest,
    target: Uuid,
) -> Result<Option<i64>, ToolError> {
    if target != actor.event {
        return Err(ToolError::Forbidden);
    }
    let phase: String =
        sqlx::query_scalar("select phase from public.concert_events where event_id=$1 for share")
            .bind(actor.event)
            .fetch_one(&mut *c)
            .await
            .map_err(storage)?;
    let row = sqlx::query(
        "select state,revision,snapshot_id from public.concert_mural_state where event_id=$1 for update",
    )
    .bind(actor.event)
    .fetch_one(&mut *c)
    .await
    .map_err(storage)?;
    let current: i64 = row.try_get("revision").map_err(row_invalid)?;
    let state = MuralState::parse(
        row.try_get::<String, _>("state")
            .map_err(row_invalid)?
            .as_str(),
    )
    .ok_or_else(invalid_state)?;
    let expected = expected_revision(command);
    if expected != current || current >= MAX_REVISION {
        return Err(ToolError::Mural(MuralError::RevisionConflict));
    }
    match command {
        AdminRequest::Control(control) => {
            if phase == "READ_ONLY"
                && control.action != concert_domain::mural::tools_b::AdminAction::Freeze
            {
                return Err(ToolError::Mural(MuralError::EventReadOnly));
            }
            let Some(next) = control_transition(state, control.action) else {
                return Ok(None);
            };
            advance(c, actor.event, next).await?;
        }
        AdminRequest::Locks(lock) => {
            if state == MuralState::Frozen || phase == "READ_ONLY" {
                return Ok(None);
            }
            if lock.locked {
                sqlx::query("insert into public.concert_mural_locks(event_id,lock_id,x_min,y_min,x_max_exclusive,y_max_exclusive) values($1,$2,$3,$4,$5,$6)")
                    .bind(actor.event).bind(Uuid::new_v4()).bind(lock.region.x_min).bind(lock.region.y_min)
                    .bind(lock.region.x_max_exclusive).bind(lock.region.y_max_exclusive)
                    .execute(&mut *c).await.map_err(storage)?;
            } else {
                sqlx::query("delete from public.concert_mural_locks where event_id=$1 and x_min=$2 and y_min=$3 and x_max_exclusive=$4 and y_max_exclusive=$5")
                    .bind(actor.event).bind(lock.region.x_min).bind(lock.region.y_min)
                    .bind(lock.region.x_max_exclusive).bind(lock.region.y_max_exclusive)
                    .execute(&mut *c).await.map_err(storage)?;
            }
            advance(c, actor.event, state).await?;
        }
        AdminRequest::Restore(restore) => {
            if state != MuralState::Paused || phase == "READ_ONLY" {
                return Ok(None);
            }
            let snapshot = sqlx::query(
                "select pixels,palette from public.concert_mural_snapshots where snapshot_id=$1 and event_id=$2 for share",
            )
            .bind(restore.snapshot_id)
            .bind(actor.event)
            .fetch_optional(&mut *c)
            .await
            .map_err(storage)?;
            let Some(snapshot) = snapshot else {
                return Ok(None);
            };
            let pixels: Vec<u8> = snapshot.try_get("pixels").map_err(row_invalid)?;
            let palette: Vec<String> = snapshot.try_get("palette").map_err(row_invalid)?;
            if pixels.len() != concert_domain::mural::PIXELS
                || palette.len() != 16
                || !palette.iter().all(|color| {
                    color.len() == 7
                        && color.starts_with('#')
                        && color[1..].bytes().all(|b| b.is_ascii_hexdigit())
                })
            {
                return Err(invalid_state());
            }
            sqlx::query(
                "update public.concert_mural_state set pixels=$2,palette=$3,state='PAUSED',revision=revision+1,reset_revision=revision+1,snapshot_id=$4,snapshot_created_at=clock_timestamp() where event_id=$1",
            )
            .bind(actor.event)
            .bind(pixels)
            .bind(palette)
            .bind(Uuid::new_v4())
            .execute(&mut *c)
            .await
            .map_err(storage)?;
        }
    }
    checkpoint(c, actor.event).await?;
    let revision: i64 =
        sqlx::query_scalar("select revision from public.concert_mural_state where event_id=$1")
            .bind(actor.event)
            .fetch_one(&mut *c)
            .await
            .map_err(storage)?;
    Ok(Some(revision))
}

fn expected_revision(request: &AdminRequest) -> i64 {
    match request {
        AdminRequest::Control(v) => v.expected_revision,
        AdminRequest::Locks(v) => v.expected_revision,
        AdminRequest::Restore(v) => v.expected_revision,
    }
}

async fn advance(c: &mut PgConnection, event: Uuid, state: MuralState) -> Result<(), ToolError> {
    let state = match state {
        MuralState::Editable => "EDITABLE",
        MuralState::Paused => "PAUSED",
        MuralState::Frozen => "FROZEN",
    };
    sqlx::query("update public.concert_mural_state set state=$2,revision=revision+1,reset_revision=revision+1,snapshot_id=$3,snapshot_created_at=clock_timestamp() where event_id=$1")
        .bind(event).bind(state).bind(Uuid::new_v4())
        .execute(&mut *c).await.map_err(storage)?;
    Ok(())
}

async fn checkpoint(c: &mut PgConnection, event: Uuid) -> Result<(), ToolError> {
    sqlx::query("insert into public.concert_mural_snapshots(snapshot_id,event_id,revision,state,pixels,palette,created_at) select snapshot_id,event_id,revision,state,pixels,palette,snapshot_created_at from public.concert_mural_state where event_id=$1")
        .bind(event).execute(&mut *c).await.map_err(storage)?;
    Ok(())
}

async fn state_in(
    c: &mut PgConnection,
    event: Uuid,
    revision: Option<i64>,
) -> Result<Value, ToolError> {
    let current: i64 =
        sqlx::query_scalar("select revision from public.concert_mural_state where event_id=$1")
            .bind(event)
            .fetch_one(&mut *c)
            .await
            .map_err(storage)?;
    let revision = revision.unwrap_or(current);
    if !(1..=current).contains(&revision) {
        return Err(invalid_state());
    }
    if revision == current {
        sqlx::query("insert into public.concert_mural_snapshots(snapshot_id,event_id,revision,state,pixels,palette,created_at) select snapshot_id,event_id,revision,state,pixels,palette,snapshot_created_at from public.concert_mural_state where event_id=$1 on conflict(snapshot_id) do nothing")
            .bind(event).execute(&mut *c).await.map_err(storage)?;
        let matches: bool = sqlx::query_scalar("select exists(select 1 from public.concert_mural_snapshots s join public.concert_mural_state m on m.snapshot_id=s.snapshot_id where m.event_id=$1 and (s.event_id,s.revision,s.state,s.pixels,s.palette,s.created_at)=(m.event_id,m.revision,m.state,m.pixels,m.palette,m.snapshot_created_at))")
            .bind(event).fetch_one(&mut *c).await.map_err(storage)?;
        if !matches {
            return Err(invalid_state());
        }
    }
    let (state, snapshot_id, revision): (String, Uuid, i64) = if revision == current {
        let row = sqlx::query(
            "select state,snapshot_id,revision from public.concert_mural_state where event_id=$1",
        )
        .bind(event)
        .fetch_one(&mut *c)
        .await
        .map_err(storage)?;
        (
            row.try_get("state").map_err(row_invalid)?,
            row.try_get("snapshot_id").map_err(row_invalid)?,
            row.try_get("revision").map_err(row_invalid)?,
        )
    } else {
        let row = sqlx::query("select state,snapshot_id,revision from public.concert_mural_snapshots where event_id=$1 and revision=$2")
            .bind(event).bind(revision).fetch_optional(&mut *c).await.map_err(storage)?
            .ok_or_else(invalid_state)?;
        (
            row.try_get("state").map_err(row_invalid)?,
            row.try_get("snapshot_id").map_err(row_invalid)?,
            row.try_get("revision").map_err(row_invalid)?,
        )
    };
    if MuralState::parse(&state).is_none() {
        return Err(invalid_state());
    }
    let locks = sqlx::query("select x_min,y_min,x_max_exclusive,y_max_exclusive from public.concert_mural_locks where event_id=$1 order by x_min,y_min,x_max_exclusive,y_max_exclusive limit 128")
        .bind(event).fetch_all(&mut *c).await.map_err(storage)?;
    let locks = locks
        .iter()
        .map(|row| {
            Ok::<_, sqlx::Error>(RegionBounds {
                x_min: row.try_get("x_min")?,
                y_min: row.try_get("y_min")?,
                x_max_exclusive: row.try_get("x_max_exclusive")?,
                y_max_exclusive: row.try_get("y_max_exclusive")?,
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(storage)?;
    if locks.iter().any(|region| !region.valid()) {
        return Err(invalid_state());
    }
    Ok(serde_json::to_value(AdminState {
        event_id: event,
        state,
        revision,
        snapshot_id,
        locks,
    })
    .map_err(|_| invalid_state())?)
}

fn from_failure(failure: SafeFailure) -> ToolError {
    match failure {
        SafeFailure::InvalidTransition => ToolError::InvalidTransition,
        SafeFailure::VersionConflict => ToolError::Mural(MuralError::RevisionConflict),
        SafeFailure::MuralPaused => ToolError::Mural(MuralError::Paused),
        SafeFailure::MuralFrozen => ToolError::Mural(MuralError::Frozen),
        SafeFailure::EventReadOnly => ToolError::Mural(MuralError::EventReadOnly),
        SafeFailure::ResourceUnavailable => ToolError::Mural(MuralError::Denied),
        _ => ToolError::Unresolved,
    }
}

fn storage(_: sqlx::Error) -> ToolError {
    ToolError::Mural(MuralError::StorageUnavailable)
}
fn invalid_state() -> ToolError {
    ToolError::Mural(MuralError::InvalidState)
}
fn row_invalid(_: sqlx::Error) -> ToolError {
    invalid_state()
}

#[cfg(test)]
mod tests {
    use super::*;
    use concert_domain::mural::tools_b::AdminAction;

    fn control(action: &str) -> Vec<u8> {
        format!(r#"{{"expected_revision":2,"action":"{action}","reason":"operator review"}}"#)
            .into_bytes()
    }

    #[test]
    fn closed_admin_bodies_reject_extra_fields_and_illegal_transitions() {
        assert!(body(WallOperation::AdminControl, &control("PAUSE")).is_ok());
        assert!(body(
            WallOperation::AdminControl,
            br#"{"expected_revision":2,"action":"PAUSE","reason":"x","role":"OPERATOR"}"#
        )
        .is_err());
        assert!(body(
            WallOperation::AdminControl,
            br#"{"expected_revision":0,"action":"PAUSE","reason":"x"}"#
        )
        .is_err());
        assert!(body(
            WallOperation::AdminControl,
            br#"{"expected_revision":2,"action":"DELETE","reason":"x"}"#
        )
        .is_err());
        assert!(body(
            WallOperation::AdminControl,
            br#"{"expected_revision":2,"action":"PAUSE","reason":"   "}"#
        )
        .is_err());
        assert!(body(WallOperation::AdminState, &control("PAUSE")).is_err());
        use MuralState::{Editable, Frozen, Paused};
        assert_eq!(
            control_transition(Editable, AdminAction::Pause),
            Some(Paused)
        );
        assert_eq!(
            control_transition(Paused, AdminAction::Resume),
            Some(Editable)
        );
        assert_eq!(
            control_transition(Editable, AdminAction::Freeze),
            Some(Frozen)
        );
        assert_eq!(
            control_transition(Paused, AdminAction::Freeze),
            Some(Frozen)
        );
        assert_eq!(control_transition(Frozen, AdminAction::Resume), None);
        assert_eq!(control_transition(Editable, AdminAction::Resume), None);
    }

    #[test]
    fn decorated_bodies_are_closed() {
        assert!(body(
            WallOperation::AdminLocks,
            br#"{"expected_revision":2,"x_min":0,"y_min":0,"x_max_exclusive":1,"y_max_exclusive":1,"locked":true,"reason":"protect"}"#
        )
        .is_ok());
        assert!(body(
            WallOperation::AdminLocks,
            br#"{"expected_revision":2,"x_min":1,"y_min":0,"x_max_exclusive":1,"y_max_exclusive":1,"locked":true,"reason":"protect"}"#
        )
        .is_err());
        assert!(body(
            WallOperation::AdminRestore,
            br#"{"expected_revision":2,"snapshot_id":"00000000-0000-0000-0000-000000000000","reason":"restore"}"#
        )
        .is_err());
    }
}
