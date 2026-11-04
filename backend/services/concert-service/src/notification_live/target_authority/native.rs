//! Read-only native exclusion. Durable intent precedes PREPARE; only exact
//! terminal Root + completed Notification evidence permits abort/release.
//! Native identity/GID proof follows public_profile/local.rs (same source revision).
use super::unavailable;
use crate::adapters::out::relationship_authority::guarded::{
    ChildNativeIdentity, ChildQuery, ChildRecoveryReference, HeldChildGuard, OwnerGuardScope,
};
use platform::actor_delegation::authority_deadline::AuthorityRpcDeadline;
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool, Postgres, Row, Transaction};
use std::{sync::Arc, time::Duration};
use tonic::Status;
use uuid::Uuid;

pub(super) async fn budget(
    c: &mut PgConnection,
    deadline: AuthorityRpcDeadline,
) -> Result<(), Status> {
    let millis = deadline
        .remaining(Duration::from_secs(5), deadline.expires_micros())?
        .as_millis();
    if millis == 0 {
        return Err(unavailable());
    }
    sqlx::query(
        "select set_config('statement_timeout',$1,true),set_config('lock_timeout',$1,true)",
    )
    .bind(millis.to_string())
    .execute(c)
    .await
    .map_err(|_| unavailable())?;
    Ok(())
}
pub(super) async fn prepare(
    pool: PgPool,
    mut tx: Transaction<'_, Postgres>,
    scope: &Arc<OwnerGuardScope>,
    deadline: AuthorityRpcDeadline,
    snapshot_digest: [u8; 32],
) -> Result<(), Status> {
    budget(&mut tx, deadline).await?;
    let count: i32 = sqlx::query_scalar("select current_setting('max_prepared_transactions')::int")
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| unavailable())?;
    if count < 256 {
        return Err(unavailable());
    }
    let mut acquired = false;
    for slot in 0i32..101 {
        budget(&mut tx, deadline).await?;
        if sqlx::query_scalar::<_, bool>(
            "select pg_try_advisory_xact_lock(1296257617::int,$1::int)",
        )
        .bind(slot)
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| unavailable())?
        {
            acquired = true;
            break;
        }
    }
    if !acquired {
        return Err(Status::resource_exhausted("Target read capacity"));
    }
    let producer = scope.producer_scope();
    let instance = super::protocol::uuid(&producer.instance_id)?;
    let root_scope_id = super::protocol::uuid(&producer.scope_id)?;
    // A callback can only attach to the one already registered actual Inbox
    // dispatch. No caller supplies its private completion capability.
    let linked: Vec<(Uuid,Vec<u8>)> = sqlx::query_as("select intent_id,reference_digest from concert_product_child_intents where instance_id=$1 and scope_id=$2 and binding_digest=$3 and kind='NOTIFICATION_INBOX' and completed_at is null order by intent_id limit 2")
        .bind(instance).bind(root_scope_id).bind(scope.binding().digest().as_slice())
        .fetch_all(&mut *tx).await.map_err(|_| unavailable())?;
    if linked.len() != 1 {
        return Err(unavailable());
    }
    let b = scope.binding();
    let reference = ChildRecoveryReference::TargetRead {
        query: ChildQuery {
            actor: b.actor(),
            event: b.event(),
            subject: b.actor(),
            family: b.session_family(),
            request: b.request_id(),
            expires_unix_micros: b.deadline().expires_micros(),
            binding_digest: b.digest(),
        },
        root_scope_id,
        child_nonce: Uuid::new_v4(),
        native: ChildNativeIdentity::capture_concert_in(&mut tx, scope).await?,
        inbox_intent_id: linked[0].0,
        inbox_reference_digest: linked[0]
            .1
            .as_slice()
            .try_into()
            .map_err(|_| unavailable())?,
        snapshot_digest,
    };
    scope.register_child_intent(&reference).await?;
    budget(&mut tx, deadline).await?;
    let gid = reference.local_prepared_gid()?;
    sqlx::query(&format!("prepare transaction '{gid}'"))
        .execute(&mut *tx)
        .await
        .map_err(|_| unavailable())?;
    tx.commit().await.map_err(|_| unavailable())?;
    let child = Arc::new(TargetReadChild {
        pool,
        reference,
        deadline,
    });
    // A lost/cancelled attach leaves the exact durable intent for recovery.
    scope.retain_child(child.clone()).await?;
    child.check().await
}
struct TargetReadChild {
    pool: PgPool,
    reference: ChildRecoveryReference,
    deadline: AuthorityRpcDeadline,
}
#[async_trait::async_trait]
impl HeldChildGuard for TargetReadChild {
    fn recovery_reference(&self) -> Result<ChildRecoveryReference, Status> {
        Ok(self.reference.clone())
    }
    async fn check(&self) -> Result<(), Status> {
        let ChildRecoveryReference::TargetRead { native, .. } = &self.reference else {
            return Err(unavailable());
        };
        let remaining = self
            .deadline
            .remaining(Duration::from_secs(5), self.deadline.expires_micros())?;
        let held:bool=tokio::time::timeout(remaining,sqlx::query_scalar("select exists(select 1 from pg_prepared_xacts where database=current_database() and owner=current_user and gid=$1 and transaction::text::numeric=mod($2::text::numeric,4294967296)) and (pg_control_system()).system_identifier::text=$3 and (select oid::bigint from pg_database where datname=current_database())=$4")
            .bind(self.reference.local_prepared_gid()?).bind(&native.full_xid).bind(&native.system_identifier).bind(i64::from(native.database_oid)).fetch_one(&self.pool))
            .await.map_err(|_| unavailable())?.map_err(|_| unavailable())?;
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
/// Typed recovery owner, constructed before the canonical core to avoid a
/// second registry or core cycle. The configured instance is immutable.
pub struct TargetReadSettler {
    pool: PgPool,
    instance: Uuid,
}
impl TargetReadSettler {
    pub fn new(pool: PgPool, instance: Uuid) -> Result<Self, Status> {
        if instance.is_nil() {
            return Err(Status::invalid_argument(
                "Target recovery instance required",
            ));
        }
        Ok(Self { pool, instance })
    }
}
#[async_trait::async_trait]
impl crate::product_rpc::child_settlement::NotificationTargetIntentSettler for TargetReadSettler {
    async fn recover_target_read(&self, reference: &ChildRecoveryReference) -> Result<(), Status> {
        let ChildRecoveryReference::TargetRead { native, .. } = reference else {
            return Err(unavailable());
        };
        if native.producer_instance_id != self.instance {
            return Err(unavailable());
        }
        recover_one(&self.pool, reference).await
    }
}
async fn recover_one(pool: &PgPool, reference: &ChildRecoveryReference) -> Result<(), Status> {
    let ChildRecoveryReference::TargetRead {
        query,
        native,
        root_scope_id,
        inbox_intent_id,
        inbox_reference_digest,
        snapshot_digest,
        ..
    } = reference
    else {
        return Err(unavailable());
    };
    reference.intent_id()?;
    if query.subject != query.actor
        || [
            query.actor,
            query.event,
            query.family,
            query.request,
            *root_scope_id,
            *inbox_intent_id,
        ]
        .iter()
        .any(Uuid::is_nil)
        || query.binding_digest == [0; 32]
        || *inbox_reference_digest == [0; 32]
        || *snapshot_digest == [0; 32]
    {
        return Err(unavailable());
    }
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
    let reference_digest =
        Sha256::digest(serde_json::to_vec(reference).map_err(|_| unavailable())?);
    tokio::time::timeout(Duration::from_secs(2),async{
        // This dependency is checked on EVERY attempt, including absent GID or
        // expired disclosure. The sweeper may visit children in any order.
        let settled:bool=sqlx::query_scalar("select exists(select 1 from concert_product_child_intents c join concert_product_owner_scopes s using(instance_id,scope_id,binding_digest) where c.instance_id=$1 and c.scope_id=$2 and c.binding_digest=$3 and c.kind='NOTIFICATION_INBOX' and c.intent_id=$4 and c.reference_digest=$5 and c.completed_at is not null and s.phase='ROOT_FINISHED' and exists(select 1 from concert_product_child_intents t where t.instance_id=c.instance_id and t.scope_id=c.scope_id and t.binding_digest=c.binding_digest and t.kind='TARGET_READ' and t.intent_id=$6 and t.reference_digest=$7))")
            .bind(native.producer_instance_id).bind(root_scope_id).bind(query.binding_digest.as_slice()).bind(inbox_intent_id).bind(inbox_reference_digest.as_slice()).bind(reference.intent_id()?).bind(reference_digest.as_slice())
            .fetch_one(pool).await.map_err(|_| unavailable())?;
        if !settled{return Err(unavailable());}
        let current=sqlx::query("select (pg_control_system()).system_identifier::text system_identifier,(select oid::bigint from pg_database where datname=current_database()) database_oid")
            .fetch_one(pool).await.map_err(|_| unavailable())?;
        if current.try_get::<String,_>("system_identifier").map_err(|_| unavailable())?!=native.system_identifier
            || current.try_get::<i64,_>("database_oid").map_err(|_| unavailable())?!=i64::from(native.database_oid){return Err(unavailable());}
        let actual:Option<String>=sqlx::query_scalar("select transaction::text from pg_prepared_xacts where database=current_database() and owner=current_user and gid=$1")
            .bind(&gid).fetch_optional(pool).await.map_err(|_| unavailable())?;
        if let Some(actual)=actual {
            let low=native.full_xid.parse::<u64>().map_err(|_| unavailable())?%4_294_967_296;
            if actual!=low.to_string(){return Err(unavailable());}
            let _=sqlx::query(&format!("rollback prepared '{gid}'")).execute(pool).await;
        }
        let state:Option<String>=sqlx::query_scalar("select pg_xact_status($1::text::xid8)").bind(&native.full_xid).fetch_one(pool).await.map_err(|_| unavailable())?;
        if state.as_deref()==Some("aborted"){Ok(())}else{Err(unavailable())}
    }).await.map_err(|_| unavailable())?
}
