use super::proto;
use base64ct::{Base64, Encoding};
use prost::Message;
use std::collections::BTreeSet;
use tonic::Status;
use uuid::Uuid;

pub(super) fn uuid(value: &str) -> Result<Uuid, Status> {
    let id = Uuid::parse_str(value).map_err(|_| invalid())?;
    if id.is_nil() || id.to_string() != value {
        return Err(invalid());
    }
    Ok(id)
}
pub(super) fn invalid() -> Status {
    Status::invalid_argument("Invalid owner authority input")
}
pub(super) fn denied() -> Status {
    Status::permission_denied("Owner authority denied")
}
pub(super) fn unavailable() -> Status {
    Status::unavailable("Owner authority unavailable")
}
pub(super) use platform::actor_delegation::owner_binding::{secret_hash, secret_matches};
pub(super) fn root_binding(
    binding: &proto::GuardBinding,
) -> Result<platform::actor_delegation::owner_binding::OwnerBinding, Status> {
    Ok(platform::actor_delegation::owner_binding::OwnerBinding {
        actor_id: uuid(&binding.actor_id)?,
        event_id: uuid(&binding.event_id)?,
        session_family_id: uuid(&binding.session_family_id)?,
        request_id: uuid(&binding.request_id)?,
        operation: binding.operation.clone(),
        expires_unix_micros: binding.expires_unix_micros,
        parent_digest: binding
            .parent_digest
            .as_slice()
            .try_into()
            .map_err(|_| invalid())?,
        parent_body_digest: binding
            .parent_body_digest
            .as_slice()
            .try_into()
            .map_err(|_| invalid())?,
        parent_method: binding.parent_method.clone(),
        http_method: binding.http_method.clone(),
        http_target: binding.http_target.clone(),
        body_digest: binding
            .body_digest
            .as_slice()
            .try_into()
            .map_err(|_| invalid())?,
    })
}
pub(super) fn binding_digest(binding: &proto::GuardBinding) -> Result<[u8; 32], Status> {
    platform::actor_delegation::owner_binding::binding_digest(&root_binding(binding)?)
}
pub(super) fn identity_digest(value: &proto::TransactionIdentity) -> Result<[u8; 32], Status> {
    let producer = proto::Producer::try_from(value.producer)
        .map_err(|_| invalid())?
        .as_str_name();
    platform::actor_delegation::owner_binding::identity_digest(
        producer,
        &value.system_identifier,
        value.database_oid,
        &value.full_xid,
        value.backend_pid,
        value.backend_started_unix_micros,
        &value
            .effect_binding_digest
            .as_slice()
            .try_into()
            .map_err(|_| invalid())?,
        uuid(&value.producer_instance_id)?,
        &value.settlement_reference,
    )
}
pub(super) fn constant_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != 32 || right.len() != 32 {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0u8, |difference, (a, b)| difference | (a ^ b))
        == 0
}
pub(super) fn secret() -> Vec<u8> {
    let mut bytes = Uuid::new_v4().as_bytes().to_vec();
    bytes.extend_from_slice(Uuid::new_v4().as_bytes());
    bytes
}
pub(super) fn binding(binding: &proto::GuardBinding) -> Result<Vec<Uuid>, Status> {
    root_binding(binding)?.validate()?;
    let actor = uuid(&binding.actor_id)?;
    uuid(&binding.event_id)?;
    uuid(&binding.session_family_id)?;
    uuid(&binding.request_id)?;
    if binding.encoded_len() > 8192
        || binding.subject_ids.len() > 100
        || binding.expires_unix_micros <= 0
        || chrono::DateTime::from_timestamp_micros(binding.expires_unix_micros).is_none()
        || binding.parent_digest.len() != 32
        || binding.parent_body_digest.len() != 32
        || binding.body_digest.len() != 32
        || binding.operation.is_empty()
        || binding.operation.len() > 128
        || !binding
            .operation
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.:-".contains(&b))
        || !binding.parent_method.starts_with('/')
        || binding.parent_method.len() > 256
        || !binding.parent_method.is_ascii()
        || binding.parent_method.bytes().any(|b| b.is_ascii_control())
        || !(3..=10).contains(&binding.http_method.len())
        || !binding.http_method.bytes().all(|b| b.is_ascii_uppercase())
        || !binding.http_target.starts_with('/')
        || binding.http_target.len() > 4096
        || !binding.http_target.is_ascii()
        || binding.http_target.bytes().any(|b| b.is_ascii_control())
    {
        return Err(invalid());
    }
    let mut subjects = BTreeSet::new();
    for value in &binding.subject_ids {
        if !subjects.insert(uuid(value)?) {
            return Err(invalid());
        }
    }
    subjects.insert(actor);
    Ok(subjects.into_iter().collect())
}
pub(super) fn live(binding: &proto::GuardBinding) -> Result<(), Status> {
    if binding.expires_unix_micros <= chrono::Utc::now().timestamp_micros() {
        Err(Status::deadline_exceeded("Owner guard expired"))
    } else {
        Ok(())
    }
}
pub(super) fn scope(
    scope: &proto::ProducerScope,
    binding: &proto::GuardBinding,
) -> Result<(), Status> {
    uuid(&scope.instance_id)?;
    uuid(&scope.scope_id)?;
    if !constant_eq(&scope.binding_digest, &binding_digest(binding)?) {
        return Err(denied());
    }
    Ok(())
}
pub(super) fn view(view: &proto::GuardView, binding: &proto::GuardBinding) -> Result<Uuid, Status> {
    let id = uuid(&view.guard_id)?;
    if view.view_secret.len() != 32
        || view.expires_unix_micros != binding.expires_unix_micros
        || !constant_eq(&view.binding_digest, &binding_digest(binding)?)
    {
        return Err(denied());
    }
    Ok(id)
}
pub(super) fn identity(value: &proto::TransactionIdentity) -> Result<(), Status> {
    let xid = value.full_xid.parse::<u64>().map_err(|_| invalid())?;
    let system = value
        .system_identifier
        .parse::<u64>()
        .map_err(|_| invalid())?;
    uuid(&value.producer_instance_id)?;
    if !matches!(
        proto::Producer::try_from(value.producer),
        Ok(proto::Producer::Concert
            | proto::Producer::Post
            | proto::Producer::Profile
            | proto::Producer::Notification)
    ) || xid < 3
        || value.full_xid != xid.to_string()
        || system == 0
        || value.system_identifier != system.to_string()
        || value.database_oid == 0
        || value.backend_pid <= 0
        || value.backend_started_unix_micros <= 0
        || value.effect_binding_digest.len() != 32
        || value.settlement_reference.is_empty()
        || value.settlement_reference.len() > 4096
        || value.encoded_len() > 8192
    {
        return Err(invalid());
    }
    Ok(())
}
pub(super) fn json<T: Message>(message: &T) -> Result<serde_json::Value, Status> {
    Ok(serde_json::Value::String(Base64::encode_string(
        &message.encode_to_vec(),
    )))
}
pub(super) fn decode<T: Message + Default>(value: serde_json::Value) -> Result<T, Status> {
    let bytes =
        Base64::decode_vec(value.as_str().ok_or_else(unavailable)?).map_err(|_| unavailable())?;
    T::decode(bytes.as_slice()).map_err(|_| unavailable())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ledger_encoding_roundtrips_and_rejects_corruption() {
        let message = proto::GuardView {
            guard_id: Uuid::new_v4().to_string(),
            binding_digest: vec![4; 32],
            view_secret: vec![5; 32],
            expires_unix_micros: 123,
        };
        assert_eq!(
            decode::<proto::GuardView>(json(&message).unwrap()).unwrap(),
            message
        );
        assert!(decode::<proto::GuardView>(serde_json::json!([1, 2, 3])).is_err());
        assert!(decode::<proto::GuardView>(serde_json::json!("%%%%")).is_err());
    }
    #[test]
    fn capabilities_and_uuid_shapes_fail_closed() {
        let secret = secret();
        let hash = secret_hash(&secret).unwrap();
        assert!(secret_matches(&secret, &hash));
        let mut tampered = secret.clone();
        tampered[0] ^= 1;
        assert!(!secret_matches(&tampered, &hash));
        assert!(!secret_matches(&secret[..31], &hash));
        assert!(uuid(&Uuid::nil().to_string()).is_err());
        assert!(uuid(&Uuid::new_v4().simple().to_string()).is_err());
    }
}
