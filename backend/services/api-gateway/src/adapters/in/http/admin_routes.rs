use super::{map_service_error, CurrentUser, GatewayError};
use crate::application::dto::facade_dto::{
    ClientProfileResponseDto, PolicySetResponseDto, RolloutPolicyResponseDto,
    RouteContractResponseDto, RouteContractVersionResponseDto, TelemetrySummaryResponseDto,
};
use crate::application::dto::operation_view_dto::{
    ManualOperationResolutionResponseDto, OperationStatusResponseDto,
};

use crate::application::dto::auth_dto::{
    DeadLetteredUserDeletionListResponse, RetryUserDeletionAcceptedResponse,
    RetryUserDeletionRequest, RetryUserDeletionRequestDto, UserDeletionSagaResponse,
};
use crate::application::dto::governance_dto::{
    ClientProfileRequest, PolicySetRequest, RolloutCreateRequest, RouteContractCreateRequest,
    RouteContractVersionCreateRequest, RouteContractVersionUpdateRequest,
};
use crate::application::dto::operation_dto::ManualOperationResolutionRequest;
use crate::application::dto::post_dto::{
    DeadLetterReplayAcceptedResponse, ResolveDeadLetterReplayRequest,
    ResolveDeadLetterReplayRequestDto,
};
use crate::config::AppState;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
struct DeadLetteredUserDeletionListQuery {
    #[serde(default = "default_deletion_limit")]
    limit: i32,
    #[serde(rename = "afterUpdatedAt")]
    after_updated_at: Option<DateTime<Utc>>,
    #[serde(rename = "afterSagaId")]
    after_saga_id: Option<Uuid>,
}

fn default_deletion_limit() -> i32 {
    20
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route(
            "/admin/contracts",
            get(list_contracts).post(create_contract),
        )
        .route("/admin/contracts/:contract_id", get(get_contract))
        .route(
            "/admin/contracts/:contract_id/versions",
            get(list_versions).post(create_version),
        )
        .route(
            "/admin/contracts/:contract_id/versions/:version",
            get(get_version).patch(update_version),
        )
        .route("/admin/policies", get(list_policies).post(create_policy))
        .route(
            "/admin/policies/:policy_id",
            get(get_policy).put(update_policy),
        )
        .route("/admin/clients", get(list_clients).post(create_client))
        .route("/admin/clients/:client_id", get(get_client))
        .route("/admin/rollouts", post(create_rollout))
        .route("/admin/rollouts/:rollout_id", get(get_rollout))
        .route("/admin/telemetry/summary", get(telemetry_summary))
        .route(
            "/admin/post-events/dead-letters/:event_id/replay",
            post(resolve_post_event_dead_letter),
        )
        .route(
            "/admin/account-deletions",
            get(list_dead_lettered_account_deletions),
        )
        .route(
            "/admin/account-deletions/:saga_id",
            get(get_account_deletion_saga),
        )
        .route(
            "/admin/account-deletions/:saga_id/retry",
            post(retry_user_deletion),
        )
        .route("/admin/operations/:operation_id", get(operation_status))
        .route(
            "/admin/operations/:operation_id/reconciliation",
            post(record_operation_reconciliation),
        )
        .with_state(state)
}

async fn list_dead_lettered_account_deletions(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Query(query): Query<DeadLetteredUserDeletionListQuery>,
) -> Result<Json<DeadLetteredUserDeletionListResponse>, GatewayError> {
    current_user.ensure_admin()?;
    if !(1..=100).contains(&query.limit) {
        return Err(GatewayError::bad_request("Limit must be between 1 and 100"));
    }
    if query.after_updated_at.is_some() != query.after_saga_id.is_some() {
        return Err(GatewayError::bad_request(
            "Cursor timestamp and saga ID must be provided together",
        ));
    }
    Ok(Json(
        state
            .auth_service
            .list_dead_lettered_user_deletions(
                query.limit,
                query.after_updated_at,
                query.after_saga_id,
            )
            .await
            .map_err(map_service_error)?,
    ))
}

async fn get_account_deletion_saga(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(saga_id): Path<Uuid>,
) -> Result<Json<UserDeletionSagaResponse>, GatewayError> {
    current_user.ensure_admin()?;
    let saga = state
        .auth_service
        .get_user_deletion_by_saga_id(saga_id.to_string())
        .await
        .map_err(map_service_error)?
        .ok_or_else(|| GatewayError::NotFound("Account deletion saga not found".into()))?;
    Ok(Json(saga))
}

async fn list_contracts(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
) -> Result<Json<Vec<RouteContractResponseDto>>, GatewayError> {
    current_user.ensure_admin()?;
    Ok(Json(
        state
            .route_contract_service
            .list_contracts()
            .await
            .map_err(map_service_error)?,
    ))
}

async fn create_contract(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Json(request): Json<RouteContractCreateRequest>,
) -> Result<(StatusCode, Json<RouteContractResponseDto>), GatewayError> {
    current_user.ensure_admin()?;
    let contract = RouteContractResponseDto {
        id: String::new(),
        route_key: request.route_key,
        owner: request.owner,
        sla_target: request.sla_target,
        status: request.status.unwrap_or_else(|| "draft".into()),
        description: request.description,
        versions: Vec::new(),
    };
    Ok((
        StatusCode::CREATED,
        Json(
            state
                .route_contract_service
                .create_contract(contract)
                .await
                .map_err(map_service_error)?,
        ),
    ))
}

async fn get_contract(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(contract_id): Path<String>,
) -> Result<Json<RouteContractResponseDto>, GatewayError> {
    current_user.ensure_admin()?;
    Ok(Json(
        state
            .route_contract_service
            .get_contract(&contract_id)
            .await
            .map_err(map_service_error)?,
    ))
}

async fn list_versions(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(contract_id): Path<String>,
) -> Result<Json<Vec<RouteContractVersionResponseDto>>, GatewayError> {
    current_user.ensure_admin()?;
    Ok(Json(
        state
            .route_contract_service
            .get_contract(&contract_id)
            .await
            .map_err(map_service_error)?
            .versions,
    ))
}

async fn create_version(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(contract_id): Path<String>,
    Json(request): Json<RouteContractVersionCreateRequest>,
) -> Result<(StatusCode, Json<RouteContractVersionResponseDto>), GatewayError> {
    current_user.ensure_admin()?;
    let version = RouteContractVersionResponseDto {
        version: request.version,
        status: request.status.unwrap_or_else(|| "active".into()),
        deprecation_date: request.deprecation_date,
        response_schema_ref: request.response_schema_ref,
    };
    Ok((
        StatusCode::CREATED,
        Json(
            state
                .route_contract_service
                .create_version(&contract_id, version)
                .await
                .map_err(map_service_error)?,
        ),
    ))
}

async fn get_version(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path((contract_id, version)): Path<(String, String)>,
) -> Result<Json<RouteContractVersionResponseDto>, GatewayError> {
    current_user.ensure_admin()?;
    Ok(Json(
        state
            .route_contract_service
            .get_version(&contract_id, &version)
            .await
            .map_err(map_service_error)?,
    ))
}

async fn update_version(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path((contract_id, version)): Path<(String, String)>,
    Json(request): Json<RouteContractVersionUpdateRequest>,
) -> Result<Json<RouteContractVersionResponseDto>, GatewayError> {
    current_user.ensure_admin()?;
    let existing = state
        .route_contract_service
        .get_version(&contract_id, &version)
        .await
        .map_err(map_service_error)?;
    let updated = RouteContractVersionResponseDto {
        version: existing.version,
        status: request.status.unwrap_or(existing.status),
        deprecation_date: request.deprecation_date.or(existing.deprecation_date),
        response_schema_ref: existing.response_schema_ref,
    };
    Ok(Json(
        state
            .route_contract_service
            .create_version(&contract_id, updated)
            .await
            .map_err(map_service_error)?,
    ))
}

async fn list_policies(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
) -> Result<Json<Vec<PolicySetResponseDto>>, GatewayError> {
    current_user.ensure_admin()?;
    Ok(Json(
        state
            .policy_service
            .list_policies()
            .await
            .map_err(map_service_error)?,
    ))
}

async fn create_policy(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Json(request): Json<PolicySetRequest>,
) -> Result<(StatusCode, Json<PolicySetResponseDto>), GatewayError> {
    current_user.ensure_admin()?;
    Ok((
        StatusCode::CREATED,
        Json(
            state
                .policy_service
                .create_policy(policy_from_request(String::new(), request))
                .await
                .map_err(map_service_error)?,
        ),
    ))
}

async fn get_policy(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(policy_id): Path<String>,
) -> Result<Json<PolicySetResponseDto>, GatewayError> {
    current_user.ensure_admin()?;
    Ok(Json(
        state
            .policy_service
            .get_policy(&policy_id)
            .await
            .map_err(map_service_error)?,
    ))
}

async fn update_policy(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(policy_id): Path<String>,
    Json(request): Json<PolicySetRequest>,
) -> Result<Json<PolicySetResponseDto>, GatewayError> {
    current_user.ensure_admin()?;
    Ok(Json(
        state
            .policy_service
            .create_policy(policy_from_request(policy_id, request))
            .await
            .map_err(map_service_error)?,
    ))
}

async fn list_clients(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
) -> Result<Json<Vec<ClientProfileResponseDto>>, GatewayError> {
    current_user.ensure_admin()?;
    Ok(Json(
        state
            .client_profile_service
            .list_profiles()
            .await
            .map_err(map_service_error)?,
    ))
}

async fn get_client(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(client_id): Path<String>,
) -> Result<Json<ClientProfileResponseDto>, GatewayError> {
    current_user.ensure_admin()?;
    Ok(Json(
        state
            .client_profile_service
            .get_profile(&client_id)
            .await
            .map_err(map_service_error)?,
    ))
}

async fn create_client(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Json(request): Json<ClientProfileRequest>,
) -> Result<(StatusCode, Json<ClientProfileResponseDto>), GatewayError> {
    current_user.ensure_admin()?;
    let profile = ClientProfileResponseDto {
        id: String::new(),
        client_type: request.client_type,
        allowed_contracts: request.allowed_contracts,
        policy_set_id: request.policy_set_id,
    };
    Ok((
        StatusCode::CREATED,
        Json(
            state
                .client_profile_service
                .create_profile(profile)
                .await
                .map_err(map_service_error)?,
        ),
    ))
}

async fn create_rollout(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Json(request): Json<RolloutCreateRequest>,
) -> Result<(StatusCode, Json<RolloutPolicyResponseDto>), GatewayError> {
    current_user.ensure_admin()?;
    let rollout = RolloutPolicyResponseDto {
        id: String::new(),
        contract_id: request.contract_id,
        version: request.version,
        strategy: request.strategy,
        status: request.status.unwrap_or_else(|| "active".into()),
        percent: request.percent,
    };
    Ok((
        StatusCode::CREATED,
        Json(
            state
                .policy_service
                .create_rollout(rollout)
                .await
                .map_err(map_service_error)?,
        ),
    ))
}

async fn get_rollout(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(rollout_id): Path<String>,
) -> Result<Json<RolloutPolicyResponseDto>, GatewayError> {
    current_user.ensure_admin()?;
    Ok(Json(
        state
            .policy_service
            .get_rollout(&rollout_id)
            .await
            .map_err(map_service_error)?,
    ))
}

async fn telemetry_summary(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
) -> Result<Json<TelemetrySummaryResponseDto>, GatewayError> {
    current_user.ensure_admin()?;
    Ok(Json(
        state
            .telemetry_service
            .summary()
            .await
            .map_err(map_service_error)?,
    ))
}

fn policy_from_request(policy_id: String, request: PolicySetRequest) -> PolicySetResponseDto {
    PolicySetResponseDto {
        id: policy_id,
        name: request.name,
        rate_limit: request.rate_limit,
        quota: request.quota,
        timeout_ms: request.timeout_ms,
        retry_policy: request.retry_policy,
        circuit_breaker_policy: request.circuit_breaker_policy,
        cache_policy: request.cache_policy,
    }
}

async fn operation_status(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(operation_id): Path<Uuid>,
) -> Result<Json<OperationStatusResponseDto>, GatewayError> {
    // Admin-only, like the reconciliation backlog it complements. The journal holds
    // request paths, actor ids, and stored response bodies, so an unscoped lookup by
    // id would let any authenticated caller read another tenant's operation.
    current_user.ensure_admin()?;
    let status = match state
        .operation_journal_service
        .operation_status(operation_id)
        .await
    {
        Ok(Some(status)) => status,
        Ok(None) => return Err(GatewayError::not_found("Unknown operation id")),
        Err(error) => return Err(GatewayError::Internal(error.into())),
    };
    let mut status = status;
    status.owner_receipt =
        super::owner_receipts::owner_receipt_status(&state, &status, &state.journal).await;
    if status.owner_receipt.lookup_status
        == crate::domain::operation::OwnerReceiptLookupStatus::Found
        && status.owner_receipt.receipt_state.as_deref() != Some("INCOMPLETE")
    {
        status.outcome = crate::domain::operation::OperationOutcome::Committed;
    }
    Ok(Json(status.into()))
}

async fn resolve_post_event_dead_letter(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(event_id): Path<Uuid>,
    Json(request): Json<ResolveDeadLetterReplayRequest>,
) -> Result<(StatusCode, Json<DeadLetterReplayAcceptedResponse>), GatewayError> {
    current_user.ensure_admin()?;
    request.validate().map_err(GatewayError::bad_request)?;
    let resolution_id = request.resolution_id;
    let response = state
        .post_service
        .resolve_dead_letter_replay(ResolveDeadLetterReplayRequestDto {
            event_id: event_id.to_string(),
            resolution_id: resolution_id.to_string(),
            actor_id: current_user.user_id,
            reason: request.reason.trim().to_owned(),
            evidence_reference: request.evidence_reference.trim().to_owned(),
        })
        .await
        .map_err(map_service_error)?;
    if response.event_id != event_id.to_string()
        || response.resolution_id != resolution_id.to_string()
    {
        return Err(GatewayError::Internal(anyhow::anyhow!(
            "PostService returned a mismatched dead-letter resolution identity"
        )));
    }
    Ok((
        StatusCode::ACCEPTED,
        Json(DeadLetterReplayAcceptedResponse {
            event_id: event_id.to_string(),
            resolution_id: resolution_id.to_string(),
            accepted: true,
        }),
    ))
}

async fn retry_user_deletion(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(saga_id): Path<Uuid>,
    Json(request): Json<RetryUserDeletionRequest>,
) -> Result<(StatusCode, Json<RetryUserDeletionAcceptedResponse>), GatewayError> {
    current_user.ensure_admin()?;
    request.validate().map_err(GatewayError::bad_request)?;
    let resolution_id = request.resolution_id;
    let saga = state
        .auth_service
        .retry_user_deletion(RetryUserDeletionRequestDto {
            saga_id: saga_id.to_string(),
            resolution_id: resolution_id.to_string(),
            actor_id: current_user.user_id,
            reason: request.reason.trim().to_owned(),
            evidence_reference: request.evidence_reference.trim().to_owned(),
        })
        .await
        .map_err(map_service_error)?;
    if saga.id != saga_id.to_string() {
        return Err(GatewayError::Internal(anyhow::anyhow!(
            "Auth service returned a mismatched deletion retry saga identity"
        )));
    }
    Ok((
        StatusCode::ACCEPTED,
        Json(RetryUserDeletionAcceptedResponse {
            resolution_id: resolution_id.to_string(),
            accepted: true,
            saga,
        }),
    ))
}

async fn record_operation_reconciliation(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(operation_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<ManualOperationResolutionRequest>,
) -> Result<(StatusCode, Json<ManualOperationResolutionResponseDto>), GatewayError> {
    use crate::domain::operation::OwnerReceiptLookupStatus as LookupStatus;
    use crate::domain::operation::{ManualResolutionDisposition, ManualResolutionEvidenceKind};

    current_user.ensure_admin()?;
    request.validate().map_err(GatewayError::bad_request)?;

    let mut operation = match state
        .operation_journal_service
        .operation_status(operation_id)
        .await
    {
        Ok(Some(operation)) => operation,
        Ok(None) => return Err(GatewayError::not_found("Unknown operation id")),
        Err(error) => return Err(GatewayError::Internal(error.into())),
    };
    let exact_retry = operation.manual_resolution.as_ref().is_some_and(|current| {
        current.reviewer_id == current_user.user_id
            && current.disposition
                == gateway_domain::operation::ManualResolutionDisposition::from(request.disposition)
            && current.evidence_kind
                == gateway_domain::operation::ManualResolutionEvidenceKind::from(
                    request.evidence_kind,
                )
            && current.evidence_reference == request.evidence_reference
            && current.reason == request.reason.trim()
            && current.supersedes_resolution_id == request.supersedes_resolution_id
    });

    if !exact_retry {
        operation.owner_receipt =
            super::owner_receipts::owner_receipt_status(&state, &operation, &state.journal).await;
        let owner_commit_verified = operation.owner_receipt.lookup_status == LookupStatus::Found
            && matches!(
                operation.owner_receipt.receipt_state.as_deref(),
                Some("REPLAYABLE" | "EXPIRED")
            );
        match operation.owner_receipt.lookup_status {
            LookupStatus::Found
                if owner_commit_verified
                    && (gateway_domain::operation::ManualResolutionDisposition::from(
                        request.disposition,
                    ) != ManualResolutionDisposition::Committed
                        || gateway_domain::operation::ManualResolutionEvidenceKind::from(
                            request.evidence_kind,
                        ) != ManualResolutionEvidenceKind::OwnerConfirmation
                        || request.evidence_reference != operation_id.to_string()) =>
            {
                return Err(GatewayError::conflict(
                    "A matching committed owner receipt is authoritative; only a committed disposition referencing that receipt is allowed",
                ));
            }
            LookupStatus::Mismatch
                if gateway_domain::operation::ManualResolutionDisposition::from(
                    request.disposition,
                ) != ManualResolutionDisposition::Unresolved =>
            {
                return Err(GatewayError::conflict(
                    "Owner receipt identity or scope mismatch must remain unresolved",
                ));
            }
            _ => {}
        }
        if gateway_domain::operation::ManualResolutionEvidenceKind::from(request.evidence_kind)
            == ManualResolutionEvidenceKind::OwnerConfirmation
            && !owner_commit_verified
        {
            return Err(GatewayError::conflict(
                "Owner-confirmation evidence must match a currently verified owner receipt",
            ));
        }
    }

    let request_id = super::header_to_string(&headers, "x-request-id")
        .or_else(|| super::header_to_string(&headers, "x-correlation-id"));
    let ip_address = super::header_to_string(&headers, "x-forwarded-for");
    let user_agent = super::header_to_string(&headers, "user-agent");
    match crate::adapters::r#in::http::request_journal::record_manual_resolution_with_service(
        &state.operation_journal_service,
        operation_id,
        &current_user.user_id,
        &request,
        request_id.as_deref(),
        ip_address.as_deref(),
        user_agent.as_deref(),
    )
    .await
    {
        Ok(crate::adapters::r#in::http::request_journal::ManualResolutionWriteOutcome::Created(
            resolution,
        )) => {
            state.metrics.inc_admin_audit_event(
                "OPERATION_MANUALLY_RECONCILED",
                resolution.disposition.as_str(),
            );
            Ok((StatusCode::CREATED, Json(resolution.into())))
        }
        Ok(crate::adapters::r#in::http::request_journal::ManualResolutionWriteOutcome::Existing(
            resolution,
        )) => Ok((StatusCode::OK, Json(resolution.into()))),
        Ok(crate::adapters::r#in::http::request_journal::ManualResolutionWriteOutcome::OperationNotFound) => {
            Err(GatewayError::not_found("Unknown operation id"))
        }
        Ok(crate::adapters::r#in::http::request_journal::ManualResolutionWriteOutcome::OperationNotUnknown) => {
            Err(GatewayError::conflict(
                "Only operations with an UNKNOWN Gateway outcome can be manually reconciled",
            ))
        }
        Ok(crate::adapters::r#in::http::request_journal::ManualResolutionWriteOutcome::Conflict) => {
            Err(GatewayError::conflict(
                "Resolution revision is stale, conflicts with an existing decision, or has an invalid transition",
            ))
        }
        Err(error) => Err(GatewayError::Internal(error)),
    }
}
