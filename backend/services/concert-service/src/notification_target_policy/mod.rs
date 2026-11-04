//! Private owner-local observations, never a Root capability or owner provider.
//!
//! Caller retains the exact borrowed transaction, native registration, canonical
//! Auth/Friend/source fences and private RootReadView through child completion
//! and Root EOF. A Visible observation grants no public projection or action.
mod gift;
mod local;
mod operations;
mod party;
mod social;

use chrono::{DateTime, Utc};
use concert_application::notification::{Candidate, Context};
use concert_domain::notification::{NotificationError, Result, TargetType, Visibility};
use sqlx::{Connection, PgConnection};

/// Inspect current owner rows without beginning, committing or releasing a TX.
/// SQLx must track the caller's transaction. An untracked/autocommit connection
/// cannot retain row exclusions and is rejected before any policy query.
pub async fn observe_local_target(
    connection: &mut PgConnection,
    context: Context,
    candidate: Candidate,
    now: DateTime<Utc>,
) -> Result<Visibility> {
    context.check(now)?;
    context.check(Utc::now())?;
    if candidate.notification_id.is_nil()
        || candidate.source_actor_id.is_some_and(|id| id.is_nil())
        || candidate
            .target
            .is_some_and(|target| target.target_id.is_nil())
    {
        return Err(NotificationError::InvalidInput);
    }
    let target = candidate.target.ok_or(NotificationError::Unavailable)?;
    // Report requires an operation-bound actual EventRoleFence. Reporter status
    // lists do not authorize the singular admin target. Post is remote-owned.
    if matches!(target.target_type, TargetType::Post | TargetType::Report) {
        return Err(NotificationError::Unavailable);
    }
    if !connection.is_in_transaction() {
        return Err(NotificationError::Unavailable);
    }
    let remaining = (context.expires_at - Utc::now())
        .to_std()
        .map_err(|_| NotificationError::Unavailable)?;
    tokio::time::timeout(remaining, async {
        local::bound(connection, context).await?;
        let event_visible = local::event(connection, context).await?;
        let visible = if !event_visible {
            false
        } else {
            match target.target_type {
                TargetType::Party => party::party(connection, context, candidate).await?,
                TargetType::Application => {
                    party::application(connection, context, candidate).await?
                }
                TargetType::Giveaway => gift::giveaway(connection, context, candidate).await?,
                TargetType::Reservation => {
                    gift::reservation(connection, context, candidate).await?
                }
                TargetType::SocialRequest => {
                    social::request(connection, context, candidate).await?
                }
                TargetType::Notice => operations::notice(connection, context, candidate).await?,
                TargetType::Post | TargetType::Report => {
                    return Err(NotificationError::Unavailable)
                }
            }
        };
        local::current(connection, context).await?;
        Ok(if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        })
    })
    .await
    .map_err(|_| NotificationError::Unavailable)?
}
