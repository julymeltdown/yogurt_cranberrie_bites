//! Exact canonical SDK to durable producer-ledger mapping. No actor observation
//! or request DTO is promoted to owner authority by these conversions.
use super::{canonical_uuid, scope_ledger::*};
use crate::adapters::out::relationship_authority::guarded::{
    proto, ChildRecoveryReference, GuardBinding, ScopeLifecycle,
};
use platform::{actor_delegation::owner_binding::OwnerBinding, grpc_auth::Workload};
use tonic::Status;

fn invalid() -> Status {
    Status::invalid_argument("Invalid canonical producer lifecycle binding")
}
pub(super) fn key(value: &proto::ProducerScope) -> Result<ScopeKey, Status> {
    Ok(ScopeKey {
        instance: canonical_uuid(&value.instance_id)?,
        scope: canonical_uuid(&value.scope_id)?,
        binding_digest: value
            .binding_digest
            .as_slice()
            .try_into()
            .map_err(|_| invalid())?,
    })
}
pub(super) fn binding(value: &proto::GuardBinding) -> Result<OwnerBinding, Status> {
    let result = OwnerBinding {
        actor_id: canonical_uuid(&value.actor_id)?,
        event_id: canonical_uuid(&value.event_id)?,
        session_family_id: canonical_uuid(&value.session_family_id)?,
        request_id: canonical_uuid(&value.request_id)?,
        operation: value.operation.clone(),
        expires_unix_micros: value.expires_unix_micros,
        parent_digest: value
            .parent_digest
            .as_slice()
            .try_into()
            .map_err(|_| invalid())?,
        parent_body_digest: value
            .parent_body_digest
            .as_slice()
            .try_into()
            .map_err(|_| invalid())?,
        parent_method: value.parent_method.clone(),
        http_method: value.http_method.clone(),
        http_target: value.http_target.clone(),
        body_digest: value
            .body_digest
            .as_slice()
            .try_into()
            .map_err(|_| invalid())?,
    };
    result.validate()?;
    Ok(result)
}
pub(super) fn transaction(
    value: &proto::TransactionIdentity,
) -> Result<ProducerTransaction, Status> {
    let result = ProducerTransaction {
        producer: match proto::Producer::try_from(value.producer).map_err(|_| invalid())? {
            proto::Producer::Concert => "CONCERT",
            proto::Producer::Post => "POST",
            proto::Producer::Profile => "PROFILE",
            _ => return Err(invalid()),
        }
        .into(),
        system_identifier: value.system_identifier.clone(),
        database_oid: value.database_oid,
        full_xid: value.full_xid.clone(),
        backend_pid: value.backend_pid,
        backend_started_unix_micros: value.backend_started_unix_micros,
        effect_binding_digest: value
            .effect_binding_digest
            .as_slice()
            .try_into()
            .map_err(|_| invalid())?,
        producer_instance: canonical_uuid(&value.producer_instance_id)?,
        settlement_reference: value.settlement_reference.clone(),
    };
    result.digest()?;
    Ok(result)
}

#[async_trait::async_trait]
impl ScopeLifecycle for PgScopeLedger {
    async fn register(
        &self,
        scope: &proto::ProducerScope,
        actual: &GuardBinding,
    ) -> Result<(), Status> {
        PgScopeLedger::register(self, &key(scope)?, &binding(&actual.wire([]))?).await
    }
    async fn register_transaction(
        &self,
        scope: &proto::ProducerScope,
        identity: &proto::TransactionIdentity,
    ) -> Result<(), Status> {
        PgScopeLedger::register_transaction(self, &key(scope)?, &transaction(identity)?).await
    }
    async fn record_tracking(
        &self,
        scope: &proto::ProducerScope,
        identity: &proto::TransactionIdentity,
        owner: Workload,
        registration: &str,
    ) -> Result<(), Status> {
        PgScopeLedger::record_tracking(
            self,
            &key(scope)?,
            &transaction(identity)?,
            owner,
            canonical_uuid(registration)?,
        )
        .await
    }
    async fn confirm_tracking(
        &self,
        scope: &proto::ProducerScope,
        identity: &proto::TransactionIdentity,
        auth: &str,
        friend: &str,
    ) -> Result<(), Status> {
        PgScopeLedger::confirm_tracking(
            self,
            &key(scope)?,
            &transaction(identity)?,
            canonical_uuid(auth)?,
            canonical_uuid(friend)?,
        )
        .await
    }
    async fn mark_serialized(&self, scope: &proto::ProducerScope) -> Result<(), Status> {
        PgScopeLedger::mark_serialized(self, &key(scope)?).await
    }
    async fn register_child_intent(
        &self,
        scope: &proto::ProducerScope,
        reference: &ChildRecoveryReference,
    ) -> Result<(), Status> {
        PgScopeLedger::register_child_intent(self, &key(scope)?, reference).await
    }
    async fn record_child_completed(
        &self,
        scope: &proto::ProducerScope,
        reference: &ChildRecoveryReference,
    ) -> Result<(), Status> {
        PgScopeLedger::record_child_completed(self, &key(scope)?, reference).await
    }
    async fn settle_child_intents(&self, scope: &proto::ProducerScope) -> Result<(), Status> {
        PgScopeLedger::settle_child_intents(self, &key(scope)?).await
    }
    async fn seal(&self, scope: &proto::ProducerScope) -> Result<(), Status> {
        PgScopeLedger::seal(self, &key(scope)?).await
    }
    async fn finish_root_work(&self, scope: &proto::ProducerScope) -> Result<(), Status> {
        PgScopeLedger::finish_root_work(self, &key(scope)?).await
    }
}
