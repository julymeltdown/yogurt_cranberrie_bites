//! Atomic internal producer seam. No HTTP adapter and no canonical identity implementation.
use async_trait::async_trait;
use concert_application::commands::*;
use sqlx::{PgConnection, PgPool, Row};
use uuid::Uuid;

/// Trusted service adapter, never a request-body implementation. Every call must lock
/// the current policy/target authority (or equivalent fenced owning-service contract)
/// through commit, fail closed on outages and recheck all phase/revision invariants.
#[async_trait]
pub trait CommandProducer: Send + Sync {
    async fn authorize(
        &self,
        connection: &mut PgConnection,
        scope: Scope,
        target: Uuid,
    ) -> Result<(), CommandError>;
    /// Only local SQL work on this connection. No external side effect inside transaction.
    /// Err rolls back everything. Failed completion is a deliberate safe terminal outcome;
    /// producer MUST NOT apply aggregate effects for Failed. Pending is not success.
    async fn apply(
        &self,
        connection: &mut PgConnection,
        scope: Scope,
        target: Uuid,
    ) -> Result<(Completion, Option<Invalidation>), CommandError>;
}
#[derive(Clone)]
pub struct PgCommandRepository {
    pool: PgPool,
}
impl PgCommandRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    pub async fn execute<P: CommandProducer>(
        &self,
        scope: Scope,
        target: Uuid,
        hash: RequestHash,
        producer: &P,
    ) -> Result<SafeReceipt, CommandError> {
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        producer.authorize(&mut tx, scope, target).await?;
        // Global admission lock also serializes new-key races. Bounded foundation favors
        // correctness; production throughput requires measured partitioned quota design.
        sqlx::query(
            "select singleton from public.concert_command_budget where singleton for update",
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(unavailable)?;
        let existing=sqlx::query("select *,clock_timestamp() >= expires_at as expired from public.concert_command_receipts where actor_id=$1 and event_id=$2 and operation_id=$3 and idempotency_key=$4 for update")
            .bind(scope.actor).bind(scope.event).bind(scope.operation.id()).bind(scope.key).fetch_optional(&mut *tx).await.map_err(unavailable)?;
        if let Some(row) = existing {
            // Current authorization gates *all* receipt metadata, even conflicts/expiry.
            let stored_target: Uuid = row.try_get("target_id").map_err(invalid)?;
            producer.authorize(&mut tx, scope, stored_target).await?;
            if row.try_get::<String, _>("request_hash").map_err(invalid)? != hash.as_storage() {
                return Err(CommandError::Conflict);
            }
            if stored_target != target {
                return Err(CommandError::Conflict);
            }
            if row.try_get::<bool, _>("expired").map_err(invalid)? {
                return Err(CommandError::Expired);
            }
            let value = project(row, scope)?;
            if value.state == ReceiptState::Retired {
                return Err(CommandError::Expired);
            }
            tx.commit().await.map_err(unavailable)?;
            return Ok(value);
        }
        producer.authorize(&mut tx, scope, target).await?;
        let used: i64 = sqlx::query_scalar(
            "select receipts_used from public.concert_command_budget where singleton",
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(unavailable)?;
        let actor_used:i64=sqlx::query_scalar("select count(*) from public.concert_command_receipts where actor_id=$1 and event_id=$2").bind(scope.actor).bind(scope.event).fetch_one(&mut *tx).await.map_err(unavailable)?;
        if used >= 1_000_000 || actor_used >= 1000 {
            return Err(CommandError::Capacity);
        }
        let id = Uuid::new_v4();
        // Use a single DB timestamp for exact TTL.
        sqlx::query("insert into public.concert_command_receipts(receipt_id,actor_id,event_id,operation_id,idempotency_key,request_hash,target_id,state,created_at,expires_at) select $1,$2,$3,$4,$5,$6,$7,'PENDING',t,t+interval '86400 seconds' from (select clock_timestamp() t) now")
            .bind(id).bind(scope.actor).bind(scope.event).bind(scope.operation.id()).bind(scope.key).bind(hash.as_storage()).bind(target).execute(&mut *tx).await.map_err(unavailable)?;
        sqlx::query("update public.concert_command_budget set receipts_used=receipts_used+1 where singleton").execute(&mut *tx).await.map_err(unavailable)?;
        // Savepoint ensures a deliberately FAILED receipt cannot retain local mutation.
        sqlx::query("savepoint command_effects")
            .execute(&mut *tx)
            .await
            .map_err(unavailable)?;
        let (completion, invalidation) = producer.apply(&mut tx, scope, target).await?;
        completion.validate()?;
        let (state, reference, status, failure) = match completion {
            Completion::Pending => (ReceiptState::Pending, None, None, None),
            Completion::Succeeded { reference, status } => {
                if reference.resource != target {
                    return Err(CommandError::InvalidState);
                }
                (ReceiptState::Succeeded, Some(reference), Some(status), None)
            }
            Completion::Failed(reason) => {
                sqlx::query("rollback to savepoint command_effects")
                    .execute(&mut *tx)
                    .await
                    .map_err(unavailable)?;
                if invalidation.is_some() {
                    return Err(CommandError::InvalidState);
                }
                (ReceiptState::Failed, None, None, Some(reason))
            }
        };
        if let Some(event) = invalidation {
            event.reference.validate()?;
            if event.reference.resource != target {
                return Err(CommandError::InvalidState);
            }
            let queued: i64 = sqlx::query_scalar(
                "select count(*) from public.concert_outbox where state in ('READY','LEASED')",
            )
            .fetch_one(&mut *tx)
            .await
            .map_err(unavailable)?;
            if queued >= 10000 {
                return Err(CommandError::Capacity);
            }
            sqlx::query("insert into public.concert_outbox(outbox_id,receipt_id,event_id,resource_id,resource_revision) values($1,$2,$3,$4,$5)")
                .bind(Uuid::new_v4()).bind(id).bind(scope.event).bind(event.reference.resource).bind(event.reference.revision).execute(&mut *tx).await.map_err(unavailable)?;
        }
        let row=sqlx::query("update public.concert_command_receipts set state=$2,resource_id=$3,resource_revision=$4,result_status=$5,failure_reason=$6 where receipt_id=$1 returning *")
            .bind(id).bind(state.as_str()).bind(reference.map(|r|r.resource)).bind(reference.map(|r|r.revision)).bind(status).bind(failure.map(|f|f.as_str())).fetch_one(&mut *tx).await.map_err(unavailable)?;
        let receipt = project(row, scope)?;
        tx.commit().await.map_err(unavailable)?;
        Ok(receipt)
    }
    /// Status lookup reauthorizes the stored target. No raw result bodies are returned.
    pub async fn lookup<P: CommandProducer>(
        &self,
        scope: Scope,
        target: Uuid,
        producer: &P,
    ) -> Result<Option<SafeReceipt>, CommandError> {
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        producer.authorize(&mut tx, scope, target).await?;
        let row=sqlx::query("select *,clock_timestamp() >= expires_at as expired from public.concert_command_receipts where actor_id=$1 and event_id=$2 and operation_id=$3 and idempotency_key=$4 for share")
            .bind(scope.actor).bind(scope.event).bind(scope.operation.id()).bind(scope.key).fetch_optional(&mut *tx).await.map_err(unavailable)?;
        let Some(row) = row else { return Ok(None) };
        let stored_target: Uuid = row.try_get("target_id").map_err(invalid)?;
        producer.authorize(&mut tx, scope, stored_target).await?;
        if stored_target != target {
            return Ok(None);
        }
        if row.try_get::<bool, _>("expired").map_err(invalid)? {
            return Err(CommandError::Expired);
        }
        let receipt = project(row, scope)?;
        tx.commit().await.map_err(unavailable)?;
        Ok(Some(receipt))
    }
    /// Bounded expiry maintenance clears safe results; correlation/hash remains forever
    /// until a separate reviewed cleanup. Retired receipts can never become executable.
    pub async fn retire_expired(&self, limit: i64) -> Result<u64, CommandError> {
        if !(1..=1000).contains(&limit) {
            return Err(CommandError::InvalidInput);
        }
        let result=sqlx::query("with expired as (select receipt_id from public.concert_command_receipts where expires_at<=clock_timestamp() and state<>'RETIRED' order by expires_at,receipt_id for update skip locked limit $1) update public.concert_command_receipts r set state='RETIRED',resource_id=null,resource_revision=null,result_status=null,failure_reason=null from expired e where r.receipt_id=e.receipt_id")
            .bind(limit).execute(&self.pool).await.map_err(unavailable)?;
        Ok(result.rows_affected())
    }
}
fn project(row: sqlx::postgres::PgRow, scope: Scope) -> Result<SafeReceipt, CommandError> {
    let resource: Option<Uuid> = row.try_get("resource_id").map_err(invalid)?;
    let revision: Option<i64> = row.try_get("resource_revision").map_err(invalid)?;
    let reference = match (resource, revision) {
        (Some(resource), Some(revision)) => Some(SafeReference { resource, revision }.validate()?),
        (None, None) => None,
        _ => return Err(CommandError::InvalidState),
    };
    Ok(SafeReceipt {
        id: row.try_get("receipt_id").map_err(invalid)?,
        scope,
        state: ReceiptState::parse(&row.try_get::<String, _>("state").map_err(invalid)?)?,
        reference,
        failure: row
            .try_get::<Option<String>, _>("failure_reason")
            .map_err(invalid)?
            .as_deref()
            .map(SafeFailure::parse)
            .transpose()?,
        result_status: row.try_get("result_status").map_err(invalid)?,
        created_at: row.try_get("created_at").map_err(invalid)?,
        expires_at: row.try_get("expires_at").map_err(invalid)?,
    })
}
fn unavailable(_: sqlx::Error) -> CommandError {
    CommandError::Unavailable
}
fn invalid(_: sqlx::Error) -> CommandError {
    CommandError::InvalidState
}
