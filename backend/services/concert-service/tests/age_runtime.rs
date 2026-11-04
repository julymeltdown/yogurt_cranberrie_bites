//! Real canonical Auth/Friend + private PostgreSQL. Only the external proof
//! verifier is synthetic; these cases do not certify a commercial provider.
#[path = "age_runtime_support/capture.rs"]
mod capture;
#[path = "age_runtime_support/environment.rs"]
mod environment;
#[path = "age_runtime_support/fixture.rs"]
mod fixture;
#[path = "age_runtime_support/owners.rs"]
mod owners;
use chrono::{DateTime, Duration, Utc};
use concert_application::{
    age_policy::{ClaimQuery, Context, PolicyError},
    commands::{FingerprintKeys, SigningKey},
    Clock,
};
use concert_domain::{
    age_policy::{AgeBounds, AgeStatus},
    AuthUserId, EventId,
};
use concert_service::{
    adapters::out::{
        age_provider::{
            claim_reader::PgClaimProjection,
            runtime::{AgeRuntime, ProviderMode},
            scoped::{ScopedAgeAuthority, ScopedProjectionAuthority},
            settlement::AgeTransactionResolver,
            trust::ClaimTrust,
            Access, Authority,
        },
        relationship_authority::guarded::OwnerGuardScope,
    },
    age_verification::*,
    product_rpc::{scope_ledger::ProducerTransaction, settlement::NativeTransactionResolver},
};
use sqlx::Row;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use uuid::Uuid;
struct RealClock;
impl Clock for RealClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}
struct SyntheticVerifier(AtomicUsize);
#[async_trait::async_trait]
impl Verifier for SyntheticVerifier {
    fn available(&self) -> bool {
        true
    }
    async fn verify(&self, c: &Challenge, _: &ProviderResponse) -> Result<VerifiedProof, Error> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(VerifiedProof {
            binding: c.binding.clone(),
            verified_at: Utc::now(),
            expires_at: Utc::now() + Duration::days(30),
            source_revision: "synthetic-v1".into(),
            birthyear: "2000".into(),
            birthday: "11-05".into(),
        })
    }
}
fn runtime(f: &fixture::Fixture, verifier: Option<Arc<SyntheticVerifier>>) -> AgeRuntime {
    let mode = verifier
        .map(|verifier| ProviderMode::Configured {
            verifier,
            config: ProviderConfig::new(
                "synthetic",
                "synthetic-v1",
                "https://verify.example/authorize",
                &["https://verify.example/authorize"],
            )
            .unwrap(),
        })
        .unwrap_or(ProviderMode::Unavailable);
    AgeRuntime::new(
        f.concert.clone(),
        Arc::new(RealClock),
        Arc::new(
            FingerprintKeys::new(SigningKey::new("age-native-v1", &[91; 32]).unwrap(), vec![])
                .unwrap(),
        ),
        mode,
    )
}
fn trusted_outage(f: &fixture::Fixture) -> AgeRuntime {
    runtime(f, None).with_claim_trust(ClaimTrust::new(&[("synthetic", "synthetic-v1")]).unwrap())
}
fn context(s: &OwnerGuardScope) -> Context {
    let b = s.binding();
    Context {
        actor: AuthUserId::from_uuid(b.actor()),
        event: EventId::from_uuid(b.event()),
        session: b.session_family(),
        request: b.request_id(),
        expires_at: DateTime::from_timestamp_micros(b.deadline().expires_micros()).unwrap(),
    }
}
async fn scope(
    f: &fixture::Fixture,
    a: Uuid,
    family: Uuid,
    event: Uuid,
    op: &str,
) -> anyhow::Result<Arc<OwnerGuardScope>> {
    let s = f
        .fence
        .acquire(capture::binding(a, family, event, op).await?)
        .await?;
    f.bind_delivery(&s).await?;
    Ok(s)
}
async fn finish(s: &OwnerGuardScope) -> anyhow::Result<()> {
    s.mark_serialized().await?;
    s.finish_response().await?;
    Ok(())
}
async fn seed(f: &fixture::Fixture) -> anyhow::Result<(Uuid, Uuid, Uuid)> {
    let (actor, family, _) = environment::identities(&f.auth, 0).await?;
    let event = Uuid::new_v4();
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Age native fixture','OPEN')").bind(event).bind(format!("age-{event}")).execute(&f.concert).await?;
    sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,true)").bind(event).bind(actor).execute(&f.concert).await?;
    Ok((actor, family, event))
}
#[tokio::test]
#[ignore = "frozen private PG16 and actual canonical mTLS owners required"]
async fn configured_canonical_callback_replay_and_unavailable_status() -> anyhow::Result<()> {
    let f = fixture::Fixture::new().await?;
    let (a, family, event) = seed(&f).await?;
    let v = Arc::new(SyntheticVerifier(AtomicUsize::new(0)));
    let r = runtime(&f, Some(v.clone()));
    assert!(r.verification_available());
    let key = Uuid::new_v4();
    let s = scope(&f, a, family, event, "startConcertAgeVerification").await?;
    let c = context(&s);
    let started = r.claims(s.clone()).start(c, key, RETURN_TARGET).await?;
    let auth = started.authorization.unwrap();
    let state = auth
        .authorization_url
        .split("state=")
        .nth(1)
        .unwrap()
        .split('&')
        .next()
        .unwrap()
        .to_owned();
    let id = auth.verification_id;
    assert_eq!(state.len(), 64);
    let replay = r.claims(s.clone()).start(c, key, RETURN_TARGET).await?;
    assert!(replay.authorization.is_none());
    assert_eq!(replay.receipt.id, started.receipt.id);
    finish(&s).await?;
    let s = scope(&f, a, family, event, "completeConcertAgeVerification").await?;
    let c = context(&s);
    let callback_key = Uuid::new_v4();
    let done = r
        .claims(s.clone())
        .callback(
            c,
            callback_key,
            id,
            StateToken::parse(state.clone())?,
            ProviderResponse::new("synthetic-exchange".into())?,
        )
        .await?;
    assert_eq!(done.status.unwrap().status, AgeStatus::Verified);
    assert_eq!(v.0.load(Ordering::SeqCst), 1);
    let replay = trusted_outage(&f)
        .claims(s.clone())
        .callback(
            c,
            callback_key,
            id,
            StateToken::parse(state)?,
            ProviderResponse::new("synthetic-exchange".into())?,
        )
        .await?;
    assert_eq!(replay.status.unwrap().status, AgeStatus::Verified);
    assert_eq!(v.0.load(Ordering::SeqCst), 1);
    finish(&s).await?;
    let s = scope(&f, a, family, event, "getConcertAgeStatus").await?;
    let status = trusted_outage(&f)
        .claims(s.clone())
        .status(context(&s))
        .await?;
    assert_eq!(status.status, AgeStatus::Verified);
    assert!(!status.verification_available);
    // Resolve a real committed witness; identity/ref/binding changes cannot be
    // upgraded to committed by the generic native abort resolver.
    let row = sqlx::query(
        "select identity from concert_age_runtime_witnesses where binding_digest=$1 limit 1",
    )
    .bind(s.binding().digest().as_slice())
    .fetch_one(&f.concert)
    .await?;
    let native: ProducerTransaction = serde_json::from_value(row.try_get("identity")?)?;
    let resolver = AgeTransactionResolver::new(f.concert.clone(), native.producer_instance)?;
    let proof = resolver
        .resolve(&s.binding().owner_binding(), &native)
        .await?;
    assert_eq!(proof.status,concert_service::adapters::out::relationship_authority::guarded::proto::TransactionStatus::Committed);
    assert!(!proof.receipt_evidence.is_empty());
    let mut mismatch = native.clone();
    mismatch.effect_binding_digest[0] ^= 1;
    assert_ne!(resolver.resolve(&s.binding().owner_binding(),&mismatch).await?.status,concert_service::adapters::out::relationship_authority::guarded::proto::TransactionStatus::Committed);
    assert!(
        sqlx::query("delete from concert_age_runtime_witnesses where binding_digest=$1")
            .bind(s.binding().digest().as_slice())
            .execute(&f.concert)
            .await
            .is_err()
    );
    finish(&s).await?;
    Ok(())
}
#[tokio::test]
#[ignore = "frozen private PG16 and actual canonical mTLS owners required"]
async fn projection_uses_same_registered_transaction_and_retains_claim_locks() -> anyhow::Result<()>
{
    let f = fixture::Fixture::new().await?;
    let (a, family, event) = seed(&f).await?;
    let s = scope(&f, a, family, event, "getConcertAgeStatus").await?;
    let c = context(&s);
    sqlx::query("insert into concert_age_owners(user_id) values($1)")
        .bind(a)
        .execute(&f.concert)
        .await?;
    let mut tx = f.concert.begin().await?;
    ScopedAgeAuthority::new(s.clone())
        .authorize_in(&mut tx, c, Access::Read)
        .await?;
    let expected: String = sqlx::query_scalar("select pg_current_xact_id()::text")
        .fetch_one(&mut *tx)
        .await?;
    let projection = runtime(&f, None).projection(s.clone());
    let q = ClaimQuery {
        context: c,
        subject: c.actor,
        requested_at: Utc::now(),
    };
    let pending = projection
        .project_in(&mut tx, q, move |conn, age| {
            Box::pin(async move {
                let actual: String = sqlx::query_scalar("select pg_current_xact_id()::text")
                    .fetch_one(conn)
                    .await
                    .map_err(|_| PolicyError::Unavailable)?;
                assert_eq!(actual, expected);
                assert_eq!(age.status(), AgeStatus::ProviderUnavailable);
                assert!(!age.permits(AgeBounds::new(Some(18), Some(30)).unwrap(), false));
                Ok(())
            })
        })
        .await?;
    let blocker = f.concert.clone();
    let writer = tokio::spawn(async move {
        sqlx::query("update concert_age_owners set user_id=user_id where user_id=$1")
            .bind(a)
            .execute(&blocker)
            .await
    });
    tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    assert!(!writer.is_finished());
    tx.commit().await?;
    pending.after_commit()?;
    writer.await??;
    let count: i64 = sqlx::query_scalar(
        "select count(*) from concert_age_runtime_witnesses where binding_digest=$1",
    )
    .bind(s.binding().digest().as_slice())
    .fetch_one(&f.concert)
    .await?;
    assert_eq!(
        count, 1,
        "Projection must reuse the registered native identity without a second witness"
    );
    finish(&s).await?;
    Ok(())
}
#[tokio::test]
#[ignore = "frozen private PG16 and actual canonical mTLS owners required"]
async fn foreign_context_and_unregistered_projection_are_denied() -> anyhow::Result<()> {
    let f = fixture::Fixture::new().await?;
    let (a, family, event) = seed(&f).await?;
    let s = scope(&f, a, family, event, "getConcertAgeStatus").await?;
    let c = context(&s);
    let projection = PgClaimProjection::new(
        f.concert.clone(),
        Arc::new(ScopedProjectionAuthority::new(s.clone(), false)),
        Arc::new(RealClock),
    );
    let mut tx = f.concert.begin().await?;
    let q = ClaimQuery {
        context: c,
        subject: c.actor,
        requested_at: Utc::now(),
    };
    assert!(projection
        .project_in(&mut tx, q, |_, _| Box::pin(async { Ok(()) }))
        .await
        .is_err());
    tx.rollback().await?;
    let mut foreign = c;
    foreign.session = Uuid::new_v4();
    assert!(runtime(&f, None)
        .claims(s.clone())
        .status(foreign)
        .await
        .is_err());
    finish(&s).await?;
    Ok(())
}
#[tokio::test]
#[ignore = "frozen private PG16 and actual canonical mTLS owners required"]
async fn unavailable_mode_and_closed_event_erasure() -> anyhow::Result<()> {
    let f = fixture::Fixture::new().await?;
    let (a, family, event) = seed(&f).await?;
    let r = runtime(&f, None);
    assert!(!r.verification_available());
    let s = scope(&f, a, family, event, "getConcertAgeStatus").await?;
    let status = r.claims(s.clone()).status(context(&s)).await?;
    assert_eq!(status.status, AgeStatus::ProviderUnavailable);
    finish(&s).await?;
    sqlx::query("delete from concert_participants where event_id=$1 and user_id=$2")
        .bind(event)
        .bind(a)
        .execute(&f.concert)
        .await?;
    sqlx::query("update concert_events set phase='ARCHIVED',revision=revision+1 where event_id=$1")
        .bind(event)
        .execute(&f.concert)
        .await?;
    let s = scope(&f, a, family, event, "eraseConcertAgePrivateData").await?;
    r.claims(s.clone()).expire_states(context(&s), 10).await?;
    finish(&s).await?;
    Ok(())
}

#[tokio::test]
#[ignore = "frozen private PG16 and actual canonical mTLS owners required"]
async fn accepted_claim_survives_outage_but_untrusted_newest_never_falls_back() -> anyhow::Result<()>
{
    // SQL records here are explicit synthetic policy inputs, not provider proof.
    // The configured callback proof path is qualified by the first native case.
    let f = fixture::Fixture::new().await?;
    let (a, family, event) = seed(&f).await?;
    sqlx::query("insert into concert_age_owners(user_id) values($1)")
        .bind(a)
        .execute(&f.concert)
        .await?;
    let older = Uuid::new_v4();
    sqlx::query("insert into concert_age_verifications(verification_id,user_id,event_id,session_id,verifier_id,source_revision,state_digest,return_target,created_at,expires_at,state) select $1,$2,$3,$4,'synthetic','synthetic-v1',decode(repeat('61',32),'hex'),'AGE_PRIVACY',t,t+interval '10 minutes','PENDING' from(select clock_timestamp() t)n")
        .bind(older).bind(a).bind(event).bind(family).execute(&f.concert).await?;
    sqlx::query("insert into concert_age_claims(claim_id,user_id,event_id,verifier_id,source_revision,status,verified_at,expires_at,age_at_verification,next_birthday_at) values($1,$2,$3,'synthetic','synthetic-v1','VERIFIED',clock_timestamp()-interval '1 minute',clock_timestamp()+interval '30 days',26,(clock_timestamp() at time zone 'Asia/Seoul')::date+1)")
        .bind(older).bind(a).bind(event).execute(&f.concert).await?;
    let s = scope(&f, a, family, event, "getConcertAgeStatus").await?;
    let c = context(&s);
    let untrusted = runtime(&f, None).claims(s.clone()).status(c).await?;
    assert_eq!(untrusted.status, AgeStatus::ProviderUnavailable);
    let r = trusted_outage(&f);
    let current = r.claims(s.clone()).status(c).await?;
    assert_eq!(current.status, AgeStatus::Verified);
    assert!(!current.verification_available);
    let mut tx = f.concert.begin().await?;
    ScopedAgeAuthority::new(s.clone())
        .authorize_in(&mut tx, c, Access::Read)
        .await?;
    let pending = r
        .projection(s.clone())
        .project_in(
            &mut tx,
            ClaimQuery {
                context: c,
                subject: c.actor,
                requested_at: Utc::now(),
            },
            |_, age| {
                Box::pin(async move {
                    assert!(!age.verification_available());
                    Ok(age.permits(AgeBounds::new(Some(18), Some(30)).unwrap(), false))
                })
            },
        )
        .await?;
    tx.commit().await?;
    assert!(pending.after_commit()?);
    finish(&s).await?;
    // A newer unaccepted source must hide the older accepted record completely.
    let newer = Uuid::new_v4();
    sqlx::query("update concert_age_claims set status='REPLACED' where claim_id=$1")
        .bind(older)
        .execute(&f.concert)
        .await?;
    sqlx::query("insert into concert_age_verifications(verification_id,user_id,event_id,session_id,verifier_id,source_revision,state_digest,return_target,created_at,expires_at,state) select $1,$2,$3,$4,'unaccepted','unaccepted-v1',decode(repeat('62',32),'hex'),'AGE_PRIVACY',t,t+interval '10 minutes','PENDING' from(select clock_timestamp() t)n")
        .bind(newer).bind(a).bind(event).bind(family).execute(&f.concert).await?;
    sqlx::query("insert into concert_age_claims(claim_id,user_id,event_id,verifier_id,source_revision,status,verified_at,expires_at,age_at_verification,next_birthday_at) values($1,$2,$3,'unaccepted','unaccepted-v1','VERIFIED',clock_timestamp(),clock_timestamp()+interval '30 days',26,(clock_timestamp() at time zone 'Asia/Seoul')::date+1)")
        .bind(newer).bind(a).bind(event).execute(&f.concert).await?;
    let s = scope(&f, a, family, event, "getConcertAgeStatus").await?;
    let c = context(&s);
    let healthy = runtime(&f, Some(Arc::new(SyntheticVerifier(AtomicUsize::new(0)))));
    assert!(healthy.verification_available());
    assert_eq!(
        healthy.claims(s.clone()).status(c).await?.status,
        AgeStatus::Unknown
    );
    let mut tx = f.concert.begin().await?;
    ScopedAgeAuthority::new(s.clone())
        .authorize_in(&mut tx, c, Access::Read)
        .await?;
    let pending = healthy
        .projection(s.clone())
        .project_in(
            &mut tx,
            ClaimQuery {
                context: c,
                subject: c.actor,
                requested_at: Utc::now(),
            },
            |_, age| {
                Box::pin(async move {
                    assert_eq!(age.status(), AgeStatus::Unknown);
                    Ok(age.permits(AgeBounds::new(Some(18), Some(30)).unwrap(), false))
                })
            },
        )
        .await?;
    tx.commit().await?;
    assert!(!pending.after_commit()?);
    finish(&s).await?;
    Ok(())
}
