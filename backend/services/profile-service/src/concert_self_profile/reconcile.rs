//! Serial settlement uses native state plus the actual coordinator's decision.
use super::{
    coordinator::{Coordinator, Decision},
    identity::{prepared_gid, NativeStatus},
    model::Error,
    operation::Operation,
};
use platform::actor_delegation::authority_deadline::AuthorityRpcDeadline;
use sqlx::{Connection, PgPool};
pub(super) async fn reconcile(
    pool: &PgPool,
    coordinator: &Coordinator,
    operation: &Operation,
    deadline: AuthorityRpcDeadline,
) -> Result<NativeStatus, Error> {
    super::identity::remaining(deadline)?;
    // A detached session owns this advisory lock. Timeout/cancellation closes
    // it, so neither locks nor session timeout settings leak into the pool.
    let mut connection = pool
        .acquire()
        .await
        .map_err(|_| Error::Unavailable)?
        .detach();
    let millis = super::identity::remaining(deadline)?.to_string();
    sqlx::query(
        "select set_config('statement_timeout',$1,false),set_config('lock_timeout',$1,false)",
    )
    .bind(millis)
    .execute(&mut connection)
    .await
    .map_err(|_| Error::Unavailable)?;
    sqlx::query("select pg_advisory_lock(hashtextextended($1,0))")
        .bind(format!("mcr-profile-self-settle:{}", operation.id))
        .execute(&mut connection)
        .await
        .map_err(|_| Error::Unavailable)?;
    let status = operation.status(pool, deadline).await?;
    if status != NativeStatus::Prepared {
        connection.close().await.map_err(|_| Error::Unavailable)?;
        observe_terminal(pool, operation, status, deadline).await?;
        return Ok(status);
    }
    observe_terminal(pool, operation, NativeStatus::Prepared, deadline).await?;
    let decision = coordinator.query(operation, deadline).await?;
    let command = match decision {
        Decision::Unresolved => {
            connection.close().await.map_err(|_| Error::Unavailable)?;
            observe_terminal(pool, operation, NativeStatus::Prepared, deadline).await?;
            return Ok(NativeStatus::Prepared);
        }
        Decision::Commit => "commit prepared",
        Decision::Abort => "rollback prepared",
    };
    super::identity::remaining(deadline)?;
    // Only our persisted, UUID-derived GID enters source-controlled SQL. A
    // changed or missing native identity never supplies a settlement command.
    if operation.status(pool, deadline).await? != NativeStatus::Prepared {
        return Err(Error::Unavailable);
    }
    let gid = prepared_gid(operation.id)?;
    sqlx::query(&format!("{command} '{gid}'"))
        .execute(&mut connection)
        .await
        .map_err(|_| Error::Unavailable)?;
    connection.close().await.map_err(|_| Error::Unavailable)?;
    // A lost/late COMMIT reply is unknown until this actual retained XID and
    // exact effect receipt are read. No retry is interpreted as another write.
    let status = operation.status(pool, deadline).await?;
    observe_terminal(pool, operation, status, deadline).await?;
    Ok(status)
}
async fn observe_terminal(
    pool: &PgPool,
    operation: &Operation,
    status: NativeStatus,
    deadline: AuthorityRpcDeadline,
) -> Result<(), Error> {
    let status = match status {
        NativeStatus::Committed => "COMMITTED",
        NativeStatus::Aborted => "ABORTED",
        _ => "UNRESOLVED",
    };
    super::identity::remaining(deadline)?;
    sqlx::query("insert into concert_self_profile_settled_observations(operation_id,effect_binding_digest,observed_status) values($1,$2,$3) on conflict(operation_id) do update set observed_status=excluded.observed_status,observed_at=clock_timestamp() where concert_self_profile_settled_observations.observed_status='UNRESOLVED' and concert_self_profile_settled_observations.effect_binding_digest=excluded.effect_binding_digest").bind(operation.id).bind(operation.identity.effect_digest.as_slice()).bind(status).execute(pool).await.map_err(|_|Error::Unavailable)?;
    super::identity::remaining(deadline)?;
    Ok(())
}
