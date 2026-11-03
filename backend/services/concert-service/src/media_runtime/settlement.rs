//! Internal owner settlement primitives. Never a public HTTP/RPC authorization grant.
use concert_application::commands::CommandError;
use sqlx::{PgConnection, PgPool};

/// Capture on the actual effects connection before allowing the producer to apply.
/// Canonical guard registration must persist this identity BEFORE authorize_in
/// returns. The configured DB role needs pg_control_system visibility; missing
/// privilege fails closed rather than substituting a process-local cluster name.
#[derive(Clone, PartialEq, Eq)]
pub struct MediaTransactionIdentity {
    system_identifier: String,
    database_oid: i64,
    xid: String,
}
impl MediaTransactionIdentity {
    pub async fn capture_in(c: &mut PgConnection) -> Result<Self, CommandError> {
        let (system_identifier, database_oid, xid): (String, i64, String) = sqlx::query_as(
            "select s.system_identifier::text,d.oid::bigint,pg_current_xact_id()::text from pg_control_system() s cross join pg_database d where d.datname=current_database()",
        ).fetch_one(c).await.map_err(|_| CommandError::Unavailable)?;
        if database_oid <= 0 || !decimal(&system_identifier) || !decimal(&xid) {
            return Err(CommandError::Unavailable);
        }
        Ok(Self {
            system_identifier,
            database_oid,
            xid,
        })
    }
    pub fn system_identifier(&self) -> &str {
        &self.system_identifier
    }
    pub fn database_oid(&self) -> i64 {
        self.database_oid
    }
    pub fn xid(&self) -> &str {
        &self.xid
    }
}
/// This is only the transaction outcome. It does not settle a whole request, seal
/// late work, authorize a receipt, or release any canonical owner prepared guard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaTransactionState {
    InProgress,
    Committed,
    Aborted,
    Unknown,
}
/// Run only while retaining the qualified owner quarantine and after sealing all
/// possible request work. Never infer abortion from pool/backend disappearance,
/// request timeout, deadline expiry or a missing command receipt.
pub async fn transaction_state(
    pool: &PgPool,
    identity: &MediaTransactionIdentity,
) -> MediaTransactionState {
    let result = async {
        let mut tx = pool.begin().await?;
        let actual: (String, i64) = sqlx::query_as("select s.system_identifier::text,d.oid::bigint from pg_control_system() s cross join pg_database d where d.datname=current_database()")
            .fetch_one(&mut *tx).await?;
        if actual != (identity.system_identifier.clone(), identity.database_oid) {
            return Ok::<_,sqlx::Error>(MediaTransactionState::Unknown);
        }
        let state: Option<String> = sqlx::query_scalar("select pg_xact_status($1::text::xid8)")
            .bind(&identity.xid).fetch_one(&mut *tx).await?;
        let state = match state.as_deref() {
            Some("in progress") => MediaTransactionState::InProgress,
            Some("committed") => MediaTransactionState::Committed,
            Some("aborted") => MediaTransactionState::Aborted,
            _ => MediaTransactionState::Unknown,
        };
        tx.commit().await?;
        Ok(state)
    }.await;
    result.unwrap_or(MediaTransactionState::Unknown)
}
fn decimal(s: &str) -> bool {
    !s.is_empty() && s.len() <= 20 && s.bytes().all(|b| b.is_ascii_digit()) && s != "0"
}
