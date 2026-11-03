use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::time::timeout;
use tower::ServiceExt;

fn peer(ip: &str) -> ConnectInfo<SocketAddr> {
    ConnectInfo(SocketAddr::new(ip.parse().expect("test peer IP"), 12345))
}

#[tokio::test]
async fn legacy_audit_port_preserves_private_columns_and_best_effort_http_on_audit_failure(
) -> anyhow::Result<()> {
    use api_gateway::adapters::out::repository::legacy_audit_repository::PgLegacyAuditRepository;
    use api_gateway::application::dto::legacy_audit_dto::{
        LegacyAuditCommandDto, LegacyAuditOutcomeDto,
    };
    use api_gateway::application::legacy_audit_error::LegacyAuditStorageError;
    use api_gateway::application::ports::LegacyAuditRepositoryPort;

    let app = crate::support::start_gateway_app(false).await?;
    let result: anyhow::Result<()> = async {
    let event_id = uuid::Uuid::new_v4();
    app.state
        .legacy_audit_repository
        .record(LegacyAuditCommandDto {
            event_id,
            actor_id: None,
            action: "POST /legacy-audit/private-canary".into(),
            resource_path: "/legacy-audit/private-canary".into(),
            outcome: LegacyAuditOutcomeDto::Failure,
            request_id: Some("request-canary".into()),
            ip_address: Some("198.51.100.12".into()),
            user_agent: Some("private-agent-canary".into()),
        })
        .await?;
    type AuditRow = (
        Option<String>,
        String,
        String,
        Option<String>,
        String,
        String,
        String,
        String,
        String,
        bool,
    );
    let row: AuditRow =
        sqlx::query_as("select actor_id, action, resource_type, resource_id, outcome, request_id, ip_address, user_agent, metadata_json, occurred_at is not null from admin_audit_events where id=$1")
        .bind(event_id).fetch_one(&app.pool).await?;
    assert_eq!(
        row,
        (
            None,
            "POST /legacy-audit/private-canary".into(),
            "/legacy-audit/private-canary".into(),
            None,
            "FAILURE".into(),
            "request-canary".into(),
            "198.51.100.12".into(),
            "private-agent-canary".into(),
            "{}".into(),
            true,
        )
    );
    let user_token =
        crate::support::bearer_token("11111111-1111-1111-1111-111111111111", &["ROLE_USER"]);
    let request = || {
        Request::builder()
            .method(Method::POST)
            .uri("/api/posts")
            .header("authorization", format!("Bearer {user_token}"))
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"content":"audit write must not alter reply","visibility":"PUBLIC"}"#,
            ))
            .expect("post request")
    };
    let success = app.router.clone().oneshot(request()).await?;
    assert_eq!(success.status(), StatusCode::OK);
    let count: i64 = sqlx::query_scalar("select count(*) from admin_audit_events where action='POST /api/posts' and outcome='SUCCESS'")
        .fetch_one(&app.pool).await?;
    assert_eq!(count, 1);
    let audit_metrics = || {
        app.state
            .metrics
            .render_prometheus()
            .lines()
            .filter(|line| line.starts_with("admin_audit_events_total{"))
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    let before = audit_metrics();
    assert!(!before.is_empty(), "successful audit increments its metric");
    // Closing the shared application pool would also break mandatory route
    // storage. Test a closed repository pool separately, then fail only the
    // actual audit INSERT so the HTTP check isolates best-effort persistence.
    let closed_pool = sqlx::postgres::PgPoolOptions::new().connect_lazy(&app.config.database_url)?;
    closed_pool.close().await;
    let unavailable = PgLegacyAuditRepository::new(closed_pool)
        .record(LegacyAuditCommandDto {
            event_id: uuid::Uuid::new_v4(),
            actor_id: None,
            action: "POST /unavailable".into(),
            resource_path: "/unavailable".into(),
            outcome: LegacyAuditOutcomeDto::Success,
            request_id: None,
            ip_address: None,
            user_agent: None,
        })
        .await;
    assert_eq!(unavailable, Err(LegacyAuditStorageError));
    assert_eq!(
        LegacyAuditStorageError.to_string(),
        "Legacy audit storage failed"
    );
    sqlx::raw_sql("create function reject_legacy_audit_insert() returns trigger language plpgsql as $$ begin raise exception 'test audit storage failure'; end $$; create trigger reject_legacy_audit_insert before insert on admin_audit_events for each row execute function reject_legacy_audit_insert();")
        .execute(&app.pool).await?;
    let response = app.router.clone().oneshot(request()).await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await?.to_bytes())?;
    assert_eq!(body["content"], "audit write must not alter reply");
    assert_eq!(
        audit_metrics(),
        before,
        "failed audit must not increment its metric"
    );
    let count: i64 = sqlx::query_scalar("select count(*) from admin_audit_events where action='POST /api/posts'")
        .fetch_one(&app.pool).await?;
    assert_eq!(count, 1, "rejected audit INSERT leaves no additional event");
    Ok(())
    }.await;
    app.shutdown().await?;
    result
}

#[tokio::test]
async fn postgres_test_app_static_permit_is_held_until_test_app_drop() {
    let limiter = crate::support::postgres_test_app_limiter();
    let first = crate::support::start_gateway_app(false)
        .await
        .expect("first app");
    let first_pool = first.pool.clone();
    assert_eq!(limiter.available_permits(), 0);

    let mut second = Box::pin(crate::support::start_gateway_app_with_profile_delay(
        true,
        Duration::from_millis(50),
        10,
    ));
    assert!(
        timeout(Duration::from_millis(50), &mut second)
            .await
            .is_err(),
        "second app must wait while the first TestApp owns the static permit"
    );

    first.shutdown().await.expect("first app shutdown");
    let second = timeout(Duration::from_secs(120), second)
        .await
        .expect("second app permit wait")
        .expect("second app");
    assert!(
        first_pool.is_closed(),
        "the previous pool must be closed before the next app receives capacity"
    );
    assert_eq!(
        first_pool.size(),
        0,
        "the previous pool must have no live connections before the next app starts"
    );
    assert_eq!(limiter.available_permits(), 0);
    let response = second
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/profile/11111111-1111-1111-1111-111111111111")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    second.shutdown().await.expect("second app shutdown");

    timeout(Duration::from_secs(120), limiter.drain())
        .await
        .expect("final cleanup wait")
        .expect("final cleanup");
}

#[tokio::test]
async fn failed_postgres_removal_poisoning_is_reported_without_releasing_capacity() {
    let limiter = crate::support::isolated_postgres_test_app_limiter();
    let app = crate::support::start_gateway_app_with_injected_cleanup_failure(limiter.clone())
        .await
        .expect("app");

    let shutdown_error = app
        .shutdown()
        .await
        .expect_err("injected removal failure must fail shutdown");
    assert!(shutdown_error
        .to_string()
        .contains("injected testcontainer removal failure"));
    let drain_error = timeout(Duration::from_secs(120), limiter.drain())
        .await
        .expect("cleanup failure wait")
        .expect_err("injected removal failure must poison cleanup capacity");
    assert!(drain_error
        .to_string()
        .contains("injected testcontainer removal failure"));
    assert_eq!(limiter.available_permits(), 0);

    let acquire_error = timeout(Duration::from_secs(1), limiter.acquire())
        .await
        .expect("failed cleanup must reject promptly")
        .err()
        .expect("failed cleanup must prevent a successful acquire");
    assert!(acquire_error
        .to_string()
        .contains("previous Gateway test app cleanup failed"));
    assert_eq!(limiter.available_permits(), 0);
}

#[tokio::test]
async fn dropping_test_app_without_shutdown_fails_loudly_and_finishes_cleanup() {
    let limiter = crate::support::postgres_test_app_limiter();
    let dropped = tokio::spawn(async {
        let app = crate::support::start_gateway_app(false).await.expect("app");
        drop(app);
    })
    .await;
    let panic = dropped.expect_err("missing shutdown must panic");
    assert!(panic.is_panic());
    timeout(Duration::from_secs(120), limiter.drain())
        .await
        .expect("fallback cleanup wait")
        .expect("fallback cleanup");
}

#[tokio::test]
async fn forged_admin_jwt_is_rejected_before_admin_routes_run() {
    let app = crate::support::start_gateway_app(true).await.expect("app");
    let forged_admin_token = crate::support::unsigned_bearer_token(
        "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
        &["ROLE_ADMIN"],
    );

    let response = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/contracts")
                .header("authorization", format!("Bearer {forged_admin_token}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    app.shutdown().await.expect("app shutdown");
}

#[tokio::test]
async fn shadow_auth_route_is_not_registered_in_production_mode() {
    let app = crate::support::start_gateway_app(true).await.expect("app");

    let response = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/shadow/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"email":"hana@example.com","password":"Passw0rd!"}"#,
                ))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    app.shutdown().await.expect("app shutdown");
}

#[tokio::test]
async fn rate_limit_returns_too_many_requests_after_configured_quota() {
    let app = crate::support::start_gateway_app_with_rate_limit(true, 2)
        .await
        .expect("app");

    for expected_status in [
        StatusCode::OK,
        StatusCode::OK,
        StatusCode::TOO_MANY_REQUESTS,
    ] {
        let response = app
            .router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/profile/11111111-1111-1111-1111-111111111111")
                    .extension(peer("203.0.113.10"))
                    .header("x-forwarded-for", "203.0.113.10")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), expected_status);
    }
    app.shutdown().await.expect("app shutdown");
}

#[tokio::test]
async fn request_timeout_returns_gateway_timeout_for_slow_downstream() {
    let app =
        crate::support::start_gateway_app_with_profile_delay(true, Duration::from_millis(50), 10)
            .await
            .expect("app");

    let response = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/profile/11111111-1111-1111-1111-111111111111")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    app.shutdown().await.expect("app shutdown");
}

#[tokio::test]
async fn admin_request_records_audit_actor_and_request_context() {
    let app = crate::support::start_gateway_app(true).await.expect("app");
    let admin_token =
        crate::support::bearer_token("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa", &["ROLE_ADMIN"]);

    let response = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/contracts")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("x-request-id", "req-admin-audit-1")
                .extension(peer("198.51.100.7"))
                .header("x-forwarded-for", "203.0.113.99")
                .header("user-agent", "audit-test/1.0")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);

    let count: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from admin_audit_events
        where actor_id = $1
          and request_id = $2
          and ip_address = $3
          and user_agent = $4
          and action = $5
          and outcome = $6
        "#,
    )
    .bind("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa")
    .bind("req-admin-audit-1")
    .bind("198.51.100.7")
    .bind("audit-test/1.0")
    .bind("GET /admin/contracts")
    .bind("SUCCESS")
    .fetch_one(&app.pool)
    .await
    .expect("audit count");

    assert_eq!(count, 1);
    app.shutdown().await.expect("app shutdown");
}

#[tokio::test]
async fn public_mutation_records_audit_actor_and_request_context() {
    let app = crate::support::start_gateway_app(true).await.expect("app");
    let user_token =
        crate::support::bearer_token("11111111-1111-1111-1111-111111111111", &["ROLE_USER"]);

    let response = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/posts")
                .header("authorization", format!("Bearer {user_token}"))
                .header("content-type", "application/json")
                .header("x-request-id", "req-public-mutation-audit-1")
                .extension(peer("198.51.100.9"))
                .header("x-forwarded-for", "203.0.113.99")
                .header("user-agent", "public-mutation-audit-test/1.0")
                .body(Body::from(
                    r#"{"content":"audited mutation","visibility":"PUBLIC"}"#,
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);

    let count: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from admin_audit_events
        where actor_id = $1
          and request_id = $2
          and ip_address = $3
          and user_agent = $4
          and action = $5
          and outcome = $6
        "#,
    )
    .bind("11111111-1111-1111-1111-111111111111")
    .bind("req-public-mutation-audit-1")
    .bind("198.51.100.9")
    .bind("public-mutation-audit-test/1.0")
    .bind("POST /api/posts")
    .bind("SUCCESS")
    .fetch_one(&app.pool)
    .await
    .expect("audit count");

    assert_eq!(count, 1);
    app.shutdown().await.expect("app shutdown");
}

#[tokio::test]
async fn public_read_does_not_write_mutation_audit() {
    let app = crate::support::start_gateway_app(true).await.expect("app");
    let user_token =
        crate::support::bearer_token("11111111-1111-1111-1111-111111111111", &["ROLE_USER"]);

    let response = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/feed")
                .header("authorization", format!("Bearer {user_token}"))
                .header("x-request-id", "req-public-read-audit-1")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);

    let count: i64 =
        sqlx::query_scalar("select count(*) from admin_audit_events where request_id = $1")
            .bind("req-public-read-audit-1")
            .fetch_one(&app.pool)
            .await
            .expect("audit count");

    assert_eq!(count, 0);
    app.shutdown().await.expect("app shutdown");
}

#[tokio::test]
async fn metrics_endpoint_reports_gateway_security_control_counters() {
    let app = crate::support::start_gateway_app_with_rate_limit(true, 1)
        .await
        .expect("app");
    let admin_token =
        crate::support::bearer_token("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa", &["ROLE_ADMIN"]);
    let forged_admin_token = crate::support::unsigned_bearer_token(
        "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
        &["ROLE_ADMIN"],
    );

    for expected_status in [StatusCode::OK, StatusCode::TOO_MANY_REQUESTS] {
        let response = app
            .router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/profile/11111111-1111-1111-1111-111111111111")
                    .extension(peer("203.0.113.44"))
                    .header("x-forwarded-for", "203.0.113.44")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("profile response");
        assert_eq!(response.status(), expected_status);
    }

    let forged_response = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/contracts")
                .header("authorization", format!("Bearer {forged_admin_token}"))
                .extension(peer("203.0.113.45"))
                .header("x-forwarded-for", "203.0.113.45")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("forged response");
    assert_eq!(forged_response.status(), StatusCode::UNAUTHORIZED);

    let admin_response = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/contracts")
                .header("authorization", format!("Bearer {admin_token}"))
                .extension(peer("203.0.113.46"))
                .header("x-forwarded-for", "203.0.113.46")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("admin response");
    assert_eq!(admin_response.status(), StatusCode::OK);

    let metrics_response =
        api_gateway::adapters::r#in::http::metrics_router(app.state.metrics.clone())
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .header("x-forwarded-for", "203.0.113.47")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("metrics response");
    assert_eq!(metrics_response.status(), StatusCode::OK);
    let metrics_body = String::from_utf8(
        metrics_response
            .into_body()
            .collect()
            .await
            .expect("metrics body")
            .to_bytes()
            .to_vec(),
    )
    .expect("utf8 metrics");

    assert!(metrics_body.contains("gateway_rate_limit_rejections_total"));
    assert!(metrics_body.contains("gateway_jwt_verification_failures_total"));
    assert!(metrics_body.contains("admin_audit_events_total"));
    assert!(metrics_body.contains("route=\"/api/profile/:user_id\""));
    assert!(!metrics_body.contains("11111111-1111-1111-1111-111111111111"));
    assert!(metrics_body.contains("reason=\"invalid_bearer_token\""));
    assert!(metrics_body.contains("action=\"GET /admin/contracts\""));
    assert!(metrics_body.contains("outcome=\"SUCCESS\""));
    app.shutdown().await.expect("app shutdown");
}

#[tokio::test]
async fn metrics_endpoint_reports_gateway_timeout_counters() {
    let app =
        crate::support::start_gateway_app_with_profile_delay(true, Duration::from_millis(50), 10)
            .await
            .expect("app");

    let timeout_response = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/profile/11111111-1111-1111-1111-111111111111")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("timeout response");
    assert_eq!(timeout_response.status(), StatusCode::GATEWAY_TIMEOUT);

    let metrics_response =
        api_gateway::adapters::r#in::http::metrics_router(app.state.metrics.clone())
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("metrics response");
    assert_eq!(metrics_response.status(), StatusCode::OK);
    let metrics_body = String::from_utf8(
        metrics_response
            .into_body()
            .collect()
            .await
            .expect("metrics body")
            .to_bytes()
            .to_vec(),
    )
    .expect("utf8 metrics");

    assert!(metrics_body.contains("gateway_request_timeouts_total"));
    assert!(metrics_body.contains("route=\"/api/profile/:user_id\""));
    app.shutdown().await.expect("app shutdown");
}
