//! Native identity is captured on the actual protected connection, never a peer hint.
use super::model::Error;
use chrono::{DateTime, Utc};
use platform::actor_delegation::authority_deadline::AuthorityRpcDeadline;
use sqlx::{Connection, PgConnection, PgPool, Row};
use std::time::Duration;
use uuid::Uuid;

const CEILING: Duration = Duration::from_secs(5);
#[derive(Clone, PartialEq, Eq)]
pub(super) struct TransactionIdentity {
    pub system_identifier: String,
    pub database_oid: u32,
    pub full_xid: String,
    pub backend_pid: i32,
    pub backend_started: DateTime<Utc>,
    pub producer_instance: Uuid,
    pub effect_digest: [u8; 32],
}
impl std::fmt::Debug for TransactionIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TransactionIdentity([REDACTED])")
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NativeStatus {
    InProgress,
    Prepared,
    Committed,
    Aborted,
    Unknown,
}

/// A detached connection closes on cancellation/drop. An ordinary transaction
/// then aborts; a transaction already prepared remains durable and quarantined.
/// No cancellation path fabricates which of those actually happened.
pub(super) struct EffectConnection {
    connection: PgConnection,
    identity: TransactionIdentity,
    deadline: AuthorityRpcDeadline,
}
impl EffectConnection {
    pub async fn open(
        pool: &PgPool,
        instance: Uuid,
        effect_digest: [u8; 32],
        deadline: AuthorityRpcDeadline,
    ) -> Result<Self, Error> {
        if instance.is_nil() || effect_digest == [0; 32] {
            return Err(Error::Invalid);
        }
        remaining(deadline)?;
        let pooled = tokio::time::timeout_at(deadline.stop(), pool.acquire())
            .await
            .map_err(|_| Error::Unavailable)?
            .map_err(unavailable)?;
        let mut connection = pooled.detach();
        remaining(deadline)?;
        sqlx::query("begin")
            .execute(&mut connection)
            .await
            .map_err(unavailable)?;
        install_timeout(&mut connection, deadline).await?;
        // Assign the full XID before any protected effect. A PID alone can be
        // recycled; cluster/database and backend start are all retained.
        let row = sqlx::query("select pg_current_xact_id()::text as full_xid, (select system_identifier::text from pg_control_system()) as system_identifier, (select oid::bigint from pg_database where datname=current_database()) as database_oid, pg_backend_pid() as backend_pid, (select backend_start from pg_stat_activity where pid=pg_backend_pid()) as backend_started")
            .fetch_one(&mut connection).await.map_err(unavailable)?;
        remaining(deadline)?;
        let full_xid: String = row.try_get("full_xid").map_err(unavailable)?;
        let system_identifier: String = row.try_get("system_identifier").map_err(unavailable)?;
        let oid: i64 = row.try_get("database_oid").map_err(unavailable)?;
        let database_oid = u32::try_from(oid).map_err(|_| Error::Unavailable)?;
        let backend_pid = row.try_get("backend_pid").map_err(unavailable)?;
        let identity = TransactionIdentity {
            full_xid,
            system_identifier,
            database_oid,
            backend_pid,
            backend_started: row.try_get("backend_started").map_err(unavailable)?,
            producer_instance: instance,
            effect_digest,
        };
        identity.validate()?;
        Ok(Self {
            connection,
            identity,
            deadline,
        })
    }
    pub fn identity(&self) -> &TransactionIdentity {
        &self.identity
    }
    pub fn connection(&mut self) -> &mut PgConnection {
        &mut self.connection
    }
    pub fn check(&self) -> Result<(), Error> {
        remaining(self.deadline).map(|_| ())
    }
    pub async fn prepare(mut self, operation: Uuid) -> Result<(), Error> {
        self.check()?;
        let gid = prepared_gid(operation)?;
        // GID is generated solely from a checked UUID, never interpolated input.
        sqlx::query(&format!("prepare transaction '{gid}'"))
            .execute(&mut self.connection)
            .await
            .map_err(unavailable)?;
        // A late/unknown result must be resolved by actual native identity. It
        // must not be converted to an abort just because this local check fails.
        self.check()?;
        self.connection.close().await.map_err(unavailable)
    }
}
impl TransactionIdentity {
    pub fn validate(&self) -> Result<(), Error> {
        if !canonical_number(&self.full_xid)
            || !canonical_number(&self.system_identifier)
            || self.database_oid == 0
            || self.backend_pid <= 0
            || self.backend_started.timestamp_micros() <= 0
            || self.producer_instance.is_nil()
            || self.effect_digest == [0; 32]
        {
            return Err(Error::Invalid);
        }
        Ok(())
    }
}
fn canonical_number(value: &str) -> bool {
    value
        .parse::<u64>()
        .ok()
        .is_some_and(|n| n > 0 && n.to_string() == value)
}
pub(super) fn prepared_gid(operation: Uuid) -> Result<String, Error> {
    if operation.is_nil() {
        return Err(Error::Invalid);
    }
    Ok(format!("mcr_sp_{}", operation.simple()))
}
pub(super) fn remaining(deadline: AuthorityRpcDeadline) -> Result<u128, Error> {
    let remaining = deadline
        .remaining(CEILING, deadline.expires_micros())
        .map_err(|_| Error::Unavailable)?
        .as_millis();
    // Zero disables PostgreSQL timeouts. Refuse rather than rounding up.
    if remaining == 0 {
        Err(Error::Unavailable)
    } else {
        Ok(remaining)
    }
}
pub(super) async fn install_timeout(
    connection: &mut PgConnection,
    deadline: AuthorityRpcDeadline,
) -> Result<(), Error> {
    let millis = remaining(deadline)?.to_string();
    sqlx::query(
        "select set_config('statement_timeout',$1,true),set_config('lock_timeout',$1,true)",
    )
    .bind(millis)
    .execute(connection)
    .await
    .map_err(unavailable)?;
    remaining(deadline).map(|_| ())
}
pub(super) async fn native_status(
    pool: &PgPool,
    identity: &TransactionIdentity,
    operation: Uuid,
    recovery: AuthorityRpcDeadline,
) -> Result<NativeStatus, Error> {
    identity.validate()?;
    remaining(recovery)?;
    let mut tx = tokio::time::timeout_at(recovery.stop(), pool.begin())
        .await
        .map_err(|_| Error::Unavailable)?
        .map_err(unavailable)?;
    install_timeout(&mut tx, recovery).await?;
    let store: (String, i64) = sqlx::query_as("select (select system_identifier::text from pg_control_system()),(select oid::bigint from pg_database where datname=current_database())").fetch_one(&mut *tx).await.map_err(unavailable)?;
    if store
        != (
            identity.system_identifier.clone(),
            i64::from(identity.database_oid),
        )
    {
        return Err(Error::Unavailable);
    }
    let status: Option<String> = sqlx::query_scalar("select pg_xact_status($1::text::xid8)")
        .bind(&identity.full_xid)
        .fetch_one(&mut *tx)
        .await
        .map_err(unavailable)?;
    let prepared: bool = sqlx::query_scalar("select exists(select 1 from pg_prepared_xacts where gid=$1 and database=current_database() and owner=current_user and transaction::text=$2)")
        .bind(prepared_gid(operation)?).bind((identity.full_xid.parse::<u64>().map_err(|_|Error::Invalid)? as u32).to_string()).fetch_one(&mut *tx).await.map_err(unavailable)?;
    let result = match status.as_deref() {
        Some("in progress") if prepared => NativeStatus::Prepared,
        Some("in progress") => NativeStatus::InProgress,
        Some("committed") if !prepared => NativeStatus::Committed,
        Some("aborted") if !prepared => NativeStatus::Aborted,
        _ => NativeStatus::Unknown,
    };
    tx.commit().await.map_err(unavailable)?;
    remaining(recovery)?;
    Ok(result)
}
fn unavailable(_: sqlx::Error) -> Error {
    Error::Unavailable
}
