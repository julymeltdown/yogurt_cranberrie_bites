//! Party transaction outcomes, never a grant or a business-success assertion.
//! Adapted from operations_live/settlement.rs at 7d5d91bd. Registration and the
//! immutable witness use the caller's actual SQL connection and acquired scope.
use super::{PartyLiveBinding, PartyLiveError, PARTY_OPERATIONS};
use crate::{
    adapters::out::relationship_authority::guarded::{
        proto, OwnerGuardScope, ProducerTransaction as GuardTransaction,
    },
    product_rpc::{
        scope_ledger::ProducerTransaction,
        settlement::{NativeTransactionResolution, NativeTransactionResolver},
    },
};
use platform::actor_delegation::owner_binding::OwnerBinding;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool, Row};
use tonic::Status;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NativeRole {
    Command,
    Read,
    Lookup,
}
impl NativeRole {
    pub fn for_operation(operation: &str) -> Result<Self, PartyLiveError> {
        match operation {
            "listConcertParties" | "getConcertParty" => Ok(Self::Read),
            value if PARTY_OPERATIONS.contains(&value) => Ok(Self::Command),
            _ => Err(PartyLiveError::InvalidInput),
        }
    }
    fn accepts(self, operation: &str) -> bool {
        match self {
            Self::Lookup => matches!(
                operation,
                "decideConcertApplication" | "cancelConcertApplication"
            ),
            _ => Self::for_operation(operation) == Ok(self),
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::Command => "COMMAND",
            Self::Read => "READ",
            Self::Lookup => "LOOKUP",
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reference {
    kind: String,
    operation: String,
    request_id: Uuid,
    scope_id: Uuid,
    request_digest: [u8; 32],
    idempotency_key: Option<Uuid>,
    role: NativeRole,
}
fn unavailable(_: impl std::fmt::Debug) -> PartyLiveError {
    PartyLiveError::DependencyUnavailable
}
fn unresolved(_: impl std::fmt::Debug) -> Status {
    Status::unavailable("Party native settlement unresolved")
}
fn unknown() -> NativeTransactionResolution {
    NativeTransactionResolution {
        status: proto::TransactionStatus::Unknown,
        receipt_evidence: vec![],
    }
}
// Reconstruct the exact PartyLiveBinding digest without extending or rechecking
// the old browser deadline. Settlement must work after expiry and process loss.
fn request_digest(binding: &OwnerBinding, key: Option<Uuid>) -> Result<[u8; 32], Status> {
    binding.validate()?;
    if binding.parent_method != "/concert.product.v1.ProductService/Dispatch"
        || super::wire::parse_operation(
            binding.event_id,
            &binding.http_method,
            &binding.http_target,
        )
        .map_err(unresolved)?
            != binding.operation
        || !PARTY_OPERATIONS.contains(&binding.operation.as_str())
        || (binding.http_method == "GET") != key.is_none()
        || key.is_some_and(|v| v.is_nil())
    {
        return Err(unresolved(()));
    }
    let mut h = Sha256::new();
    h.update(b"MCR party-live binding v1\0");
    for id in [
        binding.actor_id,
        binding.event_id,
        binding.session_family_id,
        binding.request_id,
    ] {
        h.update(id.as_bytes());
    }
    h.update(binding.expires_unix_micros.to_be_bytes());
    for s in [&binding.http_method, &binding.http_target] {
        h.update((s.len() as u64).to_be_bytes());
        h.update(s.as_bytes());
    }
    h.update(binding.body_digest);
    h.update([1]);
    h.update(binding.digest()?);
    match key {
        Some(k) => {
            h.update([1]);
            h.update(k.as_bytes());
        }
        None => h.update([0]),
    }
    Ok(h.finalize().into())
}
fn ledger_identity(native: &GuardTransaction) -> ProducerTransaction {
    ProducerTransaction {
        producer: "CONCERT".into(),
        system_identifier: native.system_identifier.clone(),
        database_oid: native.database_oid,
        full_xid: native.full_xid.clone(),
        backend_pid: native.backend_pid,
        backend_started_unix_micros: native.backend_started.timestamp_micros(),
        effect_binding_digest: native.effect_binding_digest,
        producer_instance: native.producer_instance_id,
        settlement_reference: native.settlement_reference.clone(),
    }
}

/// Registers actual native work, then persists an immutable same-transaction
/// witness. This does not authorize business work; the caller must still perform
/// all retained owner, resource, profile and Party checks on the acquired scope.
/// Exposed for the native component harness without a fake Profile reader.
pub async fn register_in(
    scope: &OwnerGuardScope,
    c: &mut PgConnection,
    binding: &PartyLiveBinding,
    role: NativeRole,
) -> Result<GuardTransaction, PartyLiveError> {
    binding.check()?;
    scope.binding().check().map_err(unavailable)?;
    let owner = scope.binding().owner_binding();
    if binding.parent_digest()? != scope.binding().digest()
        || !role.accepts(binding.operation())
        || request_digest(&owner, binding.idempotency_key()).map_err(unavailable)?
            != binding.digest()
    {
        return Err(PartyLiveError::Denied);
    }
    tokio::time::timeout_at(binding.deadline().stop(), async {
        let remaining=binding.deadline().remaining(super::REQUEST_CEILING,binding.deadline().expires_micros()).map_err(unavailable)?;
        let millis=remaining.as_millis();if millis==0 {return Err(PartyLiveError::DependencyUnavailable);}
        // Only tighten the current transaction's timeout. Never renew authority.
        for setting in ["statement_timeout","lock_timeout"] {
            sqlx::query("select set_config($1,(extract(epoch from least(coalesce(nullif(current_setting($1)::interval,interval '0'),$2::text::interval),$2::text::interval))*1000)::bigint::text||'ms',true)")
                .bind(setting).bind(format!("{millis}ms")).execute(&mut *c).await.map_err(unavailable)?;
        }
        let producer=scope.producer_scope();
        let reference=Reference {kind:"PARTY_NATIVE_V1".into(),operation:binding.operation().into(),request_id:binding.request_id(),
            scope_id:Uuid::parse_str(&producer.scope_id).map_err(unavailable)?,request_digest:binding.digest(),
            idempotency_key:binding.idempotency_key(),role};
        let native=GuardTransaction::capture_concert_in(c,binding.digest(),Uuid::parse_str(&producer.instance_id).map_err(unavailable)?,serde_json::to_vec(&reference).map_err(unavailable)?)
            .await.map_err(unavailable)?;
        scope.track_transaction(native.clone()).await.map_err(unavailable)?;
        let identity=ledger_identity(&native);let digest=identity.digest().map_err(unavailable)?;
        sqlx::query("insert into public.concert_party_native_witnesses(instance_id,scope_id,binding_digest,identity_digest,request_digest,idempotency_key,native_role,binding,identity,settlement_reference) select s.instance_id,s.scope_id,s.binding_digest,$4,$5,$6,$7,s.binding,$8,$9 from public.concert_product_owner_scopes s where s.instance_id=$1 and s.scope_id=$2 and s.binding_digest=$3 on conflict(instance_id,scope_id,identity_digest) do nothing")
            .bind(native.producer_instance_id).bind(reference.scope_id).bind(scope.binding().digest().as_slice())
            .bind(digest.as_slice()).bind(binding.digest().as_slice()).bind(binding.idempotency_key()).bind(role.name())
            .bind(serde_json::to_value(&identity).map_err(unavailable)?).bind(&native.settlement_reference)
            .execute(&mut *c).await.map_err(unavailable)?;
        let exact:bool=sqlx::query_scalar("select exists(select 1 from public.concert_party_native_witnesses where instance_id=$1 and scope_id=$2 and binding_digest=$3 and identity_digest=$4 and request_digest=$5 and idempotency_key is not distinct from $6 and native_role=$7 and identity=$8 and settlement_reference=$9)")
            .bind(native.producer_instance_id).bind(reference.scope_id).bind(scope.binding().digest().as_slice())
            .bind(digest.as_slice()).bind(binding.digest().as_slice()).bind(binding.idempotency_key()).bind(role.name())
            .bind(serde_json::to_value(&identity).map_err(unavailable)?).bind(&native.settlement_reference)
            .fetch_one(&mut *c).await.map_err(unavailable)?;
        if !exact {return Err(PartyLiveError::DependencyUnavailable);}
        scope.binding().check().map_err(unavailable)?;
        Ok(native)
    }).await.map_err(unavailable)?
}

/// System settlement leaf. Root owns authenticated RPC composition and uses the
/// identical configured instance/ledger. This resolver acquires no authority.
pub struct PartyNativeResolver {
    pool: PgPool,
    instance: Uuid,
}
impl PartyNativeResolver {
    pub fn new(pool: PgPool, instance: Uuid) -> Result<Self, Status> {
        if instance.is_nil() {
            return Err(Status::invalid_argument("Invalid Party producer instance"));
        }
        Ok(Self { pool, instance })
    }
}
#[async_trait::async_trait]
impl NativeTransactionResolver for PartyNativeResolver {
    async fn resolve(
        &self,
        binding: &OwnerBinding,
        identity: &ProducerTransaction,
    ) -> Result<NativeTransactionResolution, Status> {
        binding.validate()?;
        let digest = identity.digest()?;
        if identity.producer != "CONCERT" || identity.producer_instance != self.instance {
            return Ok(unknown());
        }
        let Ok(reference) = serde_json::from_slice::<Reference>(&identity.settlement_reference)
        else {
            return Ok(unknown());
        };
        if reference.kind != "PARTY_NATIVE_V1"
            || reference.scope_id.is_nil()
            || reference.operation != binding.operation
            || reference.request_id != binding.request_id
            || !reference.role.accepts(&reference.operation)
            || request_digest(binding, reference.idempotency_key).ok()
                != Some(reference.request_digest)
            || reference.request_digest != identity.effect_binding_digest
            || serde_json::to_vec(&reference).map_err(unresolved)? != identity.settlement_reference
        {
            return Ok(unknown());
        }
        let mut tx = self.pool.begin().await.map_err(unresolved)?;
        sqlx::query("set local statement_timeout='750ms'")
            .execute(&mut *tx)
            .await
            .map_err(unresolved)?;
        sqlx::query("set local lock_timeout='250ms'")
            .execute(&mut *tx)
            .await
            .map_err(unresolved)?;
        let actual:(String,i64)=sqlx::query_as("select s.system_identifier::text,d.oid::bigint from pg_control_system() s cross join pg_database d where d.datname=current_database()")
            .fetch_one(&mut *tx).await.map_err(unresolved)?;
        if actual
            != (
                identity.system_identifier.clone(),
                i64::from(identity.database_oid),
            )
        {
            return Ok(unknown());
        }
        // Validate *all* outcomes against the exact confirmed parent/identity.
        // A real aborted XID from another request is not this scope's rollback.
        let registered:bool=sqlx::query_scalar("select exists(select 1 from public.concert_product_owner_scopes s join public.concert_product_owner_transactions t using(instance_id,scope_id,binding_digest) where s.instance_id=$1 and s.scope_id=$2 and s.binding_digest=$3 and s.binding=$4 and s.phase='ROOT_FINISHED' and s.delivery_secret_hash is not null and t.identity_digest=$5 and t.identity=$6 and t.confirmed_at is not null and t.auth_registration_id is not null and t.friend_registration_id is not null)")
            .bind(self.instance).bind(reference.scope_id).bind(binding.digest()?.as_slice()).bind(serde_json::to_value(binding).map_err(unresolved)?)
            .bind(digest.as_slice()).bind(serde_json::to_value(identity).map_err(unresolved)?).fetch_one(&mut *tx).await.map_err(unresolved)?;
        if !registered {
            return Ok(unknown());
        }
        let status: Option<String> = sqlx::query_scalar("select pg_xact_status($1::text::xid8)")
            .bind(&identity.full_xid)
            .fetch_one(&mut *tx)
            .await
            .map_err(unresolved)?;
        let native_status = match status.as_deref() {
            Some("aborted") => proto::TransactionStatus::Aborted,
            Some("in progress") => proto::TransactionStatus::InProgress,
            Some("committed") => proto::TransactionStatus::Committed,
            _ => return Ok(unknown()),
        };
        if native_status != proto::TransactionStatus::Committed {
            tx.commit().await.map_err(unresolved)?;
            return Ok(NativeTransactionResolution {
                status: native_status,
                receipt_evidence: vec![],
            });
        }
        let row=sqlx::query("select w.binding,w.identity,w.request_digest,w.idempotency_key,w.native_role,w.settlement_reference,to_jsonb(r) receipt,c.receipt committed_receipt from public.concert_party_native_witnesses w left join public.concert_party_native_command_receipts c using(instance_id,scope_id,identity_digest) left join public.concert_command_receipts r on r.receipt_id=c.receipt_id and w.native_role='COMMAND' and r.actor_id=(w.binding->>'actor_id')::uuid and r.event_id=(w.binding->>'event_id')::uuid and r.operation_id=w.binding->>'operation' and r.idempotency_key=w.idempotency_key where w.instance_id=$1 and w.scope_id=$2 and w.binding_digest=$3 and w.identity_digest=$4")
            .bind(self.instance).bind(reference.scope_id).bind(binding.digest()?.as_slice()).bind(digest.as_slice()).fetch_optional(&mut *tx).await.map_err(unresolved)?;
        let Some(row) = row else { return Ok(unknown()) };
        let receipt: Option<serde_json::Value> = row.try_get("receipt").map_err(unresolved)?;
        let committed_receipt: Option<serde_json::Value> =
            row.try_get("committed_receipt").map_err(unresolved)?;
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
                .try_get::<String, _>("native_role")
                .map_err(unresolved)?
                != reference.role.name()
            || row
                .try_get::<Vec<u8>, _>("settlement_reference")
                .map_err(unresolved)?
                != identity.settlement_reference
            || (reference.role == NativeRole::Command
                && !receipt
                    .as_ref()
                    .zip(committed_receipt.as_ref())
                    .is_some_and(|(current, committed)| {
                        matches!(
                            current["state"].as_str(),
                            Some("PENDING" | "SUCCEEDED" | "FAILED" | "RETIRED")
                        ) && matches!(
                            committed["state"].as_str(),
                            Some("PENDING" | "SUCCEEDED" | "FAILED")
                        ) && [
                            "receipt_id",
                            "actor_id",
                            "event_id",
                            "operation_id",
                            "idempotency_key",
                            "request_hash",
                            "target_id",
                            "created_at",
                            "expires_at",
                        ]
                        .iter()
                        .all(|k| current[*k] == committed[*k])
                    }))
        {
            return Ok(unknown());
        }
        // Expose only private opaque correlation and the actual receipt outcome,
        // never raw binding/body or owner registration secrets. No success flag.
        let safe_receipt = |r: &serde_json::Value| serde_json::json!({"receipt_id":r["receipt_id"],"state":r["state"],"target_id":r["target_id"],"resource_id":r["resource_id"],"resource_revision":r["resource_revision"],"failure_reason":r["failure_reason"]});
        let receipt = receipt.as_ref().map(&safe_receipt);
        let committed_receipt = committed_receipt.as_ref().map(&safe_receipt);
        tx.commit().await.map_err(unresolved)?;
        Ok(NativeTransactionResolution{status:native_status,receipt_evidence:serde_json::to_vec(&serde_json::json!({"kind":"PARTY_NATIVE_TRANSACTION_V1","scope_id":reference.scope_id,"request_id":binding.request_id,"identity_digest":digest,"request_digest":reference.request_digest,"role":reference.role,"receipt":receipt,"committed_receipt":committed_receipt})).map_err(unresolved)?})
    }
}
