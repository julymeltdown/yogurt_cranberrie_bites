//! Internal owner settlement primitives. Never a public HTTP/RPC authorization grant.
use chrono::{DateTime, Utc};
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
    backend_pid: i32,
    backend_started_at: DateTime<Utc>,
}
impl MediaTransactionIdentity {
    pub async fn capture_in(c: &mut PgConnection) -> Result<Self, CommandError> {
        let (system_identifier, database_oid, xid, backend_pid, backend_started_at): (String, i64, String, i32, DateTime<Utc>) = sqlx::query_as(
            "select s.system_identifier::text,d.oid::bigint,pg_current_xact_id()::text,pg_backend_pid(),a.backend_start from pg_control_system() s cross join pg_database d cross join pg_stat_activity a where d.datname=current_database() and a.pid=pg_backend_pid()",
        ).fetch_one(c).await.map_err(|_| CommandError::Unavailable)?;
        if !(1..=u32::MAX as i64).contains(&database_oid)
            || backend_pid <= 0
            || backend_started_at.timestamp_micros() <= 0
            || !decimal(&system_identifier)
            || !decimal(&xid)
        {
            return Err(CommandError::Unavailable);
        }
        Ok(Self {
            system_identifier,
            database_oid,
            xid,
            backend_pid,
            backend_started_at,
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
    pub fn backend_pid(&self) -> i32 {
        self.backend_pid
    }
    pub fn backend_started_at(&self) -> DateTime<Utc> {
        self.backend_started_at
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
    !s.is_empty()
        && s.len() <= 20
        && s.bytes().all(|b| b.is_ascii_digit())
        && s.parse::<u64>().is_ok_and(|v| v > 0)
}

/// Frozen producer-owned settlement reference. It contains no credentials/image
/// bytes and is never an external client input or a substitute for system auth.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Reference {
    version: u8,
    actor: uuid::Uuid,
    event: uuid::Uuid,
    family: uuid::Uuid,
    request: uuid::Uuid,
    operation: String,
    key: Option<uuid::Uuid>,
    guard_digest: [u8; 32],
    body_digest: [u8; 32],
}
/// Call from the request-specific canonical guard adapter, then durably Track
/// before returning authorize_in success. This only builds a reference.
pub fn settlement_reference(
    binding: &super::OwnerMediaBinding,
    guard_digest: [u8; 32],
) -> Result<Vec<u8>, CommandError> {
    binding.check_current()?;
    if guard_digest == [0; 32] {
        return Err(CommandError::InvalidInput);
    }
    serde_json::to_vec(&Reference {
        version: 1,
        actor: binding.identity.actor,
        event: binding.identity.event,
        family: binding.identity.session_family,
        request: binding.identity.request_id,
        operation: binding.operation.into(),
        key: binding.key,
        guard_digest,
        body_digest: binding.body_sha256,
    })
    .map_err(|_| CommandError::InvalidState)
}
/// Actual configured Auth/Friend system caller must be authenticated by root
/// before constructing this DTO. No browser actor/expired parent grants access.
/// Root recomputes guard_digest using canonical GuardBinding::digest and resolves
/// registration_id only from the authenticated owner's durable sealed registration.
pub struct MediaResolveBinding {
    pub actor: uuid::Uuid,
    pub event: uuid::Uuid,
    pub session_family: uuid::Uuid,
    pub request_id: uuid::Uuid,
    pub operation: String,
    pub guard_digest: [u8; 32],
    pub body_digest: [u8; 32],
    pub registration_id: uuid::Uuid,
}
/// Root maps these exact fields to authority.lease.v1.TransactionIdentity.
pub struct MediaResolveTransaction {
    pub system_identifier: String,
    pub database_oid: i64,
    pub full_xid: String,
    pub backend_pid: i32,
    pub backend_started_unix_micros: i64,
    pub producer_instance_id: uuid::Uuid,
    pub settlement_reference: Vec<u8>,
    pub effect_binding_digest: [u8; 32],
}
pub struct ResolvedMediaTransaction {
    pub state: MediaTransactionState,
    pub binding_digest: [u8; 32],
    /// Bounded safe receipt evidence, never a raw command result or image.
    pub receipt_evidence: Vec<u8>,
}
/// Producer implementation behind the frozen INTERNAL ResolveTransaction RPC.
/// Caller supplies a fresh bounded system recovery budget; never reset/reuse an
/// expired original browser budget to mint user authority. Caller must retain
/// owner quarantine and seal the producer scope before considering release.
pub async fn resolve_transaction(
    pool: &PgPool,
    configured_instance: uuid::Uuid,
    binding: &MediaResolveBinding,
    transaction: &MediaResolveTransaction,
) -> Result<ResolvedMediaTransaction, CommandError> {
    if configured_instance.is_nil()
        || configured_instance != transaction.producer_instance_id
        || [
            binding.actor,
            binding.event,
            binding.session_family,
            binding.request_id,
            binding.registration_id,
        ]
        .iter()
        .any(uuid::Uuid::is_nil)
        || binding.guard_digest == [0; 32]
        || binding.guard_digest != transaction.effect_binding_digest
        || !decimal(&transaction.system_identifier)
        || !decimal(&transaction.full_xid)
        || !(1..=u32::MAX as i64).contains(&transaction.database_oid)
        || transaction.backend_pid <= 0
        || transaction.backend_started_unix_micros <= 0
        || transaction.settlement_reference.len() > 2048
    {
        return Err(CommandError::InvalidInput);
    }
    let reference: Reference = serde_json::from_slice(&transaction.settlement_reference)
        .map_err(|_| CommandError::InvalidInput)?;
    if reference.version != 1
        || (
            reference.actor,
            reference.event,
            reference.family,
            reference.request,
            reference.operation.as_str(),
            reference.guard_digest,
            reference.body_digest,
        ) != (
            binding.actor,
            binding.event,
            binding.session_family,
            binding.request_id,
            binding.operation.as_str(),
            binding.guard_digest,
            binding.body_digest,
        )
    {
        return Err(CommandError::Denied);
    }
    if !matches!(
        reference.operation.as_str(),
        "initConcertUpload"
            | "putConcertUploadBytes"
            | "finalizeConcertUpload"
            | "getConcertMedia"
            | "readConcertMedia"
            | "getConcertCommandStatus"
    ) {
        return Err(CommandError::InvalidInput);
    }
    let mutation = matches!(
        reference.operation.as_str(),
        "initConcertUpload" | "putConcertUploadBytes" | "finalizeConcertUpload"
    );
    if mutation != reference.key.is_some() || reference.key.is_some_and(|k| k.is_nil()) {
        return Err(CommandError::InvalidInput);
    }
    let identity = MediaTransactionIdentity {
        system_identifier: transaction.system_identifier.clone(),
        database_oid: transaction.database_oid,
        xid: transaction.full_xid.clone(),
        backend_pid: transaction.backend_pid,
        backend_started_at: chrono::DateTime::from_timestamp_micros(
            transaction.backend_started_unix_micros,
        )
        .ok_or(CommandError::InvalidInput)?,
    };
    let state = transaction_state(pool, &identity).await;
    let mut evidence = Vec::new();
    if state == MediaTransactionState::Committed {
        if let Some(key) = reference.key {
            if !matches!(
                reference.operation.as_str(),
                "initConcertUpload" | "putConcertUploadBytes" | "finalizeConcertUpload"
            ) || key.is_nil()
            {
                return Err(CommandError::InvalidInput);
            }
            // A missing receipt does not imply rollback. Only pg_xact_status is
            // the transaction outcome; this is safe optional correlation evidence.
            let row:Option<(uuid::Uuid,String,Option<uuid::Uuid>,Option<i64>)>=sqlx::query_as("select receipt_id,state,resource_id,resource_revision from public.concert_command_receipts where actor_id=$1 and event_id=$2 and operation_id=$3 and idempotency_key=$4")
                .bind(reference.actor).bind(reference.event).bind(&reference.operation).bind(key).fetch_optional(pool).await.map_err(|_|CommandError::Unavailable)?;
            if let Some((receipt, state, resource, revision)) = row {
                evidence=serde_json::to_vec(&serde_json::json!({"version":1,"receipt_id":receipt,"actor_id":reference.actor,"event_id":reference.event,"operation_id":reference.operation,"idempotency_key":key,"state":state,"resource_id":resource,"resource_revision":revision})).map_err(|_|CommandError::InvalidState)?;
            }
        }
    }
    Ok(ResolvedMediaTransaction {
        state,
        binding_digest: binding.guard_digest,
        receipt_evidence: evidence,
    })
}
