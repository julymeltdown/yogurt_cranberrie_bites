//! Additive canonical event-content persistence. Production construction is
//! fail-closed until a real cross-owner authority fence is qualified.
mod persistence;
mod query;
pub mod read_registry;
#[cfg(test)]
mod tests;

use chrono::{DateTime, Utc};
use post_application::concert_feed::{Access, Authority, Context, DenyAuthority, Fence};
use post_domain::concert_feed::Error;
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use std::sync::Arc;

#[derive(Clone)]
pub struct Owner {
    pool: PgPool,
    authority: Arc<dyn Authority>,
}
impl Owner {
    /// There is deliberately no production constructor accepting an injected
    /// authority and no configuration switch to enable a test implementation.
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            authority: Arc::new(DenyAuthority),
        }
    }

    async fn admit(
        &self,
        context: &Context,
        access: &Access,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<(Box<dyn Fence>, [u8; 32]), Error> {
        let now = db_now(tx).await?;
        context.validate(now)?;
        let binding = digest(&(context, access))?;
        let fence = self.authority.acquire(context, access, binding).await?;
        fence.check(context, &binding, now)?;
        Ok((fence, binding))
    }
}
fn digest<T: Serialize>(value: &T) -> Result<[u8; 32], Error> {
    Ok(Sha256::digest(serde_json::to_vec(value).map_err(|_| Error::Invalid)?).into())
}
fn storage(_: sqlx::Error) -> Error {
    Error::Unavailable
}
async fn db_now(tx: &mut Transaction<'_, Postgres>) -> Result<DateTime<Utc>, Error> {
    sqlx::query_scalar("select clock_timestamp()")
        .fetch_one(&mut **tx)
        .await
        .map_err(storage)
}
async fn checked_commit(
    tx: Transaction<'_, Postgres>,
    context: &Context,
    guards: &[(Box<dyn Fence>, [u8; 32])],
) -> Result<(), Error> {
    let mut tx = tx;
    let now = db_now(&mut tx).await?;
    context.validate(now)?;
    for (guard, binding) in guards {
        guard.check(context, binding, now)?;
    }
    // Guards remain alive in caller scope until commit has completed. A real
    // implementation must hold authority locks, rather than check a stale bool.
    tx.commit().await.map_err(storage)
}
