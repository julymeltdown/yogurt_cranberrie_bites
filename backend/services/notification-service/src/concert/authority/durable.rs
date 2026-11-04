//! Actual PostgreSQL XID plus a witness committed on the inbox connection.
//! Durable references are written outside that transaction before Track, so
//! owners can resolve cancellation/crash even when the inbox transaction aborts.
use super::{binding_digest, canonical};
use crate::concert::transport::lease;
use platform::actor_delegation::{authority_deadline::AuthorityRpcDeadline, owner_binding};
use prost::Message;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool, Row};
use tonic::Status;
use uuid::Uuid;

#[derive(Clone)]
pub(crate) struct AuthorityStore {
    pool: PgPool,
    instance: Uuid,
}
#[derive(Clone)]
pub(crate) struct Intent {
    pub id: Uuid,
    pub binding: lease::GuardBinding,
    pub binding_digest: [u8; 32],
    pub request_digest: [u8; 32],
    capability_hash: [u8; 32],
    pub sealed: bool,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NativeIdentity {
    system_identifier: String,
    database_oid: u32,
    full_xid: String,
    backend_pid: i32,
    backend_started_micros: i64,
    producer_instance: Uuid,
    effect_digest: [u8; 32],
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeOutcome {
    NoEffect,
    InProgress,
    Committed,
    Aborted,
    Unknown,
}
impl AuthorityStore {
    pub async fn open(pool: PgPool, deadline: AuthorityRpcDeadline) -> Result<Self, Status> {
        // Protected inbox transactions must not occupy every connection needed
        // to register their own durable references. Use a distinct bounded pool
        // with the SAME configured database/role/TLS options, never a URL hint.
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .min_connections(0)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .connect_lazy_with(pool.connect_options().as_ref().clone());
        bounded(deadline, async {
            sqlx::query("insert into concert_notification_producer_instance(singleton,instance_id) values(true,$1) on conflict do nothing")
                .bind(Uuid::new_v4()).execute(&pool).await.map_err(unavailable)?;
            let instance: Uuid = sqlx::query_scalar("select instance_id from concert_notification_producer_instance where singleton")
                .fetch_one(&pool).await.map_err(unavailable)?;
            if instance.is_nil() { return Err(denied()); }
            Ok(Self { pool, instance })
        }).await
    }
    /// Must finish before opening the protected repeatable-read transaction.
    /// Replays require the exact private capability, binding and request digest.
    pub async fn create_intent(
        &self,
        id: Uuid,
        capability: &[u8],
        binding: lease::GuardBinding,
        request_digest: [u8; 32],
        deadline: AuthorityRpcDeadline,
    ) -> Result<Intent, Status> {
        let digest = binding_digest(&binding)?;
        let hash = owner_binding::secret_hash(capability)?;
        if id.is_nil() || capability.iter().all(|b| *b == 0) || request_digest == [0; 32] {
            return Err(denied());
        }
        bounded(deadline, async {
            sqlx::query("insert into concert_notification_authority_intents(child_id,binding_proto,binding_digest,request_digest,capability_hash) values($1,$2,$3,$4,$5) on conflict do nothing")
                .bind(id).bind(binding.encode_to_vec()).bind(digest.as_slice()).bind(request_digest.as_slice()).bind(hash.as_slice()).execute(&self.pool).await.map_err(unavailable)?;
            let intent = self.load(id).await?.ok_or_else(denied)?;
            if intent.binding != binding || intent.request_digest != request_digest || intent.sealed || !intent.matches(capability) { return Err(denied()); }
            Ok(intent)
        }).await
    }
    pub async fn checked_intent(
        &self,
        id: Uuid,
        capability: &[u8],
        binding: &lease::GuardBinding,
        deadline: AuthorityRpcDeadline,
    ) -> Result<Intent, Status> {
        binding_digest(binding)?;
        bounded(deadline, async {
            let intent = self.load(id).await?.ok_or_else(denied)?;
            if intent.binding != *binding || !intent.matches(capability) {
                return Err(denied());
            }
            Ok(intent)
        })
        .await
    }
    /// Durable close-before-dispatch tombstone. A delayed Execute with this
    /// child ID can never start after recovery has positively sealed it.
    pub async fn close_intent(
        &self,
        id: Uuid,
        capability: &[u8],
        binding: &lease::GuardBinding,
        deadline: AuthorityRpcDeadline,
    ) -> Result<Intent, Status> {
        let digest = binding_digest(binding)?;
        let hash = owner_binding::secret_hash(capability)?;
        if id.is_nil() || capability.iter().all(|b| *b == 0) {
            return Err(denied());
        }
        bounded(deadline, async {
            let request_digest: [u8; 32] = Sha256::digest(b"mcr notification close before execute v1\0").into();
            sqlx::query("insert into concert_notification_authority_intents(child_id,binding_proto,binding_digest,request_digest,capability_hash,sealed) values($1,$2,$3,$4,$5,true) on conflict do nothing")
                .bind(id).bind(binding.encode_to_vec()).bind(digest.as_slice()).bind(request_digest.as_slice()).bind(hash.as_slice()).execute(&self.pool).await.map_err(unavailable)?;
            self.checked_intent(id,capability,binding,deadline).await
        }).await
    }
    async fn load(&self, id: Uuid) -> Result<Option<Intent>, Status> {
        let row = sqlx::query("select binding_proto,binding_digest,request_digest,capability_hash,sealed from concert_notification_authority_intents where child_id=$1")
            .bind(id).fetch_optional(&self.pool).await.map_err(unavailable)?;
        row.map(|r| {
            let bytes: Vec<u8> = r.try_get("binding_proto").map_err(unavailable)?;
            let binding = lease::GuardBinding::decode(bytes.as_slice()).map_err(|_| denied())?;
            let stored_digest: Vec<u8> = r.try_get("binding_digest").map_err(unavailable)?;
            let digest = binding_digest(&binding)?;
            if stored_digest != digest {
                return Err(denied());
            }
            Ok(Intent {
                id,
                binding,
                binding_digest: digest,
                request_digest: array(r.try_get("request_digest").map_err(unavailable)?)?,
                capability_hash: array(r.try_get("capability_hash").map_err(unavailable)?)?,
                sealed: r.try_get("sealed").map_err(unavailable)?,
            })
        })
        .transpose()
    }
    /// Capture on the SAME connection that will commit cursor/read state. The
    /// separate durable reference is not itself evidence of a committed effect.
    pub async fn capture(
        &self,
        connection: &mut PgConnection,
        intent: &Intent,
        deadline: AuthorityRpcDeadline,
    ) -> Result<NativeIdentity, Status> {
        bounded(deadline, async {
            let r = sqlx::query("select pg_current_xact_id()::text as full_xid,(select system_identifier::text from pg_control_system()) as system_identifier,(select oid::bigint from pg_database where datname=current_database()) as database_oid,pg_backend_pid() as backend_pid,(select (extract(epoch from backend_start)*1000000)::bigint from pg_stat_activity where pid=pg_backend_pid()) as backend_started_micros")
                .fetch_one(connection).await.map_err(unavailable)?;
            let mut effect = Sha256::new();
            effect.update(b"mcr notification inbox effect v1\0");
            effect.update(intent.binding_digest);
            effect.update(intent.request_digest);
            let identity = NativeIdentity {
                system_identifier: r.try_get("system_identifier").map_err(unavailable)?,
                database_oid: u32::try_from(r.try_get::<i64,_>("database_oid").map_err(unavailable)?).map_err(|_| denied())?,
                full_xid: r.try_get("full_xid").map_err(unavailable)?,
                backend_pid: r.try_get("backend_pid").map_err(unavailable)?,
                backend_started_micros: r.try_get("backend_started_micros").map_err(unavailable)?,
                producer_instance: self.instance,
                effect_digest: effect.finalize().into(),
            };
            let digest = identity.digest(intent.id)?;
            sqlx::query("insert into concert_notification_native_references(child_id,identity_json,identity_digest) values($1,$2,$3) on conflict do nothing")
                .bind(intent.id).bind(json(&identity)?).bind(digest.as_slice()).execute(&self.pool).await.map_err(unavailable)?;
            let stored = self.native(intent.id).await?.ok_or_else(denied)?;
            if stored.0 != identity { return Err(denied()); }
            Ok(identity)
        }).await
    }
    /// Called ONLY after the corresponding actual owner Track reply. No public
    /// wire request can supply these acknowledgements or manufacture a witness.
    pub async fn acknowledge(
        &self,
        intent: &Intent,
        identity: &NativeIdentity,
        auth: Uuid,
        friend: Uuid,
        deadline: AuthorityRpcDeadline,
    ) -> Result<(), Status> {
        if auth.is_nil() || friend.is_nil() {
            return Err(denied());
        }
        bounded(deadline, async {
            let affected = sqlx::query("update concert_notification_native_references set auth_registration=$3,friend_registration=$4 where child_id=$1 and identity_digest=$2 and (auth_registration is null or auth_registration=$3) and (friend_registration is null or friend_registration=$4)")
                .bind(intent.id).bind(identity.digest(intent.id)?.as_slice()).bind(auth).bind(friend).execute(&self.pool).await.map_err(unavailable)?.rows_affected();
            if affected != 1 { return Err(denied()); }
            Ok(())
        }).await
    }
    pub async fn witness(
        &self,
        connection: &mut PgConnection,
        intent: &Intent,
        identity: &NativeIdentity,
        deadline: AuthorityRpcDeadline,
    ) -> Result<(), Status> {
        bounded(deadline, async {
            // Read fresh owner metadata on the separate connection. The protected
            // RR snapshot predates these rows and must not be silently weakened.
            let (stored, auth, friend) = self.native(intent.id).await?.ok_or_else(denied)?;
            if stored != *identity || auth.is_none() || friend.is_none() { return Err(denied()); }
            sqlx::query("insert into concert_notification_transaction_witnesses(child_id,identity_json,identity_digest) values($1,$2,$3)")
                .bind(intent.id).bind(json(identity)?).bind(identity.digest(intent.id)?.as_slice()).execute(connection).await.map_err(unavailable)?;
            Ok(())
        }).await
    }
    /// Sealing races the witness's retained header row lock. It cannot permit a
    /// late commit after returning a terminal native outcome.
    pub async fn seal(
        &self,
        intent: &Intent,
        deadline: AuthorityRpcDeadline,
    ) -> Result<NativeOutcome, Status> {
        bounded(deadline, async {
            sqlx::query("update concert_notification_authority_intents set sealed=true where child_id=$1 and not sealed")
                .bind(intent.id).execute(&self.pool).await.map_err(unavailable)?;
            match self.native(intent.id).await? {
                Some((identity, _, _)) => self.outcome(intent, &identity).await,
                None => {
                    // The durable seal serializes against native assignment and
                    // witnesses. This is positive never-started exclusion proof,
                    // not inference from a missing transaction/GID or timeout.
                    let witnessed: bool = sqlx::query_scalar("select exists(select 1 from concert_notification_transaction_witnesses where child_id=$1)")
                        .bind(intent.id).fetch_one(&self.pool).await.map_err(unavailable)?;
                    Ok(if witnessed { NativeOutcome::Unknown } else { NativeOutcome::NoEffect })
                },
            }
        }).await
    }
    pub async fn resolve(
        &self,
        binding: &lease::GuardBinding,
        transaction: &lease::TransactionIdentity,
        registration: &str,
        auth_owner: bool,
        deadline: AuthorityRpcDeadline,
    ) -> Result<(Intent, NativeOutcome), Status> {
        let registration = canonical(registration)?;
        let id = Uuid::from_slice(&transaction.settlement_reference).map_err(|_| denied())?;
        if id.is_nil() || transaction.producer != lease::Producer::Notification as i32 {
            return Err(denied());
        }
        bounded(deadline, async {
            let intent = self.load(id).await?.ok_or_else(denied)?;
            if intent.binding != *binding {
                return Err(denied());
            }
            let (identity, auth, friend) = self.native(id).await?.ok_or_else(denied)?;
            let expected = if auth_owner { auth } else { friend };
            if identity.wire(id) != *transaction || expected.is_some_and(|id| id != registration) {
                return Err(denied());
            }
            let outcome = self.outcome(&intent, &identity).await?;
            Ok((intent, outcome))
        })
        .await
    }
    async fn native(
        &self,
        id: Uuid,
    ) -> Result<Option<(NativeIdentity, Option<Uuid>, Option<Uuid>)>, Status> {
        let row = sqlx::query("select identity_json,identity_digest,auth_registration,friend_registration from concert_notification_native_references where child_id=$1")
            .bind(id).fetch_optional(&self.pool).await.map_err(unavailable)?;
        row.map(|r| {
            let identity: NativeIdentity =
                serde_json::from_value(r.try_get("identity_json").map_err(unavailable)?)
                    .map_err(|_| denied())?;
            let digest: Vec<u8> = r.try_get("identity_digest").map_err(unavailable)?;
            if digest != identity.digest(id)? {
                return Err(denied());
            }
            Ok((
                identity,
                r.try_get("auth_registration").map_err(unavailable)?,
                r.try_get("friend_registration").map_err(unavailable)?,
            ))
        })
        .transpose()
    }
    async fn outcome(
        &self,
        intent: &Intent,
        identity: &NativeIdentity,
    ) -> Result<NativeOutcome, Status> {
        if identity.producer_instance != self.instance {
            return Ok(NativeOutcome::Unknown);
        }
        let store: (String, i64) = sqlx::query_as("select (select system_identifier::text from pg_control_system()),(select oid::bigint from pg_database where datname=current_database())")
            .fetch_one(&self.pool).await.map_err(unavailable)?;
        if store
            != (
                identity.system_identifier.clone(),
                i64::from(identity.database_oid),
            )
        {
            return Ok(NativeOutcome::Unknown);
        }
        let status: Option<String> = sqlx::query_scalar("select pg_xact_status($1::text::xid8)")
            .bind(&identity.full_xid)
            .fetch_one(&self.pool)
            .await
            .map_err(unavailable)?;
        let witness = sqlx::query("select identity_json,identity_digest from concert_notification_transaction_witnesses where child_id=$1")
            .bind(intent.id).fetch_optional(&self.pool).await.map_err(unavailable)?;
        let witnessed = match &witness {
            Some(r) => {
                r.try_get::<serde_json::Value, _>("identity_json")
                    .map_err(unavailable)?
                    == json(identity)?
                    && r.try_get::<Vec<u8>, _>("identity_digest")
                        .map_err(unavailable)?
                        == identity.digest(intent.id)?
            }
            None => false,
        };
        Ok(match status.as_deref() {
            Some("in progress") if witness.is_none() => NativeOutcome::InProgress,
            Some("committed") if witnessed => NativeOutcome::Committed,
            Some("aborted") if witness.is_none() => NativeOutcome::Aborted,
            _ => NativeOutcome::Unknown,
        })
    }
}
impl Intent {
    fn matches(&self, capability: &[u8]) -> bool {
        owner_binding::secret_matches(capability, &self.capability_hash)
    }
}
impl NativeIdentity {
    fn digest(&self, reference: Uuid) -> Result<[u8; 32], Status> {
        owner_binding::identity_digest(
            "NOTIFICATION",
            &self.system_identifier,
            self.database_oid,
            &self.full_xid,
            self.backend_pid,
            self.backend_started_micros,
            &self.effect_digest,
            self.producer_instance,
            reference.as_bytes(),
        )
    }
    pub fn wire(&self, reference: Uuid) -> lease::TransactionIdentity {
        lease::TransactionIdentity {
            producer: lease::Producer::Notification as i32,
            full_xid: self.full_xid.clone(),
            backend_pid: self.backend_pid,
            backend_started_unix_micros: self.backend_started_micros,
            effect_binding_digest: self.effect_digest.to_vec(),
            system_identifier: self.system_identifier.clone(),
            database_oid: self.database_oid,
            settlement_reference: reference.as_bytes().to_vec(),
            producer_instance_id: self.producer_instance.to_string(),
        }
    }
}
fn array(value: Vec<u8>) -> Result<[u8; 32], Status> {
    value.try_into().map_err(|_| denied())
}
fn json(value: &NativeIdentity) -> Result<serde_json::Value, Status> {
    serde_json::to_value(value).map_err(|_| denied())
}
fn denied() -> Status {
    Status::permission_denied("Notification native binding denied")
}
fn unavailable(_: sqlx::Error) -> Status {
    Status::unavailable("Notification native evidence unavailable")
}
async fn bounded<T>(
    deadline: AuthorityRpcDeadline,
    work: impl std::future::Future<Output = Result<T, Status>>,
) -> Result<T, Status> {
    deadline.remaining(std::time::Duration::from_secs(5), deadline.expires_micros())?;
    let result = tokio::time::timeout_at(deadline.stop(), work)
        .await
        .map_err(|_| Status::deadline_exceeded("Notification native evidence deadline"))??;
    deadline.remaining(std::time::Duration::from_secs(5), deadline.expires_micros())?;
    Ok(result)
}
