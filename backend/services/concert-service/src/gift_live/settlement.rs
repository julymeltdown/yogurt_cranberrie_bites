//! Exact native transaction evidence. COMMITTED never means business SUCCEEDED.
//! Root routes this resolver only for Gift bindings; prepared children use the
//! separate typed GiftPrivate settler, never this ordinary-transaction witness.
use super::protocol::{canonical_uuid, parse_query, GiftOperation};
use crate::{
    adapters::out::relationship_authority::guarded::{
        OwnerGuardScope, ProducerTransaction as NativeIdentity,
    },
    product_rpc::{
        scope_ledger::ProducerTransaction,
        settlement::{NativeTransactionResolution, NativeTransactionResolver},
    },
};
use platform::actor_delegation::owner_binding::OwnerBinding;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool};
use std::sync::Arc;
use tonic::Status;
use uuid::Uuid;

fn unavailable(_: impl std::fmt::Display) -> Status {
    Status::unavailable("Gift native settlement unavailable")
}
#[cfg(test)]
#[path = "native_settlement.rs"]
mod native_tests;

pub(crate) fn effect_digest(binding: &OwnerBinding, reference: &[u8]) -> Result<[u8; 32], Status> {
    let mut hash = Sha256::new();
    hash.update(b"MCR gift native transaction v2\0");
    hash.update(binding.digest()?);
    hash.update(Sha256::digest(reference));
    Ok(hash.finalize().into())
}

/// Route selection only; it cannot grant product execution or reveal content.
pub fn handles_binding(binding: &OwnerBinding) -> bool {
    if binding.validate().is_err() {
        return false;
    }
    if reference_operation(binding, &binding.operation).is_some() {
        return true;
    }
    if binding.operation != "getConcertCommandStatus"
        || binding.http_method != "GET"
        || binding.parent_method != "/concert.product.v1.ProductService/Dispatch"
        || binding.body_digest != <[u8; 32]>::from(Sha256::digest([]))
    {
        return false;
    }
    let Some((_, raw)) = binding.http_target.split_once('?') else {
        return false;
    };
    let Ok(query) = parse_query(Some(raw)) else {
        return false;
    };
    let Some(op) = query
        .get("operation_id")
        .and_then(|v| concert_application::commands::RegisteredOperation::find(v).ok())
    else {
        return false;
    };
    let Some(key) = query
        .get("idempotency_key")
        .and_then(|v| canonical_uuid(v).ok())
    else {
        return false;
    };
    super::status::query(binding.event_id, &binding.http_target, op, key).is_ok()
}
fn reference_operation(binding: &OwnerBinding, operation: &str) -> Option<GiftOperation> {
    if binding.parent_method != "/concert.product.v1.ProductService/Dispatch" {
        return None;
    }
    let op = GiftOperation::parse(operation).ok()?;
    let base = format!("/api/concert/events/{}/", binding.event_id);
    let target = binding.http_target.strip_prefix(&base)?;
    let (path, query) = target
        .split_once('?')
        .map_or((target, None), |(p, q)| (p, Some(q)));
    let parts: Vec<_> = path.split('/').collect();
    let id = |v: &&str| canonical_uuid(v).is_ok();
    let matched = match (op, binding.http_method.as_str(), parts.as_slice()) {
        (GiftOperation::List, "GET", ["giveaways"])
        | (GiftOperation::Create, "POST", ["giveaways"])
        | (GiftOperation::Activity, "GET", ["me", "giveaways"]) => true,
        (GiftOperation::Detail, "GET", ["giveaways", v])
        | (GiftOperation::Update, "PATCH", ["giveaways", v])
        | (GiftOperation::Management, "GET", ["giveaways", v, "management"])
        | (GiftOperation::SetMeeting, "PUT", ["giveaways", v, "pickup-meeting"])
        | (GiftOperation::Reserve, "POST", ["giveaways", v, "reservations"])
        | (GiftOperation::Reservation, "GET", ["reservations", v])
        | (GiftOperation::Cancel, "DELETE", ["reservations", v])
        | (GiftOperation::Claim, "POST", ["reservations", v, "claim"])
        | (GiftOperation::Pickup, "GET", ["reservations", v, "pickup"])
        | (GiftOperation::IssueCode, "POST", ["reservations", v, "pickup-code"]) => id(v),
        _ => false,
    };
    if !matched {
        return None;
    }
    if op.mutation() && op != GiftOperation::Cancel && query.is_some() {
        return None;
    }
    let params = parse_query(query).ok()?;
    if !op.mutation() && super::protocol::validate_query(op, &params).is_err() {
        return None;
    }
    if op == GiftOperation::Cancel {
        let revision = params.get("expected_revision")?.parse::<i64>().ok()?;
        if params.len() != 1
            || !(1..=concert_domain::MAX_REVISION).contains(&revision)
            || params.get("expected_revision")? != &revision.to_string()
        {
            return None;
        }
    }
    Some(op)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reference {
    kind: String,
    actor_id: Uuid,
    event_id: Uuid,
    request_id: Uuid,
    operation_id: String,
    idempotency_key: Option<Uuid>,
}
fn valid_reference(binding: &OwnerBinding, raw: &[u8]) -> bool {
    let Ok(reference) = serde_json::from_slice::<Reference>(raw) else {
        return false;
    };
    if (reference.actor_id, reference.event_id, reference.request_id)
        != (binding.actor_id, binding.event_id, binding.request_id)
    {
        return false;
    }
    let Ok(op) = GiftOperation::parse(&reference.operation_id) else {
        return false;
    };
    if reference.idempotency_key.is_some_and(|v| v.is_nil()) {
        return false;
    }
    match reference.kind.as_str() {
        "GIFT_REQUEST_V1" => {
            reference.operation_id == binding.operation
                && op.mutation() == reference.idempotency_key.is_some()
                && reference_operation(binding, &reference.operation_id).is_some()
        }
        "GIFT_STATUS_V1" => {
            binding.parent_method == "/concert.product.v1.ProductService/Dispatch"
                && binding.operation == "getConcertCommandStatus"
                && binding.http_method == "GET"
                && binding.body_digest == <[u8; 32]>::from(Sha256::digest([]))
                && op.mutation()
                && reference.idempotency_key.is_some_and(|key| {
                    super::status::query(
                        binding.event_id,
                        &binding.http_target,
                        match concert_application::commands::RegisteredOperation::find(
                            &reference.operation_id,
                        ) {
                            Ok(op) => op,
                            Err(_) => return false,
                        },
                        key,
                    )
                    .is_ok()
                })
        }
        _ => false,
    }
}

/// Persist an immutable registration witness on the exact tracked connection.
/// If any subsequent policy/effect/projection check fails, normal rollback also
/// removes this row. Commit proves this TX only, not the API result or a child.
pub(crate) async fn register_in(
    c: &mut PgConnection,
    scope: &Arc<OwnerGuardScope>,
    effect: [u8; 32],
    reference: Vec<u8>,
) -> Result<(), Status> {
    let binding = scope.binding().owner_binding();
    if !valid_reference(&binding, &reference) || effect != effect_digest(&binding, &reference)? {
        return Err(Status::permission_denied("Gift native binding mismatch"));
    }
    scope.binding().check()?;
    let producer = scope.producer_scope();
    let instance = Uuid::parse_str(&producer.instance_id).map_err(unavailable)?;
    let scope_id = Uuid::parse_str(&producer.scope_id).map_err(unavailable)?;
    let identity = NativeIdentity::capture_concert_in(c, effect, instance, reference).await?;
    let native = ProducerTransaction {
        producer: "CONCERT".into(),
        system_identifier: identity.system_identifier.clone(),
        database_oid: identity.database_oid,
        full_xid: identity.full_xid.clone(),
        backend_pid: identity.backend_pid,
        backend_started_unix_micros: identity.backend_started.timestamp_micros(),
        effect_binding_digest: effect,
        producer_instance: instance,
        settlement_reference: identity.settlement_reference.clone(),
    };
    let digest = native.digest()?;
    scope.track_transaction(identity).await?;
    sqlx::query("insert into concert_gift_native_witnesses(producer_instance,scope_id,identity_digest,binding_digest,native_identity) values($1,$2,$3,$4,$5) on conflict(producer_instance,scope_id,identity_digest) do nothing")
        .bind(instance).bind(scope_id).bind(digest.as_slice()).bind(binding.digest()?.as_slice())
        .bind(sqlx::types::Json(&native)).execute(&mut *c).await.map_err(unavailable)?;
    let exact: bool = sqlx::query_scalar("select exists(select 1 from concert_gift_native_witnesses where producer_instance=$1 and scope_id=$2 and identity_digest=$3 and binding_digest=$4 and native_identity=$5)")
        .bind(instance).bind(scope_id).bind(digest.as_slice()).bind(binding.digest()?.as_slice())
        .bind(sqlx::types::Json(&native)).fetch_one(c).await.map_err(unavailable)?;
    if !exact {
        return Err(unavailable("Witness mismatch"));
    }
    Ok(())
}

/// Called by BE15 completed(), including exact replay and safe failure.
/// Status lookup is read-only and has only a local-transaction witness.
/// Stores only the safe receipt as it existed on THIS connection; later receipt
/// retirement cannot rewrite this evidence or recreate a one-shot code.
pub(crate) async fn receipt_in(
    c: &mut PgConnection,
    scope: &Arc<OwnerGuardScope>,
    effect: [u8; 32],
    reference: Vec<u8>,
    command: concert_application::commands::Scope,
    target: Uuid,
    receipt: &concert_application::commands::SafeReceipt,
) -> Result<(), Status> {
    register_in(c, scope, effect, reference.clone()).await?;
    let parsed: Reference = serde_json::from_slice(&reference).map_err(unavailable)?;
    if receipt.scope != command
        || parsed.actor_id != command.actor
        || parsed.event_id != command.event
        || parsed.operation_id != command.operation.id()
        || parsed.idempotency_key != Some(command.key)
        || target.is_nil()
        || receipt.id.is_nil()
    {
        return Err(Status::permission_denied("Gift receipt scope mismatch"));
    }
    let producer = scope.producer_scope();
    let instance = Uuid::parse_str(&producer.instance_id).map_err(unavailable)?;
    let scope_id = Uuid::parse_str(&producer.scope_id).map_err(unavailable)?;
    let identity = NativeIdentity::capture_concert_in(c, effect, instance, reference).await?;
    let native = ProducerTransaction {
        producer: "CONCERT".into(),
        system_identifier: identity.system_identifier,
        database_oid: identity.database_oid,
        full_xid: identity.full_xid,
        backend_pid: identity.backend_pid,
        backend_started_unix_micros: identity.backend_started.timestamp_micros(),
        effect_binding_digest: effect,
        producer_instance: instance,
        settlement_reference: identity.settlement_reference,
    };
    let digest = native.digest()?;
    let snapshot = serde_json::json!({
        "receipt_id":receipt.id,"actor_id":command.actor,"event_id":command.event,
        "operation_id":command.operation.id(),"idempotency_key":command.key,"target_id":target,
        "state":receipt.state.as_str(),"resource_id":receipt.reference.map(|r|r.resource),
        "resource_revision":receipt.reference.map(|r|r.revision),"result_status":receipt.result_status,
        "failure_reason":receipt.failure.map(|f|f.as_str()),
        "created_at":receipt.created_at,"expires_at":receipt.expires_at,
    });
    sqlx::query("insert into concert_gift_receipt_witnesses(producer_instance,scope_id,identity_digest,receipt_id,receipt_snapshot) values($1,$2,$3,$4,$5) on conflict(producer_instance,scope_id,identity_digest) do nothing")
        .bind(instance).bind(scope_id).bind(digest.as_slice()).bind(receipt.id).bind(&snapshot)
        .execute(&mut *c).await.map_err(unavailable)?;
    let exact:bool = sqlx::query_scalar("select exists(select 1 from concert_gift_receipt_witnesses where producer_instance=$1 and scope_id=$2 and identity_digest=$3 and receipt_id=$4 and receipt_snapshot=$5)")
        .bind(instance).bind(scope_id).bind(digest.as_slice()).bind(receipt.id).bind(snapshot)
        .fetch_one(c).await.map_err(unavailable)?;
    if !exact {
        return Err(unavailable("Gift receipt witness mismatch"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn binding() -> OwnerBinding {
        let event = Uuid::from_u128(2);
        OwnerBinding {
            actor_id: Uuid::from_u128(1),
            event_id: event,
            session_family_id: Uuid::from_u128(3),
            request_id: Uuid::from_u128(4),
            operation: "claimConcertReservation".into(),
            expires_unix_micros: 1_900_000_000_000_000,
            parent_digest: [1; 32],
            parent_body_digest: [2; 32],
            parent_method: "/concert.product.v1.ProductService/Dispatch".into(),
            http_method: "POST".into(),
            http_target: format!(
                "/api/concert/events/{event}/reservations/{}/claim",
                Uuid::from_u128(5)
            ),
            body_digest: [3; 32],
        }
    }
    fn reference(b: &OwnerBinding) -> serde_json::Value {
        serde_json::json!({"kind":"GIFT_REQUEST_V1","actor_id":b.actor_id,"event_id":b.event_id,
            "request_id":b.request_id,"operation_id":b.operation,"idempotency_key":Uuid::from_u128(6)})
    }
    #[test]
    fn rejects_foreign_parent_route_key_and_extra_reference_material() {
        let b = binding();
        let value = reference(&b);
        let raw = serde_json::to_vec(&value).unwrap();
        assert!(valid_reference(&b, &raw));
        assert!(handles_binding(&b));
        for (field, forged) in [
            ("actor_id", serde_json::json!(Uuid::from_u128(99))),
            ("event_id", serde_json::json!(Uuid::from_u128(99))),
            ("request_id", serde_json::json!(Uuid::from_u128(99))),
            ("idempotency_key", serde_json::Value::Null),
            ("kind", serde_json::json!("GIFT_STATUS_V1")),
            ("permission", serde_json::json!(true)),
        ] {
            let mut changed = value.clone();
            changed[field] = forged;
            assert!(
                !valid_reference(&b, &serde_json::to_vec(&changed).unwrap()),
                "{field}"
            );
        }
        let mut changed = b.clone();
        changed.http_method = "GET".into();
        assert!(!handles_binding(&changed));
        changed = b.clone();
        changed.http_target.push_str("?unexpected=1");
        assert!(!handles_binding(&changed));
        changed = b.clone();
        changed.body_digest = [7; 32];
        assert_ne!(
            effect_digest(&b, &raw).unwrap(),
            effect_digest(&changed, &raw).unwrap()
        );
        let mut alternate = value;
        alternate["idempotency_key"] = serde_json::json!(Uuid::from_u128(7));
        assert_ne!(
            effect_digest(&b, &raw).unwrap(),
            effect_digest(&b, &serde_json::to_vec(&alternate).unwrap()).unwrap()
        );
    }
    #[test]
    fn status_binds_exact_original_operation_and_key_without_recreating_command() {
        let mut b = binding();
        let key = Uuid::from_u128(6);
        b.operation = "getConcertCommandStatus".into();
        b.http_method = "GET".into();
        b.body_digest = Sha256::digest([]).into();
        b.http_target=format!("/api/concert/events/{}/commands?operation_id=claimConcertReservation&idempotency_key={key}",b.event_id);
        let value = serde_json::json!({"kind":"GIFT_STATUS_V1","actor_id":b.actor_id,"event_id":b.event_id,
            "request_id":b.request_id,"operation_id":"claimConcertReservation","idempotency_key":key});
        let raw = serde_json::to_vec(&value).unwrap();
        assert!(valid_reference(&b, &raw));
        b.http_target
            .push_str("&operation_id=claimConcertReservation");
        assert!(!valid_reference(&b, &raw));
        b.http_target=format!("/api/concert/events/{}/commands?operation_id=claimConcertReservation&idempotency_key={key}",b.event_id);
        b.body_digest = [9; 32];
        assert!(!valid_reference(&b, &raw));
    }
}

pub struct GiftNativeResolver {
    pool: PgPool,
    instance: Uuid,
}
impl GiftNativeResolver {
    pub fn new(pool: PgPool, instance: Uuid) -> Result<Self, Status> {
        if instance.is_nil() || pool.is_closed() {
            return Err(Status::invalid_argument("Invalid Gift resolver"));
        }
        Ok(Self { pool, instance })
    }
}
#[async_trait::async_trait]
impl NativeTransactionResolver for GiftNativeResolver {
    async fn resolve(
        &self,
        binding: &OwnerBinding,
        identity: &ProducerTransaction,
    ) -> Result<NativeTransactionResolution, Status> {
        use crate::adapters::out::relationship_authority::guarded::proto::TransactionStatus;
        let digest = identity.digest()?;
        let binding_digest = binding.digest()?;
        let mut result = NativeTransactionResolution {
            status: TransactionStatus::Unknown,
            receipt_evidence: vec![],
        };
        if identity.producer != "CONCERT"
            || identity.producer_instance != self.instance
            || !valid_reference(binding, &identity.settlement_reference)
            || identity.effect_binding_digest
                != effect_digest(binding, &identity.settlement_reference)?
        {
            return Ok(result);
        }
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        sqlx::query("set local statement_timeout='1500ms'")
            .execute(&mut *tx)
            .await
            .map_err(unavailable)?;
        let actual:(String,i64) = sqlx::query_as("select (pg_control_system()).system_identifier::text,(select oid::bigint from pg_database where datname=current_database())")
            .fetch_one(&mut *tx).await.map_err(unavailable)?;
        if actual
            != (
                identity.system_identifier.clone(),
                i64::from(identity.database_oid),
            )
        {
            return Ok(result);
        }
        let native: Option<String> = sqlx::query_scalar("select pg_xact_status($1::text::xid8)")
            .bind(&identity.full_xid)
            .fetch_one(&mut *tx)
            .await
            .map_err(unavailable)?;
        match native.as_deref() {
            Some("aborted") => result.status = TransactionStatus::Aborted,
            Some("in progress") => result.status = TransactionStatus::InProgress,
            Some("committed") => {
                let exact:bool = sqlx::query_scalar("select exists(select 1 from concert_gift_native_witnesses w join concert_product_owner_transactions t on t.instance_id=w.producer_instance and t.scope_id=w.scope_id and t.identity_digest=w.identity_digest join concert_product_owner_scopes s on s.instance_id=w.producer_instance and s.scope_id=w.scope_id where w.producer_instance=$1 and w.identity_digest=$2 and w.binding_digest=$3 and w.native_identity=$4 and t.identity=$4 and t.binding_digest=$3 and t.confirmed_at is not null and s.binding=$5 and s.phase='ROOT_FINISHED' and s.delivery_secret_hash is not null)")
                    .bind(self.instance).bind(digest.as_slice()).bind(binding_digest.as_slice())
                    .bind(sqlx::types::Json(identity)).bind(sqlx::types::Json(binding)).fetch_one(&mut *tx).await.map_err(unavailable)?;
                if exact {
                    let mut evidence = Sha256::new();
                    evidence.update(b"MCR Gift committed TX witness v2\0");
                    evidence.update(binding_digest);
                    evidence.update(digest);
                    let receipt:Option<serde_json::Value> = sqlx::query_scalar("select receipt_snapshot from concert_gift_receipt_witnesses where producer_instance=$1 and identity_digest=$2")
                        .bind(self.instance).bind(digest.as_slice()).fetch_optional(&mut *tx).await.map_err(unavailable)?;
                    if let Some(receipt) = receipt {
                        evidence.update(b"RECEIPT\0");
                        evidence.update(serde_json::to_vec(&receipt).map_err(unavailable)?);
                    } else {
                        // An admission/policy/read TX is not a successful command.
                        evidence.update(b"LOCAL_TRANSACTION\0");
                    }
                    result.status = TransactionStatus::Committed;
                    result.receipt_evidence = evidence.finalize().to_vec();
                }
            }
            _ => (),
        }
        tx.commit().await.map_err(unavailable)?;
        Ok(result)
    }
}
