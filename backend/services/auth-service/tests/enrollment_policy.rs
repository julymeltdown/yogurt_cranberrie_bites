//! Synthetic policy documents are isolated test input, never production approval.
#[path = "support/auth_fixture_config.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "../../api-gateway/tests/unit/http_support.rs"]
mod gateway_fixture;
use auth_service::application::concert_identity::{
    AdmissionRead, ReadBudget, ReadCancellation, ReadError,
};
use auth_service::enrollment_policy::*;
use chrono::{Duration, Utc};
use serde_json::{json, Value};
use std::{sync::Arc, time::Instant};
use uuid::Uuid;

fn publication() -> ApprovedPublication {
    ApprovedPublication {
        revision: Uuid::new_v4(),
        documents: vec![PolicyDocument {
            id: "test-terms".into(),
            title: "Isolated test document - no production legal approval".into(),
            url: "https://legal.example.test/test-terms".into(),
            sha256: "ab".repeat(32),
            required_acknowledgement_id: "test-terms-v1".into(),
        }],
        approval_reference: "ISOLATED_TEST_INPUT_NOT_PRODUCTION_APPROVAL".into(),
        published_at: Utc::now() - Duration::seconds(1),
        expires_at: Utc::now() + Duration::hours(1),
    }
}
#[test]
fn publication_and_closed_inputs_reject_fabricated_or_ambiguous_values() {
    let valid = publication();
    valid.validate(Utc::now()).unwrap();
    assert!(valid
        .check_acknowledgements(&["test-terms-v1".into()])
        .is_ok());
    for acks in [
        vec![],
        vec!["other".into()],
        vec!["test-terms-v1".into(), "test-terms-v1".into()],
    ] {
        assert_eq!(
            valid.check_acknowledgements(&acks),
            Err(EnrollmentError::AcknowledgementsRequired)
        );
    }
    for url in [
        "http://legal.example.test/doc",
        "https://user:secret@legal.example.test/doc",
        "javascript:alert(1)",
    ] {
        let mut bad = valid.clone();
        bad.documents[0].url = url.into();
        assert!(bad.validate(Utc::now()).is_err());
    }
    let mut bad = valid.clone();
    bad.approval_reference.clear();
    assert!(bad.validate(Utc::now()).is_err());
    bad = valid.clone();
    bad.documents.push(bad.documents[0].clone());
    assert!(bad.validate(Utc::now()).is_err());
    assert!(serde_json::from_value::<VerifyRequest>(
        json!({"attemptId":Uuid::new_v4(),"code":"123456","email":"extra@example.test"})
    )
    .is_err());
    assert_eq!(EnrollmentError::PolicyStale.http_status(), 409);
    assert_eq!(EnrollmentError::AttemptExpired.http_status(), 410);
}

struct Incoming(tokio::net::TcpListener);
impl tokio_stream::Stream for Incoming {
    type Item = Result<tokio::net::TcpStream, std::io::Error>;
    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.0
            .poll_accept(cx)
            .map(|result| Some(result.map(|(stream, _)| stream)))
    }
}
async fn http(
    client: &reqwest::Client,
    base: &str,
    method: &str,
    path: &str,
    body: Option<Value>,
    cookie: Option<&str>,
    token: Option<&str>,
) -> anyhow::Result<(u16, Value, Option<String>)> {
    let mut request = client
        .request(
            method.parse()?,
            format!("{base}/api/auth/enrollment/{path}"),
        )
        .header("origin", "http://localhost:3000")
        .header("x-mcr-intent", "browser");
    if let Some(value) = body {
        request = request.json(&value);
    }
    if let Some(value) = cookie {
        request = request.header("cookie", value);
    }
    if let Some(value) = token {
        request = request.bearer_auth(value);
    }
    let response = request.send().await?;
    let status = response.status().as_u16();
    assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
    let cookie = response
        .headers()
        .get("set-cookie")
        .map(|v| v.to_str().unwrap().split(';').next().unwrap().to_owned());
    let body = response.json().await?;
    Ok((status, body, cookie))
}
fn denied(value: &(u16, Value, Option<String>), status: u16, reason: &str) {
    assert_eq!(value.0, status, "{}", value.1);
    assert_eq!(value.1["details"]["reason"], reason);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires a fresh allocated runner-owned native PG16 round; no silent Docker fallback"]
async fn native_policy_signup_atomicity_guarded_rpc_gateway_and_restart() -> anyhow::Result<()> {
    use api_gateway::application::ports::SigningKeySourcePort;
    use auth_service::adapters::r#in::grpc::server::{
        proto::auth_service_server::AuthServiceServer, AuthGrpcServer,
    };
    let restart = std::env::var("MCR_ENROLLMENT_PHASE").as_deref() == Ok("restart");
    let marker = std::path::PathBuf::from(std::env::var("MCR_ENROLLMENT_PRIVATE_MARKER")?);
    let saved: Option<Value> = if restart {
        Some(serde_json::from_slice(&std::fs::read(&marker)?)?)
    } else {
        None
    };
    let url = if let Some(value) = &saved {
        value["databaseUrl"].as_str().unwrap().to_owned()
    } else {
        let controller_url = std::env::var("AUTH_RUNTIME_TEST_DATABASE_URL")?;
        let controller = sqlx::PgPool::connect(&controller_url).await?;
        let system: String =
            sqlx::query_scalar("select system_identifier::text from pg_control_system()")
                .fetch_one(&controller)
                .await?;
        anyhow::ensure!(
            std::env::var("AUTH_RUNTIME_TEST_INITDB_SYSTEM_IDENTIFIER")? == system,
            "runner-owned initdb required"
        );
        let version: i32 = sqlx::query_scalar("select current_setting('server_version_num')::int")
            .fetch_one(&controller)
            .await?;
        anyhow::ensure!(version / 10000 == 16, "owned PG16 required");
        let database = format!("mcr_enrollment_{}", Uuid::new_v4().simple());
        sqlx::query(&format!("create database {database} template template0"))
            .execute(&controller)
            .await?;
        controller.close().await;
        let mut url = reqwest::Url::parse(&controller_url)?;
        url.set_path(&format!("/{database}"));
        url.to_string()
    };
    let mut config = fixture::test_config(url.clone())?;
    // Keep a fixed fixture signing key in the private marker across actual restart.
    if let Some(value) = &saved {
        config.jwt_private_key_pem = Some(value["signingKey"].as_str().unwrap().into());
    } else {
        use rsa::pkcs8::EncodePrivateKey;
        config.jwt_private_key_pem = Some(
            rsa::RsaPrivateKey::new(&mut rand::rngs::OsRng, 2048)?
                .to_pkcs8_pem(Default::default())?
                .to_string(),
        );
    }
    let pool = auth_service::config::connect_pool(&config).await?;
    let app = Arc::new(auth_service::config::build_state(&config, pool.clone()).await?);
    let store = EnrollmentStore::with_verification_url(
        pool.clone(),
        "https://mcr.example.test/verify-email",
        false,
    )?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let endpoint = format!("http://{address}");
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
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(enrollment)
            .add_service(canonical)
            .serve_with_incoming_shutdown(Incoming(listener), async {
                let _ = stopped.await;
            })
            .await
    });
    // The ordinary fixed fixture caller credential is used by the real guarded RPC.
    let token = "test-gateway-credential";
    let mut harness = gateway_fixture::RouteHarness::new().await;
    let state = Arc::make_mut(&mut harness.state);
    let wire = Arc::new(
        api_gateway::adapters::out::auth_client::GrpcAuthClient::connect(
            endpoint.clone(),
            platform::grpc_auth::Workload::Gateway,
            token,
            None,
        )
        .await?,
    );
    let keys = wire.signing_keys().await?;
    state.jwt_verifier = api_gateway::adapters::out::jwt::JwtVerifier::from_jwks_json(
        &config.issuer,
        &config.access_audience,
        &keys.jwks_json,
    )?;
    state.high_risk_sessions =
        api_gateway::application::service::HighRiskSessionGuard::over_auth_client(
            wire.clone(),
            std::time::Duration::ZERO,
            Arc::new(api_gateway::adapters::out::runtime::SystemGatewayRuntime),
        );
    state.enrollment_policy = Arc::new(
        api_gateway::adapters::out::enrollment_policy::GrpcEnrollmentClient::connect(
            &endpoint,
            token,
            None,
            platform::actor_delegation::ActorDelegationSigner::local_parity(
                platform::grpc_auth::Workload::Gateway,
            ),
            Default::default(),
        )?,
    );
    state.rate_limiter =
        api_gateway::config::InMemoryRateLimiter::new(1000, std::time::Duration::from_secs(60));
    let router = api_gateway::adapters::r#in::http::router(harness.state.clone());
    let browser = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let base = format!("http://{}", browser.local_addr()?);
    let (http_stop, http_stopped) = tokio::sync::oneshot::channel();
    let http_server = tokio::spawn(async move {
        axum::serve(browser, router)
            .with_graceful_shutdown(async {
                let _ = http_stopped.await;
            })
            .await
    });
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;
    if restart {
        let saved = saved.as_ref().unwrap();
        let cookie = saved["cookie"].as_str().unwrap();
        let command = saved["command"].clone();
        let replay = http(
            &client,
            &base,
            "POST",
            "signup",
            Some(command),
            Some(cookie),
            None,
        )
        .await?;
        assert_eq!(replay.0, 202);
        assert_eq!(replay.1, saved["receipt"]);
        let user: Uuid = saved["userId"].as_str().unwrap().parse()?;
        let admission = store.admission(user).await?;
        assert_eq!(admission.decision, AdmissionDecision::Admitted);
        let policy = store.current_policy().await?;
        assert_eq!(
            policy.revision.to_string(),
            saved["revision"].as_str().unwrap()
        );
        let acceptances: i64 =
            sqlx::query_scalar("select count(*) from mcr_enrollment_acceptances where user_id=$1")
                .bind(user)
                .fetch_one(&pool)
                .await?;
        assert_eq!(acceptances, 2);
        eprintln!("ENROLLMENT_RESTART committed_acceptances=2 exact_signup_receipt=true real_pg_restart=true");
    } else {
        denied(
            &http(&client, &base, "GET", "policy", None, None, None).await?,
            503,
            "POLICY_UNAVAILABLE",
        );
        for address in ["legacy-fresh@example.test", "legacy-existing@example.test"] {
            let retired = client
                .post(format!("{base}/api/auth/email/signup"))
                .header("origin", "http://localhost:3000")
                .header("x-mcr-intent", "browser")
                .json(&json!({"email":address,"password":"legacy-test-password"}))
                .send()
                .await?;
            assert_eq!(retired.status().as_u16(), 422);
            let body: Value = retired.json().await?;
            assert_eq!(body["details"]["reason"], "ACKNOWLEDGEMENTS_REQUIRED");
        }
        let absent_origin = client
            .post(format!("{base}/api/auth/enrollment/attempts"))
            .json(&json!({"policyRevision":Uuid::new_v4()}))
            .send()
            .await?;
        assert_eq!(absent_origin.status().as_u16(), 403);
        let before: i64 = sqlx::query_scalar("select count(*) from users")
            .fetch_one(&pool)
            .await?;
        assert_eq!(before, 0);
        {
            use api_gateway::application::enrollment_policy::{
                EnrollmentOperation, EnrollmentPolicyPort,
            };
            let denied_client =
                api_gateway::adapters::out::enrollment_policy::GrpcEnrollmentClient::connect(
                    &endpoint,
                    "wrong-gateway-credential",
                    None,
                    platform::actor_delegation::ActorDelegationSigner::local_parity(
                        platform::grpc_auth::Workload::Gateway,
                    ),
                    Default::default(),
                )?;
            assert_eq!(
                denied_client
                    .execute(EnrollmentOperation::Policy, None, None, Value::Null)
                    .await
                    .unwrap_err()
                    .status,
                401
            );
        }
        let policy = publication();
        store.publish(&policy).await?;
        let actual = http(&client, &base, "GET", "policy", None, None, None).await?;
        assert_eq!(actual.0, 200);
        assert_eq!(actual.1["contractVersion"], "1.0");
        let begin = http(
            &client,
            &base,
            "POST",
            "attempts",
            Some(json!({"policyRevision":policy.revision})),
            None,
            None,
        )
        .await?;
        assert_eq!(begin.0, 201);
        let cookie = begin.2.as_deref().unwrap();
        let mut attempt = begin.1["attemptId"].as_str().unwrap().to_owned();
        let email = format!("enrollment-{}@example.test", Uuid::new_v4().simple());
        let mut command = json!({"attemptId":attempt,"policyRevision":policy.revision,"acknowledgementIds":["test-terms-v1"],"idempotencyKey":Uuid::new_v4(),"email":email,"password":"test-password-123"});
        let mut missing = command.clone();
        missing["acknowledgementIds"] = json!([]);
        denied(
            &http(
                &client,
                &base,
                "POST",
                "signup",
                Some(missing),
                Some(cookie),
                None,
            )
            .await?,
            422,
            "ACKNOWLEDGEMENTS_REQUIRED",
        );
        let receipt = http(
            &client,
            &base,
            "POST",
            "signup",
            Some(command.clone()),
            Some(cookie),
            None,
        )
        .await?;
        assert_eq!(receipt.0, 202);
        assert_eq!(receipt.1.as_object().unwrap().len(), 5);
        assert!(receipt.1.get("userId").is_none());
        let replay = http(
            &client,
            &base,
            "POST",
            "signup",
            Some(command.clone()),
            Some(cookie),
            None,
        )
        .await?;
        assert_eq!(replay.1, receipt.1);
        let mut changed = command.clone();
        changed["password"] = json!("different-test-password");
        denied(
            &http(
                &client,
                &base,
                "POST",
                "signup",
                Some(changed),
                Some(cookie),
                None,
            )
            .await?,
            409,
            "IDEMPOTENCY_CONFLICT",
        );
        let captured = Arc::new(std::sync::Mutex::new(Vec::<Value>::new()));
        let captured_mail = captured.clone();
        let mail_router = axum::Router::new().route(
            "/mail",
            axum::routing::post(move |axum::Json(body): axum::Json<Value>| {
                let captured = captured_mail.clone();
                async move {
                    captured.lock().unwrap().push(body);
                    axum::http::StatusCode::NO_CONTENT
                }
            }),
        );
        let mail_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let mail_url = format!("http://{}/mail", mail_listener.local_addr()?);
        let mail_server =
            tokio::spawn(async move { axum::serve(mail_listener, mail_router).await });
        let worker=auth_service::adapters::r#in::messaging::mail_outbox_worker::EmailOutboxWorker::new(pool.clone(),
            Some(auth_service::adapters::r#in::messaging::mail_outbox_worker::MailDestination::new(&mail_url,"isolated-test-mail",false)?),None)?;
        assert_eq!(worker.run_once().await?, 1);
        {
            let mail = captured.lock().unwrap();
            assert_eq!(mail.len(), 1);
            assert_eq!(mail[0]["enrollment_attempt_id"], attempt);
            let link = reqwest::Url::parse(mail[0]["verification_link"].as_str().unwrap())?;
            assert_eq!(link.path(), "/verify-email");
            let query: std::collections::BTreeMap<_, _> = link.query_pairs().into_owned().collect();
            assert_eq!(query.len(), 2);
            assert_eq!(query["attemptId"], attempt);
            assert_eq!(
                query["code"],
                mail[0]["verification_code"].as_str().unwrap()
            );
            assert!(!link.as_str().contains(&email));
        }
        let user: Uuid = sqlx::query_scalar("select id from users where primary_email=$1")
            .bind(&email)
            .fetch_one(&pool)
            .await?;
        let mut code: String =
            sqlx::query_scalar("select code from email_verifications where user_id=$1")
                .bind(user)
                .fetch_one(&pool)
                .await?;
        let shadow = http(
            &client,
            &base,
            "POST",
            "attempts",
            Some(json!({"policyRevision":policy.revision})),
            Some(cookie),
            None,
        )
        .await?;
        let mut duplicate = command.clone();
        duplicate["attemptId"] = shadow.1["attemptId"].clone();
        duplicate["idempotencyKey"] = json!(Uuid::new_v4());
        duplicate["password"] = json!("wrong-password-stays-shadow");
        let duplicate = http(
            &client,
            &base,
            "POST",
            "signup",
            Some(duplicate),
            Some(cookie),
            None,
        )
        .await?;
        assert_eq!(duplicate.0, 202);
        assert_eq!(duplicate.1.as_object().unwrap().len(), 5);
        denied(
            &http(
                &client,
                &base,
                "POST",
                "verify",
                Some(json!({"attemptId":shadow.1["attemptId"],"code":code})),
                Some(cookie),
                None,
            )
            .await?,
            422,
            "VERIFICATION_FAILED",
        );
        denied(&http(&client,&base,"POST","verify",Some(json!({"attemptId":attempt,"code":code})),Some("mcr-enrollment-dev=0000000000000000000000000000000000000000000000000000000000000000"),None).await?,404,"ATTEMPT_INVALID");
        let wrong = if code == "000000" { "111111" } else { "000000" };
        denied(
            &http(
                &client,
                &base,
                "POST",
                "verify",
                Some(json!({"attemptId":attempt,"code":wrong})),
                Some(cookie),
                None,
            )
            .await?,
            422,
            "VERIFICATION_FAILED",
        );
        let tries: i32 =
            sqlx::query_scalar("select attempts from email_verifications where user_id=$1")
                .bind(user)
                .fetch_one(&pool)
                .await?;
        assert_eq!(tries, 1);
        // Negative clock fixtures model an expired original attempt and elapsed
        // canonical resend throttle; identity/password/consent are never SQL-seeded.
        sqlx::query("update mcr_enrollment_attempts set created_at=clock_timestamp()-interval '2 hours',expires_at=clock_timestamp()-interval '1 minute' where id=$1")
            .bind(Uuid::parse_str(&attempt)?).execute(&pool).await?;
        denied(
            &http(
                &client,
                &base,
                "POST",
                "verify",
                Some(json!({"attemptId":attempt,"code":code})),
                Some(cookie),
                None,
            )
            .await?,
            410,
            "ATTEMPT_EXPIRED",
        );
        sqlx::query("update auth_mail_outbox set created_at=clock_timestamp()-interval '31 minutes' where user_id=$1")
            .bind(user).execute(&pool).await?;
        let resume_begin = http(
            &client,
            &base,
            "POST",
            "attempts",
            Some(json!({"policyRevision":policy.revision})),
            Some(cookie),
            None,
        )
        .await?;
        let mut resume_command = command.clone();
        resume_command["attemptId"] = resume_begin.1["attemptId"].clone();
        resume_command["idempotencyKey"] = json!(Uuid::new_v4());
        let resumed = http(
            &client,
            &base,
            "POST",
            "signup",
            Some(resume_command),
            Some(cookie),
            None,
        )
        .await?;
        assert_eq!(resumed.0, 202);
        attempt = resume_begin.1["attemptId"].as_str().unwrap().into();
        assert_eq!(worker.run_once().await?, 1);
        {
            let mail = captured.lock().unwrap();
            assert_eq!(mail.len(), 2);
            assert_eq!(mail[1]["enrollment_attempt_id"], attempt);
            let link = reqwest::Url::parse(mail[1]["verification_link"].as_str().unwrap())?;
            let pairs: std::collections::BTreeMap<_, _> = link.query_pairs().into_owned().collect();
            assert_eq!(pairs.len(), 2);
            assert_eq!(pairs["attemptId"], attempt);
            code = mail[1]["verification_code"].as_str().unwrap().into();
            assert_eq!(pairs["code"], code);
        }
        mail_server.abort();
        let _ = mail_server.await;
        let verified = http(
            &client,
            &base,
            "POST",
            "verify",
            Some(json!({"attemptId":attempt,"code":code})),
            Some(cookie),
            None,
        )
        .await?;
        assert_eq!(verified.0, 200);
        let login = app
            .login
            .login(&email, "test-password-123", None, None)
            .await?;
        let token = &login.access_token;
        let admission = http(&client, &base, "GET", "admission", None, None, Some(token)).await?;
        assert_eq!(admission.0, 200);
        assert_eq!(admission.1["decision"], "ADMITTED");
        let family: Uuid = sqlx::query_scalar(
            "select family_id from refresh_tokens where user_id=$1 and revoked_at is null limit 1",
        )
        .bind(user)
        .fetch_one(&pool)
        .await?;
        let read = AdmissionRead::new(user, family, None)?;
        let budget = || {
            ReadBudget::new(
                Instant::now() + std::time::Duration::from_secs(2),
                ReadCancellation::default(),
            )
            .unwrap()
        };
        app.concert_admission.read_current(read, &budget()).await?;
        let mut next = publication();
        next.documents[0].required_acknowledgement_id = "test-terms-v2".into();
        store.publish(&next).await?;
        assert_eq!(
            app.concert_admission
                .read_current(read, &budget())
                .await
                .unwrap_err(),
            ReadError::ConsentRequired
        );
        let admission = http(&client, &base, "GET", "admission", None, None, Some(token)).await?;
        assert_eq!(admission.1["decision"], "CONSENT_REQUIRED");
        denied(
            &http(
                &client,
                &base,
                "POST",
                "verify",
                Some(json!({"attemptId":attempt,"code":code})),
                Some(cookie),
                None,
            )
            .await?,
            409,
            "POLICY_STALE",
        );
        let consent = json!({"policyRevision":next.revision,"acknowledgementIds":["test-terms-v2"],"idempotencyKey":Uuid::new_v4()});
        let accepted = http(
            &client,
            &base,
            "POST",
            "consent",
            Some(consent.clone()),
            None,
            Some(token),
        )
        .await?;
        assert_eq!(accepted.0, 200);
        assert_eq!(accepted.1["decision"], "ADMITTED");
        assert_eq!(
            http(
                &client,
                &base,
                "POST",
                "consent",
                Some(consent),
                None,
                Some(token)
            )
            .await?
            .1,
            accepted.1
        );
        app.concert_admission.read_current(read, &budget()).await?;
        // A real acceptance-write failure must roll back account/challenge/mail too.
        sqlx::raw_sql("create function enrollment_test_fail() returns trigger language plpgsql as $$ begin raise exception 'isolated test acceptance failure'; end $$; create trigger enrollment_test_fail before insert on mcr_enrollment_acceptances for each row execute function enrollment_test_fail()")
            .execute(&pool).await?;
        let failed_begin = http(
            &client,
            &base,
            "POST",
            "attempts",
            Some(json!({"policyRevision":next.revision})),
            Some(cookie),
            None,
        )
        .await?;
        let mut failing = command.clone();
        failing["attemptId"] = failed_begin.1["attemptId"].clone();
        failing["policyRevision"] = json!(next.revision);
        failing["acknowledgementIds"] = json!(["test-terms-v2"]);
        failing["email"] = json!("rollback-only@example.test");
        denied(
            &http(
                &client,
                &base,
                "POST",
                "signup",
                Some(failing),
                Some(cookie),
                None,
            )
            .await?,
            503,
            "DEPENDENCY_UNAVAILABLE",
        );
        let remains:bool=sqlx::query_scalar("select exists(select 1 from users where primary_email='rollback-only@example.test') or exists(select 1 from user_identities where email='rollback-only@example.test') or exists(select 1 from email_verifications where email='rollback-only@example.test') or exists(select 1 from auth_mail_outbox where payload_json like '%rollback-only@example.test%')")
            .fetch_one(&pool).await?;
        assert!(!remains);
        sqlx::raw_sql("drop trigger enrollment_test_fail on mcr_enrollment_acceptances; drop function enrollment_test_fail()").execute(&pool).await?;
        // Preserve an exact current-revision idempotent signup receipt for restart.
        let final_begin = http(
            &client,
            &base,
            "POST",
            "attempts",
            Some(json!({"policyRevision":next.revision})),
            Some(cookie),
            None,
        )
        .await?;
        command["attemptId"] = final_begin.1["attemptId"].clone();
        command["policyRevision"] = json!(next.revision);
        command["acknowledgementIds"] = json!(["test-terms-v2"]);
        command["idempotencyKey"] = json!(Uuid::new_v4());
        let final_receipt = http(
            &client,
            &base,
            "POST",
            "signup",
            Some(command.clone()),
            Some(cookie),
            None,
        )
        .await?;
        assert_eq!(final_receipt.0, 202);
        let count: i64 = sqlx::query_scalar("select count(*) from users where primary_email=$1")
            .bind(&email)
            .fetch_one(&pool)
            .await?;
        assert_eq!(count, 1);
        std::fs::write(
            &marker,
            serde_json::to_vec(
                &json!({"databaseUrl":url,"signingKey":config.jwt_private_key_pem,"userId":user,"revision":next.revision,"cookie":cookie,"command":command,"receipt":final_receipt.1}),
            )?,
        )?;
        eprintln!("ENROLLMENT_SOURCE actual_identity_mail_atomic=true duplicate_nonenumerating=true actor_consent=true rollback=true");
    }
    let migrations: i64 = sqlx::query_scalar("select count(*) from _sqlx_migrations where success")
        .fetch_one(&pool)
        .await?;
    assert_eq!(migrations, 19);
    let prepared: i64 = sqlx::query_scalar("select count(*) from pg_prepared_xacts")
        .fetch_one(&pool)
        .await?;
    assert_eq!(prepared, 0);
    let _ = http_stop.send(());
    http_server.await??;
    let _ = stop.send(());
    server.await??;
    pool.close().await;
    Ok(())
}
