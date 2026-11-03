//! Local browser -> executable Gateway middleware -> actual mTLS Auth RPC ->
//! owning PostgreSQL -> HTTP fixture code grant and signed OIDC verification.
//! Fixture configuration is not live Apple or deployment acceptance.
#[path = "support/oauth_form_post_network/activity.rs"]
mod activity;
#[path = "support/auth_fixture_config.rs"]
mod auth_fixture_config;
#[path = "support/auth_runtime_fixture.rs"]
mod auth_runtime_fixture;
#[path = "support/oauth_form_post_network/browser.rs"]
mod browser;
#[allow(dead_code)]
#[path = "../../api-gateway/tests/unit/http_support.rs"]
mod gateway_support;
#[path = "support/oauth_form_post_network/provider.rs"]
mod provider;

use api_gateway::adapters::out::auth_client::GrpcAuthClient;
use auth_service::adapters::out::persistence::oauth_authorization_store::PgOAuthAuthorizationStore;
use auth_service::adapters::out::persistence::user_repository::{
    PgAuthAuditRepository, PgUserRepository,
};
use auth_service::adapters::out::runtime::SystemAuthRuntime;
use auth_service::adapters::out::security::oauth_authorization::SecureOAuthAuthorization;
use auth_service::adapters::r#in::grpc::method_policy::AUTH_METHOD_POLICY;
use auth_service::adapters::r#in::grpc::server::{proto, AuthGrpcServer};
use auth_service::application::use_cases::oauth_login::OAuthLoginUseCase;
use auth_service::config::{build_oauth_provider, build_state, JwtTokenService, RuntimeMode};
use axum::middleware::from_fn_with_state;
use axum::routing::{get, post};
use platform::actor_delegation::ActorDelegationVerifier;
use platform::grpc_auth::{CallerRegistry, Workload};
use platform::grpc_method_policy::guard;
use platform::grpc_tls::TlsMaterial;
use rsa::pkcs8::EncodePrivateKey;
use sqlx::ConnectOptions;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;

struct Running {
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<anyhow::Result<()>>,
    port: u16,
}
impl Drop for Running {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        self.task.abort();
    }
}
impl Running {
    async fn shutdown(mut self) -> anyhow::Result<()> {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        let graceful =
            tokio::time::timeout(std::time::Duration::from_secs(5), &mut self.task).await;
        let result = match graceful {
            Ok(joined) => joined?,
            Err(_) => {
                self.task.abort();
                // Explicitly join the aborted task before returning any error.
                let joined =
                    tokio::time::timeout(std::time::Duration::from_secs(5), &mut self.task).await?;
                anyhow::ensure!(
                    joined.is_err_and(|error| error.is_cancelled()),
                    "fixture abort did not terminate task"
                );
                anyhow::bail!(
                    "fixture graceful shutdown deadline exceeded; task aborted and joined"
                );
            }
        };
        anyhow::ensure!(
            tokio::net::TcpStream::connect(("127.0.0.1", self.port))
                .await
                .is_err(),
            "fixture port remained open"
        );
        result
    }
}
fn tls(role: Workload) -> anyhow::Result<TlsMaterial> {
    let leaf = test_support::tls::issue(role.as_str())?;
    TlsMaterial::load(&leaf.cert, &leaf.key, &leaf.ca)
}
async fn http_server(listener: TcpListener, router: axum::Router) -> anyhow::Result<Running> {
    let port = listener.local_addr()?.port();
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async {
                let _ = stopped.await;
            })
            .await?;
        Ok(())
    });
    Ok(Running {
        stop: Some(stop),
        task,
        port,
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn browser_apple_flow_uses_actual_auth_mtls_postgres_and_verified_provider_tokens(
) -> anyhow::Result<()> {
    let fixture = auth_runtime_fixture::Fixture::new().await?;
    fixture.empty_effects().await?;
    let owning_database: String = sqlx::query_scalar("select current_database()")
        .fetch_one(&fixture.pool)
        .await?;
    anyhow::ensure!(
        owning_database == fixture.database,
        "owning database mismatch"
    );
    let gateway_db = format!("port_rust_gateway_oauth_{}", uuid::Uuid::new_v4().simple());
    sqlx::query(&format!("create database {gateway_db} template template0"))
        .execute(&fixture.pool)
        .await?;
    let gateway_options = fixture
        .pool
        .connect_options()
        .as_ref()
        .clone()
        .database(&gateway_db);
    let gateway_url = gateway_options.to_url_lossy().to_string();
    let gateway_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(12)
        .connect_with(gateway_options)
        .await?;
    sqlx::migrate::Migrator::new(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../api-gateway/migrations")
            .as_path(),
    )
    .await?
    .run(&gateway_pool)
    .await?;
    let mut servers = Vec::new();
    let journey: anyhow::Result<()> = async {
        let gateway_listener = TcpListener::bind("127.0.0.1:0").await?;
        let gateway_port = gateway_listener.local_addr()?.port();
        let provider_listener = TcpListener::bind("127.0.0.1:0").await?;
        let provider_port = provider_listener.local_addr()?.port();
        let redirect = format!("http://127.0.0.1:{gateway_port}/auth/oauth/apple/callback");
        let provider = Arc::new(provider::Provider::new(
            fixture.pool.clone(),
            gateway_pool.clone(),
            redirect.clone(),
        )?);
        let provider_base = format!("http://localhost:{provider_port}");
        let jwks = provider.jwks.clone();
        let provider_router =
            axum::Router::new()
                .route("/authorize", get(provider::authorize))
                .route("/token", post(provider::token))
                .route("/fixture/observe", get(provider::observe))
                .route(
                    "/jwks",
                    get(move || {
                        let jwks = jwks.clone();
                        async move {
                            axum::Json(serde_json::from_str::<serde_json::Value>(&jwks).unwrap())
                        }
                    }),
                )
                .with_state(provider.clone());
        servers.push(http_server(provider_listener, provider_router).await?);

        let mut config = auth_fixture_config::test_config(
            fixture.pool.connect_options().to_url_lossy().to_string(),
        )?;
        config.apple_redirect_uri = redirect;
        config.apple_authorize_base_uri = format!("{provider_base}/authorize");
        config.apple_token_uri = Some(format!("{provider_base}/token"));
        config.apple_userinfo_uri = Some(format!("{provider_base}/userinfo-must-not-be-used"));
        config.apple_issuer = "https://apple.fixture.invalid".into();
        config.apple_client_id = Some("apple-fixture-client".into());
        config.apple_client_secret = Some("local-fixture-only".into());
        config.apple_jwks_json = Some(provider.jwks.clone());
        config.apple_jwks_uri = Some(format!("{provider_base}/jwks"));
        config.google_client_id = Some("google-fixture-client".into());
        config.google_client_secret = Some("local-fixture-only".into());
        config.google_token_uri = config.apple_token_uri.clone();
        config.google_userinfo_uri = config.apple_userinfo_uri.clone();
        config.google_jwks_json = config.apple_jwks_json.clone();
        config.google_jwks_uri = config.apple_jwks_uri.clone();
        config.x_client_id = Some("x-fixture-client".into());
        config.x_client_secret = Some("local-fixture-only".into());
        config.x_token_uri = config.apple_token_uri.clone();
        config.x_userinfo_uri = config.apple_userinfo_uri.clone();
        let key = rsa::RsaPrivateKey::new(&mut rand::rngs::OsRng, 2048)?;
        config.jwt_private_key_pem = Some(key.to_pkcs8_pem(Default::default())?.to_string());
        config.jwt_key_id = Some("auth-fixture-key".into());
        let mut owner = build_state(&config, fixture.pool.clone()).await?;
        let mut provider_config = config.clone();
        provider_config.runtime_mode = RuntimeMode::Production;
        let real_provider = build_oauth_provider(&provider_config)?;
        let runtime = Arc::new(SystemAuthRuntime);
        let users = Arc::new(PgUserRepository::new(fixture.pool.clone(), runtime.clone()));
        owner.oauth_login = OAuthLoginUseCase::new(
            users.clone(),
            users,
            Arc::new(PgAuthAuditRepository::new(fixture.pool.clone())),
            Arc::new(JwtTokenService::from_private_key_pem(
                &config.issuer,
                &config.access_audience,
                config.access_ttl_seconds,
                config.refresh_ttl_seconds,
                config.jwt_private_key_pem.as_deref().unwrap(),
                "auth-fixture-key",
            )?),
            real_provider,
            Arc::new(PgOAuthAuthorizationStore::new(fixture.pool.clone())),
            owner.metrics.clone(),
            runtime,
            Arc::new(SecureOAuthAuthorization),
            config.google_redirect_uri.clone(),
            config.apple_redirect_uri.clone(),
            config.x_redirect_uri.clone(),
        );
        let auth_listener = TcpListener::bind("127.0.0.1:0").await?;
        let auth_port = auth_listener.local_addr()?.port();
        let auth_tls = tls(Workload::Auth)?;
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let service = guard(
            &AUTH_METHOD_POLICY,
            proto::auth_service_server::AuthServiceServer::new(AuthGrpcServer::new(Arc::new(
                owner,
            ))),
            CallerRegistry::new().register(Workload::Gateway, "local-gateway-fixture")?,
            ActorDelegationVerifier::local_parity(Workload::Auth),
        )?;
        let rpc_activity = provider.rpc_activity.clone();
        let server = Server::builder()
            .layer(tower::layer::layer_fn(move |inner| activity::RpcActivity {
                inner,
                activity: rpc_activity.clone(),
            }))
            .tls_config(auth_tls.server_config()?)?
            .add_service(service);
        let task = tokio::spawn(async move {
            server
                .serve_with_incoming_shutdown(TcpListenerStream::new(auth_listener), async {
                    let _ = stopped.await;
                })
                .await?;
            Ok(())
        });
        servers.push(Running {
            stop: Some(stop),
            task,
            port: auth_port,
        });
        let client = Arc::new(
            GrpcAuthClient::connect(
                format!("https://127.0.0.1:{auth_port}"),
                Workload::Gateway,
                "local-gateway-fixture",
                Some(&tls(Workload::Gateway)?),
            )
            .await?,
        );
        let harness = gateway_support::RouteHarness::with_pool_and_auth(
            gateway_pool.clone(),
            gateway_url,
            client,
        )
        .await?;
        let gateway_router = harness
            .transport_router()
            .layer(from_fn_with_state(provider.clone(), provider::race_gate));
        servers.push(http_server(gateway_listener, gateway_router).await?);
        let mut browser_command = tokio::process::Command::new(
            std::env::var("OAUTH_BROWSER_NODE").unwrap_or_else(|_| "node".into()),
        );
        browser_command
            .arg(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../scripts/testing/apple-oauth-network.cjs"),
            )
            .arg(gateway_port.to_string())
            .arg(provider_port.to_string());
        let output = browser::run(browser_command, std::time::Duration::from_secs(90)).await;
        let observation = provider::observe_inner(&provider).await;
        if !output.as_ref().is_ok_and(|output| output.status.success()) {
            eprintln!("APPLE_NETWORK_FAILURE_OWNERS {observation:?}");
        }
        let output = output?;
        anyhow::ensure!(
            output.status.success(),
            "browser network journey failed: {} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let observation = observation?;
        anyhow::ensure!(
            observation["observationStable"] == true,
            "unsettled final storage observation"
        );
        anyhow::ensure!(
            observation["gatewayJournalRows"] == 0
                && observation["gatewayQueueRows"] == 0
                && observation["credentialStorageMatches"] == 0,
            "OAuth credential persistence detected"
        );
        eprintln!(
            "APPLE_NETWORK_BROWSER {}",
            String::from_utf8_lossy(&output.stdout)
        );
        eprintln!("APPLE_NETWORK_OWNERS {observation}");
        eprintln!(
            "APPLE_NETWORK_PHASES {}",
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(())
    }
    .await;
    // Every returned setup/browser/observation error reaches this common cleanup.
    // Reverse order closes Gateway channels before shutting down the Auth owner.
    let mut cleanup_errors = Vec::new();
    while let Some(server) = servers.pop() {
        if let Err(error) = server.shutdown().await {
            cleanup_errors.push(error.to_string());
        }
    }
    gateway_pool.close().await;
    fixture.pool.close().await;
    anyhow::ensure!(
        cleanup_errors.is_empty(),
        "owned server cleanup failed: {cleanup_errors:?}; journey={journey:?}"
    );
    journey?;
    eprintln!("APPLE_NETWORK_CLEANUP all_three_owned_server_ports_closed=true");
    Ok(())
}
