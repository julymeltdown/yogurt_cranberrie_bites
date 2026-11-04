//! Published notice policy only. Report's role-bound policy is unavailable.
use super::local::{self, db, revision};
use concert_application::notification::{Candidate, Context};
use concert_domain::notification::{NotificationError, Result};
use sqlx::{PgConnection, Row};

pub(super) async fn notice(
    c: &mut PgConnection,
    context: Context,
    candidate: Candidate,
) -> Result<bool> {
    let row = sqlx::query("select state,revision,confirmed_at from public.concert_operation_notices where event_id=$1 and notice_id=$2 for share")
        .bind(context.event_id).bind(local::id(candidate)?).fetch_optional(&mut *c).await.map_err(db)?.ok_or(NotificationError::Unavailable)?;
    revision(row.try_get("revision").map_err(db)?)?;
    let now = local::current(c, context).await?;
    if row
        .try_get::<chrono::DateTime<chrono::Utc>, _>("confirmed_at")
        .map_err(db)?
        > now
    {
        return Err(NotificationError::Unavailable);
    }
    match row.try_get::<String, _>("state").map_err(db)?.as_str() {
        "PUBLISHED" => local::audience(c, context, candidate, vec![]).await,
        "DRAFT" | "WITHDRAWN" => Ok(false),
        _ => Err(NotificationError::Unavailable),
    }
}
