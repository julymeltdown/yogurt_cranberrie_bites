//! Real PostgreSQL tests; owned native PG16 or Docker PG16, with no silent skip.
use anyhow::Result;
use async_trait::async_trait;
use auth_service::adapters::out::persistence::oauth_authorization_store::PgOAuthAuthorizationStore;
use auth_service::adapters::out::persistence::user_repository::PgAuthAuditRepository;
use auth_service::adapters::out::persistence::user_repository::PgUserRepository;
use auth_service::adapters::out::security::oauth_authorization::SecureOAuthAuthorization;
use auth_service::application::auth_runtime::{AuthRuntimePort, OAuthAuthorizationCryptoPort};
use auth_service::application::dto::oauth_authorization_dto::OAuthAuthorizationMaterialDto;
use auth_service::application::ports::{OAuthProviderMetricsPort, OAuthProviderPort};
use auth_service::application::use_cases::oauth_login::OAuthLoginUseCase;
use auth_service::config::JwtTokenService;
use auth_service::domain::oauth_identity::OAuthUserInfo;
use auth_service::domain::user::IdentityProvider;
use chrono::{DateTime, Duration, TimeZone, Utc};
use std::sync::{
    atomic::{AtomicU64, AtomicUsize, Ordering},
    Arc, Mutex,
};
use uuid::Uuid;

#[path = "support/auth_runtime_fixture.rs"]
mod auth_runtime_fixture;
use auth_runtime_fixture::Fixture;

struct FixedRuntime {
    now: Mutex<DateTime<Utc>>,
    ids: AtomicU64,
}
impl AuthRuntimePort for FixedRuntime {
    fn now(&self) -> DateTime<Utc> {
        *self.now.lock().unwrap()
    }
    fn new_id(&self) -> Uuid {
        Uuid::from_u128(
            0x12340000000000000000000000000000
                + u128::from(self.ids.fetch_add(1, Ordering::SeqCst)),
        )
    }
}
#[derive(Default)]
struct FixtureProvider(AtomicUsize);
#[async_trait]
impl OAuthProviderPort for FixtureProvider {
    fn build_authorization_uri(
        &self,
        _: auth_service::application::dto::repository_dto::AuthIdentityProviderDto,
        _: &str,
        state: &str,
        _: &str,
        _: &str,
    ) -> auth_service::application::errors::AuthResult<String> {
        let result: anyhow::Result<_> =
            Ok(format!("https://fixture.invalid/authorize?state={state}"));
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }
    async fn fetch_user_info(
        &self,
        provider: auth_service::application::dto::repository_dto::AuthIdentityProviderDto,
        _: &str,
        _: &str,
        _: Option<&str>,
        _: &str,
    ) -> auth_service::application::errors::AuthResult<
        auth_service::application::dto::repository_dto::AuthOAuthProofDto,
    > {
        let provider = auth_service::domain::user::IdentityProvider::from(provider);
        let result: anyhow::Result<_> = async {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(OAuthUserInfo {
                provider,
                subject: "owned-fixture-subject".into(),
                email: "owned-fixture@example.com".into(),
                email_verified: true,
            })
        }
        .await;
        result
            .map_err(auth_service::adapters::out::errors::boundary_error)
            .map(|value| value.into())
    }
}
impl OAuthProviderMetricsPort for FixtureProvider {
    fn record_provider_exchange(
        &self,
        _: auth_service::application::dto::repository_dto::AuthIdentityProviderDto,
        _: &str,
    ) {
    }
}
fn use_case(
    f: &Fixture,
    runtime: Arc<FixedRuntime>,
    provider: Arc<FixtureProvider>,
    crypto: Arc<dyn OAuthAuthorizationCryptoPort>,
) -> Result<OAuthLoginUseCase> {
    let users = Arc::new(PgUserRepository::new(f.pool.clone(), runtime.clone()));
    Ok(OAuthLoginUseCase::new(
        users.clone(),
        users,
        Arc::new(PgAuthAuditRepository::new(f.pool.clone())),
        Arc::new(JwtTokenService::new(
            "fixture-issuer",
            "fixture-audience",
            900,
            1209600,
        )?),
        provider.clone(),
        Arc::new(PgOAuthAuthorizationStore::new(f.pool.clone())),
        provider,
        runtime,
        crypto,
        "https://fixture.invalid/google/callback".into(),
        "https://fixture.invalid/apple/callback".into(),
        "https://fixture.invalid/x/callback".into(),
    ))
}
#[tokio::test]
async fn real_oauth_state_expiry_and_once_only_session_follow_injected_clock() -> Result<()> {
    let f = Fixture::new().await?;
    let scenario=async {
        let initial=Utc.timestamp_opt(1700000000,0).unwrap();
        let runtime=Arc::new(FixedRuntime{now:Mutex::new(initial),ids:AtomicU64::new(1)});
        let provider=Arc::new(FixtureProvider::default());
        let app=use_case(&f,runtime.clone(),provider.clone(),Arc::new(SecureOAuthAuthorization))?;
        app.authorize(IdentityProvider::Google).await?;
        let state:String=sqlx::query_scalar("select state from oauth_authorizations").fetch_one(&f.pool).await?;
        let expiry:DateTime<Utc>=sqlx::query_scalar("select expires_at from oauth_authorizations where state=$1").bind(&state).fetch_one(&f.pool).await?;
        anyhow::ensure!(expiry==initial+Duration::minutes(10),"clock-based expiry mismatch");
        *runtime.now.lock().unwrap()=expiry;
        let error=app.oauth_callback(IdentityProvider::Google,"fixture-code",&state,None,None,None).await.expect_err("exact expiry denied");
        anyhow::ensure!(error.to_string()=="OAuth authorization state is invalid","wrong expiry failure");
        anyhow::ensure!(provider.0.load(Ordering::SeqCst)==0,"expired state called provider");
        f.empty_effects().await?;
        let consumed:Option<DateTime<Utc>>=sqlx::query_scalar("select consumed_at from oauth_authorizations where state=$1").bind(&state).fetch_one(&f.pool).await?;
        anyhow::ensure!(consumed.is_none(),"expired row marked consumed");
        // A separate authorization exercises the valid-before-expiry boundary.
        *runtime.now.lock().unwrap()=initial;
        app.authorize(IdentityProvider::Google).await?;
        let valid:String=sqlx::query_scalar("select state from oauth_authorizations where state<>$1").bind(&state).fetch_one(&f.pool).await?;
        let admitted=expiry-Duration::milliseconds(1);
        *runtime.now.lock().unwrap()=admitted;
        app.oauth_callback(IdentityProvider::Google,"fixture-code",&valid,Some("fixed-request".into()),None,None).await?;
        let consumed:DateTime<Utc>=sqlx::query_scalar("select consumed_at from oauth_authorizations where state=$1").bind(&valid).fetch_one(&f.pool).await?;
        anyhow::ensure!(consumed==admitted,"consumption used wall clock instead of injected clock");
        for table in ["users","user_identities","refresh_tokens","login_audit"] {
            let count:i64=sqlx::query_scalar(&format!("select count(*) from {table}")).fetch_one(&f.pool).await?;
            anyhow::ensure!(count==1,"expected one committed effect in {table}, got {count}");
        }
        let count:i64=sqlx::query_scalar("select count(*) from auth_audit_events").fetch_one(&f.pool).await?;
        anyhow::ensure!(count==2,"provider exchange and onboarding audit must remain distinct");
        for action in ["OAUTH_PROVIDER_EXCHANGE","OAUTH_ACCOUNT_CREATED"] {
            let count:i64=sqlx::query_scalar("select count(*) from auth_audit_events where action=$1").bind(action).fetch_one(&f.pool).await?;
            anyhow::ensure!(count==1,"exactly one audit required for {action}");
        }
        let (id,at):(Uuid,DateTime<Utc>)=sqlx::query_as("select id,occurred_at from auth_audit_events where action='OAUTH_PROVIDER_EXCHANGE'").fetch_one(&f.pool).await?;
        anyhow::ensure!(id==Uuid::from_u128(0x12340000000000000000000000000001)&&at==admitted,"provider audit identity/time not from injected runtime");
        let at:DateTime<Utc>=sqlx::query_scalar("select occurred_at from login_audit").fetch_one(&f.pool).await?;
        anyhow::ensure!(at==admitted,"login audit did not use injected time");
        app.oauth_callback(IdentityProvider::Google,"fixture-code",&valid,None,None,None).await.expect_err("second consume denied");
        anyhow::ensure!(provider.0.load(Ordering::SeqCst)==1,"replay recontacted provider");
        eprintln!("AUTH_RUNTIME_OBSERVATION {}",serde_json::json!({"database":f.database,"exactExpiryDenied":true,"expiredProviderCalls":0,"validConsumedAt":consumed,"singleSession":true,"replayProviderCalls":1,"auditIdentityAndTimeBound":true}));
        Ok::<_,anyhow::Error>(())
    }.await;
    f.pool.close().await;
    scenario
}
#[tokio::test]
async fn real_apple_state_rejects_wrong_provider_concurrent_replay_and_exact_expiry() -> Result<()>
{
    let f = Fixture::new().await?;
    let scenario = async {
        let initial = Utc.timestamp_opt(1700000000, 0).unwrap();
        let runtime = Arc::new(FixedRuntime {
            now: Mutex::new(initial),
            ids: AtomicU64::new(1),
        });
        let provider = Arc::new(FixtureProvider::default());
        let app = use_case(
            &f,
            runtime.clone(),
            provider.clone(),
            Arc::new(SecureOAuthAuthorization),
        )?;
        app.authorize(IdentityProvider::Apple).await?;
        let state: String = sqlx::query_scalar("select state from oauth_authorizations")
            .fetch_one(&f.pool)
            .await?;
        app.oauth_callback(
            IdentityProvider::Google,
            "fixture-code",
            &state,
            None,
            None,
            None,
        )
        .await
        .expect_err("provider mismatch must not consume Apple state");
        anyhow::ensure!(
            provider.0.load(Ordering::SeqCst) == 0,
            "wrong provider contacted exchange"
        );
        let consumed: Option<DateTime<Utc>> =
            sqlx::query_scalar("select consumed_at from oauth_authorizations where state=$1")
                .bind(&state)
                .fetch_one(&f.pool)
                .await?;
        anyhow::ensure!(consumed.is_none(), "wrong provider consumed state");
        let (first, second) = tokio::join!(
            app.oauth_callback(
                IdentityProvider::Apple,
                "fixture-code",
                &state,
                None,
                None,
                None
            ),
            app.oauth_callback(
                IdentityProvider::Apple,
                "fixture-code",
                &state,
                None,
                None,
                None
            )
        );
        anyhow::ensure!(
            first.is_ok() != second.is_ok(),
            "exactly one concurrent callback must succeed"
        );
        anyhow::ensure!(
            provider.0.load(Ordering::SeqCst) == 1,
            "state race exchanged more than once"
        );
        let consumed: DateTime<Utc> =
            sqlx::query_scalar("select consumed_at from oauth_authorizations where state=$1")
                .bind(&state)
                .fetch_one(&f.pool)
                .await?;
        anyhow::ensure!(consumed == initial, "consumption timestamp differs");
        let sessions: i64 = sqlx::query_scalar("select count(*) from refresh_tokens")
            .fetch_one(&f.pool)
            .await?;
        anyhow::ensure!(sessions == 1, "race issued extra sessions");
        app.oauth_callback(
            IdentityProvider::Apple,
            "fixture-code",
            &state,
            None,
            None,
            None,
        )
        .await
        .expect_err("consumed state replay must fail");
        app.authorize(IdentityProvider::Apple).await?;
        let (expired, expiry): (String, DateTime<Utc>) =
            sqlx::query_as("select state, expires_at from oauth_authorizations where state<>$1")
                .bind(&state)
                .fetch_one(&f.pool)
                .await?;
        *runtime.now.lock().unwrap() = expiry;
        app.oauth_callback(
            IdentityProvider::Apple,
            "fixture-code",
            &expired,
            None,
            None,
            None,
        )
        .await
        .expect_err("exact-expiry callback must fail");
        anyhow::ensure!(
            provider.0.load(Ordering::SeqCst) == 1,
            "replay or expiry recontacted provider"
        );
        eprintln!(
            "APPLE_STATE_OBSERVATION {}",
            serde_json::json!({
                "database": f.database, "wrongProviderDidNotConsume": true,
                "concurrentSuccessCount": 1, "sessions": sessions,
                "replayDenied": true, "exactExpiryDenied": true, "providerExchangeCount": 1
            })
        );
        Ok::<_, anyhow::Error>(())
    }
    .await;
    f.pool.close().await;
    scenario
}

struct FailedCrypto;
impl OAuthAuthorizationCryptoPort for FailedCrypto {
    fn generate(
        &self,
    ) -> auth_service::application::errors::AuthResult<OAuthAuthorizationMaterialDto> {
        let result: anyhow::Result<_> =
            (|| anyhow::bail!("fixture credential generation unavailable"))();
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }
}
#[tokio::test]
async fn real_oauth_crypto_failure_saves_no_authorization_or_session() -> Result<()> {
    let f = Fixture::new().await?;
    let scenario=async {
        let runtime=Arc::new(FixedRuntime{now:Mutex::new(Utc::now()),ids:AtomicU64::new(1)});
        let provider=Arc::new(FixtureProvider::default());
        let app=use_case(&f,runtime,provider.clone(),Arc::new(FailedCrypto))?;
        app.authorize(IdentityProvider::Google).await.expect_err("crypto failure");
        let count:i64=sqlx::query_scalar("select count(*) from oauth_authorizations").fetch_one(&f.pool).await?;
        anyhow::ensure!(count==0,"failed generation saved state");
        f.empty_effects().await?;
        anyhow::ensure!(provider.0.load(Ordering::SeqCst)==0,"provider called");
        eprintln!("AUTH_RUNTIME_OBSERVATION {}",serde_json::json!({"database":f.database,"cryptoFailureZeroAuthorizationAndSession":true}));
        Ok::<_,anyhow::Error>(())
    }.await;
    f.pool.close().await;
    scenario
}
