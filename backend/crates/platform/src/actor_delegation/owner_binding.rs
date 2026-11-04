//! Canonical binding fingerprints shared by producers and authority owners.
//! Shape validation is deliberately time-independent for outcome recovery.
//! Nothing in this module grants eligibility or substitutes for held exclusion.
use super::exchange::VerifiedParentAuthority;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tonic::Status;
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerBinding {
    pub actor_id: Uuid,
    pub event_id: Uuid,
    pub session_family_id: Uuid,
    pub request_id: Uuid,
    pub operation: String,
    pub expires_unix_micros: i64,
    pub parent_digest: [u8; 32],
    pub parent_body_digest: [u8; 32],
    pub parent_method: String,
    pub http_method: String,
    pub http_target: String,
    pub body_digest: [u8; 32],
}
fn invalid() -> Status {
    Status::invalid_argument("Invalid owner authority binding")
}
impl OwnerBinding {
    pub fn validate(&self) -> Result<(), Status> {
        if [
            self.actor_id,
            self.event_id,
            self.session_family_id,
            self.request_id,
        ]
        .iter()
        .any(Uuid::is_nil)
            || self.operation.is_empty()
            || self.operation.len() > 100
            || self.operation.chars().any(char::is_control)
            || self.expires_unix_micros <= 0
            || chrono::DateTime::from_timestamp_micros(self.expires_unix_micros).is_none()
            || !matches!(
                self.parent_method.as_str(),
                "/concert.product.v1.ProductService/Dispatch"
                    | "/concert.participant.v1.ConcertParticipantService/GetParticipant"
                    | "/concert.participant.v1.ConcertParticipantService/SetAvatar"
                    | "/concert.participant.v1.ConcertParticipantService/UpdatePreferences"
            )
            || !matches!(
                self.http_method.as_str(),
                "GET" | "POST" | "PATCH" | "PUT" | "DELETE"
            )
            || self.http_target.len() > 4096
            || self.http_target.chars().any(char::is_control)
            || !self
                .http_target
                .starts_with(&format!("/api/concert/events/{}/", self.event_id))
            || [
                self.parent_digest,
                self.parent_body_digest,
                self.body_digest,
            ]
            .contains(&[0; 32])
        {
            return Err(invalid());
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<[u8; 32], Status> {
        self.validate()?;
        let mut hash = Sha256::new();
        hash.update(b"mcr owner guard binding v1\0");
        for id in [
            self.actor_id,
            self.event_id,
            self.session_family_id,
            self.request_id,
        ] {
            hash.update(id.as_bytes());
        }
        for v in [
            &*self.operation,
            &*self.parent_method,
            &*self.http_method,
            &*self.http_target,
        ] {
            hash.update((v.len() as u64).to_be_bytes());
            hash.update(v.as_bytes());
        }
        hash.update(self.parent_digest);
        hash.update(self.parent_body_digest);
        hash.update(self.body_digest);
        hash.update(self.expires_unix_micros.to_be_bytes());
        Ok(hash.finalize().into())
    }
    /// Verify the exact admitted Gateway header/body fields rather than allowing
    /// a caller to relabel a sealed parent with another actor/event/session/path.
    pub fn matches_parent(&self, parent: &VerifiedParentAuthority) -> Result<(), Status> {
        self.validate()?;
        if self.actor_id != parent.actor()
            || self.parent_digest != parent.parent_digest()
            || self.parent_body_digest != parent.request_digest()
            || self.parent_method != parent.request().method
            || self.expires_unix_micros
                > parent
                    .expires_at()
                    .checked_mul(1_000_000)
                    .ok_or_else(invalid)?
        {
            return Err(invalid());
        }
        let message = parent.message().map_err(|_| invalid())?;
        let values = header_fields(message)?;
        for (value, expected) in values[..6].iter().zip([
            self.actor_id.to_string(),
            self.event_id.to_string(),
            self.session_family_id.to_string(),
            self.request_id.to_string(),
            self.http_method.clone(),
            self.http_target.clone(),
        ]) {
            if value.as_ref().copied().ok_or_else(invalid)? != expected.as_bytes() {
                return Err(invalid());
            }
        }
        let raw = values[6].unwrap_or_default();
        if <[u8; 32]>::from(Sha256::digest(raw)) != self.body_digest {
            return Err(invalid());
        }
        Ok(())
    }
}
/// Enrollment lists do not enter the immutable root fingerprint. Enrolling a
/// subject never authorizes the resource; actual current owners and the event
/// producer must separately permit every returned target under the held scope.
pub fn binding_digest(binding: &OwnerBinding) -> Result<[u8; 32], Status> {
    binding.digest()
}
pub fn secret_hash(secret: &[u8]) -> Result<[u8; 32], Status> {
    if secret.len() != 32 {
        return Err(invalid());
    }
    let mut hash = Sha256::new();
    hash.update(b"mcr owner guard capability v1\0");
    hash.update(secret);
    Ok(hash.finalize().into())
}
pub fn secret_matches(secret: &[u8], expected: &[u8]) -> bool {
    if expected.len() != 32 {
        return false;
    }
    let Ok(actual) = secret_hash(secret) else {
        return false;
    };
    // The public supported HMAC verifier supplies the constant-time comparison.
    let key = ring::hmac::Key::new(
        ring::hmac::HMAC_SHA256,
        b"mcr owner guard capability compare v1\0",
    );
    let expected_tag = ring::hmac::sign(&key, expected);
    ring::hmac::verify(&key, &actual, expected_tag.as_ref()).is_ok()
}
#[allow(clippy::too_many_arguments)]
pub fn identity_digest(
    producer: &str,
    system_identifier: &str,
    database_oid: u32,
    full_xid: &str,
    backend_pid: i32,
    backend_started_micros: i64,
    effect_binding_digest: &[u8; 32],
    producer_instance: Uuid,
    settlement_reference: &[u8],
) -> Result<[u8; 32], Status> {
    let canonical_decimal = |v: &str| v.parse::<u64>().is_ok_and(|n| n > 0 && n.to_string() == v);
    if !matches!(producer, "CONCERT" | "POST" | "PROFILE" | "NOTIFICATION")
        || !canonical_decimal(system_identifier)
        || !canonical_decimal(full_xid)
        || database_oid == 0
        || backend_pid <= 0
        || backend_started_micros <= 0
        || producer_instance.is_nil()
        || *effect_binding_digest == [0; 32]
        || settlement_reference.len() > 32768
    {
        return Err(invalid());
    }
    let mut hash = Sha256::new();
    hash.update(b"mcr owner producer identity v1\0");
    for value in [
        producer.as_bytes(),
        system_identifier.as_bytes(),
        &database_oid.to_be_bytes(),
        full_xid.as_bytes(),
        &backend_pid.to_be_bytes(),
        &backend_started_micros.to_be_bytes(),
        effect_binding_digest,
        producer_instance.as_bytes(),
        settlement_reference,
    ] {
        hash.update((value.len() as u64).to_be_bytes());
        hash.update(value);
    }
    Ok(hash.finalize().into())
}
fn varint(message: &[u8], at: &mut usize) -> Result<u64, Status> {
    let mut value = 0u64;
    for shift in (0..70).step_by(7) {
        let b = *message.get(*at).ok_or_else(invalid)?;
        *at += 1;
        if shift == 63 && b > 1 {
            return Err(invalid());
        }
        value |= ((b & 127) as u64) << shift;
        if b & 128 == 0 {
            return Ok(value);
        }
    }
    Err(invalid())
}
fn header_fields(message: &[u8]) -> Result<[Option<&[u8]>; 7], Status> {
    let mut values = [None; 7];
    let mut at = 0;
    while at < message.len() {
        let tag = varint(message, &mut at)?;
        let number = tag >> 3;
        let wire = tag & 7;
        if number == 0 {
            return Err(invalid());
        }
        match wire {
            2 => {
                let size = usize::try_from(varint(message, &mut at)?).map_err(|_| invalid())?;
                let end = at
                    .checked_add(size)
                    .filter(|end| *end <= message.len())
                    .ok_or_else(invalid)?;
                if (1..=7).contains(&number) {
                    let slot = &mut values[number as usize - 1];
                    if slot.is_some() {
                        return Err(invalid());
                    }
                    *slot = Some(&message[at..end]);
                }
                at = end;
            }
            0 => {
                if number <= 7 {
                    return Err(invalid());
                }
                varint(message, &mut at)?;
            }
            1 | 5 => {
                if number <= 7 {
                    return Err(invalid());
                }
                at = at
                    .checked_add(if wire == 1 { 8 } else { 4 })
                    .filter(|end| *end <= message.len())
                    .ok_or_else(invalid)?;
            }
            _ => return Err(invalid()),
        }
    }
    Ok(values)
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
            operation: "listConcertFeed".into(),
            expires_unix_micros: 1_000_000,
            parent_digest: [1; 32],
            parent_body_digest: [2; 32],
            parent_method: "/concert.product.v1.ProductService/Dispatch".into(),
            http_method: "GET".into(),
            http_target: format!("/api/concert/events/{event}/feed"),
            body_digest: Sha256::digest([]).into(),
        }
    }
    #[test]
    fn recovery_fingerprint_is_time_independent_but_request_specific() {
        let b = binding();
        let original = b.digest().unwrap();
        assert_eq!(original, b.digest().unwrap()); // deliberately expired valid shape
        for change in 0..8 {
            let mut other = b.clone();
            match change {
                0 => other.actor_id = Uuid::from_u128(9),
                1 => other.request_id = Uuid::from_u128(9),
                2 => other.session_family_id = Uuid::from_u128(9),
                3 => other.operation.push('X'),
                4 => other.http_target.push('X'),
                5 => other.body_digest[0] ^= 1,
                6 => other.parent_digest[0] ^= 1,
                _ => other.expires_unix_micros += 1,
            }
            assert_ne!(original, other.digest().unwrap());
        }
        let mut wrong = b;
        wrong.event_id = Uuid::from_u128(9);
        assert!(wrong.digest().is_err());
    }
    #[test]
    fn parser_rejects_ambiguous_truncated_and_wrong_wire_headers() {
        assert!(header_fields(&[10, 1, b'a', 10, 1, b'b']).is_err());
        assert!(header_fields(&[10, 2, b'a']).is_err());
        assert!(header_fields(&[8, 1]).is_err());
        assert!(header_fields(&[0]).is_err());
        assert!(header_fields(&[0x80; 11]).is_err());
        let fields = header_fields(&[10, 1, b'a', 64, 1]).unwrap();
        assert_eq!(fields[0], Some(&b"a"[..]));
    }
    #[test]
    fn notification_native_identity_is_a_distinct_closed_producer() {
        let id = Uuid::from_u128(1);
        let digest =
            |producer| identity_digest(producer, "123", 1, "99", 7, 100, &[2; 32], id, b"receipt");
        let notification = digest("NOTIFICATION").unwrap();
        for old in ["CONCERT", "POST", "PROFILE"] {
            assert_ne!(notification, digest(old).unwrap());
        }
        for unsupported in [
            "notification",
            "NOTIFICATION ",
            "AUTH",
            "FRIEND",
            "GATEWAY",
            "",
        ] {
            assert!(digest(unsupported).is_err());
        }
    }
    #[test]
    fn capabilities_and_producer_identity_are_not_interchangeable() {
        let hash = secret_hash(&[8; 32]).unwrap();
        assert!(secret_matches(&[8; 32], &hash));
        assert!(!secret_matches(&[9; 32], &hash));
        assert!(!secret_matches(&[8; 31], &hash));
        let id = Uuid::from_u128(1);
        let one =
            identity_digest("POST", "123", 1, "99", 7, 100, &[2; 32], id, b"receipt").unwrap();
        assert_ne!(
            one,
            identity_digest("POST", "123", 2, "99", 7, 100, &[2; 32], id, b"receipt").unwrap()
        );
        assert!(
            identity_digest("POST", "0123", 1, "99", 7, 100, &[2; 32], id, b"receipt").is_err()
        );
    }
}
