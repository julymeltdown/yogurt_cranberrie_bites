use super::{map_service_error, CurrentUser, GatewayError};
use crate::application::dto::moderation_dto::{
    ContentDecisionReplayAcceptedResponse, CreateModerationDecisionRequest, CreateReportRequest,
    ReplayContentDecisionRequest,
};
use crate::config::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
struct ReportListQuery {
    #[serde(default = "default_limit")]
    limit: i32,
}

#[derive(Debug, Deserialize)]
struct ContentDecisionListQuery {
    #[serde(default = "default_limit")]
    limit: i32,
    #[serde(rename = "afterCreatedAt")]
    after_created_at: Option<DateTime<Utc>>,
    #[serde(rename = "afterReportId")]
    after_report_id: Option<Uuid>,
}

fn default_limit() -> i32 {
    20
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/moderation/reports", post(create_report))
        .route("/api/moderation/reports/me", get(list_my_reports))
        .route("/admin/moderation/reports", get(list_open_reports))
        .route(
            "/admin/moderation/content-decisions",
            get(list_content_decisions),
        )
        .route(
            "/admin/moderation/content-decisions/:report_id/replay",
            post(replay_content_decision),
        )
        .route(
            "/admin/moderation/reports/:report_id/decisions",
            post(create_decision),
        )
        .with_state(state)
}

async fn create_report(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Json(request): Json<CreateReportRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), GatewayError> {
    let operation = platform::mutation_context::current();
    let lookup_only = operation
        .as_ref()
        .is_some_and(|context| context.lookup_only);
    let mutation = operation.map(|context| {
        crate::application::dto::moderation_dto::ModerationMutationContextDto {
            operation_id: context.operation_id,
            actor_id: context.actor_id,
            idempotency_key: context.idempotency_key,
            lookup_only: context.lookup_only,
        }
    });
    // This owner mutation is persisted by the Gateway rather than an outbound
    // gRPC client, so mark the application boundary explicitly for the journal.
    platform::mutation_context::mark_local_dispatch_may_have_occurred();
    let report = state
        .moderation_service
        .create_report_with_mutation(current_user.user_id, request, mutation)
        .await
        .map_err(|error| {
            let error: anyhow::Error = error.into();
            use crate::application::moderation_repository_error::ModerationRepositoryError;
            let repository_error = error
                .chain()
                .find_map(|cause| cause.downcast_ref::<ModerationRepositoryError>())
                .copied();
            let receipt_error = repository_error.is_some_and(ModerationRepositoryError::is_receipt);
            let persistence_error = repository_error == Some(ModerationRepositoryError::Storage);
            if receipt_error {
                GatewayError::conflict(
                    "Report operation receipt is unavailable or expired; reconcile before retrying",
                )
            } else if lookup_only && persistence_error {
                GatewayError::ServiceUnavailable(error)
            } else if lookup_only {
                GatewayError::conflict(
                    "Report operation receipt is unavailable or expired; reconcile before retrying",
                )
            } else {
                map_service_error(error)
            }
        })?;
    Ok((StatusCode::CREATED, Json(serde_json::to_value(report)?)))
}

async fn list_my_reports(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Query(query): Query<ReportListQuery>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    Ok(Json(serde_json::to_value(
        state
            .moderation_service
            .list_my_reports(current_user.user_id, query.limit)
            .await
            .map_err(map_service_error)?,
    )?))
}

async fn list_open_reports(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Query(query): Query<ReportListQuery>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    current_user.ensure_admin()?;
    Ok(Json(serde_json::to_value(
        state
            .moderation_service
            .list_open_reports(query.limit)
            .await
            .map_err(map_service_error)?,
    )?))
}

async fn list_content_decisions(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Query(query): Query<ContentDecisionListQuery>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    current_user.ensure_admin()?;
    Ok(Json(serde_json::to_value(
        state
            .moderation_service
            .list_content_decisions(query.limit, query.after_created_at, query.after_report_id)
            .await
            .map_err(map_service_error)?,
    )?))
}

async fn replay_content_decision(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(report_id): Path<Uuid>,
    Json(request): Json<ReplayContentDecisionRequest>,
) -> Result<(StatusCode, Json<ContentDecisionReplayAcceptedResponse>), GatewayError> {
    current_user.ensure_admin()?;
    if request.resolution_id.is_nil() {
        return Err(GatewayError::bad_request(
            "Replay resolution ID must be a non-nil UUID",
        ));
    }
    request.validate().map_err(GatewayError::bad_request)?;
    let resolution_id = request.resolution_id;
    state
        .moderation_service
        .replay_dead_lettered_content_decision(report_id.to_string(), current_user.user_id, request)
        .await
        .map_err(map_service_error)?;
    Ok((
        StatusCode::ACCEPTED,
        Json(ContentDecisionReplayAcceptedResponse {
            report_id: report_id.to_string(),
            resolution_id: resolution_id.to_string(),
            accepted: true,
        }),
    ))
}

async fn create_decision(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(report_id): Path<String>,
    Json(request): Json<CreateModerationDecisionRequest>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    current_user.ensure_admin()?;
    Ok(Json(serde_json::to_value(
        state
            .moderation_service
            .decide(report_id, current_user.user_id, request)
            .await
            .map_err(map_service_error)?,
    )?))
}
