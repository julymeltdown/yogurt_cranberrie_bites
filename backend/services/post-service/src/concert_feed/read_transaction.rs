//! Actual read-transaction identity and outcome lookup for canonical settlement.
//! A terminal PostgreSQL status is necessary, never sufficient: the caller must
//! also authenticate owner registration and seal/drain the matching read scope.
//! PostgreSQL semantics: https://www.postgresql.org/docs/16/functions-info.html#FUNCTIONS-TXID-SNAPSHOT
use chrono::{DateTime, Utc};
use post_domain::concert_feed::Error;
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

/// Constructed only on the actual protected transaction, not from RPC fields.
/// No Debug/Serialize: producer references and digests stay out of diagnostics.
#[derive(Clone, PartialEq, Eq)]
pub struct CapturedReadTransaction {
    instance: Uuid,
    full_xid: String,
    backend_pid: i32,
    backend_started: DateTime<Utc>,
    system_identifier: String,
    database_oid: u32,
    effect_binding: [u8; 32],
    settlement_reference: [u8; 32],
}
impl CapturedReadTransaction {
    /// Requires the producer role's explicit pg_control_system permission.
    /// A missing store-identity grant fails closed; configured URLs are not IDs.
    pub async fn capture(
        tx: &mut Transaction<'_, Postgres>,
        instance: Uuid,
        effect_binding: [u8; 32],
        settlement_reference: [u8; 32],
    ) -> Result<Self, Error> {
        if instance.is_nil() || effect_binding == [0; 32] || settlement_reference == [0; 32] {
            return Err(Error::Invalid);
        }
        let row=sqlx::query("select pg_current_xact_id()::text as full_xid,pg_backend_pid() as backend_pid,(select backend_start from pg_stat_activity where pid=pg_backend_pid()) as backend_started,(select system_identifier::text from pg_control_system()) as system_identifier,(select oid::bigint from pg_database where datname=current_database()) as database_oid")
            .fetch_one(&mut **tx).await.map_err(|_|Error::Unavailable)?;
        let result = Self {
            instance,
            effect_binding,
            settlement_reference,
            full_xid: row.try_get("full_xid").map_err(|_| Error::Unavailable)?,
            backend_pid: row.try_get("backend_pid").map_err(|_| Error::Unavailable)?,
            backend_started: row
                .try_get("backend_started")
                .map_err(|_| Error::Unavailable)?,
            system_identifier: row
                .try_get("system_identifier")
                .map_err(|_| Error::Unavailable)?,
            database_oid: u32::try_from(
                row.try_get::<i64, _>("database_oid")
                    .map_err(|_| Error::Unavailable)?,
            )
            .map_err(|_| Error::Unavailable)?,
        };
        if !canonical_number(&result.full_xid)
            || !canonical_number(&result.system_identifier)
            || result.database_oid == 0
            || result.backend_pid <= 0
            || result.backend_started > Utc::now()
        {
            return Err(Error::Unavailable);
        }
        Ok(result)
    }
    pub fn instance(&self) -> Uuid {
        self.instance
    }
    pub fn full_xid(&self) -> &str {
        &self.full_xid
    }
    pub fn backend_pid(&self) -> i32 {
        self.backend_pid
    }
    pub fn backend_started(&self) -> DateTime<Utc> {
        self.backend_started
    }
    pub fn system_identifier(&self) -> &str {
        &self.system_identifier
    }
    pub fn database_oid(&self) -> u32 {
        self.database_oid
    }
    pub fn effect_binding(&self) -> [u8; 32] {
        self.effect_binding
    }
    pub fn settlement_reference(&self) -> [u8; 32] {
        self.settlement_reference
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ReadTransactionState {
    InProgress,
    Committed,
    Aborted,
    Unknown,
}
/// Does not release a canonical owner guard or complete a read scope. Unknown
/// xid retention, unavailable store, different cluster/database/process, or any
/// SQL failure always stays Unknown. An absent backend is not abort evidence.
pub async fn resolve(
    pool: &PgPool,
    identity: &CapturedReadTransaction,
    current_instance: Uuid,
    stop: tokio::time::Instant,
) -> ReadTransactionState {
    if current_instance != identity.instance {
        return ReadTransactionState::Unknown;
    }
    let work = async {
        let mut tx = pool.begin().await.map_err(|_| Error::Unavailable)?;
        let row=sqlx::query("select (select system_identifier::text from pg_control_system()) as system_identifier,(select oid::bigint from pg_database where datname=current_database()) as database_oid,pg_xact_status($1::xid8) as transaction_status")
            .bind(&identity.full_xid).fetch_one(&mut *tx).await.map_err(|_|Error::Unavailable)?;
        let system: String = row
            .try_get("system_identifier")
            .map_err(|_| Error::Unavailable)?;
        let database: i64 = row
            .try_get("database_oid")
            .map_err(|_| Error::Unavailable)?;
        if system != identity.system_identifier || database != i64::from(identity.database_oid) {
            return Err(Error::Authority);
        }
        let state = match row
            .try_get::<Option<String>, _>("transaction_status")
            .map_err(|_| Error::Unavailable)?
            .as_deref()
        {
            Some("in progress") => ReadTransactionState::InProgress,
            Some("committed") => ReadTransactionState::Committed,
            Some("aborted") => ReadTransactionState::Aborted,
            _ => ReadTransactionState::Unknown,
        };
        tx.commit().await.map_err(|_| Error::Unavailable)?;
        Ok::<_, Error>(state)
    };
    match tokio::time::timeout_at(stop, work).await {
        Ok(Ok(state)) => state,
        _ => ReadTransactionState::Unknown,
    }
}
fn canonical_number(value: &str) -> bool {
    value
        .parse::<u64>()
        .ok()
        .is_some_and(|number| number > 0 && number.to_string() == value)
}
