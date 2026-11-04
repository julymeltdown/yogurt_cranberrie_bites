//! A bounded local serving fixture, not a browser acceptance verdict.
//! The FE owner exercises the actual browser and records its own journey.
#[path = "support/auth_fixture_config.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "../../api-gateway/tests/unit/http_support.rs"]
mod gateway_fixture;
use api_gateway::application::ports::SigningKeySourcePort;
use auth_service::enrollment_policy::{rpc, ApprovedPublication, EnrollmentStore, PolicyDocument};
use axum::response::IntoResponse;
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
fn private_control(path: &Path, output: &Path) -> anyhow::Result<Option<Value>> {
    use std::{io::Read, os::unix::fs::MetadataExt};
    let before = match std::fs::symlink_metadata(path) {
        Ok(info) => info,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    anyhow::ensure!(
        before.is_file()
            && !before.file_type().is_symlink()
            && before.mode() & 0o777 == 0o600
            && before.uid() == std::fs::metadata(output)?.uid()
            && before.len() <= 4096,
        "private bounded atomic control file required"
    );
    let mut file = std::fs::File::open(path)?;
    let opened = file.metadata()?;
    anyhow::ensure!(
        (before.dev(), before.ino()) == (opened.dev(), opened.ino()),
        "control inode changed"
    );
    let mut bytes = Vec::new();
    (&mut file).take(4097).read_to_end(&mut bytes)?;
    let after = std::fs::symlink_metadata(path)?;
    anyhow::ensure!(
        !after.file_type().is_symlink()
            && bytes.len() <= 4096
            && (
                before.dev(),
                before.ino(),
                before.len(),
                before.mtime(),
                before.mtime_nsec(),
                before.ctime(),
                before.ctime_nsec()
            ) == (
                after.dev(),
                after.ino(),
                after.len(),
                after.mtime(),
                after.mtime_nsec(),
                after.ctime(),
                after.ctime_nsec()
            ),
        "control changed during read"
    );
    Ok(Some(serde_json::from_slice(&bytes)?))
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
    let mut policy = ApprovedPublication {
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
    // The shared harness supplies unused foundation test ports. Make those
    // unreachable: this fixture can prove Auth only and must never accidentally
    // supply a synthetic bootstrap/profile to the actual frontend shell.
    let router = harness.transport_router().layer(axum::middleware::from_fn(
        |request: axum::extract::Request, next: axum::middleware::Next| async move {
            if !request.uri().path().starts_with("/api/auth/") {
                let mut response =
                    api_gateway::adapters::r#in::http::GatewayError::ServiceUnavailable(
                        anyhow::anyhow!("fixture dependency unavailable"),
                    )
                    .into_response();
                response.headers_mut().insert(
                    axum::http::header::CACHE_CONTROL,
                    axum::http::HeaderValue::from_static("no-store"),
                );
                response.headers_mut().insert(
                    "referrer-policy",
                    axum::http::HeaderValue::from_static("no-referrer"),
                );
                return response;
            }
            next.run(request).await
        },
    ));
    let browser = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let gateway_origin = format!("http://localhost:{}", browser.local_addr()?.port());
    let gateway_loopback_origin = format!("http://{}", browser.local_addr()?);
    let (http_stop, http_stopped) = tokio::sync::oneshot::channel();
    let http_server = tokio::spawn(async move {
        axum::serve(browser, router)
            .with_graceful_shutdown(async {
                let _ = http_stopped.await;
            })
            .await
    });
    let probe = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(2))
        .build()?;
    let policy_reply = probe
        .get(format!(
            "{gateway_loopback_origin}/api/auth/enrollment/policy"
        ))
        .header("origin", &frontend)
        .header("x-mcr-intent", "browser")
        .send()
        .await?;
    anyhow::ensure!(
        policy_reply.status().as_u16() == 200
            && policy_reply
                .headers()
                .get("cache-control")
                .is_some_and(|v| v == "no-store")
            && policy_reply.headers().get("set-cookie").is_none(),
        "actual policy HTTP boot probe required"
    );
    anyhow::ensure!(
        policy_reply.json::<Value>().await?["revision"] == policy.revision.to_string(),
        "boot probe must reach real published Auth policy"
    );
    let begin_path = format!("{gateway_loopback_origin}/api/auth/enrollment/attempts");
    let denied = probe
        .post(&begin_path)
        .header("x-mcr-intent", "browser")
        .json(&json!({"policyRevision":policy.revision}))
        .send()
        .await?;
    anyhow::ensure!(
        denied.status().as_u16() == 403,
        "missing Origin must remain denied"
    );
    let bound = probe
        .post(&begin_path)
        .header("origin", &frontend)
        .header("x-mcr-intent", "browser")
        .json(&json!({"policyRevision":policy.revision}))
        .send()
        .await?;
    anyhow::ensure!(
        bound.status().as_u16() == 201,
        "actual Origin-bound attempt boot required"
    );
    let cookie = bound
        .headers()
        .get("set-cookie")
        .ok_or_else(|| anyhow::anyhow!("actual bound cookie required"))?
        .to_str()?;
    anyhow::ensure!(
        cookie.starts_with("mcr-enrollment-dev=")
            && cookie.contains("HttpOnly")
            && cookie.contains("SameSite=Strict")
            && cookie.contains("Path=/api/auth/enrollment"),
        "actual browser binding cookie attributes required"
    );
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
            "gatewayLoopbackOrigin":gateway_loopback_origin,
            "binarySha256":std::env::var("MCR_ENROLLMENT_BROWSER_BINARY_SHA256")?,
            "sourceRevision":std::env::var("MCR_ENROLLMENT_BROWSER_SOURCE_REVISION")?,
            "runtimeMigrationSha256":serde_json::from_str::<Value>(&std::env::var("MCR_ENROLLMENT_BROWSER_RUNTIME_SQL")?)?,
            "runtimeMigrationPaths":serde_json::from_str::<Value>(&std::env::var("MCR_ENROLLMENT_BROWSER_RUNTIME_SQL_PATHS")?)?,
            "syntheticPolicyOnly":true,"mailCapturePattern":"browser-mail-N.json","authOnly":true,
            "verdict":"SERVING_FIXTURE_ONLY_BROWSER_ACCEPTANCE_REMAINS_FE_OWNED"
        }),
    )?;
    let mut observed = 0usize;
    let mut expired_control = false;
    let mut rotated_control = false;
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
        if !expired_control {
            if let Some(control) = private_control(&output.join("browser-expire.json"), &output)? {
                let object = control
                    .as_object()
                    .ok_or_else(|| anyhow::anyhow!("closed expiry control required"))?;
                anyhow::ensure!(
                    object.len() == 1 && object.contains_key("attemptId"),
                    "closed expiry control required"
                );
                let text = control["attemptId"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("canonical attempt required"))?;
                let attempt: Uuid = text.parse()?;
                anyhow::ensure!(
                    !attempt.is_nil() && attempt.to_string() == text,
                    "canonical attempt required"
                );
                let mail = mails
                    .iter()
                    .rev()
                    .find(|mail| mail["enrollment_attempt_id"] == text)
                    .ok_or_else(|| anyhow::anyhow!("actual captured enrollment mail required"))?;
                let code = mail["verification_code"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("actual code required"))?;
                // Negative-only fixture precondition. Identity, acceptance,
                // attempt state, audit, outbox and receipts remain untouched.
                let expired:Vec<chrono::DateTime<Utc>> = sqlx::query_scalar(
                    "update email_verifications v set expires_at=least(v.expires_at,clock_timestamp()-interval '1 second') from mcr_enrollment_attempts a,users u where a.id=$1 and a.state='PENDING' and a.expires_at>clock_timestamp() and a.user_id=u.id and u.status='PENDING_VERIFICATION' and u.deleted_at is null and v.user_id=u.id and v.code=$2 and v.expires_at>clock_timestamp() returning v.expires_at"
                ).bind(attempt).bind(code).fetch_all(&auth_pool).await?;
                anyhow::ensure!(
                    expired.len() == 1 && expired[0] < Utc::now(),
                    "one actual current challenge must expire"
                );
                private_write(
                    &output.join("browser-challenge-expired.json"),
                    &json!({
                        "attemptId":attempt,"actualChallengeExpiresAt":expired[0],"serverTime":Utc::now(),"negativeOnly":true
                    }),
                )?;
                expired_control = true;
            }
        }
        if !rotated_control {
            if let Some(control) = private_control(&output.join("browser-rotate.json"), &output)? {
                let object = control
                    .as_object()
                    .ok_or_else(|| anyhow::anyhow!("closed policy control required"))?;
                anyhow::ensure!(
                    object.len() == 1 && control["expectedRevision"] == policy.revision.to_string(),
                    "exact current synthetic policy required"
                );
                let previous = policy.revision;
                let verified:bool=sqlx::query_scalar("select exists(select 1 from mcr_enrollment_attempts a join users u on u.id=a.user_id where a.state='VERIFIED' and a.revision=$1 and u.status='ACTIVE' and u.deleted_at is null)")
                    .bind(previous).fetch_one(&auth_pool).await?;
                anyhow::ensure!(
                    verified,
                    "actual verified signup required before synthetic rotation"
                );
                let mut next = policy.clone();
                next.revision = Uuid::new_v4();
                next.documents[0].title =
                    "Isolated browser test terms v2 - not production legal approval".into();
                next.documents[0].url =
                    "https://legal.example.test/isolated-browser-terms-v2".into();
                next.documents[0].sha256 = format!(
                    "{:x}",
                    Sha256::digest(b"ISOLATED BROWSER TEST V2. NOT PRODUCTION LEGAL APPROVAL.")
                );
                next.documents[0].required_acknowledgement_id = "isolated-browser-terms-v2".into();
                next.approval_reference =
                    "ISOLATED_BROWSER_TEST_ROTATION_NOT_PRODUCTION_APPROVAL".into();
                next.published_at = Utc::now();
                next.expires_at = Utc::now() + Duration::minutes(10);
                store.publish(&next).await?;
                private_write(
                    &output.join("browser-policy-rotated.json"),
                    &json!({
                        "previousRevision":previous,"policyRevision":next.revision,
                        "requiredAcknowledgementIds":[next.documents[0].required_acknowledgement_id],
                        "serverTime":Utc::now(),"syntheticPolicyOnly":true
                    }),
                )?;
                policy = next;
                rotated_control = true;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let accounts: i64 = sqlx::query_scalar("select count(*) from users")
        .fetch_one(&auth_pool)
        .await?;
    private_write(
        &output.join("browser-fixture-stop.json"),
        &json!({"capturedMails":observed,"accounts":accounts,
        "negativeExpiryControlled":expired_control,"syntheticPolicyRotationControlled":rotated_control,
        "bootHttp":{"actualPublishedPolicy":200,"missingOriginBegin":403,"boundBegin":201},
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
