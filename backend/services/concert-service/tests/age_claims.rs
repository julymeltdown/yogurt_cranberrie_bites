//! Actual owned PostgreSQL, with explicitly synthetic transaction-locked authority/verifier.
pub use concert_application::{age_policy, identity};
pub use concert_service::adapters::out::repository;
mod age_claims_support;
#[path = "../src/adapters/out/age_provider/mod.rs"]
mod age_provider;
#[path = "../../../crates/concert-application/src/age_verification/mod.rs"]
pub mod age_verification;
use age_provider::*;
use age_verification::*;
use async_trait::async_trait;
use chrono::{DateTime, Datelike, Duration, Utc};
use concert_application::{commands::*, Clock};
use concert_domain::{age_policy::AgeStatus, AuthUserId, EventId};
use sqlx::{PgConnection, PgPool};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
use tokio::sync::Notify;
use uuid::Uuid;
struct TestClock(Mutex<Option<DateTime<Utc>>>);
impl Clock for TestClock {
    fn now(&self) -> DateTime<Utc> {
        self.0.lock().unwrap().unwrap_or_else(Utc::now)
    }
}
struct FixtureAuthority;
#[async_trait]
impl Authority for FixtureAuthority {
    async fn authorize_in(
        &self,
        c: &mut PgConnection,
        ctx: age_policy::Context,
        _: Access,
    ) -> Result<(), CommandError> {
        let allowed:Option<bool>=sqlx::query_scalar("select allowed from be26_fixture_authority where actor=$1 and event=$2 and session=$3 for share").bind(ctx.actor.uuid()).bind(ctx.event.uuid()).bind(ctx.session).fetch_optional(c).await.map_err(|_|CommandError::Unavailable)?;
        if allowed == Some(true) {
            Ok(())
        } else {
            Err(CommandError::Denied)
        }
    }
}
struct FixtureVerifier {
    calls: AtomicUsize,
    mode: u8,
    entered: Notify,
    release: Notify,
}
#[async_trait]
impl Verifier for FixtureVerifier {
    fn available(&self) -> bool {
        true
    }
    async fn verify(&self, ch: &Challenge, _: &ProviderResponse) -> Result<VerifiedProof, Error> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.entered.notify_one();
        if self.mode == 1 || self.mode == 5 {
            self.release.notified().await;
        }
        if self.mode == 2 {
            return Err(Error::ProviderUnavailable);
        }
        let mut binding = ch.binding.clone();
        if self.mode == 3 {
            binding.session = Uuid::new_v4()
        }
        let at = Utc::now();
        let tomorrow = (at + Duration::hours(9)).date_naive().succ_opt().unwrap();
        let (birthyear, birthday) = if self.mode == 4 {
            (
                (tomorrow.year() - 101).to_string(),
                tomorrow.format("%m-%d").to_string(),
            )
        } else {
            ("2000".into(), "11-05".into())
        };
        Ok(VerifiedProof {
            binding,
            verified_at: at,
            expires_at: at
                + if self.mode == 5 {
                    Duration::milliseconds(200)
                } else {
                    Duration::days(90)
                },
            source_revision: "fixture-v1".into(),
            birthyear,
            birthday,
        })
    }
}
fn keys() -> Arc<FingerprintKeys> {
    Arc::new(
        FingerprintKeys::new(
            SigningKey::new("be26_fixture", &[0x62; 32]).unwrap(),
            vec![],
        )
        .unwrap(),
    )
}
fn config() -> ProviderConfig {
    ProviderConfig::new(
        "fixture",
        "fixture-v1",
        "https://verify.example/authorize",
        &["https://verify.example/authorize"],
    )
    .unwrap()
}
fn service(p: &PgPool, v: Arc<dyn Verifier>, clock: Arc<dyn Clock>) -> PgAgeClaims {
    PgAgeClaims::new(
        p.clone(),
        Arc::new(FixtureAuthority),
        v,
        Some(config()),
        clock,
        keys(),
    )
}
struct Fixture {
    p: PgPool,
    c: age_policy::Context,
    s: PgAgeClaims,
    v: Arc<FixtureVerifier>,
    clock: Arc<TestClock>,
}
async fn setup(mode: u8) -> anyhow::Result<Fixture> {
    let p = age_claims_support::owned_pool().await?;
    let c = age_policy::Context {
        actor: AuthUserId::from_uuid(Uuid::new_v4()),
        event: EventId::from_uuid(Uuid::new_v4()),
        session: Uuid::new_v4(),
        request: Uuid::new_v4(),
        expires_at: Utc::now() + Duration::minutes(5),
    };
    seed(&p, c).await?;
    let v = Arc::new(FixtureVerifier {
        calls: AtomicUsize::new(0),
        mode,
        entered: Notify::new(),
        release: Notify::new(),
    });
    let clock = Arc::new(TestClock(Mutex::new(None)));
    let s = service(&p, v.clone(), clock.clone());
    Ok(Fixture { p, c, s, v, clock })
}
async fn seed(p: &PgPool, c: age_policy::Context) -> anyhow::Result<()> {
    sqlx::query("create table if not exists be26_fixture_authority(actor uuid,event uuid,session uuid,allowed boolean,primary key(actor,event,session))").execute(p).await?;
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Owned BE26 fixture','OPEN')").bind(c.event.uuid()).bind(format!("be26-{}",c.event.uuid())).execute(p).await?;
    sqlx::query("insert into be26_fixture_authority values($1,$2,$3,true)")
        .bind(c.actor.uuid())
        .bind(c.event.uuid())
        .bind(c.session)
        .execute(p)
        .await?;
    Ok(())
}
async fn begin(f: &Fixture) -> anyhow::Result<(Uuid, String)> {
    let started = f.s.start(f.c, Uuid::new_v4(), RETURN_TARGET).await?;
    let a = started
        .authorization
        .ok_or_else(|| anyhow::anyhow!("missing fixture authorization"))?;
    let state = a
        .authorization_url
        .split("&state=")
        .nth(1)
        .and_then(|v| v.split('&').next())
        .ok_or_else(|| anyhow::anyhow!("fixture URL shape"))?
        .to_owned();
    Ok((a.verification_id, state))
}
fn token(s: &str) -> StateToken {
    StateToken::parse(s.into()).unwrap()
}
fn response() -> ProviderResponse {
    ProviderResponse::new("FIXTURE_PROOF_NOT_REAL_PROVIDER".into()).unwrap()
}
async fn count(f: &Fixture) -> anyhow::Result<i64> {
    Ok(
        sqlx::query_scalar("select count(*) from concert_age_claims where user_id=$1")
            .bind(f.c.actor.uuid())
            .fetch_one(&f.p)
            .await?,
    )
}
#[tokio::test]
async fn digest_only_start_idempotency_and_fixed_target() -> anyhow::Result<()> {
    let f = setup(0).await?;
    let key = Uuid::new_v4();
    let first = f.s.start(f.c, key, RETURN_TARGET).await?;
    let id = first.receipt.reference.unwrap().resource;
    let url = first.authorization.unwrap().authorization_url;
    let replay = f.s.start(f.c, key, RETURN_TARGET).await?;
    assert_eq!(first.receipt, replay.receipt);
    assert!(replay.authorization.is_none());
    let (digest,target,lifetime):(Vec<u8>,String,i64)=sqlx::query_as("select state_digest,return_target,extract(epoch from expires_at-created_at)::bigint from concert_age_verifications where verification_id=$1").bind(id).fetch_one(&f.p).await?;
    assert_eq!(digest.len(), 32);
    assert_eq!(target, RETURN_TARGET);
    assert_eq!(lifetime, 600);
    assert!(!url.contains("redirect="));
    let stored: String = sqlx::query_scalar(
        "select row_to_json(v)::text from concert_age_verifications v where verification_id=$1",
    )
    .bind(id)
    .fetch_one(&f.p)
    .await?;
    let raw = url
        .split("&state=")
        .nth(1)
        .unwrap()
        .split('&')
        .next()
        .unwrap();
    assert!(!stored.contains(raw));
    let mut other = f.c;
    other.session = Uuid::new_v4();
    assert!(f.s.start(other, key, RETURN_TARGET).await.is_err());
    assert!(matches!(
        f.s.start(f.c, Uuid::new_v4(), "https://evil.example").await,
        Err(Error::Invalid)
    ));
    Ok(())
}
#[tokio::test]
async fn callback_claim_and_receipt_are_atomic_and_replay_never_exchanges() -> anyhow::Result<()> {
    let f = setup(0).await?;
    let (id, state) = begin(&f).await?;
    let key = Uuid::new_v4();
    let a =
        f.s.callback(f.c, key, id, token(&state), response())
            .await?;
    assert_eq!(a.receipt.state, ReceiptState::Succeeded);
    assert_eq!(a.status.unwrap().status, AgeStatus::Verified);
    assert_eq!(count(&f).await?, 1);
    let b =
        f.s.callback(f.c, key, id, token(&state), response())
            .await?;
    assert_eq!(a.receipt, b.receipt);
    assert_eq!(f.v.calls.load(Ordering::SeqCst), 1);
    assert_eq!(count(&f).await?, 1);
    assert!(matches!(
        f.s.callback(
            f.c,
            key,
            id,
            token(&state),
            ProviderResponse::new("other-proof".into()).unwrap()
        )
        .await,
        Err(Error::Conflict)
    ));
    let (rowstate, digest): (String, Option<Vec<u8>>) = sqlx::query_as(
        "select state,state_digest from concert_age_verifications where verification_id=$1",
    )
    .bind(id)
    .fetch_one(&f.p)
    .await?;
    assert_eq!(rowstate, "DONE");
    assert!(digest.is_none());
    Ok(())
}
#[tokio::test]
async fn racing_same_and_different_keys_consume_once() -> anyhow::Result<()> {
    let f = setup(0).await?;
    let (id, state) = begin(&f).await?;
    let key = Uuid::new_v4();
    let (a, b) = tokio::join!(
        f.s.callback(f.c, key, id, token(&state), response()),
        f.s.callback(f.c, key, id, token(&state), response())
    );
    assert!(a.is_ok() && b.is_ok());
    assert_eq!(f.v.calls.load(Ordering::SeqCst), 1);
    assert_eq!(count(&f).await?, 1);
    let (id, state) = begin(&f).await?;
    let (a, b) = tokio::join!(
        f.s.callback(f.c, Uuid::new_v4(), id, token(&state), response()),
        f.s.callback(f.c, Uuid::new_v4(), id, token(&state), response())
    );
    assert!(a.is_ok() != b.is_ok());
    assert_eq!(f.v.calls.load(Ordering::SeqCst), 2);
    assert_eq!(count(&f).await?, 2);
    Ok(())
}
#[tokio::test]
async fn foreign_session_state_and_independent_proof_binding_fail_closed() -> anyhow::Result<()> {
    let f = setup(3).await?;
    let (id, state) = begin(&f).await?;
    let mut foreign = f.c;
    foreign.session = Uuid::new_v4();
    assert!(f
        .s
        .callback(foreign, Uuid::new_v4(), id, token(&state), response())
        .await
        .is_err());
    assert!(f
        .s
        .callback(
            f.c,
            Uuid::new_v4(),
            id,
            StateToken::from_random_bytes([4; 32]),
            response()
        )
        .await
        .is_err());
    assert_eq!(f.v.calls.load(Ordering::SeqCst), 0);
    let key = Uuid::new_v4();
    assert!(matches!(
        f.s.callback(f.c, key, id, token(&state), response()).await,
        Err(Error::Denied)
    ));
    assert_eq!(count(&f).await?, 0);
    assert_eq!(f.v.calls.load(Ordering::SeqCst), 1);
    let replay =
        f.s.callback(f.c, key, id, token(&state), response())
            .await?;
    assert_eq!(replay.receipt.state, ReceiptState::Pending);
    assert!(replay.status.is_none());
    assert_eq!(f.v.calls.load(Ordering::SeqCst), 1);
    Ok(())
}
#[tokio::test]
async fn provider_unavailable_does_not_erase_valid_claim() -> anyhow::Result<()> {
    let f = setup(0).await?;
    let missing = PgAgeClaims::new(
        f.p.clone(),
        Arc::new(FixtureAuthority),
        Arc::new(UnavailableVerifier),
        None,
        f.clock.clone(),
        keys(),
    );
    assert!(matches!(
        missing.start(f.c, Uuid::new_v4(), RETURN_TARGET).await,
        Err(Error::ProviderUnavailable)
    ));
    assert_eq!(
        missing.status(f.c).await?.status,
        AgeStatus::ProviderUnavailable
    );
    let (id, state) = begin(&f).await?;
    f.s.callback(f.c, Uuid::new_v4(), id, token(&state), response())
        .await?;
    let status = missing.status(f.c).await?;
    assert_eq!(status.status, AgeStatus::Verified);
    assert!(!status.verification_available);
    assert!(
        PgAgeClaims::unavailable(f.p.clone(), f.clock.clone(), keys())
            .status(f.c)
            .await
            .is_err()
    );
    Ok(())
}
#[tokio::test]
async fn failed_exchange_is_pending_not_automatically_retried() -> anyhow::Result<()> {
    let f = setup(2).await?;
    let (id, state) = begin(&f).await?;
    let key = Uuid::new_v4();
    assert!(matches!(
        f.s.callback(f.c, key, id, token(&state), response()).await,
        Err(Error::ProviderUnavailable)
    ));
    assert_eq!(count(&f).await?, 0);
    let r = f.s.recover(f.c, key, id, true).await?.unwrap();
    assert_eq!(r.state, ReceiptState::Pending);
    f.s.callback(f.c, key, id, token(&state), response())
        .await?;
    assert_eq!(f.v.calls.load(Ordering::SeqCst), 1);
    Ok(())
}
#[tokio::test]
async fn commit_failure_rolls_back_claim_and_preserves_unknown_command() -> anyhow::Result<()> {
    let f = setup(0).await?;
    let (id, state) = begin(&f).await?;
    let key = Uuid::new_v4();
    sqlx::query("create function be26_injected_failure() returns trigger language plpgsql as $$begin raise exception 'FIXTURE_COMMIT_FAILURE';end$$").execute(&f.p).await?;
    sqlx::query("create trigger be26_fail before insert on concert_age_claims for each row execute function be26_injected_failure()").execute(&f.p).await?;
    let result = f.s.callback(f.c, key, id, token(&state), response()).await;
    assert!(result.is_err());
    assert_eq!(count(&f).await?, 0);
    assert_eq!(
        f.s.recover(f.c, key, id, true).await?.unwrap().state,
        ReceiptState::Pending
    );
    sqlx::query("drop trigger be26_fail on concert_age_claims")
        .execute(&f.p)
        .await?;
    sqlx::query("drop function be26_injected_failure()")
        .execute(&f.p)
        .await?;
    let r =
        f.s.callback(f.c, key, id, token(&state), response())
            .await?;
    assert!(r.status.is_none());
    assert_eq!(f.v.calls.load(Ordering::SeqCst), 1);
    assert_eq!(count(&f).await?, 0);
    Ok(())
}
#[tokio::test]
async fn revocation_during_exchange_cannot_commit_a_claim() -> anyhow::Result<()> {
    let f = setup(1).await?;
    let (id, state) = begin(&f).await?;
    let key = Uuid::new_v4();
    let s = f.s.clone();
    let c = f.c;
    let state2 = state.clone();
    let task =
        tokio::spawn(async move { s.callback(c, key, id, token(&state2), response()).await });
    f.v.entered.notified().await;
    sqlx::query("update be26_fixture_authority set allowed=false where actor=$1")
        .bind(f.c.actor.uuid())
        .execute(&f.p)
        .await?;
    f.v.release.notify_one();
    assert!(task.await?.is_err());
    assert_eq!(count(&f).await?, 0);
    assert_eq!(f.v.calls.load(Ordering::SeqCst), 1);
    Ok(())
}
#[tokio::test]
async fn deadline_timeout_preserves_pending_without_reexchange() -> anyhow::Result<()> {
    let f = setup(1).await?;
    let (id, state) = begin(&f).await?;
    let key = Uuid::new_v4();
    let mut short = f.c;
    short.expires_at = Utc::now() + Duration::milliseconds(150);
    assert!(f
        .s
        .callback(short, key, id, token(&state), response())
        .await
        .is_err());
    assert_eq!(count(&f).await?, 0);
    let r =
        f.s.callback(f.c, key, id, token(&state), response())
            .await?;
    assert_eq!(r.receipt.state, ReceiptState::Pending);
    assert_eq!(f.v.calls.load(Ordering::SeqCst), 1);
    Ok(())
}
#[tokio::test]
async fn canonical_replacement_expiry_revocation_and_seoul_age_are_persistent() -> anyhow::Result<()>
{
    let f = setup(0).await?;
    let (id, state) = begin(&f).await?;
    f.s.callback(f.c, Uuid::new_v4(), id, token(&state), response())
        .await?;
    let (new_id, state) = begin(&f).await?;
    f.s.callback(f.c, Uuid::new_v4(), new_id, token(&state), response())
        .await?;
    let old: String = sqlx::query_scalar("select status from concert_age_claims where claim_id=$1")
        .bind(id)
        .fetch_one(&f.p)
        .await?;
    assert_eq!(old, "REPLACED");
    let mut ctx = f.c;
    ctx.expires_at = Utc::now() + Duration::days(120);
    *f.clock.0.lock().unwrap() = Some(Utc::now() + Duration::days(91));
    assert_eq!(f.s.status(ctx).await?.status, AgeStatus::Expired);
    *f.clock.0.lock().unwrap() = None;
    sqlx::query("update concert_age_claims set status='REVOKED' where claim_id=$1")
        .bind(new_id)
        .execute(&f.p)
        .await?;
    sqlx::query("update concert_age_claims set status='VERIFIED' where claim_id=$1")
        .bind(id)
        .execute(&f.p)
        .await?;
    assert_eq!(f.s.status(f.c).await?.status, AgeStatus::Revoked);
    let next: chrono::NaiveDate =
        sqlx::query_scalar("select next_birthday_at from concert_age_claims where claim_id=$1")
            .bind(new_id)
            .fetch_one(&f.p)
            .await?;
    assert!(next > Utc::now().date_naive());
    Ok(())
}
#[tokio::test]
async fn expired_state_constraints_and_bounded_retention() -> anyhow::Result<()> {
    let f = setup(0).await?;
    let (id, state) = begin(&f).await?;
    assert!(sqlx::query("update concert_age_verifications set expires_at=created_at+interval '11 minutes' where verification_id=$1").bind(id).execute(&f.p).await.is_err());
    sqlx::query("with n as materialized(select clock_timestamp() t) update concert_age_verifications set created_at=n.t-interval '11 minutes',expires_at=n.t-interval '1 minute' from n where verification_id=$1").bind(id).execute(&f.p).await?;
    assert!(f
        .s
        .callback(f.c, Uuid::new_v4(), id, token(&state), response())
        .await
        .is_err());
    assert_eq!(f.v.calls.load(Ordering::SeqCst), 0);
    assert_eq!(f.s.expire_states(f.c, 1).await?, 1);
    assert!(f.s.expire_states(f.c, 101).await.is_err());
    let digest: Option<Vec<u8>> = sqlx::query_scalar(
        "select state_digest from concert_age_verifications where verification_id=$1",
    )
    .bind(id)
    .fetch_one(&f.p)
    .await?;
    assert!(digest.is_none());
    Ok(())
}
const RESTART_USER: Uuid = Uuid::from_u128(0xbe260000000000000000000000000001);
const RESTART_EVENT: Uuid = Uuid::from_u128(0xbe260000000000000000000000000002);
const RESTART_SESSION: Uuid = Uuid::from_u128(0xbe260000000000000000000000000003);
fn restart_context() -> age_policy::Context {
    age_policy::Context {
        actor: AuthUserId::from_uuid(RESTART_USER),
        event: EventId::from_uuid(RESTART_EVENT),
        session: RESTART_SESSION,
        request: Uuid::new_v4(),
        expires_at: Utc::now() + Duration::minutes(1),
    }
}
#[tokio::test]
async fn durable_restart_seed() -> anyhow::Result<()> {
    let mut f = setup(0).await?;
    f.c = restart_context();
    seed(&f.p, f.c).await?;
    let (id, state) = begin(&f).await?;
    f.s.callback(f.c, Uuid::new_v4(), id, token(&state), response())
        .await?;
    f.p.close().await;
    Ok(())
}
#[tokio::test]
#[ignore = "owned harness only, after real PostgreSQL stop/start"]
async fn durable_restart_verify() -> anyhow::Result<()> {
    let p = age_claims_support::owned_pool().await?;
    let clock = Arc::new(TestClock(Mutex::new(None)));
    let s = PgAgeClaims::new(
        p.clone(),
        Arc::new(FixtureAuthority),
        Arc::new(UnavailableVerifier),
        None,
        clock,
        keys(),
    );
    let c = restart_context();
    let status = s.status(c).await?;
    assert_eq!(status.status, AgeStatus::Verified);
    assert!(!status.verification_available);
    let count: i64 = sqlx::query_scalar("select count(*) from concert_age_claims where user_id=$1")
        .bind(RESTART_USER)
        .fetch_one(&p)
        .await?;
    assert_eq!(count, 1);
    let receipts: i64 = sqlx::query_scalar(
        "select count(*) from concert_command_receipts where actor_id=$1 and state='SUCCEEDED'",
    )
    .bind(RESTART_USER)
    .fetch_one(&p)
    .await?;
    assert_eq!(receipts, 2);
    p.close().await;
    Ok(())
}

#[tokio::test]
async fn persisted_100_to_101_transition_is_exact_seoul_midnight() -> anyhow::Result<()> {
    let f = setup(4).await?;
    let (id, state) = begin(&f).await?;
    f.s.callback(f.c, Uuid::new_v4(), id, token(&state), response())
        .await?;
    let (age, next): (i16, chrono::NaiveDate) = sqlx::query_as(
        "select age_at_verification,next_birthday_at from concert_age_claims where claim_id=$1",
    )
    .bind(id)
    .fetch_one(&f.p)
    .await?;
    assert_eq!(age, 100);
    let boundary = (next.and_hms_opt(0, 0, 0).unwrap() - Duration::hours(9)).and_utc();
    let mut ctx = f.c;
    ctx.expires_at = boundary + Duration::days(1);
    *f.clock.0.lock().unwrap() = Some(boundary - Duration::nanoseconds(1));
    assert_eq!(f.s.status(ctx).await?.status, AgeStatus::Verified);
    *f.clock.0.lock().unwrap() = Some(boundary);
    assert_eq!(f.s.status(ctx).await?.status, AgeStatus::Unsupported);
    let stored: i16 =
        sqlx::query_scalar("select age_at_verification from concert_age_claims where claim_id=$1")
            .bind(id)
            .fetch_one(&f.p)
            .await?;
    assert_eq!(stored, 100);
    Ok(())
}
#[tokio::test]
async fn expired_claim_details_are_erased_without_losing_canonical_tombstone() -> anyhow::Result<()>
{
    let f = setup(0).await?;
    let (id, state) = begin(&f).await?;
    f.s.callback(f.c, Uuid::new_v4(), id, token(&state), response())
        .await?;
    sqlx::query("with n as materialized(select clock_timestamp() t) update concert_age_claims set verified_at=n.t-interval '2 days',expires_at=n.t-interval '1 day' from n where claim_id=$1").bind(id).execute(&f.p).await?;
    f.s.expire_states(f.c, 100).await?;
    let (status,age,next):(String,Option<i16>,Option<chrono::NaiveDate>)=sqlx::query_as("select status,age_at_verification,next_birthday_at from concert_age_claims where claim_id=$1").bind(id).fetch_one(&f.p).await?;
    assert_eq!(status, "EXPIRED");
    assert!(age.is_none() && next.is_none());
    assert_eq!(f.s.status(f.c).await?.status, AgeStatus::Expired);
    Ok(())
}

#[tokio::test]
async fn proof_expiry_while_waiting_for_owner_lock_preserves_prior_claim() -> anyhow::Result<()> {
    let mut f = setup(0).await?;
    let (old, state) = begin(&f).await?;
    f.s.callback(f.c, Uuid::new_v4(), old, token(&state), response())
        .await?;
    f.v = Arc::new(FixtureVerifier {
        calls: AtomicUsize::new(0),
        mode: 5,
        entered: Notify::new(),
        release: Notify::new(),
    });
    f.s = service(&f.p, f.v.clone(), f.clock.clone());
    let (id, state) = begin(&f).await?;
    let key = Uuid::new_v4();
    let s = f.s.clone();
    let c = f.c;
    let task = tokio::spawn(async move { s.callback(c, key, id, token(&state), response()).await });
    f.v.entered.notified().await;
    let mut locker = f.p.begin().await?;
    sqlx::query("select user_id from concert_age_owners where user_id=$1 for update")
        .bind(f.c.actor.uuid())
        .fetch_one(&mut *locker)
        .await?;
    f.v.release.notify_one();
    // Observe the actual finalization lock wait before letting the proof expire.
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            let waiting: bool = sqlx::query_scalar("select exists(select 1 from pg_stat_activity where datname=current_database() and wait_event_type='Lock' and query like 'select user_id from concert_age_owners%')").fetch_one(&f.p).await?;
            if waiting { break Ok::<(),sqlx::Error>(()); }
            tokio::task::yield_now().await;
        }
    }).await??;
    sqlx::query("select pg_sleep(0.4)")
        .execute(&mut *locker)
        .await?;
    locker.commit().await?;
    assert!(matches!(task.await?, Err(Error::Expired)));
    assert_eq!(count(&f).await?, 1);
    let status: String =
        sqlx::query_scalar("select status from concert_age_claims where claim_id=$1")
            .bind(old)
            .fetch_one(&f.p)
            .await?;
    assert_eq!(status, "VERIFIED");
    assert_eq!(
        f.s.recover(f.c, key, id, true).await?.unwrap().state,
        ReceiptState::Pending
    );
    assert_eq!(f.v.calls.load(Ordering::SeqCst), 1);
    Ok(())
}

#[tokio::test]
async fn closed_event_allows_owner_erasure_without_verification_authority() -> anyhow::Result<()> {
    let f = setup(0).await?;
    let (id, state) = begin(&f).await?;
    f.s.callback(f.c, Uuid::new_v4(), id, token(&state), response())
        .await?;
    let (pending, _) = begin(&f).await?;
    sqlx::query("with n as materialized(select clock_timestamp() t) update concert_age_claims set verified_at=n.t-interval '2 days',expires_at=n.t-interval '1 day' from n where claim_id=$1").bind(id).execute(&f.p).await?;
    sqlx::query("with n as materialized(select clock_timestamp() t) update concert_age_verifications set created_at=n.t-interval '11 minutes',expires_at=n.t-interval '1 minute' from n where verification_id=$1").bind(pending).execute(&f.p).await?;
    sqlx::query("update concert_events set phase='CLOSED' where event_id=$1")
        .bind(f.c.event.uuid())
        .execute(&f.p)
        .await?;
    assert!(f.s.start(f.c, Uuid::new_v4(), RETURN_TARGET).await.is_err());
    assert_eq!(f.s.expire_states(f.c, 100).await?, 1);
    let erased: bool = sqlx::query_scalar("select age_at_verification is null and next_birthday_at is null from concert_age_claims where claim_id=$1").bind(id).fetch_one(&f.p).await?;
    assert!(erased);
    let digest: Option<Vec<u8>> = sqlx::query_scalar(
        "select state_digest from concert_age_verifications where verification_id=$1",
    )
    .bind(pending)
    .fetch_one(&f.p)
    .await?;
    assert!(digest.is_none());
    sqlx::query("update be26_fixture_authority set allowed=false where actor=$1")
        .bind(f.c.actor.uuid())
        .execute(&f.p)
        .await?;
    assert!(matches!(
        f.s.expire_states(f.c, 100).await,
        Err(Error::Denied)
    ));
    Ok(())
}
