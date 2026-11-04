//! Closed private recovery intents. Persist BEFORE dispatch/PREPARE; these are
//! not public content, general SQL identifiers, endpoint URLs or read grants.
use super::{contract::*, wire};
use serde::{Deserialize, Serialize};
use tonic::Status;
use uuid::Uuid;
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ChildQuery {
    pub actor: Uuid,
    pub event: Uuid,
    pub subject: Uuid,
    pub family: Uuid,
    pub request: Uuid,
    pub expires_unix_micros: i64,
    pub binding_digest: [u8; 32],
}
impl ChildQuery {
    pub fn from_subject(subject: &GuardedSubject) -> Self {
        let binding = subject.binding();
        Self {
            actor: binding.actor(),
            event: binding.event(),
            subject: subject.query().subject.uuid(),
            family: binding.session_family(),
            request: binding.request_id(),
            expires_unix_micros: binding.deadline().expires_micros(),
            binding_digest: binding.digest(),
        }
    }
    fn profile_prefix(&self) -> String {
        // Lossless encoding retained from Profile ReadBinding.child_prefix.
        let bytes: Vec<u8> = [
            self.actor,
            self.event,
            self.subject,
            self.family,
            self.request,
        ]
        .iter()
        .flat_map(|id| id.as_bytes().iter().copied())
        .collect();
        let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let mut encoded = String::new();
        for chunk in bytes.chunks(3) {
            let value = ((chunk[0] as u32) << 16)
                | ((chunk.get(1).copied().unwrap_or(0) as u32) << 8)
                | chunk.get(2).copied().unwrap_or(0) as u32;
            for shift in [18, 12, 6, 0].into_iter().take(chunk.len() + 1) {
                encoded.push(alphabet[((value >> shift) & 63) as usize] as char);
            }
        }
        format!("pp:{encoded}:")
    }
    pub fn profile_child_id(&self, nonce: Uuid) -> Result<String, Status> {
        if nonce.is_nil() {
            return Err(invalid());
        }
        Ok(format!("{}{}", self.profile_prefix(), nonce.simple()))
    }
    pub fn validate_for(&self, binding: &GuardBinding) -> Result<(), Status> {
        if self.subject.is_nil()
            || self.actor != binding.actor()
            || self.event != binding.event()
            || self.family != binding.session_family()
            || self.request != binding.request_id()
            || self.expires_unix_micros != binding.deadline().expires_micros()
            || self.binding_digest != binding.digest()
        {
            return Err(invalid());
        }
        Ok(())
    }
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ChildNativeIdentity {
    pub system_identifier: String,
    pub database_oid: u32,
    pub full_xid: String,
    pub backend_pid: i32,
    pub backend_started_unix_micros: i64,
    pub producer_instance_id: Uuid,
}
impl ChildNativeIdentity {
    pub async fn capture_concert_in(
        connection: &mut sqlx::PgConnection,
        scope: &super::OwnerGuardScope,
    ) -> Result<Self, Status> {
        let identity = ProducerTransaction::capture_concert_in(
            connection,
            scope.binding().digest(),
            wire::canonical_uuid(&scope.producer_scope().instance_id)?,
            Vec::new(),
        )
        .await?;
        Ok(Self {
            system_identifier: identity.system_identifier,
            database_oid: identity.database_oid,
            full_xid: identity.full_xid,
            backend_pid: identity.backend_pid,
            backend_started_unix_micros: identity.backend_started.timestamp_micros(),
            producer_instance_id: identity.producer_instance_id,
        })
    }
    fn validate(&self, digest: &[u8; 32]) -> Result<(), Status> {
        platform::actor_delegation::owner_binding::identity_digest(
            "CONCERT",
            &self.system_identifier,
            self.database_oid,
            &self.full_xid,
            self.backend_pid,
            self.backend_started_unix_micros,
            digest,
            self.producer_instance_id,
            &[],
        )
        .map(|_| ())
    }
}
/// Contains private completion capability; deliberately no Debug. Root stores
/// it under its explicit stable at-rest key and never returns it to a browser.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum ChildRecoveryReference {
    NotificationInbox {
        query: ChildQuery,
        child_id: String,
        completion_secret: String,
    },
    ProfilePublic {
        query: ChildQuery,
        child_id: String,
        completion_secret: String,
    },
    LocalPublic {
        query: ChildQuery,
        child_nonce: Uuid,
        native: ChildNativeIdentity,
    },
    GiftPrivate {
        query: ChildQuery,
        child_nonce: Uuid,
        native: ChildNativeIdentity,
        private_kind: GiftPrivateKind,
        giveaway_id: Uuid,
        reservation_id: Uuid,
        valid_until_unix_micros: i64,
    },
    SocialRead {
        query: ChildQuery,
        child_nonce: Uuid,
        native: ChildNativeIdentity,
        valid_until_unix_micros: i64,
    },
}
impl ChildRecoveryReference {
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::NotificationInbox { .. } => "NOTIFICATION_INBOX",
            Self::ProfilePublic { .. } => "PROFILE_PUBLIC",
            Self::LocalPublic { .. } => "LOCAL_PUBLIC",
            Self::GiftPrivate { .. } => "GIFT_PRIVATE",
            Self::SocialRead { .. } => "SOCIAL_READ",
        }
    }
    pub(super) fn key(&self) -> Result<(u8, Uuid), Status> {
        Ok((
            match self {
                Self::NotificationInbox { .. } => 5,
                Self::ProfilePublic { .. } => 1,
                Self::LocalPublic { .. } => 2,
                Self::GiftPrivate { .. } => 3,
                Self::SocialRead { .. } => 4,
            },
            self.intent_id()?,
        ))
    }
    pub fn query(&self) -> &ChildQuery {
        match self {
            Self::NotificationInbox { query, .. }
            | Self::ProfilePublic { query, .. }
            | Self::LocalPublic { query, .. }
            | Self::GiftPrivate { query, .. }
            | Self::SocialRead { query, .. } => query,
        }
    }
    pub fn intent_id(&self) -> Result<Uuid, Status> {
        match self {
            Self::NotificationInbox {
                query,
                child_id,
                completion_secret,
            } => {
                if query.subject != query.actor
                    || completion_secret.len() != 64
                    || !completion_secret
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                    || completion_secret.bytes().all(|b| b == b'0')
                {
                    return Err(invalid());
                }
                wire::canonical_uuid(child_id)
            }

            Self::ProfilePublic {
                query, child_id, ..
            } => simple_uuid(
                child_id
                    .strip_prefix(&query.profile_prefix())
                    .ok_or_else(invalid)?,
            ),
            Self::LocalPublic { child_nonce, .. }
            | Self::GiftPrivate { child_nonce, .. }
            | Self::SocialRead { child_nonce, .. } => {
                if child_nonce.is_nil() {
                    Err(invalid())
                } else {
                    Ok(*child_nonce)
                }
            }
        }
    }
    pub fn validate_for(&self, binding: &GuardBinding) -> Result<(), Status> {
        self.query().validate_for(binding)?;
        self.intent_id()?;
        match self {
            Self::NotificationInbox { .. } => {}
            Self::ProfilePublic {
                completion_secret, ..
            } => {
                simple_uuid(completion_secret)?;
            }
            Self::LocalPublic { native, .. } => native.validate(&binding.digest())?,
            Self::SocialRead {
                query,
                native,
                valid_until_unix_micros,
                ..
            } => {
                native.validate(&binding.digest())?;
                social_fields(query, *valid_until_unix_micros)?;
            }
            Self::GiftPrivate {
                query,
                native,
                giveaway_id,
                reservation_id,
                valid_until_unix_micros,
                ..
            } => {
                native.validate(&binding.digest())?;
                gift_fields(
                    query,
                    *giveaway_id,
                    *reservation_id,
                    *valid_until_unix_micros,
                )?;
            }
        }
        if serde_json::to_vec(self).map_err(unavailable)?.len() > 8192 {
            return Err(invalid());
        }
        Ok(())
    }
    /// Disclosure ceiling only, never a business expiry or settlement authority.
    /// Root must retain this ceiling at its before-byte boundary as well.
    pub fn content_expires_unix_micros(&self) -> i64 {
        match self {
            Self::GiftPrivate {
                valid_until_unix_micros,
                ..
            }
            | Self::SocialRead {
                valid_until_unix_micros,
                ..
            } => *valid_until_unix_micros,
            _ => self.query().expires_unix_micros,
        }
    }
    pub fn check_content_time(&self) -> Result<(), Status> {
        if self.content_expires_unix_micros() <= chrono::Utc::now().timestamp_micros() {
            return Err(Status::deadline_exceeded("Child disclosure expired"));
        }
        Ok(())
    }
    pub fn local_prepared_gid(&self) -> Result<String, Status> {
        match self {
            Self::LocalPublic { child_nonce, .. } if !child_nonce.is_nil() => {
                Ok(format!("mcr-public-avatar:{}", child_nonce.simple()))
            }
            Self::GiftPrivate { child_nonce, .. } if !child_nonce.is_nil() => {
                Ok(format!("mcr-gift-private:{}", child_nonce.simple()))
            }
            Self::SocialRead { child_nonce, .. } if !child_nonce.is_nil() => {
                Ok(format!("mcr-social-read:{}", child_nonce.simple()))
            }
            _ => Err(invalid()),
        }
    }
}
fn social_fields(query: &ChildQuery, until: i64) -> Result<(), Status> {
    // This read child conservatively protects the entire original parent
    // window. It cannot renew authority or introduce a hidden shorter window.
    if until <= 0
        || until != query.expires_unix_micros
        || chrono::DateTime::from_timestamp_micros(until).is_none()
    {
        return Err(invalid());
    }
    Ok(())
}
fn gift_fields(
    query: &ChildQuery,
    giveaway: Uuid,
    reservation: Uuid,
    until: i64,
) -> Result<(), Status> {
    // Time-independent for restart recovery. This clip does not rewrite business
    // expiry; actual admission and before-byte gates separately require live time.
    if giveaway.is_nil()
        || reservation.is_nil()
        || until <= 0
        || until > query.expires_unix_micros
        || chrono::DateTime::from_timestamp_micros(until).is_none()
    {
        return Err(invalid());
    }
    Ok(())
}
fn simple_uuid(value: &str) -> Result<Uuid, Status> {
    let id = Uuid::parse_str(value).map_err(|_| invalid())?;
    if id.is_nil() || id.simple().to_string() != value {
        return Err(invalid());
    }
    Ok(id)
}
fn invalid() -> Status {
    Status::invalid_argument("Invalid closed child recovery intent")
}
#[cfg(test)]
mod tests {
    use super::*;
    fn query() -> ChildQuery {
        ChildQuery {
            actor: Uuid::from_u128(1),
            event: Uuid::from_u128(2),
            subject: Uuid::from_u128(3),
            family: Uuid::from_u128(4),
            request: Uuid::from_u128(5),
            expires_unix_micros: 1_000_000,
            binding_digest: [7; 32],
        }
    }
    #[test]
    fn notification_inbox_intent_is_private_actor_owned_and_canonical() {
        let mut q = query();
        q.subject = q.actor;
        let good = ChildRecoveryReference::NotificationInbox {
            query: q.clone(),
            child_id: Uuid::from_u128(0xab).to_string(),
            completion_secret: "01".repeat(32),
        };
        assert_eq!(good.key().unwrap(), (5, Uuid::from_u128(0xab)));
        assert_eq!(good.kind_name(), "NOTIFICATION_INBOX");
        assert!(good.local_prepared_gid().is_err());
        for key in ["gid", "native", "callback_url", "owner_completion_secret"] {
            let mut value = serde_json::to_value(&good).unwrap();
            value[key] = serde_json::json!("forbidden");
            assert!(serde_json::from_value::<ChildRecoveryReference>(value).is_err());
        }
        for cap in [
            "".to_string(),
            "01".repeat(31),
            "00".repeat(32),
            "AB".repeat(32),
        ] {
            let wrong = ChildRecoveryReference::NotificationInbox {
                query: q.clone(),
                child_id: Uuid::from_u128(0xab).to_string(),
                completion_secret: cap,
            };
            assert!(wrong.intent_id().is_err());
        }
        for id in [
            Uuid::nil().to_string(),
            Uuid::from_u128(0xab).to_string().to_uppercase(),
        ] {
            let wrong = ChildRecoveryReference::NotificationInbox {
                query: q.clone(),
                child_id: id,
                completion_secret: "01".repeat(32),
            };
            assert!(wrong.intent_id().is_err());
        }
        q.subject = Uuid::from_u128(3);
        let foreign = ChildRecoveryReference::NotificationInbox {
            query: q,
            child_id: Uuid::from_u128(0xab).to_string(),
            completion_secret: "01".repeat(32),
        };
        assert!(foreign.intent_id().is_err());
    }
    #[test]
    fn preallocated_profile_child_matches_actual_owner_wire_syntax() {
        let q = query();
        let id = q.profile_child_id(Uuid::from_u128(8)).unwrap();
        let owner = profile_service::concert_public_profile::ReadBinding {
            actor: q.actor,
            event: q.event,
            subject: q.subject,
            family: q.family,
            request: q.request,
        };
        owner.validate_child(&id).unwrap();
        let reference = ChildRecoveryReference::ProfilePublic {
            query: q.clone(),
            child_id: id.clone(),
            completion_secret: Uuid::from_u128(9).simple().to_string(),
        };
        assert_eq!(reference.intent_id().unwrap(), Uuid::from_u128(8));
        let mut other = q;
        other.subject = Uuid::from_u128(10);
        assert!(!id.starts_with(&other.profile_prefix()));
        let encoded = serde_json::to_vec(&reference).unwrap();
        let decoded: ChildRecoveryReference = serde_json::from_slice(&encoded).unwrap();
        assert!(decoded == reference);
        let mut poisoned: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        poisoned["callback_url"] = serde_json::json!("https://other.invalid");
        assert!(serde_json::from_value::<ChildRecoveryReference>(poisoned).is_err());
    }
    #[test]
    fn local_child_reference_accepts_only_actual_closed_identity_shape() {
        let q = query();
        let native = ChildNativeIdentity {
            system_identifier: "123".into(),
            database_oid: 4,
            full_xid: "77".into(),
            backend_pid: 8,
            backend_started_unix_micros: 100,
            producer_instance_id: Uuid::from_u128(6),
        };
        native.validate(&q.binding_digest).unwrap();
        let reference = ChildRecoveryReference::LocalPublic {
            query: q.clone(),
            child_nonce: Uuid::from_u128(9),
            native: native.clone(),
        };
        assert_eq!(
            reference.local_prepared_gid().unwrap(),
            format!("mcr-public-avatar:{}", Uuid::from_u128(9).simple())
        );
        let mut wrong = native;
        wrong.full_xid = "077".into();
        assert!(wrong.validate(&q.binding_digest).is_err());
        let mut value = serde_json::to_value(reference).unwrap();
        value["gid"] = serde_json::json!("arbitrary SQL");
        assert!(serde_json::from_value::<ChildRecoveryReference>(value).is_err());
    }
    #[test]
    fn social_read_is_closed_and_covers_exact_original_window() {
        let q = query();
        for until in [0, -1, q.expires_unix_micros - 1, q.expires_unix_micros + 1] {
            assert!(social_fields(&q, until).is_err());
        }
        social_fields(&q, q.expires_unix_micros).unwrap();
        let reference = ChildRecoveryReference::SocialRead {
            query: q.clone(),
            child_nonce: Uuid::from_u128(13),
            native: ChildNativeIdentity {
                system_identifier: "123".into(),
                database_oid: 4,
                full_xid: "77".into(),
                backend_pid: 8,
                backend_started_unix_micros: 100,
                producer_instance_id: Uuid::from_u128(6),
            },
            valid_until_unix_micros: q.expires_unix_micros,
        };
        assert_eq!(reference.kind_name(), "SOCIAL_READ");
        assert_eq!(reference.key().unwrap(), (4, Uuid::from_u128(13)));
        assert_eq!(
            reference.local_prepared_gid().unwrap(),
            format!("mcr-social-read:{}", Uuid::from_u128(13).simple())
        );
        let bytes = serde_json::to_vec(&reference).unwrap();
        assert!(serde_json::from_slice::<ChildRecoveryReference>(&bytes).unwrap() == reference);
        let mut value = serde_json::to_value(&reference).unwrap();
        value["gid"] = serde_json::json!("mcr-public-avatar:other");
        assert!(serde_json::from_value::<ChildRecoveryReference>(value).is_err());
    }
    #[test]
    fn gift_private_metadata_cannot_expand_disclosure_or_supply_a_gid() {
        let q = query();
        for until in [0, -1, q.expires_unix_micros + 1] {
            assert!(gift_fields(&q, Uuid::from_u128(11), Uuid::from_u128(12), until).is_err());
        }
        assert!(gift_fields(&q, Uuid::nil(), Uuid::from_u128(12), 1).is_err());
        assert!(gift_fields(&q, Uuid::from_u128(11), Uuid::nil(), 1).is_err());
        for purpose in [GiftPrivateKind::Pickup, GiftPrivateKind::Code] {
            let reference = ChildRecoveryReference::GiftPrivate {
                query: q.clone(),
                child_nonce: Uuid::from_u128(13),
                native: ChildNativeIdentity {
                    system_identifier: "123".into(),
                    database_oid: 4,
                    full_xid: "77".into(),
                    backend_pid: 8,
                    backend_started_unix_micros: 100,
                    producer_instance_id: Uuid::from_u128(6),
                },
                private_kind: purpose,
                giveaway_id: Uuid::from_u128(11),
                reservation_id: Uuid::from_u128(12),
                valid_until_unix_micros: 500_000,
            };
            assert_eq!(
                reference.local_prepared_gid().unwrap(),
                format!("mcr-gift-private:{}", Uuid::from_u128(13).simple())
            );
            assert_eq!(reference.kind_name(), "GIFT_PRIVATE");
            assert_eq!(reference.key().unwrap(), (3, Uuid::from_u128(13)));
            let encoded = serde_json::to_vec(&reference).unwrap();
            let restored: ChildRecoveryReference = serde_json::from_slice(&encoded).unwrap();
            assert!(restored == reference);
            for key in ["gid", "callback_url", "code", "instructions"] {
                let mut poisoned = serde_json::to_value(&reference).unwrap();
                poisoned[key] = serde_json::json!("forbidden");
                assert!(serde_json::from_value::<ChildRecoveryReference>(poisoned).is_err());
            }
        }
    }
}
