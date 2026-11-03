use crate::application::dto::friend_dto::FollowActionDto;
use crate::application::dto::mutation_dto::{
    MutationError, MutationKind, MutationOperationDto, MutationOutcomeDto, MutationReceiptStateDto,
    MutationReceiptStatusDto,
};
use anyhow::Result;
use sqlx::{PgConnection, PgPool, Row};
use uuid::Uuid;

/// Receipt constraints serialize concurrent attempts. A losing attempt waits
/// for the winning transaction and can only observe its committed outcome.
pub(super) async fn claim(
    connection: &mut PgConnection,
    operation: &MutationOperationDto,
) -> Result<Option<FollowActionDto>> {
    sqlx::query("set local lock_timeout = '500ms'")
        .execute(&mut *connection)
        .await?;
    sqlx::query("set local statement_timeout = '2s'")
        .execute(&mut *connection)
        .await?;
    let inserted = sqlx::query(
        "insert into friend_mutation_receipts
         (operation_id, actor_id, idempotency_key, scope, command_fingerprint)
         values ($1,$2,$3,$4,encode(sha256(convert_to($5,'UTF8')),'hex'))
         on conflict do nothing",
    )
    .bind(operation.context.operation_id)
    .bind(operation.context.actor_id)
    .bind(&operation.context.idempotency_key)
    .bind(operation.kind.scope())
    .bind(&operation.canonical_command)
    .execute(&mut *connection)
    .await?;
    if inserted.rows_affected() == 1 {
        return Ok(None);
    }
    lookup(connection, operation)
        .await?
        .ok_or(MutationError::Unavailable)
        .map(Some)
        .map_err(Into::into)
}

pub(super) async fn lookup(
    connection: &mut PgConnection,
    operation: &MutationOperationDto,
) -> Result<Option<FollowActionDto>> {
    let rows = sqlx::query(
        "select operation_id = $1 and actor_id = $2
            and idempotency_key is not distinct from $3 and scope = $4
            and command_fingerprint = encode(sha256(convert_to($5,'UTF8')),'hex') as matches,
            replay_until > now() as replayable, outcome_json
         from friend_mutation_receipts
         where operation_id = $1 or (actor_id = $2 and idempotency_key = $3)",
    )
    .bind(operation.context.operation_id)
    .bind(operation.context.actor_id)
    .bind(&operation.context.idempotency_key)
    .bind(operation.kind.scope())
    .bind(&operation.canonical_command)
    .fetch_all(connection)
    .await?;
    if rows.is_empty() {
        return Ok(None);
    }
    if rows.len() != 1 || !rows[0].get::<bool, _>("matches") {
        return Err(MutationError::Conflict.into());
    }
    if !rows[0].get::<bool, _>("replayable") {
        return Err(MutationError::Expired.into());
    }
    let value: Option<serde_json::Value> = rows[0].get("outcome_json");
    let outcome: MutationOutcomeDto =
        serde_json::from_value(value.ok_or(MutationError::Unavailable)?)?;
    match (operation.kind, outcome) {
        (MutationKind::Follow, MutationOutcomeDto::Follow(action))
        | (MutationKind::Unfollow, MutationOutcomeDto::Unfollow(action)) => Ok(Some(action)),
        _ => Err(MutationError::Conflict.into()),
    }
}

pub(super) async fn complete(
    connection: &mut PgConnection,
    operation: &MutationOperationDto,
    outcome: MutationOutcomeDto,
) -> Result<()> {
    let updated = sqlx::query(
        "update friend_mutation_receipts set outcome_json = $2
         where operation_id = $1 and outcome_json is null",
    )
    .bind(operation.context.operation_id)
    .bind(serde_json::to_value(outcome)?)
    .execute(connection)
    .await?;
    anyhow::ensure!(updated.rows_affected() == 1, MutationError::Unavailable);
    Ok(())
}

pub(super) async fn status(
    pool: &PgPool,
    operation_id: Uuid,
) -> Result<Option<MutationReceiptStatusDto>> {
    let row = sqlx::query(
        "select operation_id, actor_id, scope, committed_at, replay_until,
                case
                  when replay_until <= now() then 'EXPIRED'
                  when outcome_json is null then 'INCOMPLETE'
                  else 'REPLAYABLE'
                end as receipt_state
         from friend_mutation_receipts where operation_id = $1",
    )
    .bind(operation_id)
    .fetch_optional(pool)
    .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let state = match row.get::<String, _>("receipt_state").as_str() {
        "REPLAYABLE" => MutationReceiptStateDto::Replayable,
        "EXPIRED" => MutationReceiptStateDto::Expired,
        "INCOMPLETE" => MutationReceiptStateDto::Incomplete,
        _ => return Err(MutationError::Unavailable.into()),
    };
    Ok(Some(MutationReceiptStatusDto {
        operation_id: row.get("operation_id"),
        actor_id: row.get("actor_id"),
        scope: row.get("scope"),
        committed_at: row.get("committed_at"),
        replay_until: row.get("replay_until"),
        state,
    }))
}

pub(super) async fn purge_expired_outcomes(pool: &PgPool, limit: i32) -> Result<i64> {
    let result = sqlx::query(
        "update friend_mutation_receipts set outcome_json = null
         where operation_id in (
             select operation_id from friend_mutation_receipts
             where replay_until <= now() and outcome_json is not null
             order by replay_until, operation_id
             limit $1 for update skip locked
         )",
    )
    .bind(i64::from(limit.clamp(1, 5_000)))
    .execute(pool)
    .await?;
    Ok(result.rows_affected() as i64)
}
