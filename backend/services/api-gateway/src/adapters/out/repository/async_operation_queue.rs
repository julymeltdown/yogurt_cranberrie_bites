//! Encrypted, PostgreSQL-backed command queue primitives for Gateway mutations.
//!
//! The request journal owns the actor, idempotency key, and operation identity.
//! This table holds only the encrypted request body plus replay metadata needed
//! to reconstruct the exact allowlisted HTTP command after a process restart.

use crate::application::dto::queue_repository_dto::*;
pub use crate::application::dto::queue_repository_dto::{
    ClaimedQueueOperationDto as ClaimedOperation, EncryptedQueuePayloadDto as EncryptedPayload,
    QueueEnqueueRequestDto as EnqueueRequest,
};
use crate::application::ports::{
    AsyncOperationQueueRepositoryPort, QueueClaimConnectionPort, QueuePayloadPort,
};
use crate::application::queue_operation_error::{QueueOperationError, QueueOperationResult};
use crate::config::{
    DatabasePoolCaller, DatabasePoolRole, GatewayMetrics, ObservedDatabaseConnection,
};
use anyhow::{bail, ensure, Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
use ring::rand::{SecureRandom, SystemRandom};
use sqlx::{Connection, PgConnection, PgPool, Row};
use std::collections::BTreeMap;
use std::time::Duration;
use uuid::Uuid;

const NONCE_LENGTH: usize = 12;
const TAG_LENGTH: usize = 16;
const MAX_REQUEST_BODY_BYTES: usize = 2 * 1024 * 1024;
const MAX_LEASE_SECONDS: u64 = 300;

/// A versioned keyring so deployments can retain old keys while queued payloads
/// created under them are still eligible for retry.
#[derive(Clone)]
pub struct PayloadCipher {
    keys: BTreeMap<i16, [u8; 32]>,
    active_key_id: i16,
}

impl PayloadCipher {
    /// Parse a comma-separated `key-id:64-hex-character-key` keyring.
    ///
    /// The largest key id is used for encryption. Older entries remain usable
    /// for decryption during key rotation and may be removed only after their
    /// queued jobs have drained or been dead-lettered.
    pub fn from_hex_keyring(value: &str) -> Result<Self> {
        if value.trim().is_empty() {
            bail!("Gateway async operation encryption keyring is empty");
        }
        let mut keys = BTreeMap::new();
        for entry in value.split(',') {
            let (raw_id, raw_key) = entry
                .trim()
                .split_once(':')
                .context("async operation key entries must use key-id:hex-key format")?;
            let key_id: i16 = raw_id
                .parse()
                .context("async operation key id must be a positive integer")?;
            if key_id <= 0 || raw_key.len() != 64 || !raw_key.is_ascii() {
                bail!(
                    "async operation key ids must be positive and keys must be 32-byte hex values"
                );
            }
            let mut key = [0_u8; 32];
            for (index, byte) in key.iter_mut().enumerate() {
                let offset = index * 2;
                *byte = u8::from_str_radix(&raw_key[offset..offset + 2], 16)
                    .context("async operation encryption key contains invalid hex")?;
            }
            if keys.insert(key_id, key).is_some() {
                bail!("async operation keyring contains a duplicate key id");
            }
        }
        let active_key_id = *keys
            .keys()
            .next_back()
            .context("Gateway async operation encryption keyring is empty")?;
        Ok(Self {
            keys,
            active_key_id,
        })
    }

    /// Encrypt a request body with AES-256-GCM and bind it to journal identity.
    pub fn encrypt(
        &self,
        operation_id: Uuid,
        actor_id: &str,
        request_fingerprint: &str,
        plaintext: &[u8],
    ) -> Result<EncryptedPayload> {
        if plaintext.len() > MAX_REQUEST_BODY_BYTES {
            bail!("async operation request body exceeds 2 MiB");
        }
        let key_bytes = self
            .keys
            .get(&self.active_key_id)
            .context("active async operation encryption key is unavailable")?;
        let key = aead_key(key_bytes)?;
        let mut nonce_bytes = [0_u8; NONCE_LENGTH];
        SystemRandom::new()
            .fill(&mut nonce_bytes)
            .map_err(|_| anyhow::anyhow!("secure random source unavailable"))?;
        let aad = associated_data(
            self.active_key_id,
            operation_id,
            actor_id,
            request_fingerprint,
        );
        let mut ciphertext = plaintext.to_vec();
        key.seal_in_place_append_tag(
            Nonce::assume_unique_for_key(nonce_bytes),
            Aad::from(aad.as_slice()),
            &mut ciphertext,
        )
        .map_err(|_| anyhow::anyhow!("async operation payload encryption failed"))?;
        Ok(EncryptedPayload {
            key_id: self.active_key_id,
            nonce: nonce_bytes,
            ciphertext,
        })
    }

    /// Decrypt a queued body only when its key id and journal identity match.
    pub fn decrypt(
        &self,
        operation_id: Uuid,
        actor_id: &str,
        request_fingerprint: &str,
        payload: &EncryptedPayload,
    ) -> Result<Vec<u8>> {
        if payload.ciphertext.len() > MAX_REQUEST_BODY_BYTES + TAG_LENGTH {
            bail!("async operation ciphertext exceeds the supported limit");
        }
        let key_bytes = self
            .keys
            .get(&payload.key_id)
            .context("async operation payload uses a key absent from the configured keyring")?;
        let key = aead_key(key_bytes)?;
        let aad = associated_data(payload.key_id, operation_id, actor_id, request_fingerprint);
        let mut plaintext = payload.ciphertext.clone();
        let opened = key
            .open_in_place(
                Nonce::assume_unique_for_key(payload.nonce),
                Aad::from(aad.as_slice()),
                &mut plaintext,
            )
            .map_err(|_| anyhow::anyhow!("async operation payload authentication failed"))?;
        Ok(opened.to_vec())
    }
}

fn aead_key(key: &[u8; 32]) -> Result<LessSafeKey> {
    let unbound = UnboundKey::new(&AES_256_GCM, key)
        .map_err(|_| anyhow::anyhow!("invalid async operation encryption key"))?;
    Ok(LessSafeKey::new(unbound))
}

fn associated_data(
    key_id: i16,
    operation_id: Uuid,
    actor_id: &str,
    request_fingerprint: &str,
) -> Vec<u8> {
    let mut aad = b"gateway-async-operation-v1\0".to_vec();
    aad.extend_from_slice(&key_id.to_be_bytes());
    aad.extend_from_slice(operation_id.as_bytes());
    aad.extend_from_slice(&(actor_id.len() as u32).to_be_bytes());
    aad.extend_from_slice(actor_id.as_bytes());
    aad.extend_from_slice(&(request_fingerprint.len() as u32).to_be_bytes());
    aad.extend_from_slice(request_fingerprint.as_bytes());
    aad
}

/// Persist a command only when its authenticated idempotent journal intent exists.
///
/// A crash between this insert and the HTTP response is safe: after restart, the
/// durable queue row is sufficient to reconstruct the owner-deduplicated command.
pub async fn enqueue(
    pool: &PgPool,
    cipher: &PayloadCipher,
    request: EnqueueRequest<'_>,
) -> Result<bool> {
    let origin_traceparent =
        platform::trace_context::current().map(|context| context.traceparent());
    let payload = cipher.encrypt(
        request.operation_id,
        request.actor_id,
        request.request_fingerprint,
        request.body,
    )?;
    enqueue_encrypted(
        pool,
        QueueEnqueueCommandDto {
            operation_id: request.operation_id,
            actor_id: request.actor_id,
            request_fingerprint: request.request_fingerprint,
            request_uri: request.request_uri,
            content_type: request.content_type,
            payload,
            origin_traceparent,
        },
    )
    .await
}
async fn enqueue_encrypted(pool: &PgPool, request: QueueEnqueueCommandDto<'_>) -> Result<bool> {
    let payload = &request.payload;
    let origin_traceparent = request.origin_traceparent.as_deref();
    let mut tx = pool.begin().await?;
    sqlx::query("select set_config('lock_timeout', '500ms', true), set_config('statement_timeout', '2s', true)")
        .execute(&mut *tx)
        .await?;
    let inserted = sqlx::query_scalar::<_, Uuid>(
        r#"
        with eligible as (
            select journal.id
            from gateway_request_journal journal
            where journal.id = $1 and journal.actor_id = $2
              and journal.request_fingerprint = $3
              and journal.idempotency_key is not null and journal.state = 'IN_PROGRESS'
              and not exists (
                  select 1 from gateway_operation_manual_reconciliations resolution
                  where resolution.operation_id = journal.id
              )
            for update
        ), queued as (
            update gateway_request_journal journal
            set state = 'QUEUED', outcome = 'PENDING',
                outcome_evidence = 'QUEUE_ACCEPTED'
            from eligible
            where journal.id = eligible.id
            returning journal.id
        )
        insert into gateway_async_operation_queue (
            operation_id, request_uri, content_type, encryption_key_id,
            payload_nonce, payload_ciphertext, origin_traceparent, available_at
        )
        select queued.id, $4, $5, $6, $7, $8, $9, now() from queued
        on conflict (operation_id) do nothing
        returning operation_id
        "#,
    )
    .bind(request.operation_id)
    .bind(request.actor_id)
    .bind(request.request_fingerprint)
    .bind(request.request_uri)
    .bind(request.content_type)
    .bind(payload.key_id)
    .bind(payload.nonce.as_slice())
    .bind(&payload.ciphertext)
    .bind(origin_traceparent)
    .fetch_optional(&mut *tx)
    .await?;
    if inserted.is_none() {
        tx.rollback().await?;
        return Ok(false);
    }
    tx.commit().await?;
    Ok(true)
}

fn validate_lease_duration(lease_duration: Duration) -> Result<i32> {
    if lease_duration.as_secs() == 0 || lease_duration.as_secs() > MAX_LEASE_SECONDS {
        bail!("async operation lease must be between 1 and 300 seconds");
    }
    i32::try_from(lease_duration.as_secs())
        .context("async operation lease is outside the supported range")
}

/// Atomically lease one due command, reclaiming expired leases after process loss.
pub async fn claim_next(
    pool: &PgPool,
    worker_id: Uuid,
    lease_duration: Duration,
) -> Result<Option<ClaimedOperation>> {
    validate_lease_duration(lease_duration)?;
    let mut connection = pool.acquire().await?;
    claim_next_on_connection(&mut connection, worker_id, lease_duration).await
}

/// The caller can observe acquisition separately from the bounded claim query.
/// The transaction still commits before any owner/network work starts.
pub async fn claim_next_on_connection(
    connection: &mut PgConnection,
    worker_id: Uuid,
    lease_duration: Duration,
) -> Result<Option<ClaimedOperation>> {
    let lease_seconds = validate_lease_duration(lease_duration)?;
    let mut tx = connection.begin().await?;
    sqlx::query("select set_config('lock_timeout', '500ms', true), set_config('statement_timeout', '2s', true)")
        .execute(&mut *tx)
        .await?;
    let row = sqlx::query(
        r#"
        with candidate as (
            select queue.operation_id
            from gateway_async_operation_queue queue
            join gateway_request_journal journal on journal.id = queue.operation_id
            where (
                (queue.state = 'READY' and queue.available_at <= now())
                or (queue.state = 'LEASED' and queue.lease_expires_at <= now())
            )
              and journal.state = 'QUEUED'
              and not exists (
                  select 1
                  from gateway_operation_manual_reconciliations current_resolution
                  where current_resolution.operation_id = journal.id
                    and current_resolution.revision = (
                        select max(latest.revision)
                        from gateway_operation_manual_reconciliations latest
                        where latest.operation_id = journal.id
                    )
              )
            order by queue.available_at, queue.operation_id
            for update of queue skip locked
            limit 1
        )
        update gateway_async_operation_queue queue
        set state = 'LEASED', lease_owner = $1,
            lease_expires_at = now() + make_interval(secs => $2::int),
            attempt_count = queue.attempt_count + 1, updated_at = now()
        from candidate, gateway_request_journal journal
        where queue.operation_id = candidate.operation_id
          and journal.id = queue.operation_id
        returning journal.id, journal.actor_id, journal.action, journal.resource_path,
                  journal.request_id, journal.ip_address, journal.user_agent,
                  journal.idempotency_key, journal.request_fingerprint,
                  queue.request_uri, queue.content_type, queue.origin_traceparent,
                  queue.encryption_key_id,
                  queue.payload_nonce, queue.payload_ciphertext,
                  queue.attempt_count
        "#,
    )
    .bind(worker_id)
    .bind(lease_seconds)
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    row.map(|row| {
        let nonce: Vec<u8> = row.try_get("payload_nonce")?;
        let nonce: [u8; NONCE_LENGTH] = nonce
            .try_into()
            .map_err(|_| anyhow::anyhow!("invalid async operation nonce length"))?;
        let ciphertext: Vec<u8> = row.try_get("payload_ciphertext")?;
        let attempts: i32 = row.try_get("attempt_count")?;
        Ok(ClaimedOperation {
            operation_id: row.try_get("id")?,
            actor_id: row.try_get("actor_id")?,
            action: row.try_get("action")?,
            resource_path: row.try_get("resource_path")?,
            request_id: row.try_get("request_id")?,
            ip_address: row.try_get("ip_address")?,
            user_agent: row.try_get("user_agent")?,
            idempotency_key: row.try_get("idempotency_key")?,
            request_fingerprint: row.try_get("request_fingerprint")?,
            request_uri: row.try_get("request_uri")?,
            content_type: row.try_get("content_type")?,
            origin_traceparent: row.try_get("origin_traceparent")?,
            lease_owner: worker_id,
            attempt_count: u32::try_from(attempts)
                .map_err(|_| anyhow::anyhow!("invalid async operation attempt count"))?,
            payload: EncryptedPayload {
                key_id: row.try_get("encryption_key_id")?,
                nonce,
                ciphertext,
            },
        })
    })
    .transpose()
}

/// Mark a completed command and erase its encrypted request bytes.
pub async fn mark_done(pool: &PgPool, operation_id: Uuid, worker_id: Uuid) -> Result<bool> {
    let affected = sqlx::query(
        r#"
        update gateway_async_operation_queue
        set state = 'DONE', payload_nonce = null, payload_ciphertext = null,
            origin_traceparent = null, lease_owner = null, lease_expires_at = null,
            last_error_code = null,
            updated_at = now()
        where operation_id = $1 and state = 'LEASED' and lease_owner = $2
        "#,
    )
    .bind(operation_id)
    .bind(worker_id)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(affected == 1)
}

/// Retry a command later, retaining ciphertext but relinquishing this lease.
pub async fn retry_later(
    pool: &PgPool,
    operation_id: Uuid,
    worker_id: Uuid,
    available_at: DateTime<Utc>,
    error_code: &str,
) -> Result<bool> {
    if error_code.is_empty()
        || error_code.len() > 64
        || !error_code
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
    {
        bail!("async operation error code must be a bounded uppercase identifier");
    }
    let affected = sqlx::query(
        r#"
        update gateway_async_operation_queue
        set state = 'READY', available_at = $3, lease_owner = null,
            lease_expires_at = null, last_error_code = $4, updated_at = now()
        where operation_id = $1 and state = 'LEASED' and lease_owner = $2
        "#,
    )
    .bind(operation_id)
    .bind(worker_id)
    .bind(available_at)
    .bind(error_code)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(affected == 1)
}

/// Stop automatic retries and erase command bytes for operator reconciliation.
pub async fn dead_letter(
    pool: &PgPool,
    operation_id: Uuid,
    worker_id: Uuid,
    error_code: &str,
) -> Result<bool> {
    if error_code.is_empty()
        || error_code.len() > 64
        || !error_code
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
    {
        bail!("async operation error code must be a bounded uppercase identifier");
    }
    let affected = sqlx::query(
        r#"
        update gateway_async_operation_queue
        set state = 'DEAD', payload_nonce = null, payload_ciphertext = null,
            origin_traceparent = null, lease_owner = null, lease_expires_at = null,
            last_error_code = $3,
            updated_at = now()
        where operation_id = $1 and state = 'LEASED' and lease_owner = $2
        "#,
    )
    .bind(operation_id)
    .bind(worker_id)
    .bind(error_code)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(affected == 1)
}

/// Complete a leased operation and its journal outcome in one transaction.
/// A terminal Gateway response erases the encrypted command bytes.
pub async fn complete_claim(
    pool: &PgPool,
    operation: &ClaimedOperation,
    response_status: u16,
    outcome: crate::domain::operation::OperationOutcomeDecision,
    content_type: Option<&str>,
    response_body: Option<&[u8]>,
) -> Result<bool> {
    complete_claim_with_lease(
        pool,
        &operation.lease_identity(),
        response_status,
        outcome,
        content_type,
        response_body,
    )
    .await
}

async fn complete_claim_with_lease(
    pool: &PgPool,
    operation: &QueueLeaseIdentityDto,
    response_status: u16,
    outcome: crate::domain::operation::OperationOutcomeDecision,
    content_type: Option<&str>,
    response_body: Option<&[u8]>,
) -> Result<bool> {
    if !(100..500).contains(&response_status) {
        bail!("only a known non-5xx outcome can complete an async operation");
    }
    use crate::domain::operation::{OperationOutcome, OperationOutcomeEvidence};
    match outcome.outcome {
        OperationOutcome::Committed => {
            ensure!(
                (200..300).contains(&response_status) && response_status != 202,
                "a committed outcome requires a non-202 success response"
            );
        }
        OperationOutcome::RejectedBeforeCommit => {
            ensure!(
                !(200..300).contains(&response_status)
                    && outcome.evidence == OperationOutcomeEvidence::PreDispatchRejection,
                "pre-commit rejection requires explicit no-dispatch evidence"
            );
        }
        OperationOutcome::Unknown => {}
        OperationOutcome::Pending => bail!("a terminal queue result cannot remain pending"),
    }
    if response_body.is_some_and(|body| body.len() > MAX_REQUEST_BODY_BYTES) {
        bail!("async operation response body exceeds 2 MiB");
    }
    let mut tx = pool.begin().await?;
    sqlx::query("select set_config('lock_timeout', '500ms', true), set_config('statement_timeout', '2s', true)")
        .execute(&mut *tx)
        .await?;
    let journal = sqlx::query(
        r#"
        update gateway_request_journal
        set state = case when $3 = 'UNKNOWN' then 'UNKNOWN' else 'COMPLETED' end,
            outcome = $3, outcome_evidence = $4,
            response_status = $2,
            response_content_type = $5, response_body = $6, finished_at = now()
        where id = $1 and state <> 'COMPLETED'
        returning id, actor_id, action, resource_path, request_id, ip_address, user_agent
        "#,
    )
    .bind(operation.operation_id)
    .bind(i32::from(response_status))
    .bind(outcome.outcome.as_str())
    .bind(outcome.evidence.as_str())
    .bind(content_type)
    .bind(response_body)
    .fetch_optional(&mut *tx)
    .await?;
    let affected = sqlx::query(
        r#"
        update gateway_async_operation_queue
        set state = 'DONE', payload_nonce = null, payload_ciphertext = null,
            origin_traceparent = null, lease_owner = null, lease_expires_at = null,
            last_error_code = null,
            updated_at = now()
        where operation_id = $1 and state = 'LEASED' and lease_owner = $2
        "#,
    )
    .bind(operation.operation_id)
    .bind(operation.lease_owner)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if affected != 1 {
        tx.rollback().await?;
        return Ok(false);
    }
    if let Some(journal) = journal {
        sqlx::query(
            r#"
            insert into admin_audit_events (
                id, actor_id, action, resource_type, resource_id, outcome,
                request_id, ip_address, user_agent, metadata_json, occurred_at
            ) values (
                $1, $2, 'GATEWAY_ASYNC_OPERATION_COMPLETED',
                'gateway_request_journal', $3, $4, $5, $6, $7,
                jsonb_build_object('responseStatus', $8)::text, now()
            )
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(journal.try_get::<Option<String>, _>("actor_id")?)
        .bind(operation.operation_id.to_string())
        .bind(outcome.outcome.audit_result())
        .bind(journal.try_get::<Option<String>, _>("request_id")?)
        .bind(journal.try_get::<Option<String>, _>("ip_address")?)
        .bind(journal.try_get::<Option<String>, _>("user_agent")?)
        .bind(i32::from(response_status))
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(true)
}

/// Dead-letter a command after retry exhaustion and expose UNKNOWN atomically.
pub async fn dead_letter_claim(
    pool: &PgPool,
    operation: &ClaimedOperation,
    error_code: &str,
    response_status: u16,
) -> Result<bool> {
    dead_letter_claim_with_lease(
        pool,
        &operation.lease_identity(),
        error_code,
        response_status,
    )
    .await
}

async fn dead_letter_claim_with_lease(
    pool: &PgPool,
    operation: &QueueLeaseIdentityDto,
    error_code: &str,
    response_status: u16,
) -> Result<bool> {
    if response_status < 500 {
        bail!("dead-letter outcomes must remain ambiguous server failures");
    }
    if error_code.is_empty()
        || error_code.len() > 64
        || !error_code
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
    {
        bail!("async operation error code must be a bounded uppercase identifier");
    }
    let mut tx = pool.begin().await?;
    sqlx::query("select set_config('lock_timeout', '500ms', true), set_config('statement_timeout', '2s', true)")
        .execute(&mut *tx)
        .await?;
    let affected = sqlx::query(
        r#"
        update gateway_async_operation_queue
        set state = 'DEAD', payload_nonce = null, payload_ciphertext = null,
            origin_traceparent = null, lease_owner = null, lease_expires_at = null,
            last_error_code = $3,
            updated_at = now()
        where operation_id = $1 and state = 'LEASED' and lease_owner = $2
        "#,
    )
    .bind(operation.operation_id)
    .bind(operation.lease_owner)
    .bind(error_code)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if affected != 1 {
        tx.rollback().await?;
        return Ok(false);
    }
    let journal = sqlx::query(
        r#"
        update gateway_request_journal
        set state = 'UNKNOWN', outcome = 'UNKNOWN',
            outcome_evidence = 'RESPONSE_STATUS_ONLY', response_status = $2,
            response_content_type = null, response_body = null, finished_at = now()
        where id = $1 and state <> 'COMPLETED'
        returning id, actor_id, request_id, ip_address, user_agent
        "#,
    )
    .bind(operation.operation_id)
    .bind(i32::from(response_status))
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(journal) = journal {
        sqlx::query(
            r#"
            insert into admin_audit_events (
                id, actor_id, action, resource_type, resource_id, outcome,
                request_id, ip_address, user_agent, metadata_json, occurred_at
            ) values (
                $1, $2, 'GATEWAY_ASYNC_OPERATION_DEAD_LETTERED',
                'gateway_request_journal', $3, 'UNKNOWN', $4, $5, $6,
                jsonb_build_object('errorCode', $7, 'responseStatus', $8)::text, now()
            )
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(journal.try_get::<Option<String>, _>("actor_id")?)
        .bind(operation.operation_id.to_string())
        .bind(journal.try_get::<Option<String>, _>("request_id")?)
        .bind(journal.try_get::<Option<String>, _>("ip_address")?)
        .bind(journal.try_get::<Option<String>, _>("user_agent")?)
        .bind(error_code)
        .bind(i32::from(response_status))
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(true)
}

fn classify_queue_error(error: anyhow::Error) -> QueueOperationError {
    if error.chain().any(|cause| cause.is::<sqlx::Error>()) {
        QueueOperationError::Storage
    } else {
        QueueOperationError::Rejected
    }
}
impl QueuePayloadPort for PayloadCipher {
    fn encrypt(
        &self,
        command: QueuePayloadEncryptDto<'_>,
    ) -> QueueOperationResult<EncryptedQueuePayloadDto> {
        PayloadCipher::encrypt(
            self,
            command.operation_id,
            command.actor_id,
            command.request_fingerprint,
            command.plaintext,
        )
        .map_err(|_| QueueOperationError::PayloadUnavailable)
    }
    fn decrypt(
        &self,
        query: QueuePayloadDecryptDto<'_>,
    ) -> QueueOperationResult<QueuePlaintextPayloadDto> {
        PayloadCipher::decrypt(
            self,
            query.operation_id,
            query.actor_id,
            query.request_fingerprint,
            query.payload,
        )
        .map(|body| QueuePlaintextPayloadDto { body })
        .map_err(|_| QueueOperationError::PayloadUnavailable)
    }
}
#[derive(Clone)]
pub struct PgAsyncOperationQueueRepository {
    pool: PgPool,
    metrics: Option<GatewayMetrics>,
}
impl PgAsyncOperationQueueRepository {
    pub fn new(pool: PgPool, metrics: Option<GatewayMetrics>) -> Self {
        Self { pool, metrics }
    }
}
enum QueueConnection {
    Observed(ObservedDatabaseConnection),
    Plain(sqlx::pool::PoolConnection<sqlx::Postgres>),
}
struct PgQueueClaimConnection {
    held: QueueConnection,
}
impl PgQueueClaimConnection {
    fn connection(&mut self) -> &mut PgConnection {
        match &mut self.held {
            QueueConnection::Observed(c) => c,
            QueueConnection::Plain(c) => c,
        }
    }
}
#[async_trait]
impl QueueClaimConnectionPort for PgQueueClaimConnection {
    async fn claim_next(
        &mut self,
        query: QueueClaimQueryDto,
    ) -> QueueOperationResult<Option<ClaimedQueueOperationDto>> {
        claim_next_on_connection(self.connection(), query.worker_id, query.lease_duration)
            .await
            .map_err(classify_queue_error)
    }
}
#[async_trait]
impl AsyncOperationQueueRepositoryPort for PgAsyncOperationQueueRepository {
    async fn enqueue(&self, command: QueueEnqueueCommandDto<'_>) -> QueueOperationResult<bool> {
        enqueue_encrypted(&self.pool, command)
            .await
            .map_err(classify_queue_error)
    }
    async fn acquire_claim_connection(
        &self,
    ) -> QueueOperationResult<Box<dyn QueueClaimConnectionPort>> {
        let held = match &self.metrics {
            Some(metrics) => QueueConnection::Observed(
                metrics
                    .acquire_database_connection(
                        DatabasePoolRole::Background,
                        DatabasePoolCaller::WorkerClaim,
                        &self.pool,
                    )
                    .await
                    .map_err(|e| classify_queue_error(e.into()))?,
            ),
            None => QueueConnection::Plain(
                self.pool
                    .acquire()
                    .await
                    .map_err(|e| classify_queue_error(e.into()))?,
            ),
        };
        Ok(Box::new(PgQueueClaimConnection { held }))
    }
    async fn claim_next(
        &self,
        query: QueueClaimQueryDto,
    ) -> QueueOperationResult<Option<ClaimedQueueOperationDto>> {
        claim_next(&self.pool, query.worker_id, query.lease_duration)
            .await
            .map_err(classify_queue_error)
    }
    async fn mark_done(&self, lease: QueueLeaseIdentityDto) -> QueueOperationResult<bool> {
        mark_done(&self.pool, lease.operation_id, lease.lease_owner)
            .await
            .map_err(classify_queue_error)
    }
    async fn retry_later(&self, command: QueueRetryCommandDto<'_>) -> QueueOperationResult<bool> {
        retry_later(
            &self.pool,
            command.lease.operation_id,
            command.lease.lease_owner,
            command.available_at,
            command.error_code,
        )
        .await
        .map_err(classify_queue_error)
    }
    async fn dead_letter(&self, command: QueueDeadCommandDto<'_>) -> QueueOperationResult<bool> {
        dead_letter(
            &self.pool,
            command.lease.operation_id,
            command.lease.lease_owner,
            command.error_code,
        )
        .await
        .map_err(classify_queue_error)
    }
    async fn complete_claim(
        &self,
        command: QueueCompleteCommandDto<'_>,
    ) -> QueueOperationResult<bool> {
        let outcome = crate::domain::operation::OperationOutcomeDecision {
            outcome: command.outcome.into(),
            evidence: command.evidence.into(),
        };
        complete_claim_with_lease(
            &self.pool,
            &command.lease,
            command.response_status,
            outcome,
            command.content_type,
            command.response_body,
        )
        .await
        .map_err(classify_queue_error)
    }
    async fn dead_letter_claim(
        &self,
        command: QueueDeadClaimCommandDto<'_>,
    ) -> QueueOperationResult<bool> {
        dead_letter_claim_with_lease(
            &self.pool,
            &command.lease,
            command.error_code,
            command.response_status,
        )
        .await
        .map_err(classify_queue_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keyring() -> String {
        format!("1:{},2:{}", "01".repeat(32), "02".repeat(32))
    }

    #[test]
    fn encrypted_payload_is_bound_to_journal_identity_and_rotation_key_id() {
        let cipher = PayloadCipher::from_hex_keyring(&keyring()).expect("keyring");
        let operation_id = Uuid::new_v4();
        let body = br#"{"content":"hello"}"#;
        let encrypted = cipher
            .encrypt(operation_id, "actor-1", "fingerprint-1", body)
            .expect("encrypt");
        assert_eq!(encrypted.key_id, 2);
        assert_ne!(encrypted.ciphertext, body);
        assert_eq!(
            cipher
                .decrypt(operation_id, "actor-1", "fingerprint-1", &encrypted)
                .expect("decrypt"),
            body
        );
        assert!(cipher
            .decrypt(operation_id, "actor-2", "fingerprint-1", &encrypted)
            .is_err());
        assert!(cipher
            .decrypt(operation_id, "actor-1", "fingerprint-2", &encrypted)
            .is_err());

        let rotated = PayloadCipher::from_hex_keyring(&keyring()).expect("retained keyring");
        assert_eq!(
            rotated
                .decrypt(operation_id, "actor-1", "fingerprint-1", &encrypted)
                .expect("decrypt with retained key"),
            body
        );
    }

    #[test]
    fn keyring_rejects_duplicate_ids_invalid_hex_and_non_32_byte_keys() {
        assert!(PayloadCipher::from_hex_keyring("").is_err());
        assert!(PayloadCipher::from_hex_keyring(&format!(
            "1:{},1:{}",
            "00".repeat(32),
            "11".repeat(32)
        ))
        .is_err());
        assert!(PayloadCipher::from_hex_keyring("1:xyz").is_err());
        assert!(PayloadCipher::from_hex_keyring(&format!("1:{}", "é".repeat(32))).is_err());
        assert!(PayloadCipher::from_hex_keyring(&format!("0:{}", "00".repeat(32))).is_err());
    }
}
