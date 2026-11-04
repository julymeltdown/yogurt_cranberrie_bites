//! Native effect-fence fixture copied from tests/avatar.rs at 980fc4d.
//! Real transaction registration/SQL constraints; fixture Auth/Friend registrations.
//! This does not certify real public authentication or operator admission.
use super::*;
enum WitnessFault {
    Healthy,
    MissingDescriptor,
    WrongRelease,
    WrongBody,
    WrongScope,
    Unpublished,
    Unconfirmed,
    AfterWitness,
    AfterCommit,
}
struct NativeEffectFence {
    pool: sqlx::PgPool,
    registered: std::sync::Mutex<Option<AvatarTransactionIdentity>>,
    reject_registration: bool,
    reject_after_effect: bool,
    witness: AvatarEffectWitnessBinding,
    owner_binding: Value,
    fault: WitnessFault,
}
#[async_trait::async_trait]
impl AvatarEffectFence for NativeEffectFence {
    async fn register(&self, identity: AvatarTransactionIdentity) -> Result<(), AvatarError> {
        let active: (i64, String, chrono::DateTime<chrono::Utc>, String) = sqlx::query_as(
            "select datid::bigint,backend_xid::text,backend_start,state from pg_stat_activity where pid=$1"
        ).bind(identity.backend_pid).fetch_one(&self.pool).await.unwrap();
        assert_eq!(active.0, i64::from(identity.database_oid));
        assert_eq!(
            active.1.parse::<u64>().unwrap(),
            identity.full_xid.parse::<u64>().unwrap() & 0xffff_ffff
        );
        assert_eq!(active.2, identity.backend_started);
        assert_eq!(active.3, "idle in transaction");
        let sysid: String =
            sqlx::query_scalar("select system_identifier::text from pg_control_system()")
                .fetch_one(&self.pool)
                .await
                .unwrap();
        assert_eq!(identity.system_identifier, sysid);
        *self.registered.lock().unwrap() = Some(identity);
        if self.reject_registration {
            return Err(AvatarError::Unavailable);
        }
        // Explicit component fixture: real SQL scope/identity constraints, not
        // real Auth/Friend registration or a production admission capability.
        let identity = self.registered.lock().unwrap().clone().unwrap();
        let transaction = concert_service::product_rpc::scope_ledger::ProducerTransaction {
            producer: "CONCERT".into(),
            system_identifier: identity.system_identifier,
            database_oid: identity.database_oid,
            full_xid: identity.full_xid,
            backend_pid: identity.backend_pid,
            backend_started_unix_micros: identity.backend_started.timestamp_micros(),
            effect_binding_digest: self.witness.effect_binding_digest,
            producer_instance: self.witness.producer_instance_id,
            settlement_reference: self.witness.operation_id.as_bytes().to_vec(),
        };
        let digest = transaction.digest().unwrap();
        let delivery = if matches!(self.fault, WitnessFault::Unpublished) {
            None
        } else {
            Some(vec![77u8; 32])
        };
        sqlx::query("insert into concert_product_owner_scopes(instance_id,scope_id,binding_digest,binding,delivery_secret_hash) values($1,$2,$3,$4,$5)")
            .bind(self.witness.producer_instance_id).bind(self.witness.scope_id)
            .bind(self.witness.guard_binding_digest.as_slice()).bind(&self.owner_binding).bind(delivery)
            .execute(&self.pool).await.unwrap();
        let confirmed = if matches!(self.fault, WitnessFault::Unconfirmed) {
            None
        } else {
            Some(chrono::Utc::now())
        };
        sqlx::query("insert into concert_product_owner_transactions(instance_id,scope_id,binding_digest,identity_digest,identity,auth_registration_id,friend_registration_id,confirmed_at) values($1,$2,$3,$4,$5,$6,$7,$8)")
            .bind(self.witness.producer_instance_id).bind(self.witness.scope_id)
            .bind(self.witness.guard_binding_digest.as_slice()).bind(digest.as_slice())
            .bind(serde_json::to_value(transaction).unwrap()).bind(Uuid::new_v4()).bind(Uuid::new_v4())
            .bind(confirmed).execute(&self.pool).await.unwrap();
        Ok(())
    }
    fn witness_binding(&self) -> Result<AvatarEffectWitnessBinding, AvatarError> {
        let mut value = self.witness.clone();
        match self.fault {
            WitnessFault::MissingDescriptor => return Err(AvatarError::Unavailable),
            WitnessFault::WrongRelease => value.release_digest = "b".repeat(64),
            WitnessFault::WrongBody => value.body_digest = [99; 32],
            WitnessFault::WrongScope => value.scope_id = Uuid::from_u128(42),
            _ => (),
        }
        Ok(value)
    }
    async fn check(&self) -> Result<(), AvatarError> {
        let identity = self
            .registered
            .lock()
            .unwrap()
            .clone()
            .expect("registered before checks");
        if matches!(self.fault, WitnessFault::AfterCommit) {
            let committed: bool = sqlx::query_scalar("select exists(select 1 from concert_participant_effect_witnesses where operation_id=$1)")
                .bind(self.witness.operation_id).fetch_one(&self.pool).await.unwrap();
            if committed {
                return Err(AvatarError::Unavailable);
            }
        }
        if self.reject_after_effect || matches!(self.fault, WitnessFault::AfterWitness) {
            let query: String =
                sqlx::query_scalar("select query from pg_stat_activity where pid=$1")
                    .bind(identity.backend_pid)
                    .fetch_one(&self.pool)
                    .await
                    .unwrap();
            if self.reject_after_effect && query.starts_with("update public.concert_")
                || matches!(self.fault, WitnessFault::AfterWitness)
                    && query.starts_with("insert into public.concert_participant_effect_witnesses")
            {
                return Err(AvatarError::Denied);
            }
        }
        Ok(())
    }
}
pub(super) struct FencedAvatarAuthority {
    base: support::Authority,
    fence: NativeEffectFence,
}
impl AvatarAuthority for FencedAvatarAuthority {
    fn check(&self) -> Result<AvatarGrant, AvatarError> {
        self.base.check()
    }
    fn effect_fence(&self) -> Option<&dyn AvatarEffectFence> {
        Some(&self.fence)
    }
}

pub(super) fn authority(
    key: ParticipantKey,
    digest: &str,
    body: &[u8],
    pool: &sqlx::PgPool,
    reject_after_effect: bool,
) -> FencedAvatarAuthority {
    let binding = platform::actor_delegation::owner_binding::OwnerBinding {
        actor_id: key.user_id.uuid(),
        event_id: key.event_id.uuid(),
        session_family_id: Uuid::new_v4(),
        request_id: Uuid::new_v4(),
        operation: "component_avatar_witness".into(),
        expires_unix_micros: (chrono::Utc::now() + chrono::Duration::minutes(5)).timestamp_micros(),
        parent_digest: [11; 32],
        parent_body_digest: [12; 32],
        parent_method: "/concert.participant.v1.ConcertParticipantService/SetAvatar".into(),
        http_method: "PUT".into(),
        http_target: format!("/api/concert/events/{}/avatar", key.event_id.uuid()),
        body_digest: Sha256::digest(body).into(),
    };
    let witness = AvatarEffectWitnessBinding {
        producer_instance_id: Uuid::new_v4(),
        scope_id: Uuid::new_v4(),
        operation_id: binding.request_id,
        session_family_id: binding.session_family_id,
        parent_digest: binding.parent_digest,
        guard_binding_digest: binding.digest().unwrap(),
        body_digest: binding.body_digest,
        release_digest: digest.into(),
        effect_binding_digest: [44; 32],
    };
    FencedAvatarAuthority {
        base: support::Authority::new(key, AvatarOperation::SetAvatar, digest),
        fence: NativeEffectFence {
            pool: pool.clone(),
            registered: std::sync::Mutex::new(None),
            reject_registration: false,
            reject_after_effect,
            witness,
            owner_binding: serde_json::to_value(binding).unwrap(),
            fault: WitnessFault::Healthy,
        },
    }
}
