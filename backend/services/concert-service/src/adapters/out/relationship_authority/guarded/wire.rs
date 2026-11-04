use super::{contract::*, proto};
use platform::actor_delegation::owner_binding::OwnerBinding;
use tonic::Status;
use uuid::Uuid;
fn invalid() -> Status {
    Status::invalid_argument("Invalid canonical authority wire binding")
}
pub fn canonical_uuid(value: &str) -> Result<Uuid, Status> {
    let id = Uuid::parse_str(value).map_err(|_| invalid())?;
    if id.is_nil() || id.to_string() != value {
        return Err(invalid());
    }
    Ok(id)
}
pub fn owner_binding(value: &proto::GuardBinding) -> Result<OwnerBinding, Status> {
    let binding = OwnerBinding {
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
    binding.validate()?;
    let mut subjects = std::collections::BTreeSet::new();
    for raw in &value.subject_ids {
        let id = canonical_uuid(raw)?;
        if id != binding.actor_id && !subjects.insert(id) {
            return Err(invalid());
        }
    }
    if subjects.len() > MAX_SUBJECTS || value.subject_ids.len() > MAX_SUBJECTS + 1 {
        return Err(invalid());
    }
    Ok(binding)
}
/// Pure recovery-safe fingerprint: expiry is shape-checked but not renewed.
pub fn wire_binding_digest(value: &proto::GuardBinding) -> Result<[u8; 32], Status> {
    owner_binding(value)?.digest()
}
pub(super) fn revision(
    value: Option<&proto::OwnerRevision>,
) -> Result<super::super::OwnerRevision, Status> {
    let value = value.ok_or_else(invalid)?;
    let revision = super::super::OwnerRevision {
        epoch: canonical_uuid(&value.epoch)?,
        revision: value.revision,
    };
    revision.validate().map_err(|_| invalid())
}
pub(super) fn check_view(view: &proto::GuardView, binding: &GuardBinding) -> Result<(), Status> {
    canonical_uuid(&view.guard_id)?;
    if view.binding_digest.as_slice() != binding.digest()
        || view.view_secret.len() != 32
        || view.expires_unix_micros <= chrono::Utc::now().timestamp_micros()
        || view.expires_unix_micros > binding.deadline().expires_micros()
    {
        return Err(invalid());
    }
    Ok(())
}
impl ProducerTransaction {
    pub fn wire(&self) -> Result<proto::TransactionIdentity, Status> {
        platform::actor_delegation::owner_binding::identity_digest(
            self.producer.name(),
            &self.system_identifier,
            self.database_oid,
            &self.full_xid,
            self.backend_pid,
            self.backend_started.timestamp_micros(),
            &self.effect_binding_digest,
            self.producer_instance_id,
            &self.settlement_reference,
        )?;
        Ok(proto::TransactionIdentity {
            producer: match self.producer {
                ProducerKind::Concert => 1,
                ProducerKind::Post => 2,
                ProducerKind::Profile => 3,
                ProducerKind::Notification => 4,
            },
            full_xid: self.full_xid.clone(),
            backend_pid: self.backend_pid,
            backend_started_unix_micros: self.backend_started.timestamp_micros(),
            effect_binding_digest: self.effect_binding_digest.to_vec(),
            system_identifier: self.system_identifier.clone(),
            database_oid: self.database_oid,
            settlement_reference: self.settlement_reference.clone(),
            producer_instance_id: self.producer_instance_id.to_string(),
        })
    }
    pub async fn capture_concert_in(
        connection: &mut sqlx::PgConnection,
        effect_binding_digest: [u8; 32],
        producer_instance_id: Uuid,
        settlement_reference: Vec<u8>,
    ) -> Result<Self, Status> {
        use sqlx::Row;
        let row=sqlx::query("select (pg_control_system()).system_identifier::text system_identifier,(select oid::bigint from pg_database where datname=current_database()) database_oid,pg_current_xact_id()::text full_xid,pg_backend_pid() backend_pid,(select backend_start from pg_stat_activity where pid=pg_backend_pid()) backend_started")
            .fetch_one(&mut *connection).await.map_err(unavailable)?;
        let value = Self {
            producer: ProducerKind::Concert,
            system_identifier: row.try_get("system_identifier").map_err(unavailable)?,
            database_oid: u32::try_from(
                row.try_get::<i64, _>("database_oid").map_err(unavailable)?,
            )
            .map_err(unavailable)?,
            full_xid: row.try_get("full_xid").map_err(unavailable)?,
            backend_pid: row.try_get("backend_pid").map_err(unavailable)?,
            backend_started: row.try_get("backend_started").map_err(unavailable)?,
            effect_binding_digest,
            producer_instance_id,
            settlement_reference,
        };
        value.wire()?;
        Ok(value)
    }
}
impl ProducerKind {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Concert => "CONCERT",
            Self::Post => "POST",
            Self::Profile => "PROFILE",
            Self::Notification => "NOTIFICATION",
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn binding() -> proto::GuardBinding {
        let event = Uuid::from_u128(2);
        proto::GuardBinding {
            actor_id: Uuid::from_u128(1).to_string(),
            event_id: event.to_string(),
            session_family_id: Uuid::from_u128(3).to_string(),
            request_id: Uuid::from_u128(4).to_string(),
            operation: "listConcertFeed".into(),
            subject_ids: vec![],
            expires_unix_micros: 1_000_000,
            parent_digest: vec![1; 32],
            parent_body_digest: vec![2; 32],
            parent_method: "/concert.product.v1.ProductService/Dispatch".into(),
            http_method: "GET".into(),
            http_target: format!("/api/concert/events/{event}/feed"),
            body_digest: vec![3; 32],
        }
    }
    #[test]
    fn wire_fingerprint_preserves_expired_recovery_binding_and_enrollment_shape() {
        let root = binding();
        let digest = wire_binding_digest(&root).unwrap();
        let mut enrolled = root.clone();
        enrolled.subject_ids.push(Uuid::from_u128(5).to_string());
        assert_eq!(wire_binding_digest(&enrolled).unwrap(), digest);
        enrolled.subject_ids.push(Uuid::from_u128(5).to_string());
        assert!(wire_binding_digest(&enrolled).is_err());
        let mut wrong = root;
        wrong.body_digest[0] ^= 1;
        assert_ne!(wire_binding_digest(&wrong).unwrap(), digest);
    }
    #[test]
    fn transaction_wire_rejects_invented_or_ambiguous_native_identity() {
        let tx = ProducerTransaction {
            producer: ProducerKind::Concert,
            system_identifier: "123".into(),
            database_oid: 1,
            full_xid: "77".into(),
            backend_pid: 8,
            backend_started: chrono::DateTime::from_timestamp_micros(1_000_000).unwrap(),
            effect_binding_digest: [7; 32],
            producer_instance_id: Uuid::from_u128(8),
            settlement_reference: b"receipt".to_vec(),
        };
        let wire = tx.wire().unwrap();
        assert_eq!(wire.full_xid, "77");
        assert_eq!(wire.database_oid, 1);
        let mut wrong = tx;
        wrong.full_xid = "077".into();
        assert!(wrong.wire().is_err());
    }
}
