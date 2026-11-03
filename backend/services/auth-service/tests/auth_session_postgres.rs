//! Fixed-clock session decisions over real PostgreSQL with observed lock barriers.
#[path = "support/auth_fixture_config.rs"]
mod auth_fixture_config;
#[path = "support/auth_runtime_fixture.rs"]
mod auth_runtime_fixture;

use anyhow::{Context, Result};
use auth_runtime_fixture::Fixture;
use auth_service::adapters::out::cache::session_store::PgRefreshTokenStore;
use auth_service::adapters::out::persistence::user_repository::{
    PgLoginAuditRepository, PgUserRepository,
};
use auth_service::adapters::out::security::admitted_argon2::AdmittedArgon2PasswordWork;
use auth_service::application::auth_runtime::AuthRuntimePort;
use auth_service::application::errors::AuthError;
use auth_service::application::password_work::PasswordWorkLimits;
use auth_service::application::ports::{
    RefreshTokenStorePort, TokenServicePort, UserRepositoryPort,
};
use auth_service::application::use_cases::login::{LoginUseCase, LoginUseCaseDeps};
use auth_service::config::JwtTokenService;
use auth_service::domain::session::{AuthTokenPair, RefreshTokenRecord, SessionStatus};
use auth_service::domain::user::{User, UserStatus};
use base64ct::{Base64, Encoding};
use chrono::{DateTime, Duration, Utc};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};
use tokio::time::{timeout, Duration as WallDuration};
use uuid::Uuid;

type Login = LoginUseCase<
    Arc<PgUserRepository>,
    Arc<PgLoginAuditRepository>,
    Arc<PgRefreshTokenStore>,
    Arc<JwtTokenService>,
>;

struct Clock {
    now: Mutex<DateTime<Utc>>,
    ids: AtomicU64,
}
impl Clock {
    fn set(&self, now: DateTime<Utc>) {
        *self.now.lock().unwrap() = now;
    }
}
impl AuthRuntimePort for Clock {
    fn now(&self) -> DateTime<Utc> {
        *self.now.lock().unwrap()
    }
    fn new_id(&self) -> Uuid {
        Uuid::from_u128(
            0x22340000000000000000000000000000
                + u128::from(self.ids.fetch_add(1, Ordering::SeqCst)),
        )
    }
}
struct Sessions {
    login: Login,
    clock: Arc<Clock>,
    tokens: Arc<JwtTokenService>,
    store: Arc<PgRefreshTokenStore>,
    user: User,
    original: AuthTokenPair,
    other: AuthTokenPair,
}
impl Sessions {
    async fn new(f: &Fixture) -> Result<Self> {
        f.empty_effects().await?;
        let clock = Arc::new(Clock {
            now: Mutex::new(Utc::now()),
            ids: AtomicU64::new(1),
        });
        let users = Arc::new(PgUserRepository::new(f.pool.clone(), clock.clone()));
        let store = Arc::new(PgRefreshTokenStore::new(f.pool.clone(), clock.clone()));
        let user = User {
            id: Uuid::new_v4(),
            primary_email: "session-barrier@example.invalid".into(),
            password_hash: "unused-refresh-password".into(),
            status: UserStatus::Active,
            last_login_at: None,
        };
        users.save(&(&user).into()).await?;
        let tokens = Arc::new(JwtTokenService::new(
            "session-fixture",
            "session-audience",
            900,
            1_209_600,
        )?);
        let original: AuthTokenPair = tokens
            .issue_token_pair(user.id, vec!["ROLE_USER".into()])?
            .into();
        let other: AuthTokenPair = tokens
            .issue_token_pair(user.id, vec!["ROLE_USER".into()])?
            .into();
        for pair in [&original, &other] {
            store
                .store(
                    &(&RefreshTokenRecord::new_family(
                        pair.session_id,
                        pair.refresh_jti.clone(),
                        Base64::encode_string(&Sha256::digest(pair.refresh_token.as_bytes())),
                        user.id,
                        pair.refresh_expires_at,
                    ))
                        .into(),
                )
                .await?;
        }
        let login = LoginUseCase::new(LoginUseCaseDeps {
            user_repository: users.clone(),
            login_audit_repository: Arc::new(PgLoginAuditRepository::new(f.pool.clone())),
            refresh_token_store: store.clone(),
            token_service: tokens.clone(),
            transactions: users,
            password_work: Arc::new(AdmittedArgon2PasswordWork::new(
                PasswordWorkLimits::default(),
            )?),
            runtime: clock.clone(),
        })
        .with_refresh_grace_window(Duration::seconds(30));
        Ok(Self {
            login,
            clock,
            tokens,
            store,
            user,
            original,
            other,
        })
    }
    async fn live(&self, pool: &PgPool, family: Uuid) -> Result<i64> {
        Ok(sqlx::query_scalar("select count(*) from refresh_tokens where family_id=$1 and revoked_at is null and expires_at > $2").bind(family).bind(self.clock.now()).fetch_one(pool).await?)
    }
}

async fn wait_blocked(pool: &PgPool, query_fragment: &str, expected: i64) -> Result<()> {
    timeout(WallDuration::from_secs(1), async {
        loop {
            let count: i64 = sqlx::query_scalar("select count(*) from pg_stat_activity where datname=current_database() and pid<>pg_backend_pid() and cardinality(pg_blocking_pids(pid))>0 and strpos(query,$1)>0").bind(query_fragment).fetch_one(pool).await?;
            if count == expected { return Ok::<_, anyhow::Error>(()); }
            tokio::time::sleep(WallDuration::from_millis(5)).await;
        }
    }).await.context("database lock barrier was not observed")?
}
async fn lock_user(pool: &PgPool, user: Uuid) -> Result<Transaction<'static, Postgres>> {
    let mut tx = pool.begin().await?;
    sqlx::query("select id from users where id=$1 for update")
        .bind(user)
        .execute(&mut *tx)
        .await?;
    Ok(tx)
}
async fn audit_gate(pool: &PgPool) -> Result<Transaction<'static, Postgres>> {
    sqlx::raw_sql("create function gate_rotation_audit() returns trigger language plpgsql as $$ begin if new.action='SESSION_ROTATED' then perform pg_advisory_xact_lock(909001::bigint); end if; return new; end $$; create trigger gate_rotation_audit before insert on auth_audit_events for each row execute function gate_rotation_audit();").execute(pool).await?;
    let mut gate = pool.begin().await?;
    sqlx::query("select pg_advisory_xact_lock(909001::bigint)")
        .execute(&mut *gate)
        .await?;
    Ok(gate)
}
fn refresh_task(
    s: &Sessions,
) -> tokio::task::JoinHandle<
    auth_service::application::errors::AuthResult<
        auth_service::application::dto::auth_dto::AuthResponseDto,
    >,
> {
    let login = s.login.clone();
    let token = s.original.refresh_token.clone();
    tokio::spawn(async move { login.refresh(&token).await })
}
async fn rotation_audits(pool: &PgPool) -> Result<i64> {
    Ok(
        sqlx::query_scalar("select count(*) from auth_audit_events where action='SESSION_ROTATED'")
            .fetch_one(pool)
            .await?,
    )
}
fn observe(f: &Fixture, scenario: &str, details: serde_json::Value) {
    eprintln!(
        "AUTH_RUNTIME_OBSERVATION {}",
        serde_json::json!({"database":f.database,"scenario":scenario,"details":details})
    );
}

#[tokio::test]
async fn logout_wins_after_committed_rotation() -> Result<()> {
    let f = Fixture::new().await?;
    let result: Result<()> = async {
        let s=Sessions::new(&f).await?;
        let gate=audit_gate(&f.pool).await?;
        let refresh=refresh_task(&s);
        wait_blocked(&f.pool,"insert into auth_audit_events",1).await?;
        let login=s.login.clone(); let original=s.original.refresh_token.clone();
        let logout=tokio::spawn(async move { login.logout(&original).await });
        wait_blocked(&f.pool,"",2).await?;
        gate.commit().await?;
        let refreshed=refresh.await??; logout.await??;
        let live=s.live(&f.pool,s.original.session_id).await?;
        observe(&f,"logout_after_rotation",serde_json::json!({"liveTargetFamily":live,"liveOtherFamily":s.live(&f.pool,s.other.session_id).await?,"rotationAudits":rotation_audits(&f.pool).await?}));
        anyhow::ensure!(live==0,"logout left a rotated successor active");
        anyhow::ensure!(s.live(&f.pool,s.other.session_id).await?==1,"logout disturbed another device");
        anyhow::ensure!(matches!(s.login.session_status(s.user.id,s.original.session_id).await?,SessionStatus::Ended));
        s.tokens.decode_access_token(&s.original.access_token)?;
        anyhow::ensure!(s.login.refresh(&refreshed.refresh_token).await.is_err());
        Ok(())
    }.await;
    f.pool.close().await;
    result
}

#[tokio::test]
async fn grace_is_evaluated_after_the_contested_rotation() -> Result<()> {
    let f = Fixture::new().await?;
    let result: Result<()> = async {
        let s=Sessions::new(&f).await?;
        let start=s.clock.now();
        let gate=audit_gate(&f.pool).await?;
        let user=lock_user(&f.pool,s.user.id).await?;
        let first=refresh_task(&s); let second=refresh_task(&s);
        wait_blocked(&f.pool,"select primary_email, password_hash",2).await?;
        user.commit().await?;
        wait_blocked(&f.pool,"insert into auth_audit_events",1).await?;
        s.clock.set(start+Duration::seconds(35));
        gate.commit().await?;
        let first=first.await?; let second=second.await?;
        let (winner, loser)=match (first,second) { (Ok(w),Err(e)) | (Err(e),Ok(w)) => (w,e), _=>anyhow::bail!("expected one rotation and one rejected contender") };
        let code=Some(AuthError::code(&loser));
        observe(&f,"grace_after_wait",serde_json::json!({"loserCode":code,"liveTargetFamily":s.live(&f.pool,s.original.session_id).await?,"liveOtherFamily":s.live(&f.pool,s.other.session_id).await?,"rotationAudits":rotation_audits(&f.pool).await?}));
        anyhow::ensure!(matches!(Some(&loser),Some(AuthError::CredentialRejected)),"a request-start timestamp extended refresh grace: {loser}");
        anyhow::ensure!(s.live(&f.pool,s.original.session_id).await?==0);
        anyhow::ensure!(s.live(&f.pool,s.other.session_id).await?==1);
        anyhow::ensure!(rotation_audits(&f.pool).await?==1);
        anyhow::ensure!(s.login.refresh(&winner.refresh_token).await.is_err());
        Ok(())
    }.await;
    f.pool.close().await;
    result
}

#[tokio::test]
async fn concurrent_refresh_uses_post_lock_rotation_time() -> Result<()> {
    let f = Fixture::new().await?;
    let result: Result<()> = async {
        let s = Sessions::new(&f).await?;
        let decided = s.clock.now() + Duration::seconds(10);
        let holder = lock_user(&f.pool, s.user.id).await?;
        let first = refresh_task(&s);
        let second = refresh_task(&s);
        wait_blocked(&f.pool, "select primary_email, password_hash", 2).await?;
        s.clock.set(decided);
        holder.commit().await?;
        let (winner, loser) = match (first.await?, second.await?) {
            (Ok(w), Err(e)) | (Err(e), Ok(w)) => (w, e),
            _ => anyhow::bail!("expected one winner and one conflict"),
        };
        anyhow::ensure!(matches!(Some(&loser), Some(AuthError::RefreshConflict)));
        let rotated: DateTime<Utc> = sqlx::query_scalar("select rotated_at from refresh_tokens where jti=$1")
            .bind(&s.original.refresh_jti).fetch_one(&f.pool).await?;
        let audit: (Uuid, DateTime<Utc>) = sqlx::query_as("select id, occurred_at from auth_audit_events where action='SESSION_ROTATED'")
            .fetch_one(&f.pool).await?;
        // PostgreSQL timestamps have microsecond precision.
        anyhow::ensure!(rotated.timestamp_micros() == decided.timestamp_micros());
        anyhow::ensure!(audit.1.timestamp_micros() == decided.timestamp_micros());
        anyhow::ensure!(audit.0 == Uuid::from_u128(0x22340000000000000000000000000001));
        anyhow::ensure!(s.live(&f.pool, s.original.session_id).await? == 1);
        anyhow::ensure!(rotation_audits(&f.pool).await? == 1);
        anyhow::ensure!(s.tokens.decode_access_token(&winner.access_token)?.session_id == Some(s.original.session_id));
        observe(&f, "post_lock_rotation", serde_json::json!({"decisionAt":decided,"rotatedAt":rotated,"auditAt":audit.1,"auditId":audit.0,"liveTargetFamily":1,"rotationAudits":1,"loserCode":Some(AuthError::code(&loser))}));
        Ok(())
    }.await;
    f.pool.close().await;
    result
}

#[tokio::test]
async fn token_expiring_while_waiting_for_owner_is_not_rotated() -> Result<()> {
    let f = Fixture::new().await?;
    let result: Result<()> = async {
        let s = Sessions::new(&f).await?;
        let expiry = s.clock.now() + Duration::seconds(5);
        sqlx::query("update refresh_tokens set expires_at=$2 where jti=$1")
            .bind(&s.original.refresh_jti).bind(expiry).execute(&f.pool).await?;
        let holder = lock_user(&f.pool, s.user.id).await?;
        let refresh = refresh_task(&s);
        wait_blocked(&f.pool, "select primary_email, password_hash", 1).await?;
        s.clock.set(expiry);
        holder.commit().await?;
        let rejected = refresh.await?.expect_err("expired token must not rotate after a lock wait");
        anyhow::ensure!(matches!(Some(&rejected), Some(AuthError::CredentialRejected)));
        let rows: (i64, i64) = sqlx::query_as("select count(*), count(*) filter(where rotated_at is not null) from refresh_tokens where family_id=$1")
            .bind(s.original.session_id).fetch_one(&f.pool).await?;
        anyhow::ensure!(rows == (1, 0));
        anyhow::ensure!(rotation_audits(&f.pool).await? == 0);
        anyhow::ensure!(matches!(s.login.session_status(s.user.id,s.original.session_id).await?, SessionStatus::Ended));
        anyhow::ensure!(s.live(&f.pool,s.other.session_id).await? == 1);
        observe(&f,"expiry_after_wait",serde_json::json!({"expiry":expiry,"familyRows":rows.0,"rotatedRows":rows.1,"rotationAudits":0,"liveOtherFamily":1}));
        Ok(())
    }.await;
    f.pool.close().await;
    result
}

#[tokio::test]
async fn logout_committing_first_blocks_refresh_without_resurrection() -> Result<()> {
    let f = Fixture::new().await?;
    let result: Result<()> = async {
        let s = Sessions::new(&f).await?;
        sqlx::raw_sql("create function gate_logout() returns trigger language plpgsql as $$ begin if new.revoked_reason='LOGOUT' and old.revoked_at is null then perform pg_advisory_xact_lock(909002::bigint); end if; return new; end $$; create trigger gate_logout before update on refresh_tokens for each row execute function gate_logout();").execute(&f.pool).await?;
        let mut gate=f.pool.begin().await?;
        sqlx::query("select pg_advisory_xact_lock(909002::bigint)").execute(&mut *gate).await?;
        let login=s.login.clone(); let token=s.original.refresh_token.clone();
        let logout=tokio::spawn(async move { login.logout(&token).await });
        wait_blocked(&f.pool,"update refresh_tokens",1).await?;
        let refresh=refresh_task(&s);
        wait_blocked(&f.pool,"select primary_email, password_hash",1).await?;
        gate.commit().await?;
        logout.await??;
        let denied=refresh.await?.expect_err("refresh must not revive a logged-out family");
        anyhow::ensure!(matches!(Some(&denied),Some(AuthError::CredentialRejected)));
        anyhow::ensure!(s.live(&f.pool,s.original.session_id).await?==0);
        anyhow::ensure!(s.live(&f.pool,s.other.session_id).await?==1);
        anyhow::ensure!(rotation_audits(&f.pool).await?==0);
        let rows:i64=sqlx::query_scalar("select count(*) from refresh_tokens where family_id=$1").bind(s.original.session_id).fetch_one(&f.pool).await?;
        anyhow::ensure!(rows==1);
        observe(&f,"logout_before_rotation",serde_json::json!({"liveTargetFamily":0,"liveOtherFamily":1,"rotationAudits":0,"familyRows":1}));
        Ok(())
    }.await;
    f.pool.close().await;
    result
}

#[tokio::test]
async fn exact_grace_boundary_is_conflict_then_family_reuse() -> Result<()> {
    let f = Fixture::new().await?;
    let result:Result<()>=async {
        let s=Sessions::new(&f).await?;
        s.login.refresh(&s.original.refresh_token).await?;
        let rotated:DateTime<Utc>=sqlx::query_scalar("select rotated_at from refresh_tokens where jti=$1").bind(&s.original.refresh_jti).fetch_one(&f.pool).await?;
        s.clock.set(rotated+Duration::seconds(30));
        let conflict=s.login.refresh(&s.original.refresh_token).await.expect_err("boundary is retryable");
        anyhow::ensure!(matches!(Some(&conflict),Some(AuthError::RefreshConflict)));
        anyhow::ensure!(s.live(&f.pool,s.original.session_id).await?==1);
        s.clock.set(rotated+Duration::seconds(30)+Duration::microseconds(1));
        let reuse=s.login.refresh(&s.original.refresh_token).await.expect_err("beyond grace is reuse");
        anyhow::ensure!(matches!(Some(&reuse),Some(AuthError::CredentialRejected)));
        anyhow::ensure!(s.live(&f.pool,s.original.session_id).await?==0);
        anyhow::ensure!(s.live(&f.pool,s.other.session_id).await?==1);
        observe(&f,"exact_grace_boundary",serde_json::json!({"boundaryCode":Some(AuthError::code(&conflict)),"oneMicrosecondLaterCode":Some(AuthError::code(&reuse)),"liveTargetFamily":0,"liveOtherFamily":1}));
        Ok(())
    }.await;
    f.pool.close().await;
    result
}

#[tokio::test]
async fn session_and_audit_insert_failures_roll_back_the_rotation() -> Result<()> {
    for table in ["refresh_tokens", "auth_audit_events"] {
        let f = Fixture::new().await?;
        let result:Result<()>=async {
            let s=Sessions::new(&f).await?;
            sqlx::raw_sql(&format!("create function reject_rotation() returns trigger language plpgsql as $$ begin raise exception 'session-matrix-injected'; end $$; create trigger reject_rotation before insert on {table} for each row execute function reject_rotation();")).execute(&f.pool).await?;
            anyhow::ensure!(s.login.refresh(&s.original.refresh_token).await.is_err());
            let rows:(i64,i64)=sqlx::query_as("select count(*),count(*) filter(where revoked_at is not null) from refresh_tokens where family_id=$1").bind(s.original.session_id).fetch_one(&f.pool).await?;
            anyhow::ensure!(rows==(1,0));
            anyhow::ensure!(rotation_audits(&f.pool).await?==0);
            anyhow::ensure!(s.live(&f.pool,s.original.session_id).await?==1);
            sqlx::raw_sql(&format!("drop trigger reject_rotation on {table}; drop function reject_rotation();")).execute(&f.pool).await?;
            s.login.refresh(&s.original.refresh_token).await?;
            anyhow::ensure!(rotation_audits(&f.pool).await?==1);
            anyhow::ensure!(s.live(&f.pool,s.original.session_id).await?==1);
            observe(&f,"rotation_rollback",serde_json::json!({"failedTable":table,"preRetryFamilyRows":rows.0,"preRetryRevokedRows":rows.1,"preRetryAudits":0,"postRetryAudits":1,"liveTargetFamily":1}));
            Ok(())
        }.await;
        f.pool.close().await;
        result?;
    }
    Ok(())
}

#[tokio::test]
async fn family_and_user_revocation_serialize_with_rotation() -> Result<()> {
    for all in [false, true] {
        let f = Fixture::new().await?;
        let result:Result<()>=async {
            let s=Sessions::new(&f).await?;
            let gate=audit_gate(&f.pool).await?;
            let refresh=refresh_task(&s);
            wait_blocked(&f.pool,"insert into auth_audit_events",1).await?;
            let store=s.store.clone();let user=s.user.id;let family=s.original.session_id;
            let revoke=tokio::spawn(async move { if all {store.revoke_all_for_user(user).await} else {store.revoke_family(family).await} });
            wait_blocked(&f.pool,"select id from users",1).await?;
            gate.commit().await?;
            refresh.await??;revoke.await??;
            anyhow::ensure!(s.live(&f.pool,family).await?==0);
            anyhow::ensure!(s.live(&f.pool,s.other.session_id).await?==i64::from(!all));
            observe(&f,"revocation_after_rotation",serde_json::json!({"scope":if all {"user"} else {"family"},"liveTargetFamily":0,"liveOtherFamily":i64::from(!all),"rotationAudits":rotation_audits(&f.pool).await?}));
            Ok(())
        }.await;
        f.pool.close().await;
        result?;
    }
    Ok(())
}

#[tokio::test]
async fn expired_or_mismatched_predecessors_do_not_report_a_benign_rotation_race() -> Result<()> {
    use auth_service::application::auth_transactions::AuthTransactionsPort;
    use auth_service::domain::session::RotationOutcome;
    let f = Fixture::new().await?;
    let result: Result<()> = async {
        let s = Sessions::new(&f).await?;
        s.login.refresh(&s.original.refresh_token).await?;
        let repository = PgUserRepository::new(f.pool.clone(), s.clock.clone());
        let correct_hash = Base64::encode_string(&Sha256::digest(s.original.refresh_token.as_bytes()));
        let candidate = RefreshTokenRecord::successor_of(s.original.session_id,Uuid::new_v4().to_string(),"never-written".into(),s.user.id,s.clock.now()+Duration::hours(1));
        let roles = vec!["ROLE_USER".into()];
        for hash in ["mismatched-fingerprint", correct_hash.as_str()] {
            if hash == correct_hash {
                sqlx::query("update refresh_tokens set expires_at=$2 where jti=$1")
                    .bind(&s.original.refresh_jti).bind(s.clock.now()).execute(&f.pool).await?;
            }
            anyhow::ensure!(repository.rotate_session(&(&s.user).into(),&s.original.refresh_jti,hash,&(&candidate).into(),&roles).await? == RotationOutcome::NotRotatable.into());
            anyhow::ensure!(s.store.rotate(&s.original.refresh_jti,hash,&(&candidate).into()).await? == RotationOutcome::NotRotatable.into());
        }
        anyhow::ensure!(s.live(&f.pool,s.original.session_id).await? == 1);
        anyhow::ensure!(rotation_audits(&f.pool).await? == 1);
        let rows:i64=sqlx::query_scalar("select count(*) from refresh_tokens where jti=$1").bind(candidate.jti).fetch_one(&f.pool).await?;
        anyhow::ensure!(rows == 0);
        observe(&f,"predecessor_admission",serde_json::json!({"mismatchAtomic":"NOT_ROTATABLE","mismatchCompatibility":"NOT_ROTATABLE","exactExpiryAtomic":"NOT_ROTATABLE","exactExpiryCompatibility":"NOT_ROTATABLE","candidateRows":0,"rotationAudits":1,"liveSuccessor":1}));
        Ok(())
    }.await;
    f.pool.close().await;
    result
}

// Actual loopback Tonic transport and registered-caller interceptor; no TLS,
// Gateway/browser or external-provider acceptance is claimed by this fixture.
struct Incoming(tokio::net::TcpListener);
impl tonic::codegen::tokio_stream::Stream for Incoming {
    type Item = std::io::Result<tokio::net::TcpStream>;
    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.0
            .poll_accept(cx)
            .map(|result| Some(result.map(|(stream, _)| stream)))
    }
}
fn registered<T>(body: T) -> tonic::Request<T> {
    let mut request = tonic::Request::new(body);
    request
        .metadata_mut()
        .insert("x-service-caller", "gateway".parse().unwrap());
    request.metadata_mut().insert(
        "x-service-token",
        "test-gateway-credential".parse().unwrap(),
    );
    request
}

#[tokio::test]
async fn transport_preserves_conflict_revocation_and_dependency_failure() -> Result<()> {
    use auth_service::adapters::r#in::grpc::server::{proto, AuthGrpcServer};
    use auth_service::config::build_state_with_auth_capabilities;
    use platform::grpc_auth::RequireRegisteredCallerInterceptor;
    let f = Fixture::new().await?;
    let result:Result<()>=async {
        let clock=Arc::new(Clock{now:Mutex::new(Utc::now()),ids:AtomicU64::new(1)});
        let user=User{id:Uuid::new_v4(),primary_email:"transport-session@example.invalid".into(),password_hash:auth_service::adapters::out::security::admitted_argon2::hash_password("Passw0rd!")?,status:UserStatus::Active,last_login_at:None};
        PgUserRepository::new(f.pool.clone(),clock.clone()).save(&(&user).into()).await?;
        let config=auth_fixture_config::test_config("unused-owned-pool".into())?;
        let state=build_state_with_auth_capabilities(&config,f.pool.clone(),Arc::new(AdmittedArgon2PasswordWork::new(PasswordWorkLimits::default())?),clock).await?;
        let original=state.login.login(&user.primary_email,"Passw0rd!",None,None).await?;
        let other=state.login.login(&user.primary_email,"Passw0rd!",None,None).await?;
        let verifier=api_gateway::adapters::out::jwt::JwtVerifier::from_jwks_json(&config.issuer,&config.access_audience,&auth_service::adapters::r#in::jwks::public_jwks(&(state.login.jwk_set())).to_string())?;
        let family=verifier.verify_access_token(&original.access_token)?.session_id.context("original family")?;
        let other_family=verifier.verify_access_token(&other.access_token)?.session_id.context("other family")?;
        let token=state.login.refresh(&original.refresh_token).await?;
        let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let endpoint=format!("http://{}",listener.local_addr()?);
        let server=proto::auth_service_server::AuthServiceServer::with_interceptor(AuthGrpcServer::new(Arc::new(state)),RequireRegisteredCallerInterceptor::new(config.caller_registry)?);
        let (shutdown,stopped)=tokio::sync::oneshot::channel::<()>();
        let task=tokio::spawn(tonic::transport::Server::builder().add_service(server).serve_with_incoming_shutdown(Incoming(listener),async {let _=stopped.await;}));
        let scenario:Result<()>=async {
            let mut client=proto::auth_service_client::AuthServiceClient::connect(endpoint).await?;
            let denied=client.check_session(proto::CheckSessionRequest{user_id:user.id.to_string(),session_id:family.clone()}).await.expect_err("unregistered request");
            anyhow::ensure!(denied.code()==tonic::Code::Unauthenticated);
            let conflict=client.refresh_tokens(registered(proto::RefreshTokensRequest{refresh_token:original.refresh_token.clone()})).await.expect_err("stale token in grace");
            anyhow::ensure!(conflict.code()==tonic::Code::Aborted && conflict.message()=="AUTH_REFRESH_CONFLICT");
            client.logout(registered(proto::LogoutRequest{refresh_token:original.refresh_token.clone()})).await?;
            let ended=client.check_session(registered(proto::CheckSessionRequest{user_id:user.id.to_string(),session_id:family.clone()})).await?.into_inner();
            anyhow::ensure!(!ended.active && ended.roles.is_empty());
            let other=client.check_session(registered(proto::CheckSessionRequest{user_id:user.id.to_string(),session_id:other_family.clone()})).await?.into_inner();
            anyhow::ensure!(other.active);
            let rejected=client.refresh_tokens(registered(proto::RefreshTokensRequest{refresh_token:token.refresh_token})).await.expect_err("ended successor");
            anyhow::ensure!(rejected.code()==tonic::Code::Unauthenticated && rejected.message()=="AUTH_CREDENTIAL_REJECTED");
            verifier.verify_access_token(&original.access_token)?;
            // A missing storage dependency must never become active=true.
            sqlx::query("alter table refresh_tokens rename to unavailable_session_rows").execute(&f.pool).await?;
            let failure=client.check_session(registered(proto::CheckSessionRequest{user_id:user.id.to_string(),session_id:other_family.clone()})).await.expect_err("dependency fault");
            sqlx::query("alter table unavailable_session_rows rename to refresh_tokens").execute(&f.pool).await?;
            anyhow::ensure!(failure.code()==tonic::Code::Internal && failure.message()=="AUTH_INTERNAL");
            observe(&f,"session_transport",serde_json::json!({"conflictCode":"ABORTED","conflictMessage":conflict.message(),"logoutActive":ended.active,"otherActive":other.active,"endedRefreshCode":"UNAUTHENTICATED","dependencyCode":"INTERNAL","dependencyMessage":failure.message(),"registeredCaller":true}));
            Ok(())
        }.await;
        let _=shutdown.send(());
        timeout(WallDuration::from_secs(5),task).await???;
        scenario
    }.await;
    f.pool.close().await;
    result
}
