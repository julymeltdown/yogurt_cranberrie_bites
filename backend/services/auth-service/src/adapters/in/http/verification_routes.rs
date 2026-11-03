use super::AppError;
use crate::application::onboarding::ResendVerificationRequest;
use crate::application::use_cases::signup::validate_email;
use crate::config::AppState;
use crate::domain::user::User;
use axum::extract::State;
use axum::http::{header::CACHE_CONTROL, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use std::sync::Arc;

/// Auth-service transport only; the public Gateway contract is unchanged.
pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/auth/email/resend", post(resend_verification))
        .with_state(state)
}

async fn resend_verification(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ResendVerificationRequest>,
) -> Result<Response, AppError> {
    let email = User::normalized_email(&request.email);
    validate_email(&email).map_err(|_| AppError::BadRequest("Invalid email".into()))?;
    let response = state
        .signup
        .resend_verification(&email)
        .await
        .map_err(|error| AppError::Internal(error.into()))?;
    Ok((
        StatusCode::ACCEPTED,
        [(CACHE_CONTROL, "no-store")],
        Json(response),
    )
        .into_response())
}
