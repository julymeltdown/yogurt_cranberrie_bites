use crate::application::dto::mutation_dto::{
    MutationError, MutationOperationDto, MutationOutcomeDto,
};
use anyhow::Result;
use sqlx::{PgConnection, Row};

/// The unique constraints serialize concurrent attempts. A loser waits for the
/// winner's COMMIT/ROLLBACK; it can never observe a partially written outcome.
pub(super) async fn claim(
    connection: &mut PgConnection,
    operation: Option<&MutationOperationDto>,
) -> Result<Option<MutationOutcomeDto>> {
    let Some(operation) = operation else {
        return Ok(None);
    };
    sqlx::query("set local lock_timeout = '500ms'")
        .execute(&mut *connection)
        .await?;
    sqlx::query("set local statement_timeout = '2s'")
        .execute(&mut *connection)
        .await?;
    let inserted = sqlx::query(
        "insert into post_mutation_receipts
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
        .ok_or_else(|| MutationError::Unavailable.into())
        .map(Some)
}

pub(super) async fn lookup(
    connection: &mut PgConnection,
    operation: &MutationOperationDto,
) -> Result<Option<MutationOutcomeDto>> {
    let rows = sqlx::query(
        "select operation_id = $1 and actor_id = $2
            and idempotency_key is not distinct from $3 and scope = $4
            and command_fingerprint = encode(sha256(convert_to($5,'UTF8')),'hex') as matches,
            replay_until > now() as replayable, outcome_json
         from post_mutation_receipts
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
    let value = value.ok_or(MutationError::Unavailable)?;
    Ok(Some(serde_json::from_value(value)?))
}

pub(super) async fn complete(
    connection: &mut PgConnection,
    operation: Option<&MutationOperationDto>,
    outcome: MutationOutcomeDto,
) -> Result<()> {
    let Some(operation) = operation else {
        return Ok(());
    };
    let updated = sqlx::query(
        "update post_mutation_receipts set outcome_json = $2
         where operation_id = $1 and outcome_json is null",
    )
    .bind(operation.context.operation_id)
    .bind(serde_json::to_value(outcome)?)
    .execute(connection)
    .await?;
    anyhow::ensure!(updated.rows_affected() == 1, MutationError::Unavailable);
    Ok(())
}
