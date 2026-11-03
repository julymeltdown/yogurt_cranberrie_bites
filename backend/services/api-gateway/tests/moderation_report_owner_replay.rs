#[path = "support/mod.rs"]
mod support;

use api_gateway::adapters::r#in::http;
use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::Response,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

fn journal_ports(
    pool: &sqlx::PgPool,
) -> api_gateway::application::journal_runtime_ports::JournalRuntimePorts {
    api_gateway::config::journal_runtime_ports(
        pool,
        &api_gateway::config::GatewayMetrics::default(),
    )
}

const REPORTER: &str = "11111111-1111-1111-1111-111111111111";
const TARGET: &str = "22222222-2222-2222-2222-222222222222";

#[tokio::test]
async fn moderation_repository_failure_http_missing_report_is_not_a_write() -> anyhow::Result<()> {
    let app = support::start_gateway_app(false).await?;
    let router = http::server::with_transport_layers(
        app.router.clone(),
        &app.config,
        http::client_identity::TrustedProxies::default(),
    );
    let result = async {
        let path = format!(
            "/admin/moderation/reports/{}/decisions",
            Uuid::from_u128(211)
        );
        let response = router
            .oneshot(semantic_request(
                &path,
                json!({"decisionType":"DISMISS","note":"Reviewed"}),
                true,
            ))
            .await?;
        let status = response.status();
        let response = body(response).await;
        println!(
            "MODERATION_FAILURE_HTTP {}",
            json!({"case":"missing-report","status":status.as_u16(),"message":response["message"]})
        );
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(response["message"], "Report not found");
        assert_eq!(report_count(&app.pool).await?, 0);
        let decisions: i64 = sqlx::query_scalar("select count(*) from moderation_decisions")
            .fetch_one(&app.pool)
            .await?;
        assert_eq!(decisions, 0);
        anyhow::Ok(())
    }
    .await;
    app.shutdown().await?;
    result
}

#[tokio::test]
async fn moderation_repository_failure_http_lookup_errors_never_redispatch() -> anyhow::Result<()> {
    let app = support::start_gateway_app(false).await?;
    let router = http::server::with_transport_layers(
        app.router.clone(),
        &app.config,
        http::client_identity::TrustedProxies::default(),
    );
    let result = async {
        let request = || create_report_request("repository-lookup-probe", TARGET, "Repository failure boundary probe");
        let first = router.clone().oneshot(request()).await?;
        assert_eq!(first.status(), StatusCode::CREATED);
        let operation_id = Uuid::parse_str(first.headers().get("x-operation-id").unwrap().to_str()?)?;
        let original = body(first).await;
        let journal: Value = sqlx::query_scalar("select to_jsonb(j) from gateway_request_journal j where id=$1")
            .bind(operation_id).fetch_one(&app.pool).await?;
        let receipt: Value = sqlx::query_scalar("select outcome_json from moderation_report_operation_receipts where operation_id=$1")
            .bind(operation_id).fetch_one(&app.pool).await?;
        sqlx::query("alter table moderation_report_operation_receipts rename to private_moderation_lookup_probe").execute(&app.pool).await?;
        let response = router.clone().oneshot(request()).await?;
        sqlx::query("alter table private_moderation_lookup_probe rename to moderation_report_operation_receipts").execute(&app.pool).await?;
        let status = response.status();
        let response = body(response).await;
        println!("MODERATION_FAILURE_HTTP {}", json!({"case":"lookup-storage","status":status.as_u16(),"message":response["message"]}));
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response["message"], "Service unavailable");
        assert!(!response.to_string().contains("private_moderation_lookup_probe"));
        let after: Value = sqlx::query_scalar("select to_jsonb(j) from gateway_request_journal j where id=$1")
            .bind(operation_id).fetch_one(&app.pool).await?;
        assert_eq!(after, journal);
        assert_eq!(report_count(&app.pool).await?, 1);
        let recovered = router.clone().oneshot(request()).await?;
        assert_eq!(recovered.status(), StatusCode::CREATED);
        assert_eq!(body(recovered).await, original);
        for (name, command) in [
            ("lookup-conflict", "update moderation_report_operation_receipts set outcome_json='{}'::jsonb where operation_id=$1"),
            ("lookup-expired", "update moderation_report_operation_receipts set state='EXPIRED', outcome_json=null where operation_id=$1"),
            ("lookup-missing", "delete from moderation_report_operation_receipts where operation_id=$1"),
        ] {
            if name == "lookup-expired" {
                sqlx::query("update moderation_report_operation_receipts set outcome_json=$2 where operation_id=$1")
                    .bind(operation_id).bind(receipt.clone()).execute(&app.pool).await?;
            }
            sqlx::query(command).bind(operation_id).execute(&app.pool).await?;
            let response = router.clone().oneshot(request()).await?;
            let status = response.status();
            let response = body(response).await;
            println!("MODERATION_FAILURE_HTTP {}", json!({"case":name,"status":status.as_u16(),"message":response["message"]}));
            assert_eq!(status, StatusCode::CONFLICT);
            assert_eq!(response["message"], "Report operation receipt is unavailable or expired; reconcile before retrying");
            assert_eq!(report_count(&app.pool).await?, 1);
            let after: Value = sqlx::query_scalar("select to_jsonb(j) from gateway_request_journal j where id=$1")
                .bind(operation_id).fetch_one(&app.pool).await?;
            assert_eq!(after, journal);
        }
        anyhow::Ok(())
    }.await;
    app.shutdown().await?;
    result
}

fn semantic_request(path: &str, payload: Value, admin: bool) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(path)
        .header(
            "authorization",
            format!(
                "Bearer {}",
                support::bearer_token(
                    REPORTER,
                    if admin {
                        &["ROLE_ADMIN"]
                    } else {
                        &["ROLE_USER"]
                    }
                )
            ),
        )
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap()
}

#[tokio::test]
async fn moderation_input_http_report_validation_and_storage_failures_are_distinct(
) -> anyhow::Result<()> {
    let app = support::start_gateway_app(false).await?;
    let router = http::server::with_transport_layers(
        app.router.clone(),
        &app.config,
        http::client_identity::TrustedProxies::default(),
    );
    let result = async {
        let valid = json!({"targetType":"POST", "targetId":TARGET,
            "category":"SPAM", "reason":"Repeated unsolicited advertising"});
        let cases = [
            ("self", "targetType", json!("USER"), "Cannot report yourself"),
            ("target-type", "targetType", json!("PRIVATE_BAD_TYPE"), "Report target must be USER, POST, or COMMENT"),
            ("target-id", "targetId", json!("PRIVATE_BAD_UUID"), "Report target ID is required"),
            ("category", "category", json!("PRIVATE_BAD_CATEGORY"), "Report category must be SPAM, HARASSMENT, HATE, VIOLENCE, or OTHER"),
            ("short-reason", "reason", json!(" a "), "Report reason must be 3-1000 characters"),
            ("long-reason", "reason", json!("x".repeat(1001)), "Report reason must be 3-1000 characters"),
        ];
        for (name, field, value, message) in cases {
            let mut payload = valid.clone();
            payload[field] = value;
            if name == "self" { payload["targetId"] = json!(REPORTER); }
            let response = router.clone().oneshot(semantic_request("/api/moderation/reports", payload, false)).await?;
            let status = response.status();
            let response = body(response).await;
            println!("MODERATION_INPUT_HTTP {}", json!({"case":name,"status":status.as_u16(),"message":response["message"]}));
            assert_eq!(status, StatusCode::BAD_REQUEST, "{name}");
            assert_eq!(response["message"], message);
            assert_eq!(report_count(&app.pool).await?, 0);
        }
        // A receipt-less keyed retry remains a reconciliation conflict. A400
        // response must not be used as evidence that an owner operation committed.
        let mut journal_snapshot = None;
        for attempt in 0..2 {
            let mut request = semantic_request("/api/moderation/reports", json!({
                "targetType":"USER", "targetId":REPORTER,
                "category":"SPAM", "reason":"Repeated unsolicited advertising"
            }), false);
            request.headers_mut().insert("idempotency-key", "invalid-self-report".parse()?);
            let response = router.clone().oneshot(request).await?;
            let expected = if attempt == 0 { StatusCode::BAD_REQUEST } else { StatusCode::CONFLICT };
            assert_eq!(response.status(), expected);
            let operation_id = Uuid::parse_str(response.headers().get("x-operation-id").unwrap().to_str()?)?;
            let message = if attempt == 0 { "Cannot report yourself" } else {
                "Report operation receipt is unavailable or expired; reconcile before retrying"
            };
            assert_eq!(body(response).await["message"], message);
            let snapshot: Value = sqlx::query_scalar("select to_jsonb(j) from gateway_request_journal j where id = $1")
                .bind(operation_id).fetch_one(&app.pool).await?;
            assert_eq!(snapshot["state"], "UNKNOWN");
            assert_eq!(snapshot["outcome"], "UNKNOWN");
            assert_eq!(snapshot["response_status"], 400);
            if let Some(prior) = &journal_snapshot { assert_eq!(&snapshot, prior); }
            journal_snapshot = Some(snapshot);
            assert_eq!(report_count(&app.pool).await?, 0);
        }
        let receipts: i64 = sqlx::query_scalar("select count(*) from moderation_report_operation_receipts")
            .fetch_one(&app.pool).await?;
        assert_eq!(receipts, 0);
        let response = router.clone().oneshot(semantic_request("/api/moderation/reports", json!({"targetType":"POST"}), false)).await?;
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let created = router.clone().oneshot(semantic_request("/api/moderation/reports", valid.clone(), false)).await?;
        assert_eq!(created.status(), StatusCode::CREATED);
        assert_eq!(report_count(&app.pool).await?, 1);
        sqlx::query("alter table moderation_reports add constraint private_moderation_storage_probe check (reason <> 'private-storage-probe')")
            .execute(&app.pool).await?;
        let mut payload = valid;
        payload["reason"] = json!("private-storage-probe");
        let mut request = semantic_request("/api/moderation/reports", payload, false);
        request.headers_mut().insert("idempotency-key", "input-storage-probe".parse()?);
        let response = router.oneshot(request).await?;
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let response = body(response).await;
        assert_eq!(response["message"], "Internal error");
        assert!(!response.to_string().contains("private_moderation_storage_probe"));
        assert!(!response.to_string().contains("private-storage-probe"));
        assert_eq!(report_count(&app.pool).await?, 1);
        let receipts: i64 = sqlx::query_scalar("select count(*) from moderation_report_operation_receipts where idempotency_key = 'input-storage-probe'")
            .fetch_one(&app.pool).await?;
        assert_eq!(receipts, 0);
        anyhow::Ok(())
    }.await;
    app.shutdown().await?;
    result
}

#[tokio::test]
async fn moderation_input_http_decision_validation_precedes_persistence() -> anyhow::Result<()> {
    let app = support::start_gateway_app(false).await?;
    let router = http::server::with_transport_layers(
        app.router.clone(),
        &app.config,
        http::client_identity::TrustedProxies::default(),
    );
    let result = async {
        let created = router.clone().oneshot(semantic_request("/api/moderation/reports",
            json!({"targetType":"POST","targetId":TARGET,"category":"SPAM","reason":"Repeated unsolicited advertising"}), false)).await?;
        assert_eq!(created.status(), StatusCode::CREATED);
        let report_id = body(created).await["id"].as_str().unwrap().to_owned();
        let path = format!("/admin/moderation/reports/{report_id}/decisions");
        for (name, route, payload, message) in [
            ("report-id", "/admin/moderation/reports/PRIVATE_BAD_UUID/decisions".to_owned(), json!({"decisionType":"DISMISS"}), "Report ID is required"),
            ("decision-type", path.clone(), json!({"decisionType":"PRIVATE_BAD_DECISION"}), "Moderation decision must be DISMISS, HIDE_CONTENT, RESTORE_CONTENT, RESTRICT_USER, or LIFT_RESTRICTION"),
            ("long-note", path.clone(), json!({"decisionType":"DISMISS","note":"x".repeat(2001)}), "Moderation note must be at most 2000 characters"),
            ("wrong-target", path.clone(), json!({"decisionType":"RESTRICT_USER"}), "User restriction decisions require a USER report"),
        ] {
            let response = router.clone().oneshot(semantic_request(&route, payload, true)).await?;
            let status = response.status();
            let response = body(response).await;
            println!("MODERATION_INPUT_HTTP {}", json!({"case":name,"status":status.as_u16(),"message":response["message"]}));
            assert_eq!(status, StatusCode::BAD_REQUEST, "{name}");
            assert_eq!(response["message"], message);
            let decisions: i64 = sqlx::query_scalar("select count(*) from moderation_decisions").fetch_one(&app.pool).await?;
            assert_eq!(decisions, 0);
            let status: String = sqlx::query_scalar("select status from moderation_reports where id = $1").bind(Uuid::parse_str(&report_id)?).fetch_one(&app.pool).await?;
            assert_eq!(status, "OPEN");
        }
        let accepted = router.oneshot(semantic_request(&path, json!({"decisionType":"DISMISS","note":"Reviewed"}), true)).await?;
        assert_eq!(accepted.status(), StatusCode::OK);
        anyhow::Ok(())
    }.await;
    app.shutdown().await?;
    result
}

#[tokio::test]
async fn moderation_repository_dtos_write_and_read_the_historical_v1_receipt_shape(
) -> anyhow::Result<()> {
    use api_gateway::adapters::out::repository::moderation_repository::PgModerationRepository;
    use api_gateway::application::dto::moderation_dto::ModerationMutationContextDto;
    use api_gateway::application::ports::ModerationRepositoryPort;
    use api_gateway::domain::moderation::{
        ModerationReport, ReportCategory, ReportStatus, ReportTargetType,
    };

    let app = support::start_gateway_app(false).await?;
    let result = async {
        let actor_id = Uuid::parse_str(REPORTER)?;
        let created_at = chrono::DateTime::parse_from_rfc3339("2024-02-29T12:34:56.123456Z")?
            .with_timezone(&chrono::Utc);
        let report = ModerationReport {
            id: Uuid::from_u128(19),
            reporter_id: actor_id,
            target_type: ReportTargetType::Comment,
            target_id: Uuid::parse_str(TARGET)?,
            category: ReportCategory::Harassment,
            reason: "private historical receipt 報告".into(),
            status: ReportStatus::Open,
            created_at,
            resolved_at: None,
        };
        let context = ModerationMutationContextDto {
            operation_id: Uuid::new_v4(),
            actor_id,
            idempotency_key: Some("receipt-dto-shape".into()),
            lookup_only: false,
        };
        let repository = PgModerationRepository::new(app.pool.clone());
        let stored = repository
            .create_report_with_mutation(report.clone().into(), Some(context.clone()))
            .await?;
        assert_eq!(ModerationReport::from(stored), report);
        let written: Value = sqlx::query_scalar(
            "select outcome_json from moderation_report_operation_receipts where operation_id = $1",
        )
        .bind(context.operation_id)
        .fetch_one(&app.pool)
        .await?;
        let historical = json!({
            "version": 1,
            "report": {
                "id": "00000000-0000-0000-0000-000000000013",
                "reporter_id": REPORTER,
                "target_type": "COMMENT",
                "target_id": TARGET,
                "category": "HARASSMENT",
                "reason": "private historical receipt 報告",
                "status": "OPEN",
                "created_at": "2024-02-29T12:34:56.123456Z",
                "resolved_at": null,
            }
        });
        assert_eq!(written, historical);

        // Inject independently specified historical bytes, then mutate the live row.
        // Lookup must return the receipt snapshot rather than reconstructing the report.
        sqlx::query("update moderation_report_operation_receipts set outcome_json = $2 where operation_id = $1")
            .bind(context.operation_id)
            .bind(historical)
            .execute(&app.pool)
            .await?;
        sqlx::query("update moderation_reports set reason = 'changed live report' where id = $1")
            .bind(report.id)
            .execute(&app.pool)
            .await?;
        let replayed = repository
            .create_report_with_mutation(
                report.clone().into(),
                Some(ModerationMutationContextDto { lookup_only: true, ..context }),
            )
            .await?;
        assert_eq!(ModerationReport::from(replayed), report);
        assert_eq!(report_count(&app.pool).await?, 1);
        anyhow::Ok(())
    }.await;
    app.shutdown().await?;
    result
}

fn create_report_request(key: &str, target: &str, reason: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/api/moderation/reports")
        .header(
            "authorization",
            format!("Bearer {}", support::bearer_token(REPORTER, &["ROLE_USER"])),
        )
        .header("content-type", "application/json")
        .header("idempotency-key", key)
        .body(Body::from(
            json!({
                "targetType": "POST",
                "targetId": target,
                "category": "SPAM",
                "reason": reason,
            })
            .to_string(),
        ))
        .unwrap()
}

fn operation_status_request(operation_id: Uuid) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(format!("/admin/operations/{operation_id}"))
        .header(
            "authorization",
            format!(
                "Bearer {}",
                support::bearer_token(REPORTER, &["ROLE_ADMIN"])
            ),
        )
        .body(Body::empty())
        .unwrap()
}

async fn body(response: Response) -> Value {
    serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
}

async fn report_count(pool: &sqlx::PgPool) -> anyhow::Result<i64> {
    Ok(
        sqlx::query_scalar("select count(*) from moderation_reports")
            .fetch_one(pool)
            .await?,
    )
}

#[tokio::test]
async fn moderation_reports_commit_owner_receipts_and_only_replay_through_lookup(
) -> anyhow::Result<()> {
    let app = support::start_gateway_app(false).await?;
    let router = http::server::with_transport_layers(
        app.router.clone(),
        &app.config,
        http::client_identity::TrustedProxies::default(),
    );

    let result = async {
    sqlx::query(
        "alter table moderation_report_operation_receipts add constraint reject_test_report_receipt check (actor_id <> '11111111-1111-1111-1111-111111111111')",
    )
    .execute(&app.pool)
    .await?;
    let rejected = router
        .clone()
        .oneshot(create_report_request(
            "receipt-rollback",
            TARGET,
            "duplicate content",
        ))
        .await?;
    assert_eq!(rejected.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(report_count(&app.pool).await?, 0);
    let receipt_count: i64 =
        sqlx::query_scalar("select count(*) from moderation_report_operation_receipts")
            .fetch_one(&app.pool)
            .await?;
    assert_eq!(receipt_count, 0, "report write rolls back with its receipt");
    sqlx::query(
        "alter table moderation_report_operation_receipts drop constraint reject_test_report_receipt",
    )
    .execute(&app.pool)
    .await?;

    sqlx::query(
        "alter table gateway_request_journal add constraint reject_test_report_completion check (resource_path <> '/api/moderation/reports' or state <> 'COMPLETED')",
    )
    .execute(&app.pool)
    .await?;
    let first = router
        .clone()
        .oneshot(create_report_request(
            "report-replay",
            TARGET,
            "duplicate content",
        ))
        .await?;
    assert_eq!(first.status(), StatusCode::CREATED);
    let operation_id = Uuid::parse_str(
        first
            .headers()
            .get("x-operation-id")
            .expect("operation id")
            .to_str()?,
    )?;
    let first_body = body(first).await;
    assert_eq!(report_count(&app.pool).await?, 1);
    let journal_state: String =
        sqlx::query_scalar("select state from gateway_request_journal where id = $1")
            .bind(operation_id)
            .fetch_one(&app.pool)
            .await?;
    assert_eq!(journal_state, "IN_PROGRESS");
    let receipt_state: String = sqlx::query_scalar(
        "select state from moderation_report_operation_receipts where operation_id = $1",
    )
    .bind(operation_id)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(receipt_state, "REPLAYABLE");
    sqlx::query(
        "alter table gateway_request_journal drop constraint reject_test_report_completion",
    )
    .execute(&app.pool)
    .await?;

    let pending_status = router
        .clone()
        .oneshot(operation_status_request(operation_id))
        .await?;
    assert_eq!(pending_status.status(), StatusCode::OK);
    let pending_status = body(pending_status).await;
    assert_eq!(pending_status["ownerReceipt"]["lookupStatus"], "FOUND");
    assert_eq!(
        pending_status["ownerReceipt"]["scope"],
        "gateway.v1.CreateModerationReport"
    );
    assert!(pending_status["ownerReceipt"]
        .get("idempotencyKey")
        .is_none());

    let replayed = router
        .clone()
        .oneshot(create_report_request(
            "report-replay",
            TARGET,
            "duplicate content",
        ))
        .await?;
    assert_eq!(replayed.status(), StatusCode::CREATED);
    assert_eq!(
        replayed.headers().get("idempotency-replayed").unwrap(),
        "true"
    );
    assert_eq!(body(replayed).await, first_body);
    assert_eq!(report_count(&app.pool).await?, 1);
    let journal_state: String =
        sqlx::query_scalar("select state from gateway_request_journal where id = $1")
            .bind(operation_id)
            .fetch_one(&app.pool)
            .await?;
    assert_eq!(journal_state, "COMPLETED");

    let conflict = router
        .clone()
        .oneshot(create_report_request(
            "report-replay",
            TARGET,
            "a different reason",
        ))
        .await?;
    assert_eq!(conflict.status(), StatusCode::CONFLICT);
    assert_eq!(report_count(&app.pool).await?, 1);

    sqlx::query("delete from moderation_report_operation_receipts where operation_id = $1")
        .bind(operation_id)
        .execute(&app.pool)
        .await?;
    let missing_receipt = router
        .clone()
        .oneshot(create_report_request(
            "report-replay",
            TARGET,
            "duplicate content",
        ))
        .await?;
    assert_eq!(missing_receipt.status(), StatusCode::CONFLICT);
    assert_eq!(report_count(&app.pool).await?, 1);
    let missing_status = router
        .clone()
        .oneshot(operation_status_request(operation_id))
        .await?;
    assert_eq!(
        body(missing_status).await["ownerReceipt"]["lookupStatus"],
        "NOT_FOUND"
    );

    let retained = router
        .clone()
        .oneshot(create_report_request(
            "report-retention",
            TARGET,
            "duplicate content",
        ))
        .await?;
    assert_eq!(retained.status(), StatusCode::CREATED);
    let retained_id = Uuid::parse_str(
        retained
            .headers()
            .get("x-operation-id")
            .expect("operation id")
            .to_str()?,
    )?;
    assert_eq!(report_count(&app.pool).await?, 1);
    sqlx::query(
        "update moderation_report_operation_receipts set committed_at = now() - interval '2 days', replay_until = now() - interval '1 second' where operation_id = $1",
    )
    .bind(retained_id)
    .execute(&app.pool)
    .await?;
    let unexpired_id = Uuid::new_v4();
    sqlx::query(
        "insert into moderation_report_operation_receipts (operation_id, actor_id, scope, outcome_json, committed_at, replay_until) values ($1, $2, 'gateway.v1.CreateModerationReport', '{\"version\":1}'::jsonb, now(), now() + interval '1 day')",
    )
    .bind(unexpired_id)
    .bind(Uuid::parse_str(REPORTER)?)
    .execute(&app.pool)
    .await?;
    sqlx::query(
        r#"
        insert into moderation_report_operation_receipts (
            operation_id, actor_id, scope, outcome_json, committed_at, replay_until
        )
        select md5('expired-report-receipt-' || item::text)::uuid,
               $1, 'gateway.v1.CreateModerationReport', jsonb_build_object('version', 1),
               now() - interval '4 days', now() - interval '3 days'
        from generate_series(1, 501) as series(item)
        "#,
    )
    .bind(Uuid::parse_str(REPORTER)?)
    .execute(&app.pool)
    .await?;
    let cleanup =
        api_gateway::adapters::r#in::http::request_journal::reconcile_stale(&journal_ports(&app.pool)).await?;
    assert_eq!(cleanup.report_receipt_bodies_pruned, 500);
    let next_cleanup =
        api_gateway::adapters::r#in::http::request_journal::reconcile_stale(&journal_ports(&app.pool)).await?;
    assert_eq!(next_cleanup.report_receipt_bodies_pruned, 2);
    let (state, has_outcome): (String, bool) = sqlx::query_as(
        "select state, outcome_json is not null from moderation_report_operation_receipts where operation_id = $1",
    )
    .bind(retained_id)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(state, "EXPIRED");
    assert!(!has_outcome);
    let unexpired_body_retained: bool = sqlx::query_scalar(
        "select outcome_json is not null from moderation_report_operation_receipts where operation_id = $1",
    )
    .bind(unexpired_id)
    .fetch_one(&app.pool)
    .await?;
    assert!(unexpired_body_retained);
    let expired_retry = router
        .clone()
        .oneshot(create_report_request(
            "report-retention",
            TARGET,
            "duplicate content",
        ))
        .await?;
    assert_eq!(expired_retry.status(), StatusCode::CONFLICT);
    assert_eq!(report_count(&app.pool).await?, 1);
    let expired_status = router
        .clone()
        .oneshot(operation_status_request(retained_id))
        .await?;
    let expired_status = body(expired_status).await;
    assert_eq!(expired_status["ownerReceipt"]["lookupStatus"], "FOUND");
    assert_eq!(expired_status["ownerReceipt"]["receiptState"], "EXPIRED");
    Ok(())
    }
    .await;
    app.shutdown().await?;
    result
}
