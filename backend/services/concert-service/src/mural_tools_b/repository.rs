//! Extends the incumbent aggregate/BE15 receipt transaction, never a second wall.
use crate::adapters::out::repository::{
    mural::{gate, MuralAuthority},
    receipt::{CommandProducer, PgCommandRepository},
};
use chrono::{DateTime, Duration, Utc};
use concert_application::{
    commands::*,
    mural::{tools_b::*, MuralActor, MuralError},
};
use concert_domain::{
    mural::{tools_b::*, PIXELS},
    MAX_REVISION,
};
use sqlx::{PgConnection, PgPool, Row};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

fn storage(_: sqlx::Error) -> ToolError {
    MuralError::StorageUnavailable.into()
}
fn invalid() -> ToolError {
    MuralError::InvalidState.into()
}
fn planned(error: PlanError) -> ToolError {
    match error {
        PlanError::NoChange => ToolError::NoChange,
        PlanError::FillTooLarge => ToolError::FillTooLarge,
        PlanError::HistoryMismatch => ToolError::UndoUnavailable,
        PlanError::InvalidInput => MuralError::InvalidInput.into(),
    }
}
fn operation(body: ActionBody) -> &'static str {
    match body {
        ActionBody::Pencil(_) => "placeConcertPixel",
        ActionBody::Erase(_) => "eraseConcertMuralPixel",
        ActionBody::Fill(_) => "fillConcertMuralRegion",
        ActionBody::Undo(_) => "undoConcertMuralAction",
    }
}
fn kind(body: ActionBody) -> &'static str {
    match body {
        ActionBody::Pencil(_) => "PENCIL",
        ActionBody::Erase(_) => "ERASE",
        ActionBody::Fill(_) => "FILL",
        ActionBody::Undo(_) => "UNDO",
    }
}
fn canonical(body: ActionBody) -> Result<CanonicalValue, ToolError> {
    let value = serde_json::to_value(body).map_err(|_| invalid())?;
    let input = value
        .get("body")
        .and_then(|v| v.as_object())
        .ok_or_else(invalid)?;
    let mut fields = Vec::new();
    for (name, value) in input {
        let value = if let Some(v) = value.as_i64() {
            CanonicalValue::Integer(v)
        } else if let Some(v) = value.as_str() {
            CanonicalValue::String(v.into())
        } else {
            return Err(invalid());
        };
        fields.push((name.clone(), value));
    }
    Ok(CanonicalValue::Object(fields))
}
async fn provisioned(c: &mut PgConnection, event: Uuid) -> Result<bool, ToolError> {
    let exists: bool =
        sqlx::query_scalar("select to_regclass('public.concert_mural_tool_config') is not null")
            .fetch_one(&mut *c)
            .await
            .map_err(storage)?;
    if !exists {
        return Ok(false);
    }
    sqlx::query_scalar("select exists(select 1 from public.concert_mural_tool_config where event_id=$1 and background_index=0)")
        .bind(event).fetch_one(c).await.map_err(storage)
}
pub(crate) async fn correlate_in(
    c: &mut PgConnection,
    actor: MuralActor,
    key: Uuid,
    body: ActionBody,
) -> Result<(), ToolError> {
    if !provisioned(c, actor.event).await? {
        return Ok(());
    }
    let pending: bool = sqlx::query_scalar("select exists(select 1 from public.concert_command_receipts where event_id=$1 and actor_id=$2 and operation_id in ('placeConcertPixel','eraseConcertMuralPixel','fillConcertMuralRegion','undoConcertMuralAction') and state='PENDING' and not(operation_id=$3 and idempotency_key=$4))")
        .bind(actor.event).bind(actor.actor).bind(operation(body)).bind(key).fetch_one(&mut *c).await.map_err(storage)?;
    if pending {
        return Err(ToolError::Unresolved);
    }
    let command = serde_json::to_value(body).map_err(|_| invalid())?;
    sqlx::query("insert into public.concert_mural_tool_correlations(event_id,actor_id,operation_id,idempotency_key,command) values($1,$2,$3,$4,$5) on conflict(event_id,actor_id,idempotency_key) do nothing")
        .bind(actor.event).bind(actor.actor).bind(operation(body)).bind(key).bind(&command).execute(&mut *c).await.map_err(storage)?;
    let exact: bool = sqlx::query_scalar("select exists(select 1 from public.concert_mural_tool_correlations where event_id=$1 and actor_id=$2 and idempotency_key=$3 and command=$4)")
        .bind(actor.event).bind(actor.actor).bind(key).bind(command).fetch_one(c).await.map_err(storage)?;
    if !exact {
        return Err(MuralError::Command(CommandError::Conflict).into());
    }
    Ok(())
}
async fn charges(
    c: &mut PgConnection,
    actor: MuralActor,
    now: DateTime<Utc>,
) -> Result<Vec<Charge>, ToolError> {
    let rows=sqlx::query("select committed_at,changed_cells from public.concert_mural_tool_charges where event_id=$1 and actor_id=$2 and committed_at>$3-interval '600 seconds' and committed_at<=$3 order by committed_at limit 513")
        .bind(actor.event).bind(actor.actor).bind(now).fetch_all(c).await.map_err(storage)?;
    if rows.len() > 512 {
        return Err(invalid());
    }
    rows.iter()
        .map(|r| {
            Ok(Charge {
                committed_at: r.try_get("committed_at").map_err(storage)?,
                cells: r.try_get::<i32, _>("changed_cells").map_err(storage)? as u32,
            })
        })
        .collect()
}
async fn budget(
    c: &mut PgConnection,
    actor: MuralActor,
    now: DateTime<Utc>,
    cost: usize,
) -> Result<(), ToolError> {
    let status = allowance(now, &charges(c, actor, now).await?, cost as u32);
    if cost == 0 || cost > FILL_LIMIT {
        return Err(invalid());
    }
    if cost as u32 > status.remaining {
        return Err(ToolError::CellBudget(status.next_available_at));
    }
    Ok(())
}
pub(crate) async fn pencil_plan_in(
    c: &mut PgConnection,
    actor: MuralActor,
    grid: &[u8],
    input: concert_domain::mural::PlacePixel,
    now: DateTime<Utc>,
) -> Result<Option<Vec<CellChange>>, MuralError> {
    if !provisioned(c, actor.event).await.map_err(legacy)? {
        return Ok(None);
    }
    let plan = pencil(grid, input).map_err(planned).map_err(legacy)?;
    budget(c, actor, now, plan.len()).await.map_err(legacy)?;
    Ok(Some(plan))
}
pub(crate) fn legacy(error: ToolError) -> MuralError {
    match error {
        ToolError::Mural(e) => e,
        ToolError::NoChange => MuralError::InvalidInput,
        ToolError::CellBudget(_) => MuralError::Command(CommandError::Capacity),
        _ => MuralError::StorageUnavailable,
    }
}
#[allow(clippy::too_many_arguments)]
pub(crate) async fn record_in(
    c: &mut PgConnection,
    actor: MuralActor,
    key: Uuid,
    body: ActionBody,
    id: Uuid,
    revision: i64,
    plan: &[CellChange],
    now: DateTime<Utc>,
    next: DateTime<Utc>,
    reset: bool,
    undoes: Option<Uuid>,
) -> Result<(), ToolError> {
    if plan.is_empty() || plan.len() > FILL_LIMIT {
        return Err(invalid());
    }
    sqlx::query("insert into public.concert_mural_tool_actions(action_id,event_id,actor_id,operation_id,idempotency_key,kind,revision,command,cells,changed_cells,committed_at,next_allowed_at,undo_expires_at,reset_required,undoes) values($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$11+interval '600 seconds',$13,$14)")
        .bind(id).bind(actor.event).bind(actor.actor).bind(operation(body)).bind(key).bind(kind(body)).bind(revision)
        .bind(serde_json::to_value(body).map_err(|_|invalid())?).bind(serde_json::to_value(plan).map_err(|_|invalid())?).bind(plan.len() as i32).bind(now).bind(next).bind(reset).bind(undoes)
        .execute(&mut *c).await.map_err(storage)?;
    sqlx::query("insert into public.concert_mural_tool_charges(action_id,event_id,actor_id,committed_at,changed_cells) values($1,$2,$3,$4,$5)")
        .bind(id).bind(actor.event).bind(actor.actor).bind(now).bind(plan.len() as i32).execute(c).await.map_err(storage)?;
    Ok(())
}
pub(crate) async fn complete_in(
    c: &mut PgConnection,
    scope: Scope,
    receipt: &SafeReceipt,
) -> Result<(), ToolError> {
    if !provisioned(c, scope.event).await? {
        return Ok(());
    }
    sqlx::query("update public.concert_mural_tool_correlations set receipt_id=$5 where event_id=$1 and actor_id=$2 and operation_id=$3 and idempotency_key=$4 and (receipt_id is null or receipt_id=$5)")
        .bind(scope.event).bind(scope.actor).bind(scope.operation.id()).bind(scope.key).bind(receipt.id).execute(&mut *c).await.map_err(storage)?;
    if receipt.state == ReceiptState::Succeeded {
        sqlx::query("update public.concert_mural_tool_actions set receipt_id=$5,undo_expires_at=least(undo_expires_at,$6) where event_id=$1 and actor_id=$2 and operation_id=$3 and idempotency_key=$4 and receipt_id is null")
            .bind(scope.event).bind(scope.actor).bind(scope.operation.id()).bind(scope.key).bind(receipt.id).bind(receipt.expires_at).execute(c).await.map_err(storage)?;
    }
    Ok(())
}
fn own(row: &sqlx::postgres::PgRow) -> Result<OwnAction, ToolError> {
    let command: ActionBody =
        serde_json::from_value(row.try_get("command").map_err(storage)?).map_err(|_| invalid())?;
    Ok(OwnAction {
        action_id: row.try_get("action_id").map_err(storage)?,
        receipt_id: row.try_get("receipt_id").map_err(storage)?,
        event_id: row.try_get("event_id").map_err(storage)?,
        actor_id: row.try_get("actor_id").map_err(storage)?,
        command,
        idempotency_key: row.try_get("idempotency_key").map_err(storage)?,
        kind: match kind(command) {
            "PENCIL" => ActionKind::Pencil,
            "ERASE" => ActionKind::Erase,
            "FILL" => ActionKind::Fill,
            _ => ActionKind::Undo,
        },
        revision: row.try_get("revision").map_err(storage)?,
        changed_cells: row.try_get::<i32, _>("changed_cells").map_err(storage)? as u32,
        committed_at: row.try_get("committed_at").map_err(storage)?,
        next_allowed_at: row.try_get("next_allowed_at").map_err(storage)?,
        undo_expires_at: row.try_get("undo_expires_at").map_err(storage)?,
        reset_required: row.try_get("reset_required").map_err(storage)?,
    })
}
pub struct PgMuralTools<A> {
    pool: PgPool,
    authority: A,
    keys: Arc<FingerprintKeys>,
}
impl<A: MuralAuthority> PgMuralTools<A> {
    pub fn new(pool: PgPool, authority: A, keys: Arc<FingerprintKeys>) -> Self {
        Self {
            pool,
            authority,
            keys,
        }
    }
    pub async fn command_status(
        &self,
        actor: MuralActor,
        operation: RegisteredOperation,
        key: Uuid,
    ) -> Result<serde_json::Value, ToolError> {
        if !actor.valid()
            || key.is_nil()
            || !matches!(
                operation.id(),
                "placeConcertPixel"
                    | "eraseConcertMuralPixel"
                    | "fillConcertMuralRegion"
                    | "undoConcertMuralAction"
            )
        {
            return Err(MuralError::InvalidInput.into());
        }
        let scope = Scope {
            actor: actor.actor,
            event: actor.event,
            operation,
            key,
        };
        let receipt = PgCommandRepository::new(self.pool.clone())
            .lookup(
                scope,
                actor.event,
                &WallStatusProducer { repo: self, scope },
            )
            .await
            .map_err(MuralError::from)?
            .ok_or(MuralError::Denied)?;
        if receipt.scope != scope {
            return Err(invalid());
        }
        Ok(
            serde_json::json!({"receipt_id":receipt.id,"event_id":scope.event,"actor_id":scope.actor,"operation_id":operation.id(),"idempotency_key":key,"state":receipt.state.as_str(),"resource_id":receipt.reference.map(|r|r.resource),"resource_revision":receipt.reference.map(|r|r.revision),"failure_reason":receipt.failure.map(|f|f.as_str()),"created_at":receipt.created_at,"expires_at":receipt.expires_at,"result_status":receipt.result_status}),
        )
    }
    async fn locked<'a>(
        &self,
        tx: &mut PgConnection,
        actor: MuralActor,
        write: bool,
    ) -> Result<sqlx::postgres::PgRow, ToolError> {
        gate(&self.authority, tx, actor).await?;
        let query = if write {
            "select * from public.concert_mural_state where event_id=$1 for update"
        } else {
            "select * from public.concert_mural_state where event_id=$1 for share"
        };
        sqlx::query(query)
            .bind(actor.event)
            .fetch_optional(tx)
            .await
            .map_err(storage)?
            .ok_or(MuralError::Denied.into())
    }
}
#[async_trait::async_trait]
impl<A: MuralAuthority> MuralTools for PgMuralTools<A> {
    async fn execute(
        &self,
        actor: MuralActor,
        key: Uuid,
        command: ToolCommand,
    ) -> Result<OwnAction, ToolError> {
        if !actor.valid() || key.is_nil() {
            return Err(MuralError::InvalidInput.into());
        }
        let body = match command {
            ToolCommand::Erase(p) => ActionBody::Erase(p),
            ToolCommand::Fill(p) => ActionBody::Fill(p),
            ToolCommand::Undo(p) => ActionBody::Undo(p),
        };
        // Use the same closed parser for direct callers as for transport callers.
        let bytes = serde_json::to_vec(&serde_json::to_value(body).map_err(|_| invalid())?["body"])
            .map_err(|_| invalid())?;
        let op = protocol_op(body);
        super::protocol::body(op, &bytes)
            .map_err(|_| ToolError::Mural(MuralError::InvalidInput))?;
        let scope = Scope {
            actor: actor.actor,
            event: actor.event,
            key,
            operation: RegisteredOperation::find(command.operation())
                .map_err(|e| ToolError::Mural(e.into()))?,
        };
        let value = canonical(body)?;
        let event = actor.event.to_string();
        let request = CanonicalRequest::json(scope, &[("eventId", &event)], &[], Some(&value))
            .map_err(|e| ToolError::Mural(e.into()))?;
        let producer = ToolProducer {
            repo: self,
            actor,
            key,
            command,
            body,
            error: Mutex::new(None),
        };
        let result = PgCommandRepository::new(self.pool.clone())
            .execute(&request, actor.event, &self.keys, &producer)
            .await;
        let recorded = producer.error.lock().map_err(|_| invalid())?.take();
        let receipt = match result {
            Ok(r) => r,
            Err(e) => return Err(recorded.unwrap_or(ToolError::Mural(e.into()))),
        };
        if let Some(reason) = receipt.failure {
            return Err(recorded.unwrap_or_else(|| failure(reason)));
        }
        if receipt.state != ReceiptState::Succeeded {
            return Err(ToolError::Unresolved);
        }
        self.action_result(actor, key).await
    }
    async fn action_result(&self, actor: MuralActor, key: Uuid) -> Result<OwnAction, ToolError> {
        if !actor.valid() || key.is_nil() {
            return Err(MuralError::InvalidInput.into());
        }
        let mut tx = self.pool.begin().await.map_err(storage)?;
        self.locked(&mut tx, actor, false).await?;
        let row=sqlx::query("select a.* from public.concert_mural_tool_actions a join public.concert_command_receipts r using(receipt_id) where a.event_id=$1 and a.actor_id=$2 and a.idempotency_key=$3 and r.state='SUCCEEDED' and r.expires_at>clock_timestamp() and r.actor_id=a.actor_id and r.event_id=a.event_id and r.operation_id=a.operation_id and r.idempotency_key=a.idempotency_key and r.resource_id=a.event_id and r.resource_revision=a.revision for share of a,r")
            .bind(actor.event).bind(actor.actor).bind(key).fetch_optional(&mut *tx).await.map_err(storage)?.ok_or(ToolError::Mural(MuralError::Denied))?;
        let result = own(&row)?;
        tx.commit().await.map_err(storage)?;
        Ok(result)
    }
    async fn capabilities(&self, actor: MuralActor) -> Result<ToolCapabilities, ToolError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let row = self.locked(&mut tx, actor, false).await?;
        let enabled = provisioned(&mut tx, actor.event).await?;
        let now: DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
            .fetch_one(&mut *tx)
            .await
            .map_err(storage)?;
        let revision: i64 = row.try_get("revision").map_err(storage)?;
        let phase: String =
            sqlx::query_scalar("select phase from public.concert_events where event_id=$1")
                .bind(actor.event)
                .fetch_one(&mut *tx)
                .await
                .map_err(storage)?;
        let editable = enabled
            && row.try_get::<String, _>("state").map_err(storage)? == "EDITABLE"
            && matches!(phase.as_str(), "OPEN" | "LIVE");
        let next:Option<DateTime<Utc>>=sqlx::query_scalar("select next_allowed_at from public.concert_mural_limits where event_id=$1 and actor_id=$2").bind(actor.event).bind(actor.actor).fetch_optional(&mut *tx).await.map_err(storage)?;
        let quota = allowance(now, &charges(&mut tx, actor, now).await?, 1);
        let latest=sqlx::query("select a.* from public.concert_mural_tool_actions a join public.concert_command_receipts r using(receipt_id) where a.event_id=$1 and a.actor_id=$2 and r.state='SUCCEEDED' and r.expires_at>$3 order by a.revision desc limit 1")
            .bind(actor.event).bind(actor.actor).bind(now).fetch_optional(&mut *tx).await.map_err(storage)?.map(|r|own(&r)).transpose()?;
        let unresolved=sqlx::query("select c.command,c.idempotency_key,r.receipt_id from public.concert_mural_tool_correlations c join public.concert_command_receipts r using(receipt_id) where c.event_id=$1 and c.actor_id=$2 and r.state='PENDING' order by c.created_at desc limit 1")
            .bind(actor.event).bind(actor.actor).fetch_optional(&mut *tx).await.map_err(storage)?.map(|r|->Result<UnresolvedAction,ToolError>{Ok(UnresolvedAction{command:serde_json::from_value(r.try_get("command").map_err(storage)?).map_err(|_|invalid())?,idempotency_key:r.try_get("idempotency_key").map_err(storage)?,receipt_id:r.try_get("receipt_id").map_err(storage)?})}).transpose()?;
        let undo = latest
            .as_ref()
            .filter(|a| {
                a.kind != ActionKind::Undo && a.revision == revision && a.undo_expires_at > now
            })
            .map(|a| UndoEligibility {
                action_id: a.action_id,
                revision: a.revision,
                expires_at: a.undo_expires_at,
                changed_cells: a.changed_cells,
            });
        let ready = editable && unresolved.is_none() && next.is_none_or(|t| t <= now);
        let result = ToolCapabilities {
            version: 1,
            event_id: actor.event,
            actor_id: actor.actor,
            revision,
            server_time: now,
            background_index: enabled.then_some(0),
            fill_limit: 256,
            cell_allowance: 512,
            window_seconds: 600,
            undo_window_seconds: 600,
            remaining_cells: quota.remaining,
            next_cell_available_at: quota.next_available_at,
            next_allowed_at: next,
            editable,
            available: ToolAvailability {
                pencil: ready && quota.remaining > 0,
                erase: ready && quota.remaining > 0,
                fill: ready && quota.remaining > 0,
                undo: ready
                    && undo
                        .as_ref()
                        .is_some_and(|u| u.changed_cells <= quota.remaining),
            },
            undo,
            latest_action: latest,
            unresolved,
        };
        tx.commit().await.map_err(storage)?;
        Ok(result)
    }
}
fn protocol_op(body: ActionBody) -> super::protocol::WallOperation {
    match body {
        ActionBody::Pencil(_) => super::protocol::WallOperation::Pixel,
        ActionBody::Erase(_) => super::protocol::WallOperation::Erase,
        ActionBody::Fill(_) => super::protocol::WallOperation::Fill,
        ActionBody::Undo(_) => super::protocol::WallOperation::Undo,
    }
}
struct ToolProducer<'a, A> {
    repo: &'a PgMuralTools<A>,
    actor: MuralActor,
    key: Uuid,
    command: ToolCommand,
    body: ActionBody,
    error: Mutex<Option<ToolError>>,
}
#[async_trait::async_trait]
impl<A: MuralAuthority> CommandProducer for ToolProducer<'_, A> {
    async fn authorize(
        &self,
        c: &mut PgConnection,
        s: Scope,
        target: Uuid,
    ) -> Result<(), CommandError> {
        if s.actor != self.actor.actor
            || s.event != self.actor.event
            || s.key != self.key
            || s.operation.id() != self.command.operation()
            || target != self.actor.event
        {
            return Err(CommandError::Denied);
        }
        match self.authorize_in(c).await {
            Ok(()) => Ok(()),
            Err(e) => {
                *self.error.lock().map_err(|_| CommandError::InvalidState)? = Some(e);
                Err(CommandError::Unavailable)
            }
        }
    }
    async fn apply(
        &self,
        c: &mut PgConnection,
        _: Scope,
        target: Uuid,
    ) -> Result<(Completion, Option<Invalidation>), CommandError> {
        match self.mutate(c).await {
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
                let safe = safe_failure(&error);
                *self.error.lock().map_err(|_| CommandError::InvalidState)? = Some(error);
                match safe {
                    Some(reason) => Ok((Completion::Failed(reason), None)),
                    None => Err(CommandError::Unavailable),
                }
            }
        }
    }
    async fn completed(
        &self,
        c: &mut PgConnection,
        s: Scope,
        _: Uuid,
        r: &SafeReceipt,
    ) -> Result<(), CommandError> {
        complete_in(c, s, r)
            .await
            .map_err(|_| CommandError::Unavailable)
    }
}
impl<A: MuralAuthority> ToolProducer<'_, A> {
    async fn authorize_in(&self, c: &mut PgConnection) -> Result<(), ToolError> {
        self.repo.locked(c, self.actor, true).await?;
        if !provisioned(c, self.actor.event).await? {
            return Err(ToolError::Unavailable);
        }
        correlate_in(c, self.actor, self.key, self.body).await
    }
    async fn mutate(&self, c: &mut PgConnection) -> Result<i64, ToolError> {
        let phase: String =
            sqlx::query_scalar("select phase from public.concert_events where event_id=$1")
                .bind(self.actor.event)
                .fetch_one(&mut *c)
                .await
                .map_err(storage)?;
        if !matches!(phase.as_str(), "OPEN" | "LIVE") {
            return Err(MuralError::EventReadOnly.into());
        }
        let row =
            sqlx::query("select * from public.concert_mural_state where event_id=$1 for update")
                .bind(self.actor.event)
                .fetch_one(&mut *c)
                .await
                .map_err(storage)?;
        match row.try_get::<String, _>("state").map_err(storage)?.as_str() {
            "EDITABLE" => {}
            "PAUSED" => return Err(MuralError::Paused.into()),
            "FROZEN" => return Err(MuralError::Frozen.into()),
            _ => return Err(invalid()),
        }
        let revision: i64 = row.try_get("revision").map_err(storage)?;
        if revision != self.command.expected_revision() || revision >= MAX_REVISION {
            return Err(MuralError::RevisionConflict.into());
        }
        let now: DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
            .fetch_one(&mut *c)
            .await
            .map_err(storage)?;
        let next:Option<DateTime<Utc>>=sqlx::query_scalar("select next_allowed_at from public.concert_mural_limits where event_id=$1 and actor_id=$2").bind(self.actor.event).bind(self.actor.actor).fetch_optional(&mut *c).await.map_err(storage)?;
        if next.is_some_and(|t| t > now) {
            return Err(MuralError::Cooldown(next).into());
        }
        let pending:bool=sqlx::query_scalar("select exists(select 1 from public.concert_command_receipts where actor_id=$1 and event_id=$2 and operation_id in ('placeConcertPixel','eraseConcertMuralPixel','fillConcertMuralRegion','undoConcertMuralAction') and state='PENDING' and not(operation_id=$3 and idempotency_key=$4))")
            .bind(self.actor.actor).bind(self.actor.event).bind(self.command.operation()).bind(self.key).fetch_one(&mut *c).await.map_err(storage)?;
        if pending {
            return Err(ToolError::Unresolved);
        }
        let mut grid: Vec<u8> = row.try_get("pixels").map_err(storage)?;
        if grid.len() != PIXELS {
            return Err(invalid());
        }
        let (plan, undoes) = match self.command {
            ToolCommand::Erase(p) => (erase(&grid, p).map_err(planned)?, None),
            ToolCommand::Fill(p) => (fill(&grid, p).map_err(planned)?, None),
            ToolCommand::Undo(p) => {
                let original=sqlx::query("select a.*,r.expires_at receipt_expiry,r.state receipt_state from public.concert_mural_tool_actions a join public.concert_command_receipts r using(receipt_id) where a.action_id=$1 and a.actor_id=$2 and a.event_id=$3 and a.kind<>'UNDO' for share of a,r")
                    .bind(p.target_action_id).bind(self.actor.actor).bind(self.actor.event).fetch_optional(&mut *c).await.map_err(storage)?.ok_or(ToolError::UndoUnavailable)?;
                if original
                    .try_get::<DateTime<Utc>, _>("undo_expires_at")
                    .map_err(storage)?
                    <= now
                    || original
                        .try_get::<DateTime<Utc>, _>("receipt_expiry")
                        .map_err(storage)?
                        <= now
                    || original
                        .try_get::<String, _>("receipt_state")
                        .map_err(storage)?
                        != "SUCCEEDED"
                {
                    return Err(ToolError::UndoExpired);
                }
                if original.try_get::<i64, _>("revision").map_err(storage)? != revision {
                    return Err(ToolError::UndoNotLatest);
                }
                let cells: Vec<CellChange> =
                    serde_json::from_value(original.try_get("cells").map_err(storage)?)
                        .map_err(|_| invalid())?;
                (
                    compensate(&grid, &cells).map_err(planned)?,
                    Some(p.target_action_id),
                )
            }
        };
        let indices: Vec<i32> = plan.iter().map(|v| i32::from(v.index)).collect();
        let locked:bool=sqlx::query_scalar("select exists(select 1 from public.concert_mural_locks l cross join unnest($2::integer[]) i where l.event_id=$1 and i%128>=l.x_min and i%128<l.x_max_exclusive and i/128>=l.y_min and i/128<l.y_max_exclusive)")
            .bind(self.actor.event).bind(&indices).fetch_one(&mut *c).await.map_err(storage)?;
        if locked {
            return Err(MuralError::RegionLocked.into());
        }
        budget(c, self.actor, now, plan.len()).await?;
        for cell in &plan {
            grid[usize::from(cell.index)] = cell.after;
        }
        let next = now + Duration::milliseconds(row.try_get("cooldown_ms").map_err(storage)?);
        sqlx::query("update public.concert_mural_state set pixels=$2,revision=revision+1,reset_revision=revision+1,snapshot_id=$3,snapshot_created_at=$4 where event_id=$1")
            .bind(self.actor.event).bind(grid).bind(Uuid::new_v4()).bind(now).execute(&mut *c).await.map_err(storage)?;
        sqlx::query("insert into public.concert_mural_limits(event_id,actor_id,next_allowed_at) values($1,$2,$3) on conflict(event_id,actor_id) do update set next_allowed_at=excluded.next_allowed_at")
            .bind(self.actor.event).bind(self.actor.actor).bind(next).execute(&mut *c).await.map_err(storage)?;
        record_in(
            c,
            self.actor,
            self.key,
            self.body,
            Uuid::new_v4(),
            revision + 1,
            &plan,
            now,
            next,
            true,
            undoes,
        )
        .await?;
        Ok(revision + 1)
    }
}
pub(crate) fn safe_failure(e: &ToolError) -> Option<SafeFailure> {
    Some(match e {
        ToolError::NoChange => SafeFailure::MuralNoChange,
        ToolError::FillTooLarge => SafeFailure::MuralFillTooLarge,
        ToolError::CellBudget(_) => SafeFailure::MuralBudgetExceeded,
        ToolError::UndoUnavailable => SafeFailure::MuralUndoUnavailable,
        ToolError::UndoExpired => SafeFailure::MuralUndoExpired,
        ToolError::UndoNotLatest => SafeFailure::MuralUndoNotLatest,
        ToolError::Unavailable => SafeFailure::MuralToolUnavailable,
        ToolError::Unresolved => SafeFailure::MuralActionUnresolved,
        ToolError::Mural(MuralError::Paused) => SafeFailure::MuralPaused,
        ToolError::Mural(MuralError::Frozen) => SafeFailure::MuralFrozen,
        ToolError::Mural(MuralError::Cooldown(_)) => SafeFailure::MuralCooldown,
        ToolError::Mural(MuralError::RegionLocked) => SafeFailure::MuralRegionLocked,
        ToolError::Mural(MuralError::RevisionConflict) => SafeFailure::VersionConflict,
        ToolError::Mural(MuralError::EventReadOnly) => SafeFailure::EventReadOnly,
        _ => return None,
    })
}
fn failure(f: SafeFailure) -> ToolError {
    match f {
        SafeFailure::MuralNoChange => ToolError::NoChange,
        SafeFailure::MuralFillTooLarge => ToolError::FillTooLarge,
        SafeFailure::MuralBudgetExceeded => ToolError::CellBudget(None),
        SafeFailure::MuralUndoExpired => ToolError::UndoExpired,
        SafeFailure::MuralUndoNotLatest => ToolError::UndoNotLatest,
        SafeFailure::MuralUndoUnavailable => ToolError::UndoUnavailable,
        SafeFailure::MuralToolUnavailable => ToolError::Unavailable,
        SafeFailure::MuralActionUnresolved => ToolError::Unresolved,
        SafeFailure::MuralPaused => MuralError::Paused.into(),
        SafeFailure::MuralFrozen => MuralError::Frozen.into(),
        SafeFailure::MuralCooldown => MuralError::Cooldown(None).into(),
        SafeFailure::MuralRegionLocked => MuralError::RegionLocked.into(),
        SafeFailure::VersionConflict => MuralError::RevisionConflict.into(),
        SafeFailure::EventReadOnly => MuralError::EventReadOnly.into(),
        _ => invalid(),
    }
}

/// Current authorization only; lookup never invokes apply or inserts correlation.
struct WallStatusProducer<'a, A> {
    repo: &'a PgMuralTools<A>,
    scope: Scope,
}
#[async_trait::async_trait]
impl<A: MuralAuthority> CommandProducer for WallStatusProducer<'_, A> {
    async fn authorize(
        &self,
        c: &mut PgConnection,
        s: Scope,
        target: Uuid,
    ) -> Result<(), CommandError> {
        if s != self.scope || target != s.event {
            return Err(CommandError::Denied);
        }
        self.repo
            .locked(
                c,
                MuralActor {
                    actor: s.actor,
                    event: s.event,
                },
                false,
            )
            .await
            .map_err(|e| match e {
                ToolError::Mural(MuralError::Denied) => CommandError::Denied,
                _ => CommandError::Unavailable,
            })?;
        Ok(())
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
