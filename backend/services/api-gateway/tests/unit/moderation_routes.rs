use crate::http_support::{response_json, RouteHarness, OTHER_USER_ID, USER_ID};
use api_gateway::adapters::r#in::http::{admin_routes, moderation_routes, post_routes};
use api_gateway::application::dto::moderation_repository_dto::{
    ContentModerationDecisionOutboxEntryDto as ContentModerationDecisionOutboxEntry,
    ContentModerationDecisionOutboxStateDto as ContentModerationDecisionOutboxState,
    ModerationDecisionTypeDto as ModerationDecisionType, ReportTargetTypeDto as ReportTargetType,
};
use api_gateway::application::ports::ModerationRepositoryPort;
use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use chrono::Utc;
use serde_json::json;
use tower::ServiceExt;
use uuid::Uuid;

#[tokio::test]
async fn neutral_repository_failures_keep_safe_http_categories_without_causes() {
    use api_gateway::application::moderation_repository_error::ModerationRepositoryError as Failure;
    use axum::response::IntoResponse;
    for (failure, status, message) in [
        (
            Failure::ReportMissing,
            StatusCode::NOT_FOUND,
            "Report not found",
        ),
        (
            Failure::ReceiptMissing,
            StatusCode::CONFLICT,
            "Report operation receipt is unavailable or expired; reconcile before retrying",
        ),
        (
            Failure::ReceiptExpired,
            StatusCode::CONFLICT,
            "Report operation receipt is unavailable or expired; reconcile before retrying",
        ),
        (
            Failure::ReceiptConflict,
            StatusCode::CONFLICT,
            "Report operation receipt is unavailable or expired; reconcile before retrying",
        ),
        (
            Failure::TargetUnavailable,
            StatusCode::CONFLICT,
            "Moderation target is no longer available",
        ),
        (
            Failure::DecisionInProgress,
            StatusCode::CONFLICT,
            "Moderation decision is still being processed",
        ),
        (
            Failure::DecisionConflict,
            StatusCode::CONFLICT,
            "Moderation report already has a different decision",
        ),
        (
            Failure::ResolutionNoteRequired,
            StatusCode::BAD_REQUEST,
            "A non-empty note is required to dismiss a report with an unavailable content target",
        ),
        (
            Failure::ReplayConflict,
            StatusCode::CONFLICT,
            "Moderation content decision is not available for the requested replay",
        ),
        (
            Failure::Storage,
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal error",
        ),
        (
            Failure::Unexpected,
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal error",
        ),
    ] {
        assert!(std::error::Error::source(&failure).is_none());
        let response = api_gateway::adapters::r#in::http::map_service_error(
            anyhow::Error::new(failure).context("private repository diagnostic"),
        )
        .into_response();
        assert_eq!(response.status(), status);
        let response = response_json(response).await;
        assert_eq!(response["message"], message);
        assert!(!response
            .to_string()
            .contains("private repository diagnostic"));
        let forged = api_gateway::adapters::r#in::http::map_service_error(anyhow::Error::msg(
            failure.to_string(),
        ))
        .into_response();
        assert_eq!(forged.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(response_json(forged).await["message"], "Internal error");
    }
}

#[test]
fn repository_adapter_classifies_concrete_causes_and_never_diagnostic_text() {
    use api_gateway::adapters::out::repository::moderation_repository::classify_moderation_repository_error as classify;
    use api_gateway::application::moderation_repository_error::ModerationRepositoryError as Failure;
    use api_gateway::domain::moderation::*;
    for (error, expected) in [
        (
            anyhow::Error::new(ReportMutationReceiptError::Missing),
            Failure::ReceiptMissing,
        ),
        (
            anyhow::Error::new(ReportMutationReceiptError::Expired),
            Failure::ReceiptExpired,
        ),
        (
            anyhow::Error::new(ReportMutationReceiptError::Conflict),
            Failure::ReceiptConflict,
        ),
        (
            anyhow::Error::new(ModerationTargetUnavailable),
            Failure::TargetUnavailable,
        ),
        (
            anyhow::Error::new(ModerationDecisionInProgress),
            Failure::DecisionInProgress,
        ),
        (
            anyhow::Error::new(ModerationDecisionConflict),
            Failure::DecisionConflict,
        ),
        (
            anyhow::Error::new(ModerationResolutionNoteRequired),
            Failure::ResolutionNoteRequired,
        ),
        (
            anyhow::Error::new(ContentDecisionReplayConflict),
            Failure::ReplayConflict,
        ),
        (
            anyhow::Error::new(sqlx::Error::Protocol("private database diagnostic".into())),
            Failure::Storage,
        ),
        (
            anyhow::Error::new(sqlx::Error::RowNotFound),
            Failure::Storage,
        ),
        (anyhow::anyhow!("Report not found"), Failure::Unexpected),
        (
            anyhow::anyhow!("moderation report mutation receipt is expired"),
            Failure::Unexpected,
        ),
    ] {
        let classified = classify(error.context("private outer diagnostic"));
        assert_eq!(classified, expected);
        assert!(std::error::Error::source(&classified).is_none());
        assert!(!classified.to_string().contains("private"));
    }
    for failure in [
        Failure::ReportMissing,
        Failure::ReceiptMissing,
        Failure::ReceiptExpired,
        Failure::ReceiptConflict,
        Failure::TargetUnavailable,
        Failure::DecisionInProgress,
        Failure::DecisionConflict,
        Failure::ResolutionNoteRequired,
        Failure::ReplayConflict,
        Failure::Storage,
        Failure::Unexpected,
    ] {
        assert_eq!(
            classify(anyhow::Error::new(failure).context("private cause")),
            failure
        );
        assert_eq!(
            classify(anyhow::Error::msg(failure.to_string())),
            Failure::Unexpected
        );
        assert_eq!(
            failure.is_receipt(),
            matches!(
                failure,
                Failure::ReceiptMissing | Failure::ReceiptExpired | Failure::ReceiptConflict
            )
        );
    }
}

#[tokio::test]
async fn typed_moderation_inputs_project_safe_messages_and_text_cannot_forge_them() {
    use api_gateway::application::moderation_input::ModerationInputError as Input;
    use axum::response::IntoResponse;
    let cases = [
        (Input::ReporterId, "Reporter ID is required"),
        (Input::TargetId, "Report target ID is required"),
        (Input::ReportId, "Report ID is required"),
        (Input::ReviewerId, "Reviewer ID is required"),
        (Input::ActorId, "Actor ID is required"),
        (Input::UserId, "User ID is required"),
        (Input::TargetType, "Report target must be USER, POST, or COMMENT"),
        (Input::SelfReport, "Cannot report yourself"),
        (Input::Category, "Report category must be SPAM, HARASSMENT, HATE, VIOLENCE, or OTHER"),
        (Input::Reason, "Report reason must be 3-1000 characters"),
        (Input::DecisionType, "Moderation decision must be DISMISS, HIDE_CONTENT, RESTORE_CONTENT, RESTRICT_USER, or LIFT_RESTRICTION"),
        (Input::Note, "Moderation note must be at most 2000 characters"),
        (Input::ContentTarget, "Content decisions require a POST or COMMENT report"),
        (Input::UserTarget, "User restriction decisions require a USER report"),
        (Input::ReplayResolutionId, "Replay resolution ID is required"),
        (Input::ReplayReason, "reason must contain 20 to 1024 non-control characters and at most 1024 UTF-8 bytes"),
        (Input::ReplayEvidence, "evidenceReference must be a 3 to 256 character protected-record identifier"),
    ];
    for (input, message) in cases {
        assert_eq!(input.to_string(), message);
        assert!(std::error::Error::source(&input).is_none());
        let error = anyhow::Error::new(input).context("private moderation diagnostic");
        let response = api_gateway::adapters::r#in::http::map_service_error(error).into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let response = response_json(response).await;
        assert_eq!(response["message"], message);
        assert!(!response
            .to_string()
            .contains("private moderation diagnostic"));
        let response =
            api_gateway::adapters::r#in::http::map_service_error(anyhow::Error::msg(message))
                .into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(response_json(response).await["message"], "Internal error");
    }
}

#[tokio::test]
async fn authenticated_user_can_create_and_list_own_report() {
    let harness = RouteHarness::new().await;
    let app = moderation_routes::router(harness.state.clone());
    let create = Request::builder()
        .method(Method::POST)
        .uri("/api/moderation/reports")
        .header(header::AUTHORIZATION, harness.bearer_token(USER_ID))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "targetType": "POST",
                "targetId": OTHER_USER_ID,
                "category": "SPAM",
                "reason": "Repeated unsolicited advertising"
            })
            .to_string(),
        ))
        .expect("request");

    let response = app.clone().oneshot(create).await.expect("response");
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = response_json(response).await;
    assert_eq!(body["reporterId"], USER_ID);

    let list = Request::builder()
        .method(Method::GET)
        .uri("/api/moderation/reports/me?limit=20")
        .header(header::AUTHORIZATION, harness.bearer_token(USER_ID))
        .body(Body::empty())
        .expect("request");
    let response = app.oneshot(list).await.expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["reports"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn moderation_admin_routes_require_admin_role() {
    let harness = RouteHarness::new().await;
    let app = moderation_routes::router(harness.state.clone());
    let request = Request::builder()
        .method(Method::GET)
        .uri("/admin/moderation/reports")
        .header(header::AUTHORIZATION, harness.bearer_token(USER_ID))
        .body(Body::empty())
        .expect("request");

    let response = app.oneshot(request).await.expect("response");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn administrator_can_page_recoverable_content_decision_status() {
    let harness = RouteHarness::new().await;
    let app = moderation_routes::router(harness.state.clone());
    let created_at = chrono::DateTime::parse_from_rfc3339("2026-09-28T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let first_id = Uuid::from_u128(1);
    let second_id = Uuid::from_u128(2);
    let third_id = Uuid::from_u128(3);
    harness.moderation_repository.set_content_decisions(vec![
        ContentModerationDecisionOutboxEntry {
            report_id: second_id,
            target_type: ReportTargetType::Comment,
            target_id: Uuid::new_v4(),
            decision_type: ModerationDecisionType::RestoreContent,
            state: ContentModerationDecisionOutboxState::TargetUnavailable,
            attempt_count: 3,
            replay_count: 0,
            last_error_code: Some("TARGET_UNAVAILABLE".into()),
            created_at,
            updated_at: created_at,
            next_attempt_at: created_at,
        },
        ContentModerationDecisionOutboxEntry {
            report_id: third_id,
            target_type: ReportTargetType::Post,
            target_id: Uuid::new_v4(),
            decision_type: ModerationDecisionType::HideContent,
            state: ContentModerationDecisionOutboxState::RetryExhausted,
            attempt_count: 8,
            replay_count: 1,
            last_error_code: Some("POST_SERVICE_CALL_FAILED".into()),
            created_at,
            updated_at: created_at,
            next_attempt_at: created_at,
        },
        ContentModerationDecisionOutboxEntry {
            report_id: first_id,
            target_type: ReportTargetType::Post,
            target_id: Uuid::new_v4(),
            decision_type: ModerationDecisionType::HideContent,
            state: ContentModerationDecisionOutboxState::Pending,
            attempt_count: 1,
            replay_count: 0,
            last_error_code: None,
            created_at,
            updated_at: created_at,
            next_attempt_at: created_at,
        },
    ]);

    let request = Request::builder()
        .method(Method::GET)
        .uri("/admin/moderation/content-decisions?limit=1")
        .header(
            header::AUTHORIZATION,
            harness.bearer_token_with_roles(USER_ID, vec!["ROLE_ADMIN".into()]),
        )
        .body(Body::empty())
        .expect("request");
    let response = app.clone().oneshot(request).await.expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let first_page = response_json(response).await;
    assert_eq!(first_page["decisions"][0]["reportId"], first_id.to_string());
    assert_eq!(first_page["decisions"][0]["state"], "PENDING");
    assert_eq!(first_page["decisions"][0]["attemptCount"], 1);
    let cursor = &first_page["nextCursor"];
    assert_eq!(cursor["reportId"], first_id.to_string());

    let request = Request::builder()
        .method(Method::GET)
        .uri(format!(
            "/admin/moderation/content-decisions?limit=1&afterCreatedAt={}&afterReportId={}",
            cursor["createdAt"].as_str().unwrap(),
            cursor["reportId"].as_str().unwrap(),
        ))
        .header(
            header::AUTHORIZATION,
            harness.bearer_token_with_roles(USER_ID, vec!["ROLE_ADMIN".into()]),
        )
        .body(Body::empty())
        .expect("request");
    let response = app.clone().oneshot(request).await.expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let second_page = response_json(response).await;
    assert_eq!(
        second_page["decisions"][0]["reportId"],
        second_id.to_string()
    );
    assert_eq!(second_page["decisions"][0]["state"], "TARGET_UNAVAILABLE");
    let second_cursor = &second_page["nextCursor"];
    assert_eq!(second_cursor["reportId"], second_id.to_string());

    let request = Request::builder()
        .method(Method::GET)
        .uri(format!(
            "/admin/moderation/content-decisions?limit=1&afterCreatedAt={}&afterReportId={}",
            second_cursor["createdAt"].as_str().unwrap(),
            second_cursor["reportId"].as_str().unwrap(),
        ))
        .header(
            header::AUTHORIZATION,
            harness.bearer_token_with_roles(USER_ID, vec!["ROLE_ADMIN".into()]),
        )
        .body(Body::empty())
        .expect("request");
    let response = app.clone().oneshot(request).await.expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let third_page = response_json(response).await;
    assert_eq!(third_page["decisions"][0]["reportId"], third_id.to_string());
    assert_eq!(third_page["decisions"][0]["state"], "RETRY_EXHAUSTED");
    assert_eq!(third_page["decisions"][0]["attemptCount"], 8);
    assert_eq!(third_page["decisions"][0]["replayCount"], 1);
    assert!(third_page["nextCursor"].is_null());

    let partial_cursor = Request::builder()
        .method(Method::GET)
        .uri(format!(
            "/admin/moderation/content-decisions?afterCreatedAt={}",
            cursor["createdAt"].as_str().unwrap(),
        ))
        .header(
            header::AUTHORIZATION,
            harness.bearer_token_with_roles(USER_ID, vec!["ROLE_ADMIN".into()]),
        )
        .body(Body::empty())
        .expect("request");
    let response = app.clone().oneshot(partial_cursor).await.expect("response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let non_admin = Request::builder()
        .method(Method::GET)
        .uri("/admin/moderation/content-decisions")
        .header(header::AUTHORIZATION, harness.bearer_token(USER_ID))
        .body(Body::empty())
        .expect("request");
    let response = app.oneshot(non_admin).await.expect("response");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn administrator_can_authorize_an_audited_content_decision_replay() {
    let harness = RouteHarness::new().await;
    let report_id = Uuid::new_v4();
    let resolution_id = Uuid::new_v4();
    let app = moderation_routes::router(harness.state.clone());
    let request = Request::builder()
        .method(Method::POST)
        .uri(format!(
            "/admin/moderation/content-decisions/{report_id}/replay"
        ))
        .header(
            header::AUTHORIZATION,
            harness.bearer_token_with_roles(USER_ID, vec!["ROLE_ADMIN".into()]),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "resolutionId": resolution_id,
                "reason": "The downstream incident is resolved; retry is safe",
                "evidenceReference": "OPS-INC-2026-1042"
            })
            .to_string(),
        ))
        .expect("request");

    let response = app.oneshot(request).await.expect("response");
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let body = response_json(response).await;
    assert_eq!(body["reportId"], report_id.to_string());
    assert_eq!(body["resolutionId"], resolution_id.to_string());
    assert_eq!(body["accepted"], true);
    let replay_call = harness.moderation_repository.replay_calls.lock().unwrap()[0].clone();
    assert_eq!(replay_call.report_id, report_id);
    assert_eq!(replay_call.resolution_id, resolution_id);
    assert_eq!(replay_call.actor_id, Uuid::parse_str(USER_ID).unwrap());
    assert_eq!(
        replay_call.reason,
        "The downstream incident is resolved; retry is safe"
    );
    assert_eq!(replay_call.evidence_reference, "OPS-INC-2026-1042");
}

#[tokio::test]
async fn content_decision_replay_requires_admin_and_valid_evidence() {
    let harness = RouteHarness::new().await;
    let report_id = Uuid::new_v4();
    let resolution_id = Uuid::new_v4();
    let body = json!({
        "resolutionId": resolution_id,
        "reason": "The downstream incident is resolved; retry is safe",
        "evidenceReference": "OPS-INC-2026-1042"
    });
    let app = moderation_routes::router(harness.state.clone());

    let non_admin = Request::builder()
        .method(Method::POST)
        .uri(format!(
            "/admin/moderation/content-decisions/{report_id}/replay"
        ))
        .header(header::AUTHORIZATION, harness.bearer_token(USER_ID))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .expect("request");
    let response = app.clone().oneshot(non_admin).await.expect("response");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let invalid_evidence = Request::builder()
        .method(Method::POST)
        .uri(format!(
            "/admin/moderation/content-decisions/{report_id}/replay"
        ))
        .header(
            header::AUTHORIZATION,
            harness.bearer_token_with_roles(USER_ID, vec!["ROLE_ADMIN".into()]),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "resolutionId": resolution_id,
                "reason": "too short",
                "evidenceReference": "OPS-INC-2026-1042"
            })
            .to_string(),
        ))
        .expect("request");
    let response = app.oneshot(invalid_evidence).await.expect("response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(harness
        .moderation_repository
        .replay_calls
        .lock()
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn administrator_can_audit_dismissal_of_an_unavailable_target() {
    let harness = RouteHarness::new().await;
    let report = harness.moderation_repository.insert_report(
        Uuid::parse_str(USER_ID).unwrap(),
        ReportTargetType::Post,
        Uuid::new_v4(),
    );
    harness
        .moderation_repository
        .mark_target_unavailable(report.id);
    let app = moderation_routes::router(harness.state.clone());

    let hidden = Request::builder()
        .method(Method::POST)
        .uri(format!("/admin/moderation/reports/{}/decisions", report.id))
        .header(
            header::AUTHORIZATION,
            harness.bearer_token_with_roles(USER_ID, vec!["ROLE_ADMIN".into()]),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({"decisionType":"HIDE_CONTENT","note":"Retry"}).to_string(),
        ))
        .expect("request");
    let response = app.clone().oneshot(hidden).await.expect("response");
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert!(harness
        .clients
        .content_moderation_calls
        .lock()
        .unwrap()
        .is_empty());

    let missing_note = Request::builder()
        .method(Method::POST)
        .uri(format!("/admin/moderation/reports/{}/decisions", report.id))
        .header(
            header::AUTHORIZATION,
            harness.bearer_token_with_roles(USER_ID, vec!["ROLE_ADMIN".into()]),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({"decisionType":"DISMISS"}).to_string()))
        .expect("request");
    let response = app.clone().oneshot(missing_note).await.expect("response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let request = Request::builder()
        .method(Method::POST)
        .uri(format!("/admin/moderation/reports/{}/decisions", report.id))
        .header(
            header::AUTHORIZATION,
            harness.bearer_token_with_roles(USER_ID, vec!["ROLE_ADMIN".into()]),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "decisionType": "DISMISS",
                "note": "The target no longer exists; closing this report"
            })
            .to_string(),
        ))
        .expect("request");
    let response = app.oneshot(request).await.expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["report"]["status"], "DISMISSED");
    assert_eq!(body["decision"]["actorId"], USER_ID);
    assert_eq!(
        body["decision"]["note"],
        "The target no longer exists; closing this report"
    );
}

#[tokio::test]
async fn administrator_can_authorize_an_audited_dead_letter_retry() {
    let harness = RouteHarness::new().await;
    let event_id = Uuid::new_v4();
    let resolution_id = Uuid::new_v4();
    let app = admin_routes::router(harness.state.clone());
    let request = Request::builder()
        .method(Method::POST)
        .uri(format!("/admin/post-events/dead-letters/{event_id}/replay"))
        .header(
            header::AUTHORIZATION,
            harness.bearer_token_with_roles(USER_ID, vec!["ROLE_ADMIN".into()]),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "resolutionId": resolution_id,
                "reason": "The broker incident is resolved and a retry is safe",
                "evidenceReference": "OPS-incident/INC-2026-1042"
            })
            .to_string(),
        ))
        .expect("request");

    let response = app.oneshot(request).await.expect("response");
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let body = response_json(response).await;
    assert_eq!(body["eventId"], event_id.to_string());
    assert_eq!(body["resolutionId"], resolution_id.to_string());
    assert_eq!(body["accepted"], true);
    let calls = harness
        .clients
        .dead_letter_replay_calls
        .lock()
        .unwrap()
        .clone();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].event_id, event_id.to_string());
    assert_eq!(calls[0].resolution_id, resolution_id.to_string());
    assert_eq!(calls[0].actor_id, USER_ID);
    assert_eq!(calls[0].evidence_reference, "OPS-incident/INC-2026-1042");
}

#[tokio::test]
async fn non_admin_cannot_authorize_a_dead_letter_retry() {
    let harness = RouteHarness::new().await;
    let event_id = Uuid::new_v4();
    let app = admin_routes::router(harness.state.clone());
    let request = Request::builder()
        .method(Method::POST)
        .uri(format!("/admin/post-events/dead-letters/{event_id}/replay"))
        .header(header::AUTHORIZATION, harness.bearer_token(USER_ID))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "resolutionId": Uuid::new_v4(),
                "reason": "The broker incident is resolved and a retry is safe",
                "evidenceReference": "OPS-incident/INC-2026-1042"
            })
            .to_string(),
        ))
        .expect("request");

    let response = app.oneshot(request).await.expect("response");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(harness
        .clients
        .dead_letter_replay_calls
        .lock()
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn dead_letter_retry_rejects_invalid_reason_without_calling_post_service() {
    let harness = RouteHarness::new().await;
    let event_id = Uuid::new_v4();
    let app = admin_routes::router(harness.state.clone());
    let request = Request::builder()
        .method(Method::POST)
        .uri(format!("/admin/post-events/dead-letters/{event_id}/replay"))
        .header(
            header::AUTHORIZATION,
            harness.bearer_token_with_roles(USER_ID, vec!["ROLE_ADMIN".into()]),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "resolutionId": Uuid::new_v4(),
                "reason": "too short",
                "evidenceReference": "OPS-1234"
            })
            .to_string(),
        ))
        .expect("request");

    let response = app.oneshot(request).await.expect("response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(harness
        .clients
        .dead_letter_replay_calls
        .lock()
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn administrator_can_hide_reported_content() {
    let harness = RouteHarness::new().await;
    let report = harness.moderation_repository.insert_report(
        Uuid::parse_str(USER_ID).unwrap(),
        ReportTargetType::Post,
        Uuid::parse_str(OTHER_USER_ID).unwrap(),
    );
    let app = moderation_routes::router(harness.state.clone());
    let request = Request::builder()
        .method(Method::POST)
        .uri(format!("/admin/moderation/reports/{}/decisions", report.id))
        .header(
            header::AUTHORIZATION,
            harness.bearer_token_with_roles(USER_ID, vec!["ROLE_ADMIN".into()]),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "decisionType": "HIDE_CONTENT",
                "note": "Confirmed spam"
            })
            .to_string(),
        ))
        .expect("request");

    let response = app.oneshot(request).await.expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        harness
            .clients
            .content_moderation_calls
            .lock()
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn missing_content_target_returns_conflict_and_keeps_report_open() {
    let harness = RouteHarness::new().await;
    let report = harness.moderation_repository.insert_report(
        Uuid::parse_str(USER_ID).unwrap(),
        ReportTargetType::Post,
        Uuid::parse_str(OTHER_USER_ID).unwrap(),
    );
    *harness.clients.content_moderation_result.lock().unwrap() = Some(false);
    let app = moderation_routes::router(harness.state.clone());
    let request = Request::builder()
        .method(Method::POST)
        .uri(format!("/admin/moderation/reports/{}/decisions", report.id))
        .header(
            header::AUTHORIZATION,
            harness.bearer_token_with_roles(USER_ID, vec!["ROLE_ADMIN".into()]),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "decisionType": "HIDE_CONTENT",
                "note": "Confirmed spam"
            })
            .to_string(),
        ))
        .expect("request");

    let response = app.oneshot(request).await.expect("response");
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body = response_json(response).await;
    assert_eq!(body["code"], "GATEWAY_409");
    assert_eq!(body["message"], "Moderation target is no longer available");
    assert_eq!(
        harness
            .moderation_repository
            .list_open(10)
            .await
            .expect("open reports")
            .iter()
            .filter(|open| open.id == report.id)
            .count(),
        1
    );
    assert_eq!(
        harness
            .clients
            .content_moderation_calls
            .lock()
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn restricted_user_cannot_create_post() {
    let harness = RouteHarness::new().await;
    harness
        .moderation_repository
        .restrict(Uuid::parse_str(USER_ID).unwrap());
    let app = post_routes::router(harness.state.clone());
    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/posts")
        .header(header::AUTHORIZATION, harness.bearer_token(USER_ID))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({"content": "blocked mutation", "visibility": "PUBLIC"}).to_string(),
        ))
        .expect("request");

    let response = app.oneshot(request).await.expect("response");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(harness.clients.create_post_calls.lock().unwrap().is_empty());
}
