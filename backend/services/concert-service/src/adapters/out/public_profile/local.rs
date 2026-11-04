//! Retains actual Concert event/admission/avatar locks through outer response assembly.
use super::{profile::Content, unavailable, HeldChildGuard, LocalPolicySnapshot};
use platform::actor_delegation::authority_deadline::AuthorityRpcDeadline;
use sqlx::{PgPool, Postgres, Transaction};
use std::time::Duration;
use tonic::Status;
pub(super) struct LocalChild {
    pool: PgPool,
    gid: String,
    deadline: AuthorityRpcDeadline,
    pub(super) policy: LocalPolicySnapshot,
    pub(super) avatar: Result<Option<serde_json::Value>, ()>,
    pub(super) content: tokio::sync::Mutex<Option<Content>>,
}
pub(super) async fn prepare(
    pool: PgPool,
    mut tx: Transaction<'_, Postgres>,
    deadline: AuthorityRpcDeadline,
    policy: LocalPolicySnapshot,
    avatar: Result<Option<serde_json::Value>, ()>,
) -> Result<LocalChild, Status> {
    budget(&mut tx, deadline).await?;
    let slots: i32 = sqlx::query_scalar("select current_setting('max_prepared_transactions')::int")
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| unavailable())?;
    if slots < 256 {
        return Err(unavailable());
    }
    let mut acquired = false;
    for slot in 0i32..101 {
        budget(&mut tx, deadline).await?;
        let ok: bool =
            sqlx::query_scalar("select pg_try_advisory_xact_lock(1296257617::int,$1::int)")
                .bind(slot)
                .fetch_one(&mut *tx)
                .await
                .map_err(|_| unavailable())?;
        if ok {
            acquired = true;
            break;
        }
    }
    if !acquired {
        return Err(unavailable());
    }
    let gid = format!("mcr-public-avatar:{}", uuid::Uuid::new_v4().simple());
    budget(&mut tx, deadline).await?;
    sqlx::query(&format!("prepare transaction '{gid}'"))
        .execute(&mut *tx)
        .await
        .map_err(|_| unavailable())?;
    tx.commit().await.map_err(|_| unavailable())?;
    deadline.remaining(Duration::from_secs(5), deadline.expires_micros())?;
    Ok(LocalChild {
        pool,
        gid,
        deadline,
        policy,
        avatar,
        content: tokio::sync::Mutex::new(None),
    })
}
#[async_trait::async_trait]
impl HeldChildGuard for LocalChild {
    async fn check(&self) -> Result<(), Status> {
        let remaining = self
            .deadline
            .remaining(Duration::from_secs(5), self.deadline.expires_micros())?;
        let held:bool=tokio::time::timeout(remaining,sqlx::query_scalar("select exists(select 1 from pg_prepared_xacts where database=current_database() and owner=current_user and gid=$1)").bind(&self.gid).fetch_one(&self.pool)).await.map_err(|_|unavailable())?.map_err(|_|unavailable())?;
        if held {
            Ok(())
        } else {
            Err(unavailable())
        }
    }
    async fn complete_settled(&self) -> Result<(), Status> {
        tokio::time::timeout(
            Duration::from_secs(2),
            sqlx::query(&format!("rollback prepared '{}'", self.gid)).execute(&self.pool),
        )
        .await
        .map_err(|_| unavailable())?
        .map_err(|_| unavailable())?;
        Ok(())
    }
}

pub(super) async fn budget(
    tx: &mut Transaction<'_, Postgres>,
    deadline: AuthorityRpcDeadline,
) -> Result<(), Status> {
    let millis = deadline
        .remaining(Duration::from_secs(5), deadline.expires_micros())?
        .as_millis();
    if millis == 0 {
        return Err(unavailable());
    }
    sqlx::query("select set_config('statement_timeout',$1,true)")
        .bind(millis.to_string())
        .execute(&mut **tx)
        .await
        .map_err(|_| unavailable())?;
    deadline.remaining(Duration::from_secs(5), deadline.expires_micros())?;
    Ok(())
}
