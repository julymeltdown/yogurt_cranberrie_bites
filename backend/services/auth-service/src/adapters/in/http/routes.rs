use super::{map_application_error as map_bad_request, AppError};
use crate::application::dto::auth_dto::{
    EmailCheckRequestDto, LoginRequestDto, LogoutRequestDto, PasswordResetConfirmRequestDto,
    PasswordResetRequestDto, RefreshRequestDto, SignupRequestDto, VerifyEmailRequestDto,
};
use crate::config::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use std::sync::Arc;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/auth/email/signup", post(signup))
        .route("/auth/email/verify", post(verify))
        .route("/auth/email/check", post(check_email))
        .route("/auth/login", post(login))
        .route("/auth/refresh", post(refresh))
        .route("/auth/logout", post(logout))
        .route("/auth/password/reset/request", post(request_password_reset))
        .route("/auth/password/reset/confirm", post(confirm_password_reset))
        .with_state(state)
        .layer(platform::trace_context::TraceContextLayer)
}

async fn signup(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SignupRequestDto>,
) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(serde_json::to_value(
        state
            .signup
            .signup_email(&request.email, &request.password)
            .await
            .map_err(map_bad_request)?,
    )?))
}

async fn verify(
    State(state): State<Arc<AppState>>,
    Json(request): Json<VerifyEmailRequestDto>,
) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(serde_json::to_value(
        state
            .signup
            .verify_email(&request.email, &request.code)
            .await
            .map_err(map_bad_request)?,
    )?))
}

async fn check_email(
    State(state): State<Arc<AppState>>,
    Json(request): Json<EmailCheckRequestDto>,
) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(serde_json::to_value(
        state
            .signup
            .check_email(&request.email)
            .await
            .map_err(map_bad_request)?,
    )?))
}

async fn login(
    State(state): State<Arc<AppState>>,
    Json(request): Json<LoginRequestDto>,
) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(serde_json::to_value(
        state
            .login
            .login(&request.email, &request.password, None, None)
            .await
            .map_err(map_bad_request)?,
    )?))
}

async fn refresh(
    State(state): State<Arc<AppState>>,
    Json(request): Json<RefreshRequestDto>,
) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(serde_json::to_value(
        state
            .login
            .refresh(&request.refresh_token)
            .await
            .map_err(map_bad_request)?,
    )?))
}

async fn logout(
    State(state): State<Arc<AppState>>,
    Json(request): Json<LogoutRequestDto>,
) -> Result<StatusCode, AppError> {
    state
        .login
        .logout(&request.refresh_token)
        .await
        .map_err(map_bad_request)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn request_password_reset(
    State(state): State<Arc<AppState>>,
    Json(request): Json<PasswordResetRequestDto>,
) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(serde_json::to_value(
        state
            .password_reset
            .request_reset(&request.email)
            .await
            .map_err(map_bad_request)?,
    )?))
}

async fn confirm_password_reset(
    State(state): State<Arc<AppState>>,
    Json(request): Json<PasswordResetConfirmRequestDto>,
) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(serde_json::to_value(
        state
            .password_reset
            .confirm_reset(&request.token, &request.new_password)
            .await
            .map_err(map_bad_request)?,
    )?))
}

impl From<serde_json::Error> for AppError {
    fn from(error: serde_json::Error) -> Self {
        Self::Internal(error.into())
    }
}
