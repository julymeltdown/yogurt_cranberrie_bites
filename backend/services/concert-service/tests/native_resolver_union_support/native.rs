//! Actual canonical guards and actual producer witness implementations.
//! Initial identities/rows are explicit fixtures, not browser signup proof.
use super::{environment, fixture, participant_capture, product_capture};
use concert_application::{
    age_policy::Context,
    avatar::{
        AvatarAuthority, AvatarEffectFence, AvatarEffectWitnessBinding, AvatarError, AvatarGrant,
        AvatarMutation, AvatarOperation, AvatarRepository, AvatarTransactionIdentity,
    },
    commands::{FingerprintKeys, SigningKey},
};
use concert_domain::{
    avatar::AvatarCatalog, AuthUserId, EventId, ParticipantKey, Preferences, Revision, Theme,
};
use concert_service::adapters::out::relationship_authority::guarded::proto::TransactionStatus as Outcome;
use concert_service::{
    adapters::out::{
        age_provider::{scoped::ScopedAgeAuthority, Access, Authority},
        relationship_authority::guarded::{
            OwnerGuardScope, ProducerKind, ProducerTransaction as GuardTransaction,
        },
        repository::avatar::PgAvatarRepository,
    },
    config::native_resolver::{NativeResolverDomain as Domain, NativeResolverUnion},
    operations_live::{OperationsOwners, OperationsRuntime},
    product_rpc::{
        scope_ledger::ProducerTransaction,
        settlement::{
            NativeTransactionResolution, NativeTransactionResolver, PgNativeAbortResolver,
        },
    },
    relationship::social::{SocialEffect, SocialTransactionHooks},
    social_live::witness::SocialWitness,
};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use uuid::Uuid;

async fn product(
    f: &fixture::Fixture,
    actor: Uuid,
    family: Uuid,
    event: Uuid,
    operation: &str,
    method: &str,
    suffix: &str,
    body: Vec<u8>,
    key: Option<Uuid>,
) -> anyhow::Result<(
    Arc<OwnerGuardScope>,
    concert_service::product_rpc::proto::ProductRequest,
)> {
    let (binding, request) =
        product_capture::binding(actor, family, event, operation, method, suffix, body, key)
            .await?;
    let scope = f.fence.acquire(binding).await?;
    f.bind_delivery(&scope).await?;
    Ok((scope, request))
}
async fn identity(
    f: &fixture::Fixture,
    scope: &OwnerGuardScope,
) -> anyhow::Result<ProducerTransaction> {
    let value:serde_json::Value=sqlx::query_scalar("select identity from concert_product_owner_transactions where instance_id=$1 and scope_id=$2")
        .bind(f.ledger.instance()).bind(Uuid::parse_str(&scope.producer_scope().scope_id)?).fetch_one(&f.concert).await?;
    Ok(serde_json::from_value(value)?)
}
async fn finish(scope: &OwnerGuardScope) -> anyhow::Result<()> {
    scope.mark_serialized().await?;
    scope.finish_response().await?;
    Ok(())
}
fn ledger(native: GuardTransaction) -> ProducerTransaction {
    ProducerTransaction {
        producer: "CONCERT".into(),
        system_identifier: native.system_identifier,
        database_oid: native.database_oid,
        full_xid: native.full_xid,
        backend_pid: native.backend_pid,
        backend_started_unix_micros: native.backend_started.timestamp_micros(),
        effect_binding_digest: native.effect_binding_digest,
        producer_instance: native.producer_instance_id,
        settlement_reference: native.settlement_reference,
    }
}
async fn committed(
    f: &fixture::Fixture,
    scope: &OwnerGuardScope,
    expected: Domain,
) -> anyhow::Result<(ProducerTransaction, NativeTransactionResolution)> {
    let binding = scope.binding().owner_binding();
    let identity = identity(f, scope).await?;
    assert_eq!(NativeResolverUnion::domain(&binding)?, expected);
    finish(scope).await?;
    let union = NativeResolverUnion::new(f.concert.clone(), f.ledger.instance())?;
    let proof = union.resolve(&binding, &identity).await?;
    assert_eq!(proof.status, Outcome::Committed);
    assert!(!proof.receipt_evidence.is_empty());
    assert_ne!(
        identity.effect_binding_digest,
        binding.digest()?,
        "effect and guard are distinct bindings"
    );
    let foreign = NativeResolverUnion::new(f.concert.clone(), Uuid::new_v4())?;
    assert_eq!(
        foreign.resolve(&binding, &identity).await?.status,
        Outcome::Unknown
    );
    for dimension in 0..3 {
        let mut wrong = identity.clone();
        match dimension {
            0 => wrong.database_oid = 1,
            1 => wrong.system_identifier = "1".into(),
            _ => wrong.effect_binding_digest[0] ^= 1,
        }
        let result = union.resolve(&binding, &wrong).await?;
        assert_eq!(result.status, Outcome::Unknown);
        assert!(result.receipt_evidence.is_empty());
    }
    Ok((identity, proof))
}

/// Fixture adapter forwards real held-scope checks/registration. It cannot
/// insert a witness: the retained PgAvatarRepository writes that in its actual
/// revision-effect transaction and the immutable SQL trigger validates it.
struct PreferencesFence {
    scope: Arc<OwnerGuardScope>,
    key: ParticipantKey,
}
impl PreferencesFence {
    fn effect(&self) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(b"mcr participant avatar effect v1\0");
        h.update(self.scope.binding().digest());
        h.update("8".repeat(64));
        h.update(self.scope.binding().owner_binding().body_digest);
        h.finalize().into()
    }
}
impl AvatarAuthority for PreferencesFence {
    fn check(&self) -> Result<AvatarGrant, AvatarError> {
        self.scope
            .binding()
            .check()
            .map_err(|_| AvatarError::Unavailable)?;
        Ok(AvatarGrant {
            key: self.key,
            operation: AvatarOperation::SetPreferences,
            release_digest: "8".repeat(64),
        })
    }
    fn effect_fence(&self) -> Option<&dyn AvatarEffectFence> {
        Some(self)
    }
}
#[async_trait::async_trait]
impl AvatarEffectFence for PreferencesFence {
    async fn register(&self, n: AvatarTransactionIdentity) -> Result<(), AvatarError> {
        let instance = Uuid::parse_str(&self.scope.producer_scope().instance_id)
            .map_err(|_| AvatarError::Unavailable)?;
        self.scope
            .track_transaction(GuardTransaction {
                producer: ProducerKind::Concert,
                system_identifier: n.system_identifier,
                database_oid: n.database_oid,
                full_xid: n.full_xid,
                backend_pid: n.backend_pid,
                backend_started: n.backend_started,
                effect_binding_digest: self.effect(),
                producer_instance_id: instance,
                settlement_reference: self.scope.binding().request_id().as_bytes().to_vec(),
            })
            .await
            .map_err(|_| AvatarError::Unavailable)
    }
    async fn check(&self) -> Result<(), AvatarError> {
        self.scope
            .check_subject(concert_domain::relationship::PolicyQuery {
                actor: self.key.user_id,
                event: self.key.event_id,
                subject: self.key.user_id,
                field: concert_domain::relationship::Field::SafetySelf,
            })
            .await
            .map_err(|_| AvatarError::Unavailable)
    }
    fn witness_binding(&self) -> Result<AvatarEffectWitnessBinding, AvatarError> {
        let b = self.scope.binding().owner_binding();
        let s = self.scope.producer_scope();
        Ok(AvatarEffectWitnessBinding {
            producer_instance_id: Uuid::parse_str(&s.instance_id)
                .map_err(|_| AvatarError::Unavailable)?,
            scope_id: Uuid::parse_str(&s.scope_id).map_err(|_| AvatarError::Unavailable)?,
            operation_id: b.request_id,
            session_family_id: b.session_family_id,
            parent_digest: b.parent_digest,
            guard_binding_digest: self.scope.binding().digest(),
            body_digest: b.body_digest,
            release_digest: "8".repeat(64),
            effect_binding_digest: self.effect(),
        })
    }
}

#[tokio::test]
#[ignore = "frozen CU01 owned socket-only PG16 and actual canonical mTLS fixtures required"]
async fn actual_producer_witnesses_dispatch_without_unknown_or_error_fallback() -> anyhow::Result<()>
{
    let f = fixture::Fixture::new().await?;
    let (actor, family, subjects) = environment::identities(&f.auth, 1).await?;
    let subject = subjects[0];
    let event = Uuid::new_v4();
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'CU01 native union','OPEN')")
        .bind(event).bind(format!("cu01-{event}")).execute(&f.concert).await?;
    for user in [actor, subject] {
        sqlx::query(
            "insert into concert_event_admissions(event_id,user_id,state) values($1,$2,'ACTIVE')",
        )
        .bind(event)
        .bind(user)
        .execute(&f.concert)
        .await?;
    }
    sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,false)")
        .bind(event).bind(actor).execute(&f.concert).await?;
    // Selected source-pack metadata only. No image is copied or generated.
    let manifest: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../contracts/fixtures/assets/selected-static/avatar-manifest.json"
    ))?;
    let catalog = AvatarCatalog::from_validated_manifest(&manifest)
        .map_err(|_| anyhow::anyhow!("selected catalog"))?;
    let preset = &manifest["presets"][0]["config"];
    sqlx::query("insert into concert_avatar_settings(event_id,user_id,contract_version,representation,preset_id,manifest_id,manifest_version) values($1,$2,'1.1.0','whole_character',$3,$4,1)")
        .bind(event).bind(actor).bind(preset["preset_id"].as_str().unwrap()).bind(manifest["manifest_id"].as_str().unwrap()).execute(&f.concert).await?;
    let body = br#"{"expected_revision":1,"theme":"PARADE"}"#.to_vec();
    let (binding, _) = participant_capture::binding(
        actor,
        family,
        event,
        "updateConcertParticipant",
        "PATCH",
        "me",
        body,
        None,
    )
    .await?;
    let s = f.fence.acquire(binding).await?;
    f.bind_delivery(&s).await?;
    let key = ParticipantKey {
        user_id: AuthUserId::from_uuid(actor),
        event_id: EventId::from_uuid(event),
    };
    let auth = PreferencesFence {
        scope: s.clone(),
        key,
    };
    PgAvatarRepository::new(f.concert.clone())
        .execute(
            key,
            AvatarMutation::Preferences {
                expected: Revision::new(1).unwrap(),
                value: Preferences {
                    theme: Theme::Parade,
                    low_power: false,
                    reduced_motion: false,
                    sound_enabled: false,
                },
            },
            &catalog,
            &"8".repeat(64),
            &auth,
        )
        .await
        .map_err(|e| anyhow::anyhow!("actual preferences producer {e:?}"))?;
    let (_, participant) = committed(&f, &s, Domain::Participant).await?;

    let (s, _) = product(
        &f,
        actor,
        family,
        event,
        "getConcertAgeStatus",
        "GET",
        "me/age-status",
        vec![],
        None,
    )
    .await?;
    let b = s.binding();
    let context = Context {
        actor: key.user_id,
        event: key.event_id,
        session: family,
        request: b.request_id(),
        expires_at: chrono::DateTime::from_timestamp_micros(b.deadline().expires_micros()).unwrap(),
    };
    let mut tx = f.concert.begin().await?;
    ScopedAgeAuthority::new(s.clone())
        .authorize_in(&mut tx, context, Access::Read)
        .await
        .map_err(|e| anyhow::anyhow!("actual age authority {e:?}"))?;
    tx.commit().await?;
    let (_, age) = committed(&f, &s, Domain::Age).await?;

    let (s, _) = product(
        &f,
        actor,
        family,
        event,
        "listOwnConcertHandles",
        "GET",
        "me/social-handles",
        vec![],
        None,
    )
    .await?;
    let mut tx = f.concert.begin().await?;
    let witness = SocialWitness::new(s.clone(), None);
    witness
        .register(&mut tx)
        .await
        .map_err(|e| anyhow::anyhow!("social register {e:?}"))?;
    let _: i64 = sqlx::query_scalar(
        "select count(*) from concert_social_handles where event_id=$1 and user_id=$2",
    )
    .bind(event)
    .bind(actor)
    .fetch_one(&mut *tx)
    .await?;
    witness
        .succeeded(
            &mut tx,
            SocialEffect {
                operation: "listOwnConcertHandles",
                resource: actor,
                revision: 0,
                projection: true,
            },
        )
        .await
        .map_err(|e| anyhow::anyhow!("actual social projection {e:?}"))?;
    tx.commit().await?;
    let (social_identity, social) = committed(&f, &s, Domain::Social).await?;
    let social_binding = s.binding().owner_binding();

    let body = serde_json::to_vec(
        &serde_json::json!({"target_type":"USER","target_id":subject,"reason":"SPAM","text":"CU01 fixture","evidence_media_ids":[]}),
    )?;
    let (s, request) = product(
        &f,
        actor,
        family,
        event,
        "createConcertReport",
        "POST",
        "reports",
        body,
        Some(Uuid::new_v4()),
    )
    .await?;
    let keys = Arc::new(FingerprintKeys::new(
        SigningKey::new("cu01-fixture", &[91; 32])?,
        vec![],
    )?);
    let runtime = OperationsRuntime::new(
        f.concert.clone(),
        OperationsOwners::default(),
        keys,
        &[92; 32],
    )?;
    let reply = runtime
        .dispatch(s.clone(), request)
        .await
        .map_err(|e| anyhow::anyhow!("actual operations {e:?}"))?;
    assert_eq!(reply.status, 202);
    let (_, operations) = committed(&f, &s, Domain::Operations).await?;
    assert_ne!(participant.receipt_evidence, operations.receipt_evidence);
    assert_ne!(social.receipt_evidence, age.receipt_evidence);

    // A selected adapter error must survive; AbortOnly would instead return
    // UNKNOWN for this committed XID. Restore the isolated table before assert.
    sqlx::query(
        "alter table concert_social_effect_witnesses rename to cu01_hidden_social_witnesses",
    )
    .execute(&f.concert)
    .await?;
    let result = NativeResolverUnion::new(f.concert.clone(), f.ledger.instance())?
        .resolve(&social_binding, &social_identity)
        .await;
    sqlx::query(
        "alter table cu01_hidden_social_witnesses rename to concert_social_effect_witnesses",
    )
    .execute(&f.concert)
    .await?;
    assert!(matches!(result,Err(e) if e.code()==tonic::Code::Unavailable));

    let union = NativeResolverUnion::new(f.concert.clone(), f.ledger.instance())?;
    let (s, _) = product(
        &f,
        actor,
        family,
        event,
        "createConcertParty",
        "POST",
        "parties",
        b"{}".to_vec(),
        Some(Uuid::new_v4()),
    )
    .await?;
    let mut tx = f.concert.begin().await?;
    let n = GuardTransaction::capture_concert_in(
        &mut tx,
        [71; 32],
        f.ledger.instance(),
        s.binding().request_id().as_bytes().to_vec(),
    )
    .await?;
    s.track_transaction(n.clone()).await?;
    let i = ledger(n);
    let b = s.binding().owner_binding();
    assert_eq!(NativeResolverUnion::domain(&b)?, Domain::AbortOnly);
    assert_eq!(union.resolve(&b, &i).await?.status, Outcome::InProgress);
    tx.rollback().await?;
    assert_eq!(union.resolve(&b, &i).await?.status, Outcome::Aborted);
    finish(&s).await?;

    // Unsupported actual COMMIT and selected Participant UNKNOWN stay unknown.
    let (s, _) = product(
        &f,
        actor,
        family,
        event,
        "createConcertParty",
        "POST",
        "parties",
        b"{}".to_vec(),
        Some(Uuid::new_v4()),
    )
    .await?;
    let mut tx = f.concert.begin().await?;
    let n = GuardTransaction::capture_concert_in(
        &mut tx,
        [72; 32],
        f.ledger.instance(),
        s.binding().request_id().as_bytes().to_vec(),
    )
    .await?;
    s.track_transaction(n.clone()).await?;
    tx.commit().await?;
    let proof = union
        .resolve(&s.binding().owner_binding(), &ledger(n))
        .await?;
    assert_eq!(proof.status, Outcome::Unknown);
    assert!(proof.receipt_evidence.is_empty());
    assert!(finish(&s).await.is_err());
    let (b, _) = participant_capture::binding(
        actor,
        family,
        event,
        "updateConcertParticipant",
        "PATCH",
        "me",
        b"{}".to_vec(),
        None,
    )
    .await?;
    let s = f.fence.acquire(b).await?;
    f.bind_delivery(&s).await?;
    let mut tx = f.concert.begin().await?;
    let n = GuardTransaction::capture_concert_in(
        &mut tx,
        [73; 32],
        f.ledger.instance(),
        b"not-the-request-reference".to_vec(),
    )
    .await?;
    s.track_transaction(n.clone()).await?;
    tx.rollback().await?;
    let i = ledger(n);
    let b = s.binding().owner_binding();
    assert_eq!(
        PgNativeAbortResolver::new(f.concert.clone(), f.ledger.instance())?
            .resolve(&b, &i)
            .await?
            .status,
        Outcome::Aborted
    );
    assert_eq!(
        union.resolve(&b, &i).await?.status,
        Outcome::Unknown,
        "must not fall through after Participant UNKNOWN"
    );
    assert!(finish(&s).await.is_err());
    let prepared: i64 = sqlx::query_scalar("select count(*) from pg_prepared_xacts")
        .fetch_one(&f.concert)
        .await?;
    assert_eq!(
        prepared, 4,
        "two expected unresolved scopes retain both owner fences"
    );
    println!("CU01: four actual producer-specific commit witnesses; foreign identity refusals; actual abort/in-progress; UNKNOWN/error no fallback; four expected prepared fences before whole-owned teardown");
    Ok(())
}
