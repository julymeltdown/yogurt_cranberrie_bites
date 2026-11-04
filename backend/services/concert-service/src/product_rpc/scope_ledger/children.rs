//! Closed, encrypted pre-dispatch intents. Missing memory, deadline expiry and
//! remote reply loss never certify child completion.
use super::*;
use crate::adapters::out::relationship_authority::guarded::ChildRecoveryReference;
use ring::{
    aead,
    rand::{SecureRandom, SystemRandom},
};
use sha2::{Digest, Sha256};

const MAX_CHILDREN: i64 = 202;
const MAX_REFERENCE: usize = 8192;

/// Counters describe confirmed durable ACKs, never a delivery receipt. A pending
/// attempt may have settled remotely but lost its ACK; retry stays idempotent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChildRecoveryCounts {
    pub selected: u32,
    pub attempted: u32,
    pub completed: u32,
    pub pending: u32,
    pub timed_out: u32,
    pub scan_wrapped: bool,
}

/// One global lexicographic position, independent of the number of roots or
/// failed children. Kept private to the configured worker, never a capability.
#[derive(Default)]
pub(crate) struct ChildRecoveryCursor(Option<(Uuid, String, Uuid)>);
fn crypto_error() -> Status {
    Status::unavailable("Private child recovery unavailable")
}

/// Deliberately no Debug: this configuration is an operator-owned at-rest key.
pub struct ChildRecoveryKey {
    id: String,
    key: aead::LessSafeKey,
}
impl ChildRecoveryKey {
    pub fn new(id: &str, secret: &[u8]) -> Result<Self, Status> {
        if id.is_empty()
            || id.len() > 64
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
            || secret.len() != 32
            || secret.iter().all(|b| *b == 0)
        {
            return Err(invalid());
        }
        Ok(Self {
            id: id.into(),
            key: aead::LessSafeKey::new(
                aead::UnboundKey::new(&aead::AES_256_GCM, secret).map_err(|_| invalid())?,
            ),
        })
    }
}
pub struct ChildRecoveryKeys {
    active: ChildRecoveryKey,
    retained: Vec<ChildRecoveryKey>,
}
impl ChildRecoveryKeys {
    pub fn new(active: ChildRecoveryKey, retained: Vec<ChildRecoveryKey>) -> Result<Self, Status> {
        if retained.len() > 3 {
            return Err(invalid());
        }
        let mut ids = std::collections::HashSet::new();
        for key in std::iter::once(&active).chain(retained.iter()) {
            if !ids.insert(key.id.clone()) {
                return Err(invalid());
            }
        }
        Ok(Self { active, retained })
    }
    fn find(&self, id: &str) -> Result<&ChildRecoveryKey, Status> {
        std::iter::once(&self.active)
            .chain(self.retained.iter())
            .find(|key| key.id == id)
            .ok_or_else(crypto_error)
    }
    fn seal(
        &self,
        scope: &ScopeKey,
        reference: &ChildRecoveryReference,
    ) -> Result<Vec<u8>, Status> {
        let mut bytes = serde_json::to_vec(reference).map_err(|_| invalid())?;
        if bytes.len() > MAX_REFERENCE {
            return Err(invalid());
        }
        let mut nonce = [0; 12];
        SystemRandom::new()
            .fill(&mut nonce)
            .map_err(|_| crypto_error())?;
        self.active
            .key
            .seal_in_place_append_tag(
                aead::Nonce::assume_unique_for_key(nonce),
                aead::Aad::from(aad(
                    scope,
                    reference.kind_name(),
                    reference.intent_id()?,
                    &self.active.id,
                )),
                &mut bytes,
            )
            .map_err(|_| crypto_error())?;
        let mut sealed = nonce.to_vec();
        sealed.extend_from_slice(&bytes);
        Ok(sealed)
    }
    fn open(
        &self,
        scope: &ScopeKey,
        kind: &str,
        intent: Uuid,
        key_id: &str,
        sealed: &[u8],
    ) -> Result<ChildRecoveryReference, Status> {
        if !(28..=MAX_REFERENCE + 28).contains(&sealed.len()) {
            return Err(crypto_error());
        }
        let nonce: [u8; 12] = sealed[..12].try_into().map_err(|_| crypto_error())?;
        let mut bytes = sealed[12..].to_vec();
        let plain = self
            .find(key_id)?
            .key
            .open_in_place(
                aead::Nonce::assume_unique_for_key(nonce),
                aead::Aad::from(aad(scope, kind, intent, key_id)),
                &mut bytes,
            )
            .map_err(|_| crypto_error())?;
        let reference: ChildRecoveryReference =
            serde_json::from_slice(plain).map_err(|_| crypto_error())?;
        if reference.kind_name() != kind || reference.intent_id()? != intent {
            return Err(crypto_error());
        }
        Ok(reference)
    }
}
fn aad(scope: &ScopeKey, kind: &str, intent: Uuid, key_id: &str) -> Vec<u8> {
    let mut value = b"mcr-product-child-intent-v1\0".to_vec();
    value.extend_from_slice(scope.instance.as_bytes());
    value.extend_from_slice(scope.scope.as_bytes());
    value.extend_from_slice(&scope.binding_digest);
    value.extend_from_slice(intent.as_bytes());
    value.extend_from_slice(kind.as_bytes());
    value.push(0);
    value.extend_from_slice(key_id.as_bytes());
    value
}
fn digest(reference: &ChildRecoveryReference) -> Result<Vec<u8>, Status> {
    Ok(Sha256::digest(serde_json::to_vec(reference).map_err(|_| invalid())?).to_vec())
}
/// Implemented by the actual configured Profile/local-native recovery adapter.
/// No callback URL, SQL/GID input or successful default is supported.
#[async_trait::async_trait]
pub trait ChildIntentSettler: Send + Sync {
    async fn recover_one(&self, reference: &ChildRecoveryReference) -> Result<(), Status>;
}

impl PgScopeLedger {
    async fn validate_child_in(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        scope: &ScopeKey,
        reference: &ChildRecoveryReference,
    ) -> Result<(), Status> {
        let value: serde_json::Value = sqlx::query_scalar(
            "SELECT binding FROM concert_product_owner_scopes WHERE instance_id=$1 AND scope_id=$2 AND binding_digest=$3")
            .bind(scope.instance).bind(scope.scope).bind(scope.binding_digest.as_slice())
            .fetch_one(&mut **tx).await.map_err(unavailable)?;
        let binding: OwnerBinding = serde_json::from_value(value).map_err(|_| invalid())?;
        let q = reference.query();
        if q.subject.is_nil()
            || q.actor != binding.actor_id
            || q.event != binding.event_id
            || q.family != binding.session_family_id
            || q.request != binding.request_id
            || q.expires_unix_micros != binding.expires_unix_micros
            || q.binding_digest != scope.binding_digest
            || binding.digest()? != scope.binding_digest
            || serde_json::to_vec(reference).map_err(|_| invalid())?.len() > MAX_REFERENCE
        {
            return Err(Status::permission_denied("Child intent binding mismatch"));
        }
        reference.intent_id()?;
        match reference {
            ChildRecoveryReference::NotificationInbox { query, .. } => {
                // Remote Notification owns its actual native identity. The
                // common binding checks and intent_id validate query/capability.
                if query.subject != binding.actor_id {
                    return Err(invalid());
                }
            }
            ChildRecoveryReference::ProfilePublic {
                completion_secret, ..
            } => {
                let id = Uuid::parse_str(completion_secret).map_err(|_| invalid())?;
                if id.is_nil() || id.simple().to_string() != *completion_secret {
                    return Err(invalid());
                }
            }
            ChildRecoveryReference::LocalPublic { native, .. }
            | ChildRecoveryReference::GiftPrivate { native, .. }
            | ChildRecoveryReference::SocialRead { native, .. }
            | ChildRecoveryReference::TargetRead { native, .. } => {
                if native.producer_instance_id != scope.instance {
                    return Err(invalid());
                }
                identity_digest(
                    "CONCERT",
                    &native.system_identifier,
                    native.database_oid,
                    &native.full_xid,
                    native.backend_pid,
                    native.backend_started_unix_micros,
                    &scope.binding_digest,
                    native.producer_instance_id,
                    &[],
                )?;
                reference.local_prepared_gid()?;
                if let ChildRecoveryReference::TargetRead {
                    root_scope_id,
                    inbox_intent_id,
                    inbox_reference_digest,
                    snapshot_digest,
                    ..
                } = reference
                {
                    if *root_scope_id != scope.scope
                        || q.subject != q.actor
                        || inbox_intent_id.is_nil()
                        || *inbox_reference_digest == [0; 32]
                        || *snapshot_digest == [0; 32]
                    {
                        return Err(invalid());
                    }
                    let linked: Option<Vec<u8>> = sqlx::query_scalar("SELECT reference_digest FROM concert_product_child_intents WHERE instance_id=$1 AND scope_id=$2 AND binding_digest=$3 AND kind='NOTIFICATION_INBOX' AND intent_id=$4")
                        .bind(scope.instance).bind(scope.scope).bind(scope.binding_digest.as_slice()).bind(inbox_intent_id)
                        .fetch_optional(&mut **tx).await.map_err(unavailable)?;
                    if linked.as_deref() != Some(inbox_reference_digest.as_slice()) {
                        return Err(invalid());
                    }
                }

                if let ChildRecoveryReference::SocialRead {
                    valid_until_unix_micros,
                    ..
                } = reference
                {
                    if *valid_until_unix_micros != q.expires_unix_micros
                        || *valid_until_unix_micros <= 0
                        || chrono::DateTime::from_timestamp_micros(*valid_until_unix_micros)
                            .is_none()
                    {
                        return Err(invalid());
                    }
                }
                if let ChildRecoveryReference::GiftPrivate {
                    giveaway_id,
                    reservation_id,
                    valid_until_unix_micros,
                    ..
                } = reference
                {
                    // Shape validation is deliberately time independent so an
                    // expired disclosure can still be recovered after restart.
                    if giveaway_id.is_nil()
                        || reservation_id.is_nil()
                        || *valid_until_unix_micros <= 0
                        || *valid_until_unix_micros > q.expires_unix_micros
                        || chrono::DateTime::from_timestamp_micros(*valid_until_unix_micros)
                            .is_none()
                    {
                        return Err(invalid());
                    }
                }
            }
        }
        Ok(())
    }
    pub async fn register_child_intent(
        &self,
        scope: &ScopeKey,
        reference: &ChildRecoveryReference,
    ) -> Result<(), Status> {
        let mut tx = self.transaction().await?;
        if self.lock(&mut tx, scope).await? != ScopePhase::Active {
            return Err(Status::failed_precondition("Product scope is sealed"));
        }
        self.validate_child_in(&mut tx, scope, reference).await?;
        let intent = reference.intent_id()?;
        let old: Option<(String,Vec<u8>)> = sqlx::query_as(
            "SELECT key_id,sealed_reference FROM concert_product_child_intents WHERE instance_id=$1 AND scope_id=$2 AND kind=$3 AND intent_id=$4")
            .bind(scope.instance).bind(scope.scope).bind(reference.kind_name()).bind(intent)
            .fetch_optional(&mut *tx).await.map_err(unavailable)?;
        if let Some((key_id, sealed)) = old {
            if self
                .child_keys
                .open(scope, reference.kind_name(), intent, &key_id, &sealed)?
                != *reference
            {
                return Err(Status::permission_denied("Child intent cannot rebind"));
            }
        } else {
            let count: i64 = sqlx::query_scalar("SELECT count(*) FROM concert_product_child_intents WHERE instance_id=$1 AND scope_id=$2")
                .bind(scope.instance).bind(scope.scope).fetch_one(&mut *tx).await.map_err(unavailable)?;
            if count >= MAX_CHILDREN {
                return Err(Status::resource_exhausted(
                    "Child intent capacity unavailable",
                ));
            }
            let sealed = self.child_keys.seal(scope, reference)?;
            sqlx::query("INSERT INTO concert_product_child_intents(instance_id,scope_id,binding_digest,kind,intent_id,key_id,sealed_reference,reference_digest) VALUES($1,$2,$3,$4,$5,$6,$7,$8)")
                .bind(scope.instance).bind(scope.scope).bind(scope.binding_digest.as_slice())
                .bind(reference.kind_name()).bind(intent).bind(&self.child_keys.active.id).bind(sealed).bind(digest(reference)?)
                .execute(&mut *tx).await.map_err(unavailable)?;
        }
        tx.commit().await.map_err(unavailable)
    }
    pub async fn record_child_completed(
        &self,
        scope: &ScopeKey,
        reference: &ChildRecoveryReference,
    ) -> Result<(), Status> {
        let mut tx = self.transaction().await?;
        if self.lock(&mut tx, scope).await? != ScopePhase::RootFinished {
            return Err(Status::failed_precondition(
                "Child completion needs finished root",
            ));
        }
        self.validate_child_in(&mut tx, scope, reference).await?;
        let intent = reference.intent_id()?;
        let row: Option<(String,Vec<u8>,bool)> = sqlx::query_as("SELECT key_id,sealed_reference,completed_at IS NOT NULL FROM concert_product_child_intents WHERE instance_id=$1 AND scope_id=$2 AND kind=$3 AND intent_id=$4")
            .bind(scope.instance).bind(scope.scope).bind(reference.kind_name()).bind(intent)
            .fetch_optional(&mut *tx).await.map_err(unavailable)?;
        let (key_id, sealed, completed) =
            row.ok_or_else(|| Status::failed_precondition("Unknown child intent"))?;
        if self
            .child_keys
            .open(scope, reference.kind_name(), intent, &key_id, &sealed)?
            != *reference
        {
            return Err(Status::permission_denied("Child completion mismatch"));
        }
        if !completed {
            sqlx::query("UPDATE concert_product_child_intents SET completed_at=clock_timestamp() WHERE instance_id=$1 AND scope_id=$2 AND kind=$3 AND intent_id=$4")
                .bind(scope.instance).bind(scope.scope).bind(reference.kind_name()).bind(intent)
                .execute(&mut *tx).await.map_err(unavailable)?;
        }
        tx.commit().await.map_err(unavailable)
    }
    /// Best-effort background recovery of immutable terminal-root children.
    /// No producer transaction, live registry, content deadline or delivery ACK
    /// is required or manufactured. Every attempted candidate advances the
    /// cursor, including failures, so an unknown first child cannot starve later
    /// children of either the same root or another root.
    pub(crate) async fn drain_finished_children(
        &self,
        cursor: &mut ChildRecoveryCursor,
        batch_size: u16,
        per_child_timeout: std::time::Duration,
        stop: tokio::time::Instant,
    ) -> Result<ChildRecoveryCounts, Status> {
        if batch_size == 0 || i64::from(batch_size) > MAX_CHILDREN || per_child_timeout.is_zero() {
            return Err(invalid());
        }
        type Candidate = (Uuid, Vec<u8>, String, Uuid, String, Vec<u8>);
        let after = cursor.0.as_ref();
        let rows: Vec<Candidate> = tokio::time::timeout_at(stop, async {
            let mut tx = self.transaction().await?;
            let rows = sqlx::query_as(
                "SELECT c.scope_id,c.binding_digest,c.kind,c.intent_id,c.key_id,c.sealed_reference
                 FROM concert_product_child_intents c
                 JOIN concert_product_owner_scopes s USING (instance_id,scope_id,binding_digest)
                 WHERE c.instance_id=$1 AND s.phase='ROOT_FINISHED' AND c.completed_at IS NULL
                   AND ($2::uuid IS NULL OR (c.scope_id,c.kind,c.intent_id)>($2,$3,$4))
                 ORDER BY c.scope_id,c.kind,c.intent_id LIMIT $5",
            )
            .bind(self.instance)
            .bind(after.map(|v| v.0))
            .bind(after.map(|v| v.1.as_str()))
            .bind(after.map(|v| v.2))
            .bind(i64::from(batch_size))
            .fetch_all(&mut *tx)
            .await
            .map_err(unavailable)?;
            tx.commit().await.map_err(unavailable)?;
            Ok::<_, Status>(rows)
        })
        .await
        .map_err(|_| Status::deadline_exceeded("Child recovery scan budget exhausted"))??;
        let mut counts = ChildRecoveryCounts {
            selected: rows.len() as u32,
            ..Default::default()
        };
        for (scope, digest, kind, intent, key_id, sealed) in rows {
            if tokio::time::Instant::now() >= stop {
                break;
            }
            // Advance before I/O: lost replies, decrypt failures, timeouts and
            // even a failed durable ACK must not pin the enumeration head.
            cursor.0 = Some((scope, kind.clone(), intent));
            counts.attempted += 1;
            let child_stop = stop.min(tokio::time::Instant::now() + per_child_timeout);
            let outcome = tokio::time::timeout_at(child_stop, async {
                let scope = ScopeKey {
                    instance: self.instance,
                    scope,
                    binding_digest: digest.try_into().map_err(|_| crypto_error())?,
                };
                scope.validate(self.instance)?;
                let reference = self
                    .child_keys
                    .open(&scope, &kind, intent, &key_id, &sealed)?;
                // Validate the original full binding before native dispatch.
                // This transaction is committed BEFORE the owner/native call.
                let mut tx = self.transaction().await?;
                if self.lock(&mut tx, &scope).await? != ScopePhase::RootFinished {
                    return Err(Status::failed_precondition("Child root is unfinished"));
                }
                self.validate_child_in(&mut tx, &scope, &reference).await?;
                tx.commit().await.map_err(unavailable)?;
                self.child_settler.recover_one(&reference).await?;
                self.record_child_completed(&scope, &reference).await
            })
            .await;
            match outcome {
                Ok(Ok(())) => counts.completed += 1,
                Ok(Err(_)) => counts.pending += 1,
                Err(_) => {
                    counts.pending += 1;
                    counts.timed_out += 1;
                }
            }
        }
        if counts.attempted == counts.selected && counts.selected < u32::from(batch_size) {
            cursor.0 = None;
            counts.scan_wrapped = true;
        }
        Ok(counts)
    }
    pub async fn settle_child_intents(&self, scope: &ScopeKey) -> Result<(), Status> {
        if self.resolve_scope(scope).await? != ScopePhase::RootFinished {
            return Err(Status::failed_precondition(
                "Child recovery needs finished root",
            ));
        }
        let rows: Vec<(String,Uuid,String,Vec<u8>)> = sqlx::query_as("SELECT kind,intent_id,key_id,sealed_reference FROM concert_product_child_intents WHERE instance_id=$1 AND scope_id=$2 AND binding_digest=$3 AND completed_at IS NULL ORDER BY kind,intent_id LIMIT 203")
            .bind(scope.instance).bind(scope.scope).bind(scope.binding_digest.as_slice())
            .fetch_all(&self.pool).await.map_err(unavailable)?;
        if rows.len() > MAX_CHILDREN as usize {
            return Err(crypto_error());
        }
        for (kind, intent, key_id, sealed) in rows {
            let reference = self
                .child_keys
                .open(scope, &kind, intent, &key_id, &sealed)?;
            // No DB transaction is held across the actual owner/native RPC.
            self.child_settler.recover_one(&reference).await?;
            self.record_child_completed(scope, &reference).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gift_private_reference_stays_authenticated_and_recoverable_after_disclosure_expiry() {
        use crate::adapters::out::relationship_authority::guarded::{
            ChildNativeIdentity, ChildQuery, GiftPrivateKind,
        };
        let scope = ScopeKey {
            instance: Uuid::from_u128(1),
            scope: Uuid::from_u128(2),
            binding_digest: [3; 32],
        };
        let reference = ChildRecoveryReference::GiftPrivate {
            query: ChildQuery {
                actor: Uuid::from_u128(4),
                event: Uuid::from_u128(5),
                subject: Uuid::from_u128(4),
                family: Uuid::from_u128(6),
                request: Uuid::from_u128(7),
                expires_unix_micros: 2_000_000,
                binding_digest: scope.binding_digest,
            },
            child_nonce: Uuid::from_u128(8),
            native: ChildNativeIdentity {
                system_identifier: "1234".into(),
                database_oid: 42,
                full_xid: "90".into(),
                backend_pid: 12,
                backend_started_unix_micros: 1,
                producer_instance_id: scope.instance,
            },
            private_kind: GiftPrivateKind::Pickup,
            giveaway_id: Uuid::from_u128(9),
            reservation_id: Uuid::from_u128(10),
            valid_until_unix_micros: 1_000_000,
        };
        assert!(reference.check_content_time().is_err());
        let keys =
            ChildRecoveryKeys::new(ChildRecoveryKey::new("gift", &[41; 32]).unwrap(), vec![])
                .unwrap();
        let sealed = keys.seal(&scope, &reference).unwrap();
        let intent = reference.intent_id().unwrap();
        assert!(
            keys.open(&scope, "GIFT_PRIVATE", intent, "gift", &sealed)
                .unwrap()
                == reference
        );
        assert!(keys
            .open(&scope, "LOCAL_PUBLIC", intent, "gift", &sealed)
            .is_err());
        assert_eq!(
            reference.local_prepared_gid().unwrap(),
            format!("mcr-gift-private:{}", intent.simple())
        );
        let mut changed = reference.clone();
        if let ChildRecoveryReference::GiftPrivate { private_kind, .. } = &mut changed {
            *private_kind = GiftPrivateKind::Code;
        }
        assert_ne!(digest(&reference).unwrap(), digest(&changed).unwrap());
    }
    use crate::adapters::out::relationship_authority::guarded::ChildQuery;
    #[test]
    fn private_intent_ciphertext_authenticates_every_storage_coordinate_and_retains_old_keys() {
        let scope = ScopeKey {
            instance: Uuid::from_u128(1),
            scope: Uuid::from_u128(2),
            binding_digest: [7; 32],
        };
        let query = ChildQuery {
            actor: Uuid::from_u128(3),
            event: Uuid::from_u128(4),
            subject: Uuid::from_u128(5),
            family: Uuid::from_u128(6),
            request: Uuid::from_u128(7),
            expires_unix_micros: 100,
            binding_digest: [7; 32],
        };
        let intent = Uuid::from_u128(8);
        let reference = ChildRecoveryReference::ProfilePublic {
            child_id: query.profile_child_id(intent).unwrap(),
            query,
            completion_secret: Uuid::from_u128(9).simple().to_string(),
        };
        let old = ChildRecoveryKeys::new(ChildRecoveryKey::new("old", &[1; 32]).unwrap(), vec![])
            .unwrap();
        let bytes = old.seal(&scope, &reference).unwrap();
        let rotated = ChildRecoveryKeys::new(
            ChildRecoveryKey::new("new", &[2; 32]).unwrap(),
            vec![ChildRecoveryKey::new("old", &[1; 32]).unwrap()],
        )
        .unwrap();
        let kind = reference.kind_name();
        assert!(rotated.open(&scope, kind, intent, "old", &bytes).unwrap() == reference);
        for changed in [
            ScopeKey {
                instance: Uuid::from_u128(10),
                ..scope.clone()
            },
            ScopeKey {
                scope: Uuid::from_u128(11),
                ..scope.clone()
            },
            ScopeKey {
                binding_digest: [8; 32],
                ..scope.clone()
            },
        ] {
            assert!(rotated.open(&changed, kind, intent, "old", &bytes).is_err());
        }
        assert!(rotated
            .open(&scope, "LOCAL_PUBLIC", intent, "old", &bytes)
            .is_err());
        assert!(rotated
            .open(&scope, kind, Uuid::from_u128(12), "old", &bytes)
            .is_err());
        assert!(rotated.open(&scope, kind, intent, "new", &bytes).is_err());
        let mut tampered = bytes.clone();
        tampered[13] ^= 1;
        assert!(rotated
            .open(&scope, kind, intent, "old", &tampered)
            .is_err());
        assert!(ChildRecoveryKey::new("bad key", &[1; 32]).is_err());
        assert!(ChildRecoveryKey::new("zero", &[0; 32]).is_err());
        assert!(ChildRecoveryKeys::new(
            ChildRecoveryKey::new("same", &[1; 32]).unwrap(),
            vec![ChildRecoveryKey::new("same", &[2; 32]).unwrap()]
        )
        .is_err());
    }
}
