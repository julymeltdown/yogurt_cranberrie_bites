//! A bounded local serving fixture, not a browser acceptance verdict.
//! The FE owner exercises the actual browser and records its own journey.
#[path = "support/auth_fixture_config.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "../../api-gateway/tests/unit/http_support.rs"]
mod gateway_fixture;
use api_gateway::application::ports::SigningKeySourcePort;
use auth_service::enrollment_policy::{rpc, ApprovedPublication, EnrollmentStore, PolicyDocument};
use chrono::{Duration, Utc};
use rsa::pkcs8::EncodePrivateKey;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Instant,
};
use uuid::Uuid;

struct Incoming(tokio::net::TcpListener);
impl tokio_stream::Stream for Incoming {
    type Item = Result<tokio::net::TcpStream, std::io::Error>;
    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.0
            .poll_accept(cx)
            .map(|r| Some(r.map(|(stream, _)| stream)))
    }
}
fn private_write(path: &Path, value: &Value) -> anyhow::Result<()> {
    use std::{io::Write, os::unix::fs::OpenOptionsExt};
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(&serde_json::to_vec_pretty(value)?)?;
    Ok(())
}
async fn database(controller: &sqlx::PgPool, base: &str, prefix: &str) -> anyhow::Result<String> {
    let name = format!("mcr_enrollment_{prefix}_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("create database {name} template template0"))
        .execute(controller)
        .await?;
    let mut url = reqwest::Url::parse(base)?;
    url.set_path(&format!("/{name}"));
    Ok(url.to_string())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires a fresh BE-owned finite PG/browser pairing allocation and independent guardian"]
async fn paired_browser_fixture_uses_real_auth_rpc_gateway_and_mail() -> anyhow::Result<()> {
    use auth_service::adapters::r#in::grpc::server::{
        proto::auth_service_server::AuthServiceServer, AuthGrpcServer,
    };
    let output = PathBuf::from(std::env::var("MCR_ENROLLMENT_BROWSER_OUTPUT")?);
    let stop_file = output.join("browser-stop");
    let seconds: u64 = std::env::var("MCR_ENROLLMENT_BROWSER_SECONDS")?.parse()?;
    anyhow::ensure!(
        (1..=300).contains(&seconds),
        "finite browser fixture <=300s required"
    );
    let original_stop = Instant::now() + std::time::Duration::from_secs(seconds);
    // This exact allowlist comes from the shared actual transport fixture.
    let frontend = std::env::var("MCR_ENROLLMENT_BROWSER_FRONTEND_ORIGIN")?;
    anyhow::ensure!(
        frontend == "http://localhost:3000",
        "explicit paired local origin required"
    );
    anyhow::ensure!(
        output.is_dir() && !stop_file.exists(),
        "new owned fixture output required"
    );
    let controller_url = std::env::var("AUTH_RUNTIME_TEST_DATABASE_URL")?;
    let controller = sqlx::PgPool::connect(&controller_url).await?;
    let system: String =
        sqlx::query_scalar("select system_identifier::text from pg_control_system()")
            .fetch_one(&controller)
            .await?;
    anyhow::ensure!(
        system == std::env::var("AUTH_RUNTIME_TEST_INITDB_SYSTEM_IDENTIFIER")?,
        "runner-owned initdb required"
    );
    let version: i32 = sqlx::query_scalar("select current_setting('server_version_num')::int")
        .fetch_one(&controller)
        .await?;
    anyhow::ensure!(version / 10000 == 16, "owned PG16 required");
    let auth_url = database(&controller, &controller_url, "auth").await?;
    let gateway_url = database(&controller, &controller_url, "gateway").await?;
    controller.close().await;
    let mut config = fixture::test_config(auth_url)?;
    config.jwt_private_key_pem = Some(
        rsa::RsaPrivateKey::new(&mut rand::rngs::OsRng, 2048)?
            .to_pkcs8_pem(Default::default())?
            .to_string(),
    );
    let auth_pool = auth_service::config::connect_pool(&config).await?;
    let app = Arc::new(auth_service::config::build_state(&config, auth_pool.clone()).await?);
    let store = EnrollmentStore::with_verification_url(
        auth_pool.clone(),
        &format!("{frontend}/verify-email"),
        false,
    )?;
    let document = b"ISOLATED BROWSER TEST DOCUMENT. NOT PRODUCTION LEGAL APPROVAL.";
    let policy = ApprovedPublication {
        revision: Uuid::new_v4(),
        documents: vec![PolicyDocument {
            id: "isolated-browser-terms".into(),
            title: "Isolated browser test terms - not production legal approval".into(),
            url: "https://legal.example.test/isolated-browser-terms-v1".into(),
            sha256: format!("{:x}", Sha256::digest(document)),
            required_acknowledgement_id: "isolated-browser-terms-v1".into(),
        }],
        approval_reference: "ISOLATED_BROWSER_TEST_OPERATOR_INPUT_NOT_PRODUCTION_APPROVAL".into(),
        published_at: Utc::now() - Duration::seconds(1),
        expires_at: Utc::now() + Duration::minutes(10),
    };
    store.publish(&policy).await?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let auth_endpoint = format!("http://{}", listener.local_addr()?);
    let enrollment = rpc::guarded_server(
        store.clone(),
        app.signup.clone(),
        config.caller_registry.clone(),
        config.actor_delegation.clone(),
    )?;
    let canonical = platform::grpc_method_policy::guard(
        &auth_service::adapters::r#in::grpc::method_policy::AUTH_METHOD_POLICY,
        AuthServiceServer::new(AuthGrpcServer::new(app.clone())),
        config.caller_registry.clone(),
        config.actor_delegation.clone(),
    )?;
    let (auth_stop, auth_stopped) = tokio::sync::oneshot::channel();
    let auth_server = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(enrollment)
            .add_service(canonical)
            .serve_with_incoming_shutdown(Incoming(listener), async {
                let _ = auth_stopped.await;
            })
            .await
    });
    let wire = Arc::new(
        api_gateway::adapters::out::auth_client::GrpcAuthClient::connect(
            auth_endpoint.clone(),
            platform::grpc_auth::Workload::Gateway,
            "test-gateway-credential",
            None,
        )
        .await?,
    );
    let gateway_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&gateway_url)
        .await?;
    let migrations = Path::new(env!("CARGO_MANIFEST_DIR")).join("../api-gateway/migrations");
    migration_runtime::load_migrations(&migrations)
        .await?
        .run(&gateway_pool)
        .await?;
    // Auth login/refresh/logout and high-risk reads use the actual RPC client.
    // Unused Profile/Friend/Post/Notification ports remain explicitly unclaimed.
    let mut harness = gateway_fixture::RouteHarness::with_pool_and_auth(
        gateway_pool.clone(),
        gateway_url,
        wire.clone(),
    )
    .await?;
    let keys = wire.signing_keys().await?;
    let state = Arc::make_mut(&mut harness.state);
    state.jwt_verifier = api_gateway::adapters::out::jwt::JwtVerifier::from_jwks_json(
        &config.issuer,
        &config.access_audience,
        &keys.jwks_json,
    )?;
    state.high_risk_sessions =
        api_gateway::application::service::HighRiskSessionGuard::over_auth_client(
            wire,
            std::time::Duration::ZERO,
            Arc::new(api_gateway::adapters::out::runtime::SystemGatewayRuntime),
        );
    state.enrollment_policy = Arc::new(
        api_gateway::adapters::out::enrollment_policy::GrpcEnrollmentClient::connect(
            &auth_endpoint,
            "test-gateway-credential",
            None,
            platform::actor_delegation::ActorDelegationSigner::local_parity(
                platform::grpc_auth::Workload::Gateway,
            ),
            Default::default(),
        )?,
    );
    state.rate_limiter =
        api_gateway::config::InMemoryRateLimiter::new(1000, std::time::Duration::from_secs(60));
    let router = harness.transport_router();
    let browser = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let gateway_origin = format!("http://localhost:{}", browser.local_addr()?.port());
    let (http_stop, http_stopped) = tokio::sync::oneshot::channel();
    let http_server = tokio::spawn(async move {
        axum::serve(browser, router)
            .with_graceful_shutdown(async {
                let _ = http_stopped.await;
            })
            .await
    });
    let captured = Arc::new(Mutex::new(Vec::<Value>::new()));
    let capture = captured.clone();
    let mail_router = axum::Router::new().route(
        "/mail",
        axum::routing::post(move |axum::Json(body): axum::Json<Value>| {
            let capture = capture.clone();
            async move {
                capture.lock().unwrap().push(body);
                axum::http::StatusCode::NO_CONTENT
            }
        }),
    );
    let mail_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let mail_origin = format!("http://{}/mail", mail_listener.local_addr()?);
    let mail_server = tokio::spawn(async move { axum::serve(mail_listener, mail_router).await });
    let worker =
        auth_service::adapters::r#in::messaging::mail_outbox_worker::EmailOutboxWorker::new(
            auth_pool.clone(),
            Some(
                auth_service::adapters::r#in::messaging::mail_outbox_worker::MailDestination::new(
                    &mail_origin,
                    "isolated-browser-mail",
                    false,
                )?,
            ),
            None,
        )?;
    private_write(
        &output.join("browser-ready.json"),
        &json!({
            "gatewayOrigin":gateway_origin,"frontendOrigin":frontend,"policyRevision":policy.revision,
            "syntheticPolicyOnly":true,"mailCapturePattern":"browser-mail-N.json",
            "verdict":"SERVING_FIXTURE_ONLY_BROWSER_ACCEPTANCE_REMAINS_FE_OWNED"
        }),
    )?;
    let mut observed = 0usize;
    while !stop_file.exists() && Instant::now() < original_stop {
        tokio::time::timeout(std::time::Duration::from_secs(2), worker.run_once()).await??;
        let mails = captured.lock().unwrap().clone();
        for mail in mails.iter().skip(observed) {
            private_write(
                &output.join(format!("browser-mail-{observed}.json")),
                &json!({
                    "attemptId":mail["enrollment_attempt_id"],"verificationLink":mail["verification_link"],
                    "verificationCode":mail["verification_code"],"expiresAt":mail["expires_at"],
                }),
            )?;
            observed += 1;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let accounts: i64 = sqlx::query_scalar("select count(*) from users")
        .fetch_one(&auth_pool)
        .await?;
    private_write(
        &output.join("browser-fixture-stop.json"),
        &json!({"capturedMails":observed,"accounts":accounts,
        "explicitStop":stop_file.exists(),"verdict":"NO_BROWSER_ACCEPTANCE_VERDICT"}),
    )?;
    mail_server.abort();
    let _ = mail_server.await;
    let _ = http_stop.send(());
    http_server.await??;
    let _ = auth_stop.send(());
    auth_server.await??;
    gateway_pool.close().await;
    auth_pool.close().await;
    Ok(())
}
