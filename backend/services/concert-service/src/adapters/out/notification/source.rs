//! Source receipt verification uses Concert's own database only.
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::notification::NotificationProjectionOwner;
use concert_domain::notification::*;
use sqlx::{PgConnection, PgPool, Row};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub envelope: Envelope,
    pub actor_id: Uuid,
    pub operation: String,
}
/// Resolve current safe recipients/typed target from the actual aggregate,
/// honoring event, deletion, blocks and owning policy. An unavailable router
/// cannot silently treat an operation as having no recipients.
#[async_trait]
pub trait ProjectionRouting: Send + Sync {
    async fn route_in(
        &self,
        connection: &mut PgConnection,
        source: &Source,
        now: DateTime<Utc>,
    ) -> Result<Vec<Recipient>>;
}
pub struct UnavailableRouting;
#[async_trait]
impl ProjectionRouting for UnavailableRouting {
    async fn route_in(
        &self,
        _: &mut PgConnection,
        _: &Source,
        _: DateTime<Utc>,
    ) -> Result<Vec<Recipient>> {
        Err(NotificationError::Unavailable)
    }
}
pub struct PgProjectionOwner<R> {
    pool: PgPool,
    routing: R,
}
impl<R: ProjectionRouting> PgProjectionOwner<R> {
    pub fn new(pool: PgPool, routing: R) -> Self {
        Self { pool, routing }
    }
}
#[async_trait]
impl<R: ProjectionRouting> NotificationProjectionOwner for PgProjectionOwner<R> {
    async fn resolve(&self, envelope: Envelope) -> Result<Vec<Recipient>> {
        envelope.validate()?;
        let budget = platform::call_deadline::bounded(std::time::Duration::from_secs(3));
        let stop = tokio::time::Instant::now() + budget;
        tokio::time::timeout_at(stop, async {
            let mut tx = self.pool.begin().await.map_err(db)?;
            // This request is subordinate to BE15's ten-second sink deadline.
            sqlx::query("set local statement_timeout='2s'")
                .execute(&mut *tx)
                .await
                .map_err(db)?;
            let row=sqlx::query("select r.actor_id,r.operation_id,r.state,r.resource_id,r.resource_revision,o.event_id,o.receipt_id,o.resource_id as outbox_resource,o.resource_revision as outbox_revision from concert_command_receipts r join concert_outbox o on o.receipt_id=r.receipt_id where o.outbox_id=$1 for share of r,o")
                .bind(envelope.outbox_id).fetch_optional(&mut *tx).await.map_err(db)?.ok_or(NotificationError::ResourceUnavailable)?;
            // A retired command retains its outbox correlation but no longer its result.
            // Its typed outbox reference is still immutable and sufficient for routing.
            let state: String = row.get("state");
            if !matches!(state.as_str(), "SUCCEEDED" | "RETIRED")
                || row.get::<Uuid, _>("event_id") != envelope.event_id
                || row.get::<Uuid, _>("receipt_id") != envelope.receipt_id
                || row.get::<Uuid, _>("outbox_resource") != envelope.resource_id
                || row.get::<i64, _>("outbox_revision") != envelope.resource_revision
                || (state == "SUCCEEDED"
                    && (row.get::<Option<Uuid>, _>("resource_id") != Some(envelope.resource_id)
                        || row.get::<Option<i64>, _>("resource_revision")
                            != Some(envelope.resource_revision)))
            {
                return Err(NotificationError::IdempotencyConflict);
            }
            let source = Source {
                envelope,
                actor_id: row.get("actor_id"),
                operation: row.get("operation_id"),
            };
            let recipients = self.routing.route_in(&mut tx, &source, Utc::now()).await?;
            if recipients.len() > 1000 {
                return Err(NotificationError::Unavailable);
            }
            for r in &recipients {
                r.validate().map_err(|_| NotificationError::Unavailable)?;
            }
            tx.commit().await.map_err(db)?;
            Ok(recipients)
        }).await.map_err(|_| NotificationError::Unavailable)?
    }
}
fn db(_: sqlx::Error) -> NotificationError {
    NotificationError::Unavailable
}
