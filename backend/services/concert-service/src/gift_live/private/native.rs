//! Exact native read exclusion. No content or canonical authority is minted here.
use crate::adapters::out::relationship_authority::guarded::{
    ChildRecoveryReference, HeldChildGuard,
};
use platform::actor_delegation::authority_deadline::AuthorityRpcDeadline;
use sqlx::{PgConnection, PgPool, Postgres, Row, Transaction};
use std::time::Duration;
use tonic::Status;

pub(super) fn unavailable() -> Status {
    Status::unavailable("Gift private exclusion unavailable")
}
pub(super) async fn budget(
    c: &mut PgConnection,
    deadline: AuthorityRpcDeadline,
    until: i64,
) -> Result<(), Status> {
    let remaining = deadline.remaining(Duration::from_secs(2), until)?;
    let millis = remaining.as_millis();
    if millis == 0 {
        return Err(unavailable());
    }
    // Zero means unbounded in PG. Clamp an existing nonzero timeout instead of
    // widening a caller/resource's shorter limit. Leave lock_timeout unchanged.
    sqlx::query("select set_config('statement_timeout',(extract(epoch from least(coalesce(nullif(current_setting('statement_timeout')::interval,interval '0'),$1::text::interval),$1::text::interval))*1000)::bigint::text||'ms',true)")
        .bind(format!("{millis}ms"))
        .execute(&mut *c)
        .await
        .map_err(|_| unavailable())?;
    let until_time =
        chrono::DateTime::<chrono::Utc>::from_timestamp_micros(until).ok_or_else(unavailable)?;
    let alive: bool = sqlx::query_scalar("select clock_timestamp()<$1::timestamptz")
        .bind(until_time)
        .fetch_one(c)
        .await
        .map_err(|_| unavailable())?;
    if !alive {
        return Err(Status::deadline_exceeded("Gift disclosure expired"));
    }
    deadline.remaining(Duration::from_secs(2), until)?;
    Ok(())
}
pub(super) async fn reserve(c: &mut PgConnection) -> Result<(), Status> {
    let slots: i32 = sqlx::query_scalar("select current_setting('max_prepared_transactions')::int")
        .fetch_one(&mut *c)
        .await
        .map_err(|_| unavailable())?;
    if slots < 256 {
        return Err(unavailable());
    }
    // Gift-only global budget, distinct from PublicProfile/avatar's 101 slots.
    for slot in 0i32..101 {
        let acquired: bool =
            sqlx::query_scalar("select pg_try_advisory_xact_lock(1296257623::int,$1::int)")
                .bind(slot)
                .fetch_one(&mut *c)
                .await
                .map_err(|_| unavailable())?;
        if acquired {
            return Ok(());
        }
    }
    Err(Status::resource_exhausted("Gift private guard capacity"))
}
pub(super) struct NativeChild {
    pool: PgPool,
    reference: ChildRecoveryReference,
    deadline: AuthorityRpcDeadline,
}
/// Caller MUST persist the exact typed intent before calling this helper.
/// Cancellation/drop is not a terminal action on an uncertain prepared child.
pub(super) async fn prepare_after_intent(
    pool: PgPool,
    mut tx: Transaction<'_, Postgres>,
    reference: ChildRecoveryReference,
    deadline: AuthorityRpcDeadline,
) -> Result<NativeChild, Status> {
    shape(&reference)?;
    let until = reference.content_expires_unix_micros();
    budget(&mut tx, deadline, until).await?;
    let gid = reference.local_prepared_gid()?;
    sqlx::query(&format!("prepare transaction '{gid}'"))
        .execute(&mut *tx)
        .await
        .map_err(|_| unavailable())?;
    // PREPARE ended the native transaction; clear SQLx's transaction bookkeeping.
    // The actual prepared transaction remains durable after this COMMIT.
    tx.commit().await.map_err(|_| unavailable())?;
    let child = NativeChild {
        pool,
        reference,
        deadline,
    };
    child.check().await?;
    Ok(child)
}
fn shape(reference: &ChildRecoveryReference) -> Result<(), Status> {
    let ChildRecoveryReference::GiftPrivate {
        query,
        native,
        giveaway_id,
        reservation_id,
        valid_until_unix_micros,
        ..
    } = reference
    else {
        return Err(Status::failed_precondition("Not a Gift private intent"));
    };
    reference.intent_id()?;
    reference.local_prepared_gid()?;
    if [
        query.actor,
        query.event,
        query.subject,
        query.family,
        query.request,
        *giveaway_id,
        *reservation_id,
    ]
    .iter()
    .any(uuid::Uuid::is_nil)
        || query.binding_digest == [0; 32]
        || *valid_until_unix_micros <= 0
        || *valid_until_unix_micros > query.expires_unix_micros
        || chrono::DateTime::from_timestamp_micros(*valid_until_unix_micros).is_none()
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
    Ok(())
}
#[async_trait::async_trait]
impl HeldChildGuard for NativeChild {
    fn recovery_reference(&self) -> Result<ChildRecoveryReference, Status> {
        Ok(self.reference.clone())
    }
    async fn check(&self) -> Result<(), Status> {
        shape(&self.reference)?;
        self.reference.check_content_time()?;
        let remaining = self.deadline.remaining(
            Duration::from_secs(2),
            self.reference.content_expires_unix_micros(),
        )?;
        let ChildRecoveryReference::GiftPrivate {
            native,
            valid_until_unix_micros,
            ..
        } = &self.reference
        else {
            return Err(unavailable());
        };
        let gid = self.reference.local_prepared_gid()?;
        let held:bool=tokio::time::timeout(remaining,sqlx::query_scalar(
            "select clock_timestamp()<$1::timestamptz and exists(select 1 from pg_prepared_xacts where database=current_database() and owner=current_user and gid=$2 and transaction::text::numeric=mod($3::text::numeric,4294967296)) and pg_xact_status($3::text::xid8)='in progress' and (pg_control_system()).system_identifier::text=$4 and (select oid::bigint from pg_database where datname=current_database())=$5")
            .bind(chrono::DateTime::<chrono::Utc>::from_timestamp_micros(*valid_until_unix_micros).ok_or_else(unavailable)?).bind(gid).bind(&native.full_xid)
            .bind(&native.system_identifier).bind(i64::from(native.database_oid)).fetch_one(&self.pool))
            .await.map_err(|_|unavailable())?.map_err(|_|unavailable())?;
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
/// Root must authenticate/match the exact encrypted saved intent before invoking
/// this recovery port. Expiry/missing GID/Drop alone never certifies completion.
pub(super) async fn recover_one(
    pool: &PgPool,
    reference: &ChildRecoveryReference,
) -> Result<(), Status> {
    shape(reference)?;
    let ChildRecoveryReference::GiftPrivate { native, .. } = reference else {
        return Err(unavailable());
    };
    let gid = reference.local_prepared_gid()?;
    tokio::time::timeout(Duration::from_secs(2),async {
        let current=sqlx::query("select (pg_control_system()).system_identifier::text system_identifier,(select oid::bigint from pg_database where datname=current_database()) database_oid")
            .fetch_one(pool).await.map_err(|_|unavailable())?;
        if current.try_get::<String,_>("system_identifier").map_err(|_|unavailable())?!=native.system_identifier
            || current.try_get::<i64,_>("database_oid").map_err(|_|unavailable())?!=i64::from(native.database_oid){return Err(unavailable());}
        let actual:Option<String>=sqlx::query_scalar("select transaction::text from pg_prepared_xacts where database=current_database() and owner=current_user and gid=$1")
            .bind(&gid).fetch_optional(pool).await.map_err(|_|unavailable())?;
        if let Some(actual)=actual {
            let low=native.full_xid.parse::<u64>().map_err(|_|unavailable())?%4_294_967_296;
            if actual!=low.to_string(){return Err(unavailable());}
            // A matching 32-bit prepared XID alone cannot prove the saved full
            // epoch identity. Check that exact full XID before any rollback.
            let state:Option<String>=sqlx::query_scalar("select pg_xact_status($1::text::xid8)")
                .bind(&native.full_xid).fetch_one(pool).await.map_err(|_|unavailable())?;
            if state.as_deref()!=Some("in progress"){return Err(unavailable());}
            // A concurrent exact retry may win the rollback; verify actual full
            // XID abort below rather than accepting a missing GID as evidence.
            let _=sqlx::query(&format!("rollback prepared '{gid}'")).execute(pool).await;
        }
        let state:Option<String>=sqlx::query_scalar("select pg_xact_status($1::text::xid8)")
            .bind(&native.full_xid).fetch_one(pool).await.map_err(|_|unavailable())?;
        if state.as_deref()==Some("aborted"){Ok(())}else{Err(unavailable())}
    }).await.map_err(|_|unavailable())?
}
