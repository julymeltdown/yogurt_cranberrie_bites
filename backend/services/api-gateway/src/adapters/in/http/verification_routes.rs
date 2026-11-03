use crate::application::dto::auth_dto::ErrorResponse;
use crate::application::dto::verification_dto::ResendVerificationRequest;
use crate::application::verification::VerificationError;
use crate::config::AppState;
use crate::domain::admission::RouteClass;
use axum::extract::{rejection::JsonRejection, DefaultBodyLimit, State};
use axum::http::{header::CACHE_CONTROL, HeaderValue, Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use chrono::Utc;
use std::sync::Arc;
use uuid::Uuid;

pub const PATH: &str = "/auth/email/resend";
pub const MAX_BODY_BYTES: usize = 4096;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/auth/email/resend", post(resend_verification))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .with_state(state)
}

pub fn browser_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/auth/email/resend", post(resend_verification))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .with_state(state)
}

async fn resend_verification(
    State(state): State<Arc<AppState>>,
    body: Result<Json<ResendVerificationRequest>, JsonRejection>,
) -> Response {
    let Json(request) = match body {
        Ok(request) => request,
        Err(rejection) => {
            let status = rejection.status();
            let message = match status {
                StatusCode::PAYLOAD_TOO_LARGE => "Request body too large",
                StatusCode::UNSUPPORTED_MEDIA_TYPE => "JSON Content-Type required",
                StatusCode::UNPROCESSABLE_ENTITY => "Invalid request body",
                _ => "Invalid JSON",
            };
            return failure(status, message);
        }
    };
    if let Err(error) =
        super::admit_anonymous_identity(&state, RouteClass::VerificationResend, &request.email)
            .await
    {
        return error.into_response();
    }
    match state.auth_service.resend_verification(request).await {
        Ok(response) => (StatusCode::ACCEPTED, Json(response)).into_response(),
        Err(error) => {
            let status = match error {
                VerificationError::InvalidEmail => StatusCode::BAD_REQUEST,
                VerificationError::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
                VerificationError::Timeout => StatusCode::GATEWAY_TIMEOUT,
                VerificationError::InvalidResponse => StatusCode::BAD_GATEWAY,
                VerificationError::Internal => StatusCode::INTERNAL_SERVER_ERROR,
            };
            failure(status, &error.to_string())
        }
    }
}

fn failure(status: StatusCode, message: &str) -> Response {
    (
        status,
        Json(ErrorResponse {
            code: format!("GATEWAY_{}", status.as_str()),
            message: message.into(),
            trace_id: Some(Uuid::new_v4().to_string()),
            timestamp: Some(Utc::now()),
            details: None,
        }),
    )
        .into_response()
}

/// Wrap the global deadline and quota layers, not only the handler. Rejections
/// and timeouts must never acquire a browser/proxy cache entry for an email.
pub async fn protect_response(mut request: Request<axum::body::Body>, next: Next) -> Response {
    if request.uri().path() != PATH {
        return next.run(request).await;
    }
    let correlation = request
        .headers()
        .get("x-correlation-id")
        .filter(|value| {
            value.as_bytes().len() <= 128
                && value.to_str().is_ok_and(|text| !text.trim().is_empty())
        })
        .cloned()
        .unwrap_or_else(|| {
            HeaderValue::from_str(&Uuid::new_v4().to_string()).expect("UUID header")
        });
    request
        .headers_mut()
        .insert("x-correlation-id", correlation.clone());
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
        .headers_mut()
        .insert("x-correlation-id", correlation);
    response
}
