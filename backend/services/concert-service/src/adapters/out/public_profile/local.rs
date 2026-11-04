//! Retains actual Concert event/admission/avatar locks through outer response assembly.
use super::super::relationship_authority::guarded::ChildQuery;
use super::{
    profile::Content, unavailable, ChildRecoveryReference, GuardedSubject, HeldChildGuard,
    LocalPolicySnapshot,
};
use platform::actor_delegation::authority_deadline::AuthorityRpcDeadline;
use sqlx::Row;
use sqlx::{PgPool, Postgres, Transaction};
use std::time::Duration;
use tonic::Status;
pub(super) struct LocalChild {
    pool: PgPool,
    gid: String,
    reference: ChildRecoveryReference,
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
    subject: &GuardedSubject,
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
    let native = subject.capture_child_identity_in(&mut *tx).await?;
    let reference = ChildRecoveryReference::LocalPublic {
        query: ChildQuery::from_subject(subject),
        child_nonce: uuid::Uuid::new_v4(),
        native,
    };
    subject.register_child_intent(&reference).await?;
    let gid = reference.local_prepared_gid()?;
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
        reference,
        deadline,
        policy,
        avatar,
        content: tokio::sync::Mutex::new(None),
    })
}
#[async_trait::async_trait]
impl HeldChildGuard for LocalChild {
    fn recovery_reference(&self) -> Result<ChildRecoveryReference, Status> {
        Ok(self.reference.clone())
    }
    async fn check(&self) -> Result<(), Status> {
        let remaining = self
            .deadline
            .remaining(Duration::from_secs(5), self.deadline.expires_micros())?;
        let ChildRecoveryReference::LocalPublic { native, .. } = &self.reference else {
            return Err(unavailable());
        };
        let held:bool=tokio::time::timeout(remaining,sqlx::query_scalar("select exists(select 1 from pg_prepared_xacts where database=current_database() and owner=current_user and gid=$1 and transaction::text::numeric=mod($2::text::numeric,4294967296)) and (pg_control_system()).system_identifier::text=$3 and (select oid::bigint from pg_database where datname=current_database())=$4")
            .bind(&self.gid).bind(&native.full_xid).bind(&native.system_identifier).bind(i64::from(native.database_oid)).fetch_one(&self.pool)).await.map_err(|_|unavailable())?.map_err(|_|unavailable())?;
        if held {
            Ok(())
        } else {
            Err(unavailable())
        }
    }
    async fn complete_settled(&self) -> Result<(), Status> {
        recover_one(&self.pool, &self.reference).await
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
    // The pair reservation writes metadata, so its deferred PREPARE guard must
    // receive the original verified authority deadline too. Reuse the owning
    // policy helper: repeated calls can only shorten an existing deadline.
    let until = chrono::DateTime::from_timestamp_micros(deadline.expires_micros())
        .ok_or_else(unavailable)?;
    crate::relationship::pin_deadline(&mut **tx, until)
        .await
        .map_err(|_| unavailable())?;
    sqlx::query("select set_config('statement_timeout',$1,true)")
        .bind(millis.to_string())
        .execute(&mut **tx)
        .await
        .map_err(|_| unavailable())?;
    deadline.remaining(Duration::from_secs(5), deadline.expires_micros())?;
    Ok(())
}

/// Idempotence requires actual abort evidence from the exact immutable database.
/// Neither a missing prepared GID nor an expired response is a terminal proof.
pub(super) async fn recover_one(
    pool: &PgPool,
    reference: &ChildRecoveryReference,
) -> Result<(), Status> {
    let ChildRecoveryReference::LocalPublic { query, native, .. } = reference else {
        return Err(unavailable());
    };
    platform::actor_delegation::owner_binding::identity_digest(
        "CONCERT",
        &native.system_identifier,
        native.database_oid,
        &native.full_xid,
        native.backend_pid,
        native.backend_started_unix_micros,
        &query.binding_digest,
        native.producer_instance_id,
        &[],
    )?;
    let gid = reference.local_prepared_gid()?;
    tokio::time::timeout(Duration::from_secs(2),async {
        let current=sqlx::query("select (pg_control_system()).system_identifier::text system_identifier,(select oid::bigint from pg_database where datname=current_database()) database_oid").fetch_one(pool).await.map_err(|_|unavailable())?;
        if current.try_get::<String,_>("system_identifier").map_err(|_|unavailable())? != native.system_identifier
            || current.try_get::<i64,_>("database_oid").map_err(|_|unavailable())? != i64::from(native.database_oid) {return Err(unavailable());}
        let actual:Option<String>=sqlx::query_scalar("select transaction::text from pg_prepared_xacts where database=current_database() and owner=current_user and gid=$1").bind(&gid).fetch_optional(pool).await.map_err(|_|unavailable())?;
        if let Some(actual)=actual {
            let low=native.full_xid.parse::<u64>().map_err(|_|unavailable())? %4_294_967_296;
            if actual!=low.to_string(){return Err(unavailable());}
            let _=sqlx::query(&format!("rollback prepared '{gid}'")).execute(pool).await;
        }
        let state:Option<String>=sqlx::query_scalar("select pg_xact_status($1::text::xid8)").bind(&native.full_xid).fetch_one(pool).await.map_err(|_|unavailable())?;
        if state.as_deref()==Some("aborted"){Ok(())}else{Err(unavailable())}
    }).await.map_err(|_|unavailable())?
}
