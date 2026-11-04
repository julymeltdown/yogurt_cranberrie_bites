//! Same-transaction completion evidence. No successful outcome is inferred from
//! a committed xid alone, and no product/private value enters this module.
use super::protocol::{route, SocialOperation};
use crate::{
    adapters::out::relationship_authority::guarded::{
        OwnerGuardScope, ProducerTransaction as NativeIdentity,
    },
    product_rpc::{
        scope_ledger::ProducerTransaction,
        settlement::{NativeTransactionResolution, NativeTransactionResolver},
    },
    relationship::social::{SocialEffect, SocialTransactionHooks},
};
use concert_domain::relationship::RelationshipError;
use platform::actor_delegation::owner_binding::OwnerBinding;
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool, Row};
use std::sync::Arc;
use tonic::Status;
use uuid::Uuid;

fn unavailable(_: impl std::fmt::Display) -> Status {
    Status::unavailable("Social settlement unavailable")
}
fn relationship(_: impl std::fmt::Display) -> RelationshipError {
    RelationshipError::Unavailable
}
fn effect_digest(binding: &OwnerBinding) -> Result<[u8; 32], Status> {
    let mut hash = Sha256::new();
    hash.update(b"MCR social transaction v1\0");
    hash.update(binding.digest()?);
    Ok(hash.finalize().into())
}
fn ledger_identity(identity: &NativeIdentity) -> ProducerTransaction {
    ProducerTransaction {
        producer: "CONCERT".into(),
        system_identifier: identity.system_identifier.clone(),
        database_oid: identity.database_oid,
        full_xid: identity.full_xid.clone(),
        backend_pid: identity.backend_pid,
        backend_started_unix_micros: identity.backend_started.timestamp_micros(),
        effect_binding_digest: identity.effect_binding_digest,
        producer_instance: identity.producer_instance_id,
        settlement_reference: identity.settlement_reference.clone(),
    }
}
/// Constructed only by the trusted producer with its actual acquired scope.
/// It neither acquires another scope nor completes the root delivery capability.
pub struct SocialWitness {
    scope: Arc<OwnerGuardScope>,
    key: Option<Uuid>,
}
impl SocialWitness {
    pub fn new(scope: Arc<OwnerGuardScope>, key: Option<Uuid>) -> Self {
        Self { scope, key }
    }
    async fn identity(&self, c: &mut PgConnection) -> Result<NativeIdentity, RelationshipError> {
        let producer = self.scope.producer_scope();
        let instance = Uuid::parse_str(&producer.instance_id).map_err(relationship)?;
        let binding = self.scope.binding().owner_binding();
        NativeIdentity::capture_concert_in(
            c,
            effect_digest(&binding).map_err(relationship)?,
            instance,
            binding.request_id.as_bytes().to_vec(),
        )
        .await
        .map_err(relationship)
    }
}
#[async_trait::async_trait]
impl SocialTransactionHooks for SocialWitness {
    async fn register(&self, c: &mut PgConnection) -> Result<(), RelationshipError> {
        self.scope.binding().check().map_err(relationship)?;
        let identity = self.identity(c).await?;
        // Exact full identity is deduplicated only after both actual owner Track
        // acknowledgements and durable confirmation by OwnerGuardScope.
        self.scope
            .track_transaction(identity)
            .await
            .map_err(relationship)
    }
    async fn succeeded(
        &self,
        c: &mut PgConnection,
        effect: SocialEffect,
    ) -> Result<(), RelationshipError> {
        let binding = self.scope.binding().owner_binding();
        self.scope.binding().check().map_err(relationship)?;
        let operation = route(binding.event_id, &binding.http_method, &binding.http_target)
            .map_err(relationship)?;
        if effect.operation != binding.operation
            || effect.operation != operation.id()
            || operation.mutation() != self.key.is_some()
            || effect.resource.is_nil()
            || !(0..=concert_domain::MAX_REVISION).contains(&effect.revision)
        {
            return Err(RelationshipError::Unavailable);
        }
        let expected = match operation {
            SocialOperation::Handles
            | SocialOperation::SetHandle(_)
            | SocialOperation::RemoveHandle(_)
            | SocialOperation::Requests
            | SocialOperation::Blocks => Some(binding.actor_id),
            SocialOperation::Request(id)
            | SocialOperation::Decide(id)
            | SocialOperation::Cancel(id)
            | SocialOperation::Revoke(id)
            | SocialOperation::Reveal(id)
            | SocialOperation::Block(id)
            | SocialOperation::Unblock(id) => Some(id),
            SocialOperation::Create => None,
        };
        if expected.is_some_and(|id| id != effect.resource) {
            return Err(RelationshipError::Unavailable);
        }
        let identity = ledger_identity(&self.identity(c).await?);
        let digest = identity.digest().map_err(relationship)?;
        let producer = self.scope.producer_scope();
        let scope_id = Uuid::parse_str(&producer.scope_id).map_err(relationship)?;
        let existing=sqlx::query("select operation,resource_id,resource_revision from concert_social_effect_witnesses where producer_instance=$1 and scope_id=$2 and identity_digest=$3")
            .bind(identity.producer_instance).bind(scope_id).bind(digest.as_slice()).fetch_optional(&mut *c).await.map_err(relationship)?;
        if let Some(row) = existing {
            if row.get::<String, _>("operation") != effect.operation
                || row.get::<Uuid, _>("resource_id") != effect.resource
                || row.get::<i64, _>("resource_revision") != effect.revision
            {
                return Err(RelationshipError::Unavailable);
            }
            return Ok(());
        }
        sqlx::query("insert into concert_social_effect_witnesses(producer_instance,scope_id,identity_digest,guard_binding_digest,effect_binding_digest,request_id,actor_id,event_id,session_family_id,operation,parent_digest,body_digest,native_identity,resource_id,resource_revision,effect_kind,idempotency_key) values($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17)")
            .bind(identity.producer_instance).bind(scope_id).bind(digest.as_slice()).bind(self.scope.binding().digest().as_slice()).bind(identity.effect_binding_digest.as_slice())
            .bind(binding.request_id).bind(binding.actor_id).bind(binding.event_id).bind(binding.session_family_id).bind(effect.operation).bind(binding.parent_digest.as_slice()).bind(binding.body_digest.as_slice())
            .bind(sqlx::types::Json(&identity)).bind(effect.resource).bind(effect.revision).bind(if effect.projection {"PROJECTION"}else{"MUTATION"})
            .bind(self.key).execute(&mut *c).await.map_err(relationship)?;
        Ok(())
    }
}

pub struct SocialNativeResolver {
    pool: PgPool,
    instance: Uuid,
}
impl SocialNativeResolver {
    pub fn new(pool: PgPool, instance: Uuid) -> Result<Self, Status> {
        if instance.is_nil() {
            return Err(Status::invalid_argument("Invalid Social producer instance"));
        }
        Ok(Self { pool, instance })
    }
}
#[async_trait::async_trait]
impl NativeTransactionResolver for SocialNativeResolver {
    async fn resolve(
        &self,
        binding: &OwnerBinding,
        identity: &ProducerTransaction,
    ) -> Result<NativeTransactionResolution, Status> {
        use crate::adapters::out::relationship_authority::guarded::proto::TransactionStatus;
        let mut result = NativeTransactionResolution {
            status: TransactionStatus::Unknown,
            receipt_evidence: vec![],
        };
        let identity_digest = identity.digest()?;
        let guard_digest = binding.digest()?;
        let operation = route(binding.event_id, &binding.http_method, &binding.http_target)?;
        if identity.producer != "CONCERT"
            || identity.producer_instance != self.instance
            || binding.operation != operation.id()
            || identity.effect_binding_digest != effect_digest(binding)?
            || identity.settlement_reference != binding.request_id.as_bytes()
        {
            return Ok(result);
        }
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        sqlx::query("set local statement_timeout='1500ms'")
            .execute(&mut *tx)
            .await
            .map_err(unavailable)?;
        let actual:(String,i64)=sqlx::query_as("select system_identifier::text,(select oid::bigint from pg_database where datname=current_database()) from pg_control_system()")
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
                let witness=sqlx::query("select w.resource_id,w.resource_revision,w.effect_kind from concert_social_effect_witnesses w join concert_product_owner_transactions t on t.instance_id=w.producer_instance and t.scope_id=w.scope_id and t.identity_digest=w.identity_digest join concert_product_owner_scopes s on s.instance_id=w.producer_instance and s.scope_id=w.scope_id where w.producer_instance=$1 and w.identity_digest=$2 and w.guard_binding_digest=$3 and w.native_identity=$4 and s.binding=$5 and t.identity=$4 and t.binding_digest=$3 and t.confirmed_at is not null and s.phase='ROOT_FINISHED' and s.delivery_secret_hash is not null and w.operation=$6 and w.effect_binding_digest=$7")
                    .bind(self.instance).bind(identity_digest.as_slice()).bind(guard_digest.as_slice()).bind(sqlx::types::Json(identity)).bind(sqlx::types::Json(binding)).bind(&binding.operation).bind(identity.effect_binding_digest.as_slice())
                    .fetch_optional(&mut *tx).await.map_err(unavailable)?;
                if let Some(w) = witness {
                    let mut proof = Sha256::new();
                    proof.update(b"MCR social committed witness v1\0");
                    proof.update(guard_digest);
                    proof.update(identity_digest);
                    proof.update(w.get::<Uuid, _>("resource_id").as_bytes());
                    proof.update(w.get::<i64, _>("resource_revision").to_be_bytes());
                    proof.update(w.get::<String, _>("effect_kind").as_bytes());
                    result.status = TransactionStatus::Committed;
                    result.receipt_evidence = proof.finalize().to_vec();
                }
            }
            _ => {}
        }
        tx.commit().await.map_err(unavailable)?;
        Ok(result)
    }
}
