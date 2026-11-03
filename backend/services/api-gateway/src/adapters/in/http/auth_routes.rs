use super::{
    append_set_cookie, build_refresh_cookie, clear_refresh_cookie, client_ip, map_service_error,
    resolve_cookie, strip_refresh_token, user_agent, CurrentUser, GatewayError,
    REFRESH_COOKIE_NAME,
};
use crate::application::dto::auth_dto::{
    EmailCheckRequest, EmailSignupRequest, EmailVerifyRequest, LoginRequest,
    PasswordResetConfirmRequest, PasswordResetRequest,
};
use crate::config::AppState;
use crate::domain::admission::RouteClass;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use std::sync::Arc;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/auth/email/signup", post(signup))
        .route("/auth/email/verify", post(verify))
        .route("/auth/email/check", post(check_email))
        .route("/auth/password/reset/request", post(request_password_reset))
        .route("/auth/password/reset/confirm", post(confirm_password_reset))
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route(
            "/auth/account",
            get(get_account_deletion).delete(delete_account),
        )
        .route("/auth/account/deletion", get(get_account_deletion))
        .with_state(state)
}

async fn signup(
    State(state): State<Arc<AppState>>,
    Json(request): Json<EmailSignupRequest>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    super::admit_anonymous_identity(&state, RouteClass::Signup, &request.email).await?;
    Ok(Json(serde_json::to_value(
        state
            .auth_service
            .signup(request)
            .await
            .map_err(map_service_error)?,
    )?))
}

async fn verify(
    State(state): State<Arc<AppState>>,
    Json(request): Json<EmailVerifyRequest>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    super::admit_anonymous_identity(&state, RouteClass::Signup, &request.email).await?;
    Ok(Json(serde_json::to_value(
        state
            .auth_service
            .verify(request)
            .await
            .map_err(map_service_error)?,
    )?))
}

async fn check_email(
    State(state): State<Arc<AppState>>,
    Json(request): Json<EmailCheckRequest>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    super::admit_anonymous_identity(&state, RouteClass::Signup, &request.email).await?;
    Ok(Json(serde_json::to_value(
        state
            .auth_service
            .check_email(request)
            .await
            .map_err(map_service_error)?,
    )?))
}

async fn request_password_reset(
    State(state): State<Arc<AppState>>,
    Json(request): Json<PasswordResetRequest>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    super::admit_anonymous_identity(&state, RouteClass::Reset, &request.email).await?;
    Ok(Json(serde_json::to_value(
        state
            .auth_service
            .request_password_reset(request)
            .await
            .map_err(map_service_error)?,
    )?))
}

async fn confirm_password_reset(
    State(state): State<Arc<AppState>>,
    Json(request): Json<PasswordResetConfirmRequest>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    Ok(Json(serde_json::to_value(
        state
            .auth_service
            .confirm_password_reset(request)
            .await
            .map_err(map_service_error)?,
    )?))
}

async fn login(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<LoginRequest>,
) -> Result<Response, GatewayError> {
    super::admit_anonymous_identity(&state, RouteClass::Login, &request.email).await?;
    let response = state
        .auth_service
        .login(request, client_ip(&headers), user_agent(&headers))
        .await
        .map_err(map_service_error)?;
    let mut response_headers = HeaderMap::new();
    append_set_cookie(
        &mut response_headers,
        build_refresh_cookie(&state, &response.refresh_token),
    )?;
    Ok((
        StatusCode::OK,
        response_headers,
        Json(strip_refresh_token(response)),
    )
        .into_response())
}

async fn logout(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    if let Some(refresh_token) = resolve_cookie(&headers, REFRESH_COOKIE_NAME) {
        state
            .auth_service
            .logout(refresh_token)
            .await
            .map_err(map_service_error)?;
    }
    let mut response_headers = HeaderMap::new();
    append_set_cookie(&mut response_headers, clear_refresh_cookie(&state))?;
    Ok((StatusCode::NO_CONTENT, response_headers).into_response())
}

async fn delete_account(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    let response = state
        .auth_service
        .delete_account(
            current_user.user_id,
            request_id(&headers),
            client_ip(&headers),
            user_agent(&headers),
        )
        .await
        .map_err(map_service_error)?;
    let mut response_headers = HeaderMap::new();
    append_set_cookie(&mut response_headers, clear_refresh_cookie(&state))?;
    Ok((StatusCode::ACCEPTED, response_headers, Json(response)).into_response())
}

async fn get_account_deletion(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
) -> Result<Json<crate::application::dto::auth_dto::UserDeletionSagaResponse>, GatewayError> {
    state
        .auth_service
        .get_user_deletion(current_user.user_id)
        .await
        .map_err(map_service_error)?
        .map(Json)
        .ok_or_else(|| GatewayError::not_found("Account deletion not found"))
}

fn request_id(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-request-id")
        .or_else(|| headers.get("x-correlation-id"))
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}
