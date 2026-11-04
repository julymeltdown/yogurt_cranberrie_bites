//! Terminal recovery for actual registered C03 Concert transactions only.
//! A native COMMIT is insufficient without its immutable producer witness.
use super::coordinator::{profile_effect_digest, CanonicalProfileVersion};
use crate::{
    adapters::out::relationship_authority::guarded::{proto, wire_binding_digest},
    product_rpc::{
        scope_ledger::ProducerTransaction,
        settlement::{NativeTransactionResolution, NativeTransactionResolver},
    },
};
use platform::actor_delegation::owner_binding::OwnerBinding;
use prost::Message;
use sha2::{Digest, Sha256};
use sqlx::{postgres::PgRow, PgPool, Row};
use tonic::Status;
use uuid::Uuid;

fn unavailable(_: impl std::fmt::Debug) -> Status {
    Status::unavailable("Participant native settlement unresolved")
}
fn unknown() -> NativeTransactionResolution {
    NativeTransactionResolution {
        status: proto::TransactionStatus::Unknown,
        receipt_evidence: Vec::new(),
    }
}
fn bytes(row: &PgRow, key: &str) -> Result<Vec<u8>, Status> {
    row.try_get(key).map_err(unavailable)
}
fn matches_request(row: &PgRow, binding: &OwnerBinding) -> Result<bool, Status> {
    Ok(
        row.try_get::<Uuid, _>("actor_id").map_err(unavailable)? == binding.actor_id
            && row.try_get::<Uuid, _>("event_id").map_err(unavailable)? == binding.event_id
            && row
                .try_get::<Uuid, _>("session_family_id")
                .map_err(unavailable)?
                == binding.session_family_id
            && bytes(row, "parent_digest")? == binding.parent_digest
            && bytes(row, "body_digest")? == binding.body_digest
            && bytes(row, "guard_binding_digest")? == binding.digest()?,
    )
}
fn matches_native(row: &PgRow, identity: &ProducerTransaction) -> Result<bool, Status> {
    Ok(row
        .try_get::<String, _>("concert_system_identifier")
        .map_err(unavailable)?
        == identity.system_identifier
        && row
            .try_get::<i64, _>("concert_database_oid")
            .map_err(unavailable)?
            == i64::from(identity.database_oid)
        && row
            .try_get::<String, _>("concert_full_xid")
            .map_err(unavailable)?
            == identity.full_xid
        && row
            .try_get::<i32, _>("concert_backend_pid")
            .map_err(unavailable)?
            == identity.backend_pid
        && row
            .try_get::<chrono::DateTime<chrono::Utc>, _>("concert_backend_started")
            .map_err(unavailable)?
            .timestamp_micros()
            == identity.backend_started_unix_micros)
}
fn release(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
/// Stable instance identity is the same configured identity used by the root
/// registry. This adapter creates no guard, registration, decision or receipt.
pub struct ParticipantNativeResolver {
    pool: PgPool,
    instance: Uuid,
}
impl ParticipantNativeResolver {
    pub fn new(pool: PgPool, instance: Uuid) -> Result<Self, Status> {
        if instance.is_nil() {
            return Err(Status::invalid_argument(
                "Invalid participant producer instance",
            ));
        }
        Ok(Self { pool, instance })
    }
    async fn witness(
        &self,
        binding: &OwnerBinding,
        identity: &ProducerTransaction,
        scope: Uuid,
    ) -> Result<Option<(&'static str, i64)>, Status> {
        if let Some(row) = sqlx::query(
            "SELECT * FROM public.concert_participant_effect_witnesses WHERE operation_id=$1",
        )
        .bind(binding.request_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(unavailable)?
        {
            let kind: String = row.try_get("mutation_kind").map_err(unavailable)?;
            let method = match kind.as_str() {
                "AVATAR" => "SetAvatar",
                "PREFERENCES" => "UpdatePreferences",
                _ => return Ok(None),
            };
            let digest: String = row.try_get("release_digest").map_err(unavailable)?;
            let expected: i64 = row.try_get("expected_revision").map_err(unavailable)?;
            let committed: i64 = row.try_get("committed_revision").map_err(unavailable)?;
            let mut effect = Sha256::new();
            effect.update(b"mcr participant avatar effect v1\0");
            effect.update(binding.digest()?);
            effect.update(digest.as_bytes());
            effect.update(binding.body_digest);
            let effect: [u8; 32] = effect.finalize().into();
            if matches_request(&row, binding)?
                && matches_native(&row, identity)?
                && row
                    .try_get::<Uuid, _>("producer_instance_id")
                    .map_err(unavailable)?
                    == self.instance
                && row.try_get::<Uuid, _>("scope_id").map_err(unavailable)? == scope
                && bytes(&row, "identity_digest")? == identity.digest()?
                && bytes(&row, "effect_binding_digest")? == identity.effect_binding_digest
                && identity.effect_binding_digest == effect
                && release(&digest)
                && expected > 0
                && committed <= 9_007_199_254_740_991
                && expected.checked_add(1) == Some(committed)
                && binding.parent_method
                    == format!("/concert.participant.v1.ConcertParticipantService/{method}")
            {
                return Ok(Some((
                    if kind == "AVATAR" {
                        "AVATAR"
                    } else {
                        "PREFERENCES"
                    },
                    committed,
                )));
            }
            return Ok(None);
        }
        // 0027's immutable decision is written by the actual coordinator in the
        // same transaction as its locked participant CAS; expiry never decides.
        let Some(row) = sqlx::query("SELECT * FROM public.concert_participant_profile_operations WHERE operation_id=$1 AND decision='COMMIT'")
            .bind(binding.request_id).fetch_optional(&self.pool).await.map_err(unavailable)?
        else { return Ok(None) };
        let owner = proto::GuardBinding::decode(bytes(&row, "owner_binding_proto")?.as_slice())
            .map_err(unavailable)?;
        let version = CanonicalProfileVersion {
            epoch: row.try_get("profile_epoch").map_err(unavailable)?,
            revision: concert_domain::Revision::new(
                row.try_get("profile_revision").map_err(unavailable)?,
            )
            .map_err(unavailable)?,
        };
        let expected: i64 = row.try_get("expected_revision").map_err(unavailable)?;
        let committed: i64 = row.try_get("committed_revision").map_err(unavailable)?;
        let digest: String = row.try_get("release_digest").map_err(unavailable)?;
        if matches_request(&row, binding)?
            && matches_native(&row, identity)?
            && wire_binding_digest(&owner)? == binding.digest()?
            && bytes(&row, "profile_effect_digest")? == identity.effect_binding_digest
            && profile_effect_digest(
                binding.digest()?,
                binding.request_id,
                version,
                binding.body_digest,
            ) == identity.effect_binding_digest
            && row
                .try_get::<String, _>("profile_prepared_gid")
                .map_err(unavailable)?
                == format!("mcr_sp_{}", binding.request_id.simple())
            && !version.epoch.is_nil()
            && release(&digest)
            && expected > 0
            && expected.checked_add(1) == Some(committed)
            && committed <= 9_007_199_254_740_991
            && binding.parent_method
                == "/concert.participant.v1.ConcertParticipantService/UpdatePreferences"
        {
            Ok(Some(("PROFILE_PARTICIPANT", committed)))
        } else {
            Ok(None)
        }
    }
}
#[async_trait::async_trait]
impl NativeTransactionResolver for ParticipantNativeResolver {
    async fn resolve(
        &self,
        binding: &OwnerBinding,
        identity: &ProducerTransaction,
    ) -> Result<NativeTransactionResolution, Status> {
        let digest = binding.digest()?;
        let native_digest = identity.digest()?;
        if identity.producer != "CONCERT"
            || identity.producer_instance != self.instance
            || identity.settlement_reference.as_slice() != binding.request_id.as_bytes()
            || binding.http_method
                != if binding.operation == "setConcertAvatar" {
                    "PUT"
                } else {
                    "PATCH"
                }
            || !matches!(
                binding.operation.as_str(),
                "setConcertAvatar" | "updateConcertParticipant"
            )
            || binding.http_target
                != format!(
                    "/api/concert/events/{}/me{}",
                    binding.event_id,
                    if binding.operation == "setConcertAvatar" {
                        "/avatar"
                    } else {
                        ""
                    }
                )
            || binding.parent_method
                != format!(
                    "/concert.participant.v1.ConcertParticipantService/{}",
                    if binding.operation == "setConcertAvatar" {
                        "SetAvatar"
                    } else {
                        "UpdatePreferences"
                    }
                )
        {
            return Ok(unknown());
        }
        // Both full immutable identity and its distinct root guard binding must
        // already exist. A caller cannot rebind a known XID to a new request.
        let registrations = sqlx::query("SELECT s.scope_id,s.binding,t.identity,t.confirmed_at IS NOT NULL AS confirmed FROM public.concert_product_owner_transactions t JOIN public.concert_product_owner_scopes s USING(instance_id,scope_id,binding_digest) WHERE t.instance_id=$1 AND t.identity_digest=$2 AND s.binding_digest=$3 LIMIT 2")
            .bind(self.instance).bind(native_digest.as_slice()).bind(digest.as_slice())
            .fetch_all(&self.pool).await.map_err(unavailable)?;
        if registrations.len() != 1 {
            return Ok(unknown());
        }
        let row = &registrations[0];
        let stored: OwnerBinding =
            serde_json::from_value(row.try_get("binding").map_err(unavailable)?)
                .map_err(unavailable)?;
        let tracked: ProducerTransaction =
            serde_json::from_value(row.try_get("identity").map_err(unavailable)?)
                .map_err(unavailable)?;
        if stored.digest()? != digest || &tracked != identity {
            return Ok(unknown());
        }
        let actual: (String, i64) = sqlx::query_as("SELECT s.system_identifier::text,d.oid::bigint FROM pg_control_system() s CROSS JOIN pg_database d WHERE d.datname=current_database()")
            .fetch_one(&self.pool).await.map_err(unavailable)?;
        if actual
            != (
                identity.system_identifier.clone(),
                i64::from(identity.database_oid),
            )
        {
            return Ok(unknown());
        }
        let native: Option<String> = sqlx::query_scalar("SELECT pg_xact_status($1::text::xid8)")
            .bind(&identity.full_xid)
            .fetch_one(&self.pool)
            .await
            .map_err(unavailable)?;
        let mut result = unknown();
        result.status = match native.as_deref() {
            Some("aborted") => proto::TransactionStatus::Aborted,
            Some("in progress") => proto::TransactionStatus::InProgress,
            Some("committed") => {
                if !row.try_get::<bool, _>("confirmed").map_err(unavailable)? {
                    return Ok(unknown());
                }
                let scope: Uuid = row.try_get("scope_id").map_err(unavailable)?;
                if let Some((kind, revision)) = self.witness(binding, identity, scope).await? {
                    result.receipt_evidence = serde_json::to_vec(&serde_json::json!({
                        "kind":"PARTICIPANT_NATIVE_EFFECT_V1","operation_id":binding.request_id,
                        "mutation_kind":kind,"committed_revision":revision,
                        "identity_digest":native_digest
                    }))
                    .map_err(unavailable)?;
                    proto::TransactionStatus::Committed
                } else {
                    proto::TransactionStatus::Unknown
                }
            }
            _ => proto::TransactionStatus::Unknown,
        };
        Ok(result)
    }
}
