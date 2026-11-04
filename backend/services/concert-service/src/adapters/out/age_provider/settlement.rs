//! Private same-native-transaction witnesses. These are settlement evidence only.
use crate::{
    adapters::out::relationship_authority::guarded::{self, OwnerGuardScope},
    product_rpc::{
        scope_ledger::ProducerTransaction,
        settlement::{
            NativeTransactionResolution, NativeTransactionResolver, PgNativeAbortResolver,
        },
    },
};
use platform::actor_delegation::owner_binding::OwnerBinding;
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool};
use tonic::Status;
use uuid::Uuid;
const PREFIX: &[u8] = b"mcr-age-runtime-v1:";
fn unavailable(_: impl std::fmt::Display) -> Status {
    Status::unavailable("Age settlement unavailable")
}
fn reference(id: Uuid) -> Vec<u8> {
    [PREFIX, id.as_bytes()].concat()
}
fn reference_id(bytes: &[u8]) -> Option<Uuid> {
    Uuid::from_slice(bytes.strip_prefix(PREFIX)?)
        .ok()
        .filter(|id| !id.is_nil())
}
fn ledger(i: &guarded::ProducerTransaction) -> ProducerTransaction {
    ProducerTransaction {
        producer: "CONCERT".into(),
        system_identifier: i.system_identifier.clone(),
        database_oid: i.database_oid,
        full_xid: i.full_xid.clone(),
        backend_pid: i.backend_pid,
        backend_started_unix_micros: i.backend_started.timestamp_micros(),
        effect_binding_digest: i.effect_binding_digest,
        producer_instance: i.producer_instance_id,
        settlement_reference: i.settlement_reference.clone(),
    }
}
pub(super) async fn register_in(
    c: &mut PgConnection,
    scope: &OwnerGuardScope,
) -> Result<(), Status> {
    scope.binding().check()?;
    let instance = Uuid::parse_str(&scope.producer_scope().instance_id).map_err(unavailable)?;
    let mut identity = guarded::ProducerTransaction::capture_concert_in(
        c,
        scope.binding().digest(),
        instance,
        Vec::new(),
    )
    .await?;
    // A row visible in THIS transaction was inserted only after actual root and
    // both source owner registration ACKs. Rechecks never allocate another ref.
    let found: Option<Vec<u8>> = sqlx::query_scalar("select binding_digest from concert_age_runtime_witnesses where system_identifier=$1 and database_oid=$2 and full_xid=$3")
        .bind(&identity.system_identifier).bind(identity.database_oid as i64).bind(&identity.full_xid).fetch_optional(&mut *c).await.map_err(unavailable)?;
    if let Some(binding) = found {
        if binding != scope.binding().digest() {
            return Err(Status::permission_denied("Age native binding mismatch"));
        }
        return Ok(());
    }
    let id = Uuid::new_v4();
    identity.settlement_reference = reference(id);
    let mut h = Sha256::new();
    h.update(b"MCR-age-native-effect-v1");
    h.update(scope.binding().digest());
    h.update(identity.full_xid.as_bytes());
    h.update(id.as_bytes());
    identity.effect_binding_digest = h.finalize().into();
    scope.track_transaction(identity.clone()).await?;
    let value = ledger(&identity);
    let digest = value.digest()?;
    sqlx::query("insert into concert_age_runtime_witnesses(witness_id,binding_digest,identity_digest,system_identifier,database_oid,full_xid,identity) values($1,$2,$3,$4,$5,$6,$7)")
        .bind(id).bind(scope.binding().digest().as_slice()).bind(digest.as_slice()).bind(&identity.system_identifier)
        .bind(identity.database_oid as i64).bind(&identity.full_xid).bind(serde_json::to_value(value).map_err(unavailable)?)
        .execute(c).await.map_err(unavailable)?;
    Ok(())
}
pub struct AgeTransactionResolver {
    pool: PgPool,
    instance: Uuid,
    fallback: PgNativeAbortResolver,
}
impl AgeTransactionResolver {
    pub fn new(pool: PgPool, instance: Uuid) -> Result<Self, Status> {
        Ok(Self {
            fallback: PgNativeAbortResolver::new(pool.clone(), instance)?,
            pool,
            instance,
        })
    }
}
#[async_trait::async_trait]
impl NativeTransactionResolver for AgeTransactionResolver {
    async fn resolve(
        &self,
        binding: &OwnerBinding,
        identity: &ProducerTransaction,
    ) -> Result<NativeTransactionResolution, Status> {
        let digest = identity.digest()?;
        let binding_digest = binding.digest()?;
        let mut fallback = self.fallback.resolve(binding, identity).await?;
        let Some(id) = reference_id(&identity.settlement_reference) else {
            return Ok(fallback);
        };
        if identity.producer != "CONCERT" || identity.producer_instance != self.instance {
            return Ok(fallback);
        }
        let row: Option<(Vec<u8>, Vec<u8>, serde_json::Value)> = sqlx::query_as("select binding_digest,identity_digest,identity from concert_age_runtime_witnesses where witness_id=$1")
            .bind(id).fetch_optional(&self.pool).await.map_err(unavailable)?;
        if let Some((parent, hash, stored)) = row {
            let stored: ProducerTransaction =
                serde_json::from_value(stored).map_err(unavailable)?;
            if parent == binding_digest && hash == digest && stored == *identity {
                // The insertion trigger proved exact native cluster/database/XID/
                // backend identity. Visibility to a different connection proves
                // that very transaction committed, never merely pg_xact_status.
                let actual: (String,i64) = sqlx::query_as("select s.system_identifier::text,d.oid::bigint from pg_control_system() s cross join pg_database d where d.datname=current_database()")
                    .fetch_one(&self.pool).await.map_err(unavailable)?;
                if actual
                    == (
                        identity.system_identifier.clone(),
                        identity.database_oid as i64,
                    )
                {
                    fallback.status = guarded::proto::TransactionStatus::Committed;
                    let mut h = Sha256::new();
                    h.update(b"MCR-age-committed-v1");
                    h.update(binding_digest);
                    h.update(digest);
                    h.update(id.as_bytes());
                    fallback.receipt_evidence = h.finalize().to_vec();
                }
            }
        }
        Ok(fallback)
    }
}
