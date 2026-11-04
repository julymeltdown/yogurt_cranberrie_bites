//! Private canonical supervisor terminal evidence. No content or delivery grant.
use super::*;

impl PgScopeLedger {
    pub(crate) async fn finish_cancelled_root_work(&self, key: &ScopeKey) -> Result<(), Status> {
        let mut tx = self.transaction().await?;
        let state = self.lock(&mut tx, key).await?;
        let (serialized, abandoned): (bool, bool) = sqlx::query_as(
            "SELECT serialized_at IS NOT NULL,response_abandoned_at IS NOT NULL FROM concert_product_owner_scopes WHERE instance_id=$1 AND scope_id=$2",
        ).bind(key.instance).bind(key.scope).fetch_one(&mut *tx).await.map_err(unavailable)?;
        if serialized || (state == ScopePhase::RootFinished && !abandoned) {
            return Err(Status::failed_precondition("Response cannot be abandoned"));
        }
        match state {
            ScopePhase::RootFinished if abandoned => (),
            ScopePhase::Sealed => {
                sqlx::query("UPDATE concert_product_owner_scopes SET phase='ROOT_FINISHED',root_finished_at=clock_timestamp(),response_abandoned_at=clock_timestamp() WHERE instance_id=$1 AND scope_id=$2")
                    .bind(key.instance).bind(key.scope).execute(&mut *tx).await.map_err(unavailable)?;
            }
            _ => return Err(Status::failed_precondition("Cancelled root not sealed")),
        }
        tx.commit().await.map_err(unavailable)
    }
    pub(crate) async fn record_cancellation_settled(&self, key: &ScopeKey) -> Result<(), Status> {
        let mut tx = self.transaction().await?;
        if self.lock(&mut tx, key).await? != ScopePhase::RootFinished {
            return Err(Status::failed_precondition("Cancelled root unfinished"));
        }
        let (abandoned, settled): (bool, bool) = sqlx::query_as(
            "SELECT response_abandoned_at IS NOT NULL,cancellation_settled_at IS NOT NULL FROM concert_product_owner_scopes WHERE instance_id=$1 AND scope_id=$2",
        ).bind(key.instance).bind(key.scope).fetch_one(&mut *tx).await.map_err(unavailable)?;
        if !abandoned {
            return Err(Status::failed_precondition(
                "Root has no cancellation proof",
            ));
        }
        if !settled {
            sqlx::query("UPDATE concert_product_owner_scopes SET cancellation_settled_at=clock_timestamp() WHERE instance_id=$1 AND scope_id=$2")
                .bind(key.instance).bind(key.scope).execute(&mut *tx).await.map_err(unavailable)?;
        }
        tx.commit().await.map_err(unavailable)
    }
}
