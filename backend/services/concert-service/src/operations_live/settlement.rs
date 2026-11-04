//! Transaction outcome evidence, never a claim that a command succeeded.
use super::{authority_error, unavailable};
use crate::{
    adapters::out::relationship_authority::guarded::{
        proto, OwnerGuardScope, ProducerTransaction as GuardTransaction,
    },
    product_rpc::{
        scope_ledger::ProducerTransaction,
        settlement::{
            NativeTransactionResolution, NativeTransactionResolver, PgNativeAbortResolver,
        },
    },
};
use concert_application::commands::CommandError;
use platform::actor_delegation::owner_binding::OwnerBinding;
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, PgPool, Row};
use tonic::Status;
use uuid::Uuid;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Reference {
    kind: String,
    operation: String,
    request_id: Uuid,
    scope_id: Uuid,
    request_digest: [u8; 32],
    idempotency_key: Option<Uuid>,
}
fn supported(operation: &str) -> bool {
    matches!(
        operation,
        "createConcertReport"
            | "listOwnConcertReports"
            | "listConcertAdminReports"
            | "getConcertAdminReport"
            | "getConcertSettings"
            | "setConcertSettings"
            | "setConcertEventPhase"
            | "listConcertAdminPlaces"
            | "setConcertPlace"
            | "listConcertNotices"
            | "getConcertNotice"
            | "listConcertAdminNotices"
            | "createConcertNotice"
            | "updateConcertNotice"
    )
}
pub(super) async fn register_in(
    scope: &OwnerGuardScope,
    c: &mut PgConnection,
    digest: [u8; 32],
    key: Option<Uuid>,
) -> Result<GuardTransaction, CommandError> {
    let producer = scope.producer_scope();
    let reference = Reference {
        kind: "OPERATIONS_NATIVE_V1".into(),
        operation: scope.binding().operation().into(),
        request_id: scope.binding().request_id(),
        scope_id: Uuid::parse_str(&producer.scope_id).map_err(unavailable)?,
        request_digest: digest,
        idempotency_key: key,
    };
    let identity = GuardTransaction::capture_concert_in(
        c,
        digest,
        Uuid::parse_str(&producer.instance_id).map_err(unavailable)?,
        serde_json::to_vec(&reference).map_err(unavailable)?,
    )
    .await
    .map_err(authority_error)?;
    scope
        .track_transaction(identity.clone())
        .await
        .map_err(authority_error)?;
    Ok(identity)
}

/// Called after the actual role/admission checks, before the retained business
/// producer continues on this same connection. A rollback removes this witness.
/// Deferred SQL checks bind any command receipt without requiring SUCCEEDED.
pub(super) async fn witness_in(
    scope: &OwnerGuardScope,
    c: &mut PgConnection,
    native: &GuardTransaction,
    key: Option<Uuid>,
) -> Result<(), CommandError> {
    let producer = scope.producer_scope();
    let scope_id = Uuid::parse_str(&producer.scope_id).map_err(unavailable)?;
    let identity = ProducerTransaction {
        producer: "CONCERT".into(),
        system_identifier: native.system_identifier.clone(),
        database_oid: native.database_oid,
        full_xid: native.full_xid.clone(),
        backend_pid: native.backend_pid,
        backend_started_unix_micros: native.backend_started.timestamp_micros(),
        effect_binding_digest: native.effect_binding_digest,
        producer_instance: native.producer_instance_id,
        settlement_reference: native.settlement_reference.clone(),
    };
    let identity_digest = identity.digest().map_err(authority_error)?;
    // Select the exact persisted parent, rather than constructing another guard.
    // The trigger joins its confirmed registration and validates native identity.
    sqlx::query("INSERT INTO public.concert_operations_native_witnesses(instance_id,scope_id,binding_digest,identity_digest,request_digest,idempotency_key,binding,identity,settlement_reference) SELECT s.instance_id,s.scope_id,s.binding_digest,$4,$5,$6,s.binding,$7,$8 FROM public.concert_product_owner_scopes s WHERE s.instance_id=$1 AND s.scope_id=$2 AND s.binding_digest=$3 ON CONFLICT(instance_id,scope_id,identity_digest) DO NOTHING")
        .bind(native.producer_instance_id).bind(scope_id).bind(scope.binding().digest().as_slice())
        .bind(identity_digest.as_slice()).bind(native.effect_binding_digest.as_slice()).bind(key)
        .bind(serde_json::to_value(&identity).map_err(unavailable)?).bind(&native.settlement_reference)
        .execute(&mut *c).await.map_err(unavailable)?;
    let matched: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM public.concert_operations_native_witnesses WHERE instance_id=$1 AND scope_id=$2 AND identity_digest=$3 AND binding_digest=$4 AND request_digest=$5 AND idempotency_key IS NOT DISTINCT FROM $6)")
        .bind(native.producer_instance_id).bind(scope_id).bind(identity_digest.as_slice())
        .bind(scope.binding().digest().as_slice()).bind(native.effect_binding_digest.as_slice()).bind(key)
        .fetch_one(c).await.map_err(unavailable)?;
    if !matched {
        return Err(CommandError::Unavailable);
    }
    Ok(())
}

fn unresolved(_: impl std::fmt::Debug) -> Status {
    Status::unavailable("Operations native settlement unresolved")
}
fn unknown() -> NativeTransactionResolution {
    NativeTransactionResolution {
        status: proto::TransactionStatus::Unknown,
        receipt_evidence: vec![],
    }
}
/// Compose behind Root's system-only settlement RPC with the same configured
/// instance as its ledger. It acquires no browser authority or owner scope.
pub struct OperationsNativeResolver {
    pool: PgPool,
    instance: Uuid,
    aborts: PgNativeAbortResolver,
}
impl OperationsNativeResolver {
    pub fn new(pool: PgPool, instance: Uuid) -> Result<Self, Status> {
        Ok(Self {
            aborts: PgNativeAbortResolver::new(pool.clone(), instance)?,
            pool,
            instance,
        })
    }
}
#[async_trait::async_trait]
impl NativeTransactionResolver for OperationsNativeResolver {
    async fn resolve(
        &self,
        binding: &OwnerBinding,
        identity: &ProducerTransaction,
    ) -> Result<NativeTransactionResolution, Status> {
        binding.validate()?;
        let digest = identity.digest()?;
        let native = self.aborts.resolve(binding, identity).await?;
        if native.status != proto::TransactionStatus::Unknown {
            return Ok(native);
        }
        if identity.producer != "CONCERT"
            || identity.producer_instance != self.instance
            || !supported(&binding.operation)
        {
            return Ok(unknown());
        }
        let Ok(reference) = serde_json::from_slice::<Reference>(&identity.settlement_reference)
        else {
            return Ok(unknown());
        };
        if reference.kind != "OPERATIONS_NATIVE_V1"
            || reference.scope_id.is_nil()
            || reference.operation != binding.operation
            || reference.request_id != binding.request_id
            || reference.request_digest != identity.effect_binding_digest
            || reference.request_digest == [0; 32]
            || reference.idempotency_key.is_some_and(|key| key.is_nil())
            || (binding.http_method == "GET") != reference.idempotency_key.is_none()
            || serde_json::to_vec(&reference).map_err(unresolved)? != identity.settlement_reference
        {
            return Ok(unknown());
        }
        let mut tx = self.pool.begin().await.map_err(unresolved)?;
        sqlx::query("SET LOCAL statement_timeout='750ms'")
            .execute(&mut *tx)
            .await
            .map_err(unresolved)?;
        sqlx::query("SET LOCAL lock_timeout='250ms'")
            .execute(&mut *tx)
            .await
            .map_err(unresolved)?;
        let actual: (String, i64) = sqlx::query_as("SELECT s.system_identifier::text,d.oid::bigint FROM pg_control_system() s CROSS JOIN pg_database d WHERE d.datname=current_database()")
            .fetch_one(&mut *tx).await.map_err(unresolved)?;
        if actual
            != (
                identity.system_identifier.clone(),
                i64::from(identity.database_oid),
            )
        {
            return Ok(unknown());
        }
        let status: Option<String> = sqlx::query_scalar("SELECT pg_xact_status($1::text::xid8)")
            .bind(&identity.full_xid)
            .fetch_one(&mut *tx)
            .await
            .map_err(unresolved)?;
        if status.as_deref() != Some("committed") {
            return Ok(unknown());
        }
        let row = sqlx::query("SELECT w.binding,w.identity,w.request_digest,w.idempotency_key,w.settlement_reference FROM public.concert_operations_native_witnesses w JOIN public.concert_product_owner_scopes s USING(instance_id,scope_id,binding_digest) JOIN public.concert_product_owner_transactions t USING(instance_id,scope_id,binding_digest,identity_digest) WHERE w.instance_id=$1 AND w.scope_id=$2 AND w.binding_digest=$3 AND w.identity_digest=$4 AND s.phase='ROOT_FINISHED' AND s.delivery_secret_hash IS NOT NULL AND t.confirmed_at IS NOT NULL AND w.binding=s.binding AND w.identity=t.identity")
            .bind(self.instance).bind(reference.scope_id).bind(binding.digest()?.as_slice()).bind(digest.as_slice())
            .fetch_optional(&mut *tx).await.map_err(unresolved)?;
        let Some(row) = row else { return Ok(unknown()) };
        if row
            .try_get::<serde_json::Value, _>("binding")
            .map_err(unresolved)?
            != serde_json::to_value(binding).map_err(unresolved)?
            || row
                .try_get::<serde_json::Value, _>("identity")
                .map_err(unresolved)?
                != serde_json::to_value(identity).map_err(unresolved)?
            || row
                .try_get::<Vec<u8>, _>("request_digest")
                .map_err(unresolved)?
                != reference.request_digest
            || row
                .try_get::<Option<Uuid>, _>("idempotency_key")
                .map_err(unresolved)?
                != reference.idempotency_key
            || row
                .try_get::<Vec<u8>, _>("settlement_reference")
                .map_err(unresolved)?
                != identity.settlement_reference
        {
            return Ok(unknown());
        }
        tx.commit().await.map_err(unresolved)?;
        Ok(NativeTransactionResolution {
            status: proto::TransactionStatus::Committed,
            receipt_evidence: serde_json::to_vec(&serde_json::json!({
                "kind":"OPERATIONS_NATIVE_TRANSACTION_V1",
                "scope_id":reference.scope_id,"request_id":binding.request_id,
                "identity_digest":digest,"request_digest":reference.request_digest
            }))
            .map_err(unresolved)?,
        })
    }
}
