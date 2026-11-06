use api_gateway::adapters::out::friend_client::GrpcFriendClient;
use api_gateway::adapters::r#in::http::{map_service_error, GatewayError};
use api_gateway::application::ports::FriendClientPort;
use axum::body::{to_bytes, Body};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::Value;

#[tokio::test]
async fn neutral_post_failures_preserve_http_categories_and_drop_private_context() {
    use api_gateway::application::post_client_error::{PostClientError as F, PostClientMessage};
    let cases = [
        (
            F::InvalidArgument(PostClientMessage::new("Public validation")),
            400,
            "Public validation",
        ),
        (F::Unauthenticated, 401, "Invalid credentials"),
        (F::PermissionDenied, 403, "Forbidden"),
        (F::NotFound, 404, "Not found"),
        (F::AlreadyExists, 409, "Resource already exists"),
        (
            F::OperationOutcomeUnknown,
            409,
            "Operation outcome is unknown; reconcile using the original operation ID and key",
        ),
        (
            F::OperationReplayExpired,
            409,
            "Operation replay window expired; reconcile before issuing another command",
        ),
        (
            F::OperationConflict,
            409,
            "Operation identity or request conflicts with its durable receipt",
        ),
        (F::Aborted, 409, "Operation conflict; retry"),
        (F::RateLimited, 429, "Too many requests"),
        (
            F::FailedPrecondition,
            400,
            "Request is not valid in the current state",
        ),
        (F::Busy, 503, "Service busy; retry later"),
        (F::Unavailable, 503, "Service unavailable"),
        (F::DeadlineExceeded, 504, "Downstream request timed out"),
        (F::Unexpected, 500, "Internal error"),
    ];
    for (failure, status, message) in cases {
        let busy = matches!(failure, F::Busy);
        // Both a saturated owner and a refused admission budget tell the client
        // when a retry can succeed; no other category carries a retry hint.
        let retry_hint = (busy || status == 429).then_some("1");
        let response =
            map_service_error(anyhow::Error::new(failure).context("PRIVATE_POST_CONTEXT"))
                .into_response();
        assert_eq!(response.status().as_u16(), status);
        assert_eq!(
            response
                .headers()
                .get(axum::http::header::RETRY_AFTER)
                .map(|v| v.to_str().unwrap()),
            retry_hint
        );
        let body = response_json(response).await;
        assert_eq!(body["message"], message);
        assert!(!body.to_string().contains("PRIVATE_POST_CONTEXT"));
    }
    assert_eq!(
        api_gateway::application::post_client_error::MAX_POST_CLIENT_MESSAGE_CHARS,
        ERROR_RESPONSE_MAX_MESSAGE_CHARS
    );
}

#[test]
fn post_adapter_classifies_status_and_owner_metadata_without_diagnostic_guessing() {
    use api_gateway::adapters::out::post_client::classify_post_client_error;
    use api_gateway::application::post_client_error::{PostClientError as F, PostClientMessage};
    use std::error::Error;
    let rows = [
        (
            tonic::Code::InvalidArgument,
            F::InvalidArgument(PostClientMessage::new("Public validation")),
        ),
        (tonic::Code::Unauthenticated, F::Unauthenticated),
        (tonic::Code::PermissionDenied, F::PermissionDenied),
        (tonic::Code::NotFound, F::NotFound),
        (tonic::Code::AlreadyExists, F::AlreadyExists),
        (tonic::Code::Aborted, F::Aborted),
        (tonic::Code::ResourceExhausted, F::RateLimited),
        (tonic::Code::FailedPrecondition, F::FailedPrecondition),
        (tonic::Code::Unavailable, F::Unavailable),
        (tonic::Code::DeadlineExceeded, F::DeadlineExceeded),
        (tonic::Code::Ok, F::Unexpected),
        (tonic::Code::Cancelled, F::Unexpected),
        (tonic::Code::Unknown, F::Unexpected),
        (tonic::Code::OutOfRange, F::Unexpected),
        (tonic::Code::Unimplemented, F::Unexpected),
        (tonic::Code::Internal, F::Unexpected),
        (tonic::Code::DataLoss, F::Unexpected),
    ];
    for (code, expected) in rows {
        let failure = classify_post_client_error(
            anyhow::Error::new(tonic::Status::new(
                code,
                if code == tonic::Code::InvalidArgument {
                    "Public validation"
                } else {
                    "PRIVATE_POST_CAUSE"
                },
            ))
            .context("PRIVATE_POST_CONTEXT"),
        );
        assert_eq!(failure, expected);
        assert!(failure.source().is_none());
        assert!(!format!("{failure:?}").contains("PRIVATE_POST"));
        assert_eq!(
            classify_post_client_error(
                anyhow::Error::new(failure.clone()).context("other context")
            ),
            failure
        );
    }
    for (code, expected) in [
        ("unknown", F::OperationOutcomeUnknown),
        ("expired", F::OperationReplayExpired),
        ("conflict", F::OperationConflict),
        ("UNKNOWN", F::AlreadyExists),
    ] {
        let mut status = tonic::Status::already_exists("PRIVATE_OWNER_CAUSE");
        status.metadata_mut().insert(
            platform::mutation_context::ERROR_CODE,
            code.parse().unwrap(),
        );
        assert_eq!(classify_post_client_error(status.into()), expected);
    }
    assert_eq!(
        classify_post_client_error(tonic::Status::unavailable("AUTH_BUSY").into()),
        F::Busy
    );
    for message in [
        "AUTH_BUSY",
        "unknown",
        "expired",
        "conflict",
        "Not found",
        "Public validation",
    ] {
        assert_eq!(
            classify_post_client_error(anyhow::Error::msg(message)),
            F::Unexpected
        );
    }
    for message in ["not AUTH_BUSY", "auth_busy", "AUTH_DEPENDENCY_UNAVAILABLE"] {
        assert_eq!(
            classify_post_client_error(tonic::Status::unavailable(message).into()),
            F::Unavailable
        );
    }
}

#[tokio::test]
async fn gateway_error_response_preserves_client_messages_and_metadata() {
    let response = GatewayError::rate_limited("Rate Limit", 60).into_response();

    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(response.headers()["retry-after"], "60");
    let body = response_json(response).await;
    assert_eq!(body["code"], "GATEWAY_429");
    assert_eq!(body["message"], "Rate Limit");
    assert!(body["traceId"].is_string());
    assert!(body["timestamp"].is_string());
}

#[tokio::test]
async fn oversized_payload_has_its_own_status_and_no_retry_hint() {
    let response =
        GatewayError::payload_too_large("Avatar image must be <= 1048576 bytes").into_response();

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert!(!response.headers().contains_key("retry-after"));
    let body = response_json(response).await;
    assert_eq!(body["code"], "GATEWAY_413");
}

#[tokio::test]
async fn internal_gateway_error_response_redacts_source_error() {
    let response =
        GatewayError::Internal(anyhow::anyhow!("database password leaked")).into_response();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = response_json(response).await;
    assert_eq!(body["code"], "GATEWAY_500");
    assert_eq!(body["message"], "Internal error");
}

#[test]
fn untyped_service_errors_never_become_client_errors() {
    // A bare error with no transport status carries no classification. Guessing
    // from its text previously turned outages into HTTP 400 responses and could
    // surface SQL, pool, or provider detail to callers.
    for message in [
        "Profile not found",
        "profile not found",
        "not allowed to delete",
        "email already exists",
        "malformed downstream payload",
        "connection refused to postgres",
    ] {
        assert!(
            matches!(
                map_service_error(anyhow::anyhow!(message)),
                GatewayError::Internal(_)
            ),
            "`{message}` must map to a server fault"
        );
    }
}

#[tokio::test]
async fn invalid_search_cursor_maps_to_a_safe_bad_request() {
    let response = map_service_error(anyhow::Error::new(
        api_gateway::application::search_cursor::InvalidSearchCursor,
    ))
    .into_response();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response_json(response).await;
    assert_eq!(body["code"], "GATEWAY_400");
    assert_eq!(body["message"], "Invalid search cursor");
}

#[test]
fn missing_moderation_target_maps_to_state_conflict() {
    let response = map_service_error(anyhow::Error::new(
        api_gateway::domain::moderation::ModerationTargetUnavailable,
    ))
    .into_response();

    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn staged_moderation_conflicts_map_to_safe_gateway_conflicts() {
    let cases = [
        (
            anyhow::Error::new(api_gateway::domain::moderation::ModerationDecisionInProgress),
            "Moderation decision is still being processed",
        ),
        (
            anyhow::Error::new(api_gateway::domain::moderation::ModerationDecisionConflict),
            "Moderation report already has a different decision",
        ),
        (
            anyhow::Error::new(api_gateway::domain::moderation::ContentDecisionReplayConflict),
            "Moderation content decision is not available for the requested replay",
        ),
    ];

    for (error, expected_message) in cases {
        let response = map_service_error(error).into_response();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(response_json(response).await["message"], expected_message);
    }
}

#[test]
fn concealed_profile_policy_failures_map_to_not_found() {
    let response = map_service_error(anyhow::Error::new(
        api_gateway::domain::profile::ProfileNotVisible,
    ))
    .into_response();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[test]
fn public_grpc_statuses_map_to_stable_http_statuses() {
    let cases = [
        (tonic::Code::InvalidArgument, "Invalid input", 400),
        (
            tonic::Code::Unauthenticated,
            "AUTH_INVALID_CREDENTIALS",
            401,
        ),
        (tonic::Code::PermissionDenied, "AUTH_FORBIDDEN", 403),
        (tonic::Code::NotFound, "AUTH_NOT_FOUND", 404),
        (tonic::Code::AlreadyExists, "Email already registered", 409),
        (tonic::Code::Aborted, "AUTH_REFRESH_CONFLICT", 409),
        (tonic::Code::ResourceExhausted, "AUTH_RATE_LIMITED", 429),
        (tonic::Code::FailedPrecondition, "state", 400),
        (tonic::Code::Unavailable, "AUTH_BUSY", 503),
        (tonic::Code::DeadlineExceeded, "slow", 504),
    ];
    for (code, message, expected) in cases {
        let error = anyhow::Error::new(tonic::Status::new(code, message));
        let status = map_service_error(error).into_response().status();
        assert_eq!(status.as_u16(), expected, "{code:?} must map to {expected}");
    }

    // Credential failures collapse to one message so the response cannot be used
    // to tell an unknown account from a wrong password.
    let unauthorized = map_service_error(anyhow::Error::new(tonic::Status::unauthenticated(
        "AUTH_INVALID_CREDENTIALS",
    )));
    assert_eq!(
        unauthorized.into_response().status(),
        StatusCode::UNAUTHORIZED
    );

    // Unknown transport codes are defects, not caller mistakes.
    let unknown = map_service_error(anyhow::Error::new(tonic::Status::internal("boom")));
    assert!(matches!(unknown, GatewayError::Internal(_)));
}

#[test]
fn forwarded_invalid_argument_messages_are_bounded_and_single_line() {
    let long = "x".repeat(400);
    let mapped = map_service_error(anyhow::Error::new(tonic::Status::invalid_argument(
        format!("bad\ninput {long}"),
    )));
    let GatewayError::BadRequest(message) = mapped else {
        panic!("invalid argument must map to a client error");
    };
    assert!(
        !message.contains('\n'),
        "control characters must be stripped"
    );
    assert!(
        message.chars().count() <= 256,
        "message must be length bounded"
    );

    let empty = map_service_error(anyhow::Error::new(tonic::Status::invalid_argument("")));
    let GatewayError::BadRequest(message) = empty else {
        panic!("empty invalid argument still maps to a client error");
    };
    assert!(!message.is_empty());
}

use api_gateway::adapters::r#in::http::ERROR_RESPONSE_MAX_MESSAGE_CHARS;

#[test]
fn message_bound_is_the_documented_constant() {
    assert_eq!(ERROR_RESPONSE_MAX_MESSAGE_CHARS, 256);
}

#[tokio::test]
async fn tonic_service_errors_map_to_safe_standard_http_envelopes_before_legacy_fallback() {
    let cases = [
        (
            tonic::Code::InvalidArgument,
            StatusCode::BAD_REQUEST,
            "GATEWAY_400",
        ),
        (
            tonic::Code::PermissionDenied,
            StatusCode::FORBIDDEN,
            "GATEWAY_403",
        ),
        (tonic::Code::NotFound, StatusCode::NOT_FOUND, "GATEWAY_404"),
        (
            tonic::Code::Unavailable,
            StatusCode::SERVICE_UNAVAILABLE,
            "GATEWAY_503",
        ),
        (
            tonic::Code::DeadlineExceeded,
            StatusCode::GATEWAY_TIMEOUT,
            "GATEWAY_504",
        ),
        (
            tonic::Code::Internal,
            StatusCode::INTERNAL_SERVER_ERROR,
            "GATEWAY_500",
        ),
    ];

    for (code, expected_status, expected_code) in cases {
        // Given: a tonic status wrapped by an outbound adapter context whose text matches no legacy rule.
        let error = anyhow::Error::new(tonic::Status::new(code, "database password leaked"))
            .context("friend outbound adapter failed");

        // When: the public HTTP boundary maps and renders the error.
        let response = map_service_error(error).into_response();

        // Then: tonic decides the status and the standard JSON metadata is present.
        assert_eq!(response.status(), expected_status);
        assert_eq!(
            response.headers()[axum::http::header::CONTENT_TYPE],
            "application/json"
        );
        let body = response_json(response).await;
        assert_eq!(body["code"], expected_code);
        assert!(body["message"].is_string());
        assert!(body["traceId"].is_string());
        assert!(body["timestamp"].is_string());
        if expected_status.is_server_error() {
            assert_ne!(body["message"], "database password leaked");
            assert!(!body.to_string().contains("database password leaked"));
        }
    }
}

#[tokio::test]
async fn auth_saturation_is_a_retryable_503_and_an_outage_is_not() {
    // Given: the auth service refused work because a hashing class is full.
    let busy = map_service_error(anyhow::Error::new(tonic::Status::unavailable("AUTH_BUSY")))
        .into_response();

    // Then: the client gets 503 with a bounded retry hint and a message that
    // cannot be mistaken for a credential failure.
    assert_eq!(busy.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(busy.headers()[axum::http::header::RETRY_AFTER], "1");
    let body = response_json(busy).await;
    assert_eq!(body["code"], "GATEWAY_503");
    assert_eq!(body["message"], "Service busy; retry later");

    // And: a dependency outage stays a plain 503 without a retry hint, and the
    // busy mapping requires the exact identifier rather than a substring.
    for message in ["AUTH_DEPENDENCY_UNAVAILABLE", "not AUTH_BUSY", "auth_busy"] {
        let outage = map_service_error(anyhow::Error::new(tonic::Status::unavailable(message)))
            .into_response();
        assert_eq!(outage.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert!(outage
            .headers()
            .get(axum::http::header::RETRY_AFTER)
            .is_none());
        let body = response_json(outage).await;
        assert_eq!(body["message"], "Service unavailable");
    }
}

#[tokio::test]
async fn stopped_friend_listener_maps_through_the_real_outbound_transport_to_http_503() {
    // Given: an ephemeral downstream listener whose port has been released before client use.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("ephemeral listener");
    let address = listener.local_addr().expect("listener address");
    drop(listener);
    let client = GrpcFriendClient::connect(
        format!("http://{address}"),
        platform::grpc_auth::Workload::Gateway,
        "gateway-friend-transport-token",
        None,
    )
    .await
    .expect("lazy friend client");

    // When: the production outbound adapter calls the stopped listener.
    // Actors are user UUIDs; the outbound client signs a delegation for them.
    let error = client
        .follow(
            "11111111-1111-4111-8111-111111111111".into(),
            "22222222-2222-4222-8222-222222222222".into(),
        )
        .await
        .expect_err("stopped friend listener must fail");
    let response = map_service_error(error).into_response();

    // Then: the public HTTP contract reports a safe standard 503 envelope.
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        response.headers()[axum::http::header::CONTENT_TYPE],
        "application/json"
    );
    let body = response_json(response).await;
    assert_eq!(body["code"], "GATEWAY_503");
    assert_eq!(body["message"], "Service unavailable");
    assert!(body["traceId"].is_string());
    assert!(body["timestamp"].is_string());
}

async fn response_json(response: Response<Body>) -> Value {
    let bytes = to_bytes(response.into_body(), 1 << 20)
        .await
        .expect("response body");
    serde_json::from_slice(&bytes).expect("json body")
}

#[tokio::test]
async fn service_error_envelopes_preserve_all_http_meanings_and_only_controlled_sources() {
    use api_gateway::application::client_error::{
        GatewayClientError as F, GatewayClientMessage as PostClientMessage,
    };
    use api_gateway::application::infrastructure_error::GatewayInfrastructureError as I;
    use api_gateway::application::moderation_input::ModerationInputError as Input;
    use api_gateway::application::moderation_repository_error::ModerationRepositoryError as M;
    use api_gateway::application::search_cursor::{
        InvalidSearchCursor, SearchCursorEncodingError as C,
    };
    use api_gateway::application::service_error::GatewayServiceError as S;
    use std::error::Error;
    let cases = [
        (
            F::InvalidArgument(PostClientMessage::new("Public validation")),
            400,
            "Public validation",
        ),
        (F::Unauthenticated, 401, "Invalid credentials"),
        (F::PermissionDenied, 403, "Forbidden"),
        (F::NotFound, 404, "Not found"),
        (F::AlreadyExists, 409, "Resource already exists"),
        (
            F::OperationOutcomeUnknown,
            409,
            "Operation outcome is unknown; reconcile using the original operation ID and key",
        ),
        (
            F::OperationReplayExpired,
            409,
            "Operation replay window expired; reconcile before issuing another command",
        ),
        (
            F::OperationConflict,
            409,
            "Operation identity or request conflicts with its durable receipt",
        ),
        (F::Aborted, 409, "Operation conflict; retry"),
        (F::RateLimited, 429, "Too many requests"),
        (
            F::FailedPrecondition,
            400,
            "Request is not valid in the current state",
        ),
        (F::Busy, 503, "Service busy; retry later"),
        (F::Unavailable, 503, "Service unavailable"),
        (F::DeadlineExceeded, 504, "Downstream request timed out"),
        (F::Unexpected, 500, "Internal error"),
    ];

    let mut cases: Vec<(S, u16, &str)> = cases
        .into_iter()
        .map(|(error, status, message)| (error.into(), status, message))
        .collect();
    for error in [I::Storage, I::Encoding, I::Unexpected] {
        cases.push((error.into(), 500, "Internal error"));
    }
    for input in [
        Input::ReporterId,
        Input::TargetId,
        Input::ReportId,
        Input::ReviewerId,
        Input::ActorId,
        Input::UserId,
        Input::TargetType,
        Input::SelfReport,
        Input::Category,
        Input::Reason,
        Input::DecisionType,
        Input::Note,
        Input::ContentTarget,
        Input::UserTarget,
        Input::ReplayResolutionId,
        Input::ReplayReason,
        Input::ReplayEvidence,
    ] {
        cases.push((input.into(), 400, input.message()));
    }
    for (error, status, message) in [
        (M::ReportMissing, 404, "Report not found"),
        (
            M::ReceiptMissing,
            409,
            "Report operation receipt is unavailable or expired; reconcile before retrying",
        ),
        (
            M::ReceiptExpired,
            409,
            "Report operation receipt is unavailable or expired; reconcile before retrying",
        ),
        (
            M::ReceiptConflict,
            409,
            "Report operation receipt is unavailable or expired; reconcile before retrying",
        ),
        (
            M::TargetUnavailable,
            409,
            "Moderation target is no longer available",
        ),
        (
            M::DecisionInProgress,
            409,
            "Moderation decision is still being processed",
        ),
        (
            M::DecisionConflict,
            409,
            "Moderation report already has a different decision",
        ),
        (
            M::ResolutionNoteRequired,
            400,
            "A non-empty note is required to dismiss a report with an unavailable content target",
        ),
        (
            M::ReplayConflict,
            409,
            "Moderation content decision is not available for the requested replay",
        ),
        (M::Storage, 500, "Internal error"),
        (M::Unexpected, 500, "Internal error"),
    ] {
        cases.push((error.into(), status, message));
    }
    cases.push((InvalidSearchCursor.into(), 400, "Invalid search cursor"));
    for error in [C::Position, C::SigningKey, C::Serialization, C::PublicBound] {
        cases.push((error.into(), 500, "Internal error"));
    }
    cases.push((
        api_gateway::domain::profile::ProfileNotVisible.into(),
        404,
        "Not found",
    ));
    cases.push((
        api_gateway::domain::moderation::InvalidContentModerationDecisionCursor.into(),
        400,
        "Both afterCreatedAt and afterReportId are required together",
    ));
    cases.push((
        api_gateway::domain::moderation::ModerationTargetUnavailable.into(),
        409,
        "Moderation target is no longer available",
    ));
    for error in [
        S::NotificationPageBound,
        S::NotificationAuthorizationIncomplete,
        S::ProfileRelationshipIncomplete,
        S::InvalidModerationAction,
        S::RouteContractMissing,
        S::RouteContractVersionMissing,
        S::PolicyMissing,
        S::RolloutMissing,
        S::ClientProfileMissing,
        S::AggregationRecipeMissing,
    ] {
        cases.push((error, 500, "Internal error"));
    }
    assert_eq!(cases.len(), 64);
    for (error, status, message) in cases {
        let busy = matches!(&error, S::Client(F::Busy));
        let debug = format!("{error:?}");
        assert!(!debug.contains("Public validation"));
        let mut cause: Option<&(dyn Error + 'static)> = Some(&error);
        let mut depth = 0;
        while let Some(value) = cause {
            assert!(value.downcast_ref::<tonic::Status>().is_none());
            assert!(value.downcast_ref::<sqlx::Error>().is_none());
            assert!(value.downcast_ref::<serde_json::Error>().is_none());
            depth += 1;
            cause = value.source();
        }
        assert!(
            depth <= 2,
            "service error sources must contain only one controlled child"
        );
        let response = map_service_error(error).into_response();
        assert_eq!(response.status().as_u16(), status);
        assert_eq!(
            response
                .headers()
                .get(axum::http::header::RETRY_AFTER)
                .map(|value| value.to_str().unwrap()),
            (busy || status == 429).then_some("1")
        );
        let correlation = response.headers()["x-correlation-id"]
            .to_str()
            .unwrap()
            .to_owned();
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 1 << 20).await.unwrap())
                .unwrap();
        assert_eq!(body["code"], format!("GATEWAY_{status}"));
        assert_eq!(body["message"], message);
        assert_eq!(body["traceId"], correlation);
        assert!(body["details"].is_null());
    }
}
