//! Uses the existing outbox/leases. Stable maintenance tombstones survive queue pruning.
use concert_application::commands::{maintenance::MaintenanceInvalidation, CommandError};
use sqlx::{PgConnection, Row};
use uuid::Uuid;
pub async fn enqueue_maintenance_in(
    c: &mut PgConnection,
    value: MaintenanceInvalidation,
) -> Result<Uuid, CommandError> {
    let value = value.validate()?;
    // Same queue admission lock as BE15. Caller must hold local aggregate locks first.
    sqlx::query("select singleton from concert_command_budget where singleton for update")
        .fetch_one(&mut *c)
        .await
        .map_err(db)?;
    if let Some(row)=sqlx::query("select event_id,resource_id,resource_revision from concert_maintenance_invalidations where maintenance_id=$1 for share").bind(value.id).fetch_optional(&mut *c).await.map_err(db)?{
  if row.try_get::<Uuid,_>("event_id").map_err(db)?!=value.event||row.try_get::<Uuid,_>("resource_id").map_err(db)?!=value.reference.resource||row.try_get::<i64,_>("resource_revision").map_err(db)?!=value.reference.revision{return Err(CommandError::Conflict)}return Ok(value.id)
 }
    let queued: i64 =
        sqlx::query_scalar("select count(*) from concert_outbox where state in ('READY','LEASED')")
            .fetch_one(&mut *c)
            .await
            .map_err(db)?;
    let used: i64 = sqlx::query_scalar("select count(*) from concert_maintenance_invalidations")
        .fetch_one(&mut *c)
        .await
        .map_err(db)?;
    if queued >= 10000 || used >= 1000000 {
        return Err(CommandError::Capacity);
    }
    sqlx::query("insert into concert_maintenance_invalidations(maintenance_id,event_id,resource_id,resource_revision) values($1,$2,$3,$4)").bind(value.id).bind(value.event).bind(value.reference.resource).bind(value.reference.revision).execute(&mut *c).await.map_err(db)?;
    sqlx::query("insert into concert_outbox(outbox_id,maintenance_id,receipt_id,event_id,resource_id,resource_revision,kind) values($1,$1,null,$2,$3,$4,'MAINTENANCE_INVALIDATED')").bind(value.id).bind(value.event).bind(value.reference.resource).bind(value.reference.revision).execute(&mut *c).await.map_err(db)?;
    Ok(value.id)
}
fn db(_: sqlx::Error) -> CommandError {
    CommandError::Unavailable
}
