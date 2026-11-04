//! Native witness adapted from intro_live/witness.rs at7d5d91bd; real confirmed owner registration is mandatory.
//! Immutable same-transaction native settlement; never new request authority.
use super::authority::guard_error;
use crate::adapters::out::relationship_authority::guarded::{OwnerGuardScope, ProducerTransaction};
use crate::product_rpc::settlement::NativeTransactionResolver;
use chrono::{DateTime, Utc};
use concert_application::mural::MuralError;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool};
use std::sync::Arc;
use uuid::Uuid;

/// Register the actual connection before any producer effect. A committed
/// witness is private settlement evidence, never authority for a new request.
pub(super) async fn register_wall_transaction(
    c: &mut PgConnection,
    scope: &Arc<OwnerGuardScope>,
) -> Result<(), MuralError> {
    scope.binding().check().map_err(guard_error)?;
    let instance = Uuid::parse_str(&scope.producer_scope().instance_id)
        .map_err(|_| MuralError::StorageUnavailable)?;
    let mut identity =
        ProducerTransaction::capture_concert_in(c, scope.binding().digest(), instance, Vec::new())
            .await
            .map_err(guard_error)?;
    let mut digest = Sha256::new();
    digest.update(b"mcr-wall-b-witness-v1:");
    digest.update(identity.system_identifier.as_bytes());
    digest.update(identity.database_oid.to_be_bytes());
    digest.update(identity.full_xid.as_bytes());
    digest.update(scope.binding().digest());
    let digest = digest.finalize();
    let mut bytes = [0; 16];
    bytes.copy_from_slice(&digest[..16]);
    let witness = Uuid::from_bytes(bytes);
    identity.settlement_reference =
        serde_json::to_vec(&json!({"kind":"wall_tools_b_native_v1","witness_id":witness}))
            .map_err(|_| MuralError::StorageUnavailable)?;
    let wire = identity.wire().map_err(guard_error)?;
    let native = crate::product_rpc::scope_ledger::ProducerTransaction {
        producer: "CONCERT".into(),
        system_identifier: identity.system_identifier.clone(),
        database_oid: identity.database_oid,
        full_xid: identity.full_xid.clone(),
        backend_pid: identity.backend_pid,
        backend_started_unix_micros: identity.backend_started.timestamp_micros(),
        effect_binding_digest: identity.effect_binding_digest,
        producer_instance: instance,
        settlement_reference: identity.settlement_reference.clone(),
    };
    let scope_id = Uuid::parse_str(&scope.producer_scope().scope_id)
        .map_err(|_| MuralError::StorageUnavailable)?;
    scope
        .track_transaction(identity)
        .await
        .map_err(guard_error)?;
    let until = DateTime::<Utc>::from_timestamp_micros(scope.binding().deadline().expires_micros())
        .ok_or(MuralError::StorageUnavailable)?;
    sqlx::query("insert into public.concert_mural_tools_b_native_witness(witness_id,system_identifier,database_oid,full_xid,backend_pid,backend_started_micros,producer_instance,effect_digest,settlement_reference,authority_until,scope_id,identity_digest,native_identity) values($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13) on conflict(witness_id) do nothing")
        .bind(witness).bind(&wire.system_identifier).bind(i64::from(wire.database_oid)).bind(&wire.full_xid)
        .bind(wire.backend_pid).bind(wire.backend_started_unix_micros).bind(instance)
        .bind(wire.effect_binding_digest).bind(wire.settlement_reference).bind(until)
        .bind(scope_id).bind(native.digest().map_err(guard_error)?.to_vec())
        .bind(serde_json::to_value(native).map_err(|_| MuralError::StorageUnavailable)?)
        .execute(&mut *c).await.map_err(|_| MuralError::StorageUnavailable)?;
    sqlx::query("select set_config('mcr.mural_tools_b_authority_until',least(coalesce(nullif(current_setting('mcr.mural_tools_b_authority_until',true),'')::timestamptz,'infinity'::timestamptz),$1::timestamptz)::text,true)")
        .bind(until).execute(c).await.map_err(|_| MuralError::StorageUnavailable)?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WallSettlementReference {
    kind: String,
    witness_id: Uuid,
}

/// Root installs this for Wall B references alongside the other producers. A
/// committed native transaction without an exact durable witness stays UNKNOWN.
pub struct MuralToolsNativeResolver {
    pool: PgPool,
    instance: Uuid,
    abort: crate::product_rpc::settlement::PgNativeAbortResolver,
}
impl MuralToolsNativeResolver {
    pub fn new(pool: PgPool, instance: Uuid) -> Result<Self, tonic::Status> {
        Ok(Self {
            abort: crate::product_rpc::settlement::PgNativeAbortResolver::new(
                pool.clone(),
                instance,
            )?,
            pool,
            instance,
        })
    }
}
#[async_trait::async_trait]
impl crate::product_rpc::settlement::NativeTransactionResolver for MuralToolsNativeResolver {
    async fn resolve(
        &self,
        binding: &platform::actor_delegation::owner_binding::OwnerBinding,
        identity: &crate::product_rpc::scope_ledger::ProducerTransaction,
    ) -> Result<crate::product_rpc::settlement::NativeTransactionResolution, tonic::Status> {
        use crate::adapters::out::relationship_authority::guarded::proto::TransactionStatus;
        let mut result = self.abort.resolve(binding, identity).await?;
        if result.status != TransactionStatus::Unknown
            || identity.producer_instance != self.instance
            || identity.producer != "CONCERT"
            || identity.effect_binding_digest != binding.digest()?
        {
            return Ok(result);
        }
        let Ok(reference) =
            serde_json::from_slice::<WallSettlementReference>(&identity.settlement_reference)
        else {
            return Ok(result);
        };
        if reference.kind != "wall_tools_b_native_v1" || reference.witness_id.is_nil() {
            return Ok(result);
        }
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|_| tonic::Status::unavailable("Wall settlement unavailable"))?;
        let native: bool = sqlx::query_scalar("select (pg_control_system()).system_identifier::text=$1 and (select oid::bigint from pg_database where datname=current_database())=$2 and pg_xact_status($3::text::xid8)='committed'")
            .bind(&identity.system_identifier).bind(i64::from(identity.database_oid)).bind(&identity.full_xid)
            .fetch_one(&mut *tx).await.map_err(|_| tonic::Status::unavailable("Wall settlement unavailable"))?;
        if native {
            let exact: bool = sqlx::query_scalar("select exists(select 1 from public.concert_mural_tools_b_native_witness where witness_id=$1 and system_identifier=$2 and database_oid=$3 and full_xid=$4 and backend_pid=$5 and backend_started_micros=$6 and producer_instance=$7 and effect_digest=$8 and settlement_reference=$9)")
                .bind(reference.witness_id).bind(&identity.system_identifier).bind(i64::from(identity.database_oid))
                .bind(&identity.full_xid).bind(identity.backend_pid).bind(identity.backend_started_unix_micros)
                .bind(identity.producer_instance).bind(identity.effect_binding_digest.to_vec()).bind(&identity.settlement_reference)
                .fetch_one(&mut *tx).await.map_err(|_| tonic::Status::unavailable("Wall settlement unavailable"))?;
            if exact {
                result.status = TransactionStatus::Committed;
                result.receipt_evidence = serde_json::to_vec(&json!({"kind":"wall_tools_b_native_v1","witness_id":reference.witness_id,"identity_digest":identity.digest()?.to_vec()}))
                    .map_err(|_| tonic::Status::unavailable("Wall settlement unavailable"))?;
            }
        }
        tx.commit()
            .await
            .map_err(|_| tonic::Status::unavailable("Wall settlement unavailable"))?;
        Ok(result)
    }
}
