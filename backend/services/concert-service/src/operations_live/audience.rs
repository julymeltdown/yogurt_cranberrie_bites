//! Publisher seam over committed BE15 correlation. These internal audiences must
//! never be serialized as public notifications or broadcast to every event user.
use super::unavailable;
use concert_application::commands::CommandError;
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// Intentionally no Debug or Serialize: reporter identity is internal routing.
pub enum OperationsAudience {
    ReporterAndCurrentModerators {
        event: Uuid,
        reporter: Uuid,
        report: Uuid,
    },
    CurrentOperators {
        event: Uuid,
        resource: Uuid,
    },
    EventRevision {
        event: Uuid,
        resource: Uuid,
    },
}
/// Resolve only a committed, successful, matching outbox/receipt revision. The
/// publisher must reauthorize each recipient against the current real role owner.
/// No callback or successful external delivery is recorded by this function.
pub async fn committed_audience(
    pool: &PgPool,
    outbox: Uuid,
) -> Result<OperationsAudience, CommandError> {
    // A fresh read transaction cannot observe an uncommitted producer's row.
    let mut tx = pool.begin().await.map_err(unavailable)?;
    let row=sqlx::query("select o.event_id,o.resource_id,o.resource_revision,r.actor_id,r.operation_id,r.state,r.resource_id receipt_resource,r.resource_revision receipt_revision from concert_outbox o join concert_command_receipts r on r.receipt_id=o.receipt_id and r.event_id=o.event_id where o.outbox_id=$1 for share of o,r")
        .bind(outbox).fetch_optional(&mut *tx).await.map_err(unavailable)?.ok_or(CommandError::Denied)?;
    let event: Uuid = row.try_get("event_id").map_err(unavailable)?;
    let resource: Uuid = row.try_get("resource_id").map_err(unavailable)?;
    let revision: i64 = row.try_get("resource_revision").map_err(unavailable)?;
    if row.try_get::<String, _>("state").map_err(unavailable)? != "SUCCEEDED"
        || row
            .try_get::<Option<Uuid>, _>("receipt_resource")
            .map_err(unavailable)?
            != Some(resource)
        || row
            .try_get::<Option<i64>, _>("receipt_revision")
            .map_err(unavailable)?
            != Some(revision)
    {
        return Err(CommandError::Unavailable);
    }
    match row
        .try_get::<String, _>("operation_id")
        .map_err(unavailable)?
        .as_str()
    {
        "createConcertReport" => Ok(OperationsAudience::ReporterAndCurrentModerators {
            event,
            reporter: row.try_get("actor_id").map_err(unavailable)?,
            report: resource,
        }),
        "setConcertSettings"
        | "setConcertEventPhase"
        | "setConcertPlace"
        | "createConcertNotice"
        | "updateConcertNotice" => Ok(OperationsAudience::EventRevision { event, resource }),
        _ => Err(CommandError::Denied),
    }
}
