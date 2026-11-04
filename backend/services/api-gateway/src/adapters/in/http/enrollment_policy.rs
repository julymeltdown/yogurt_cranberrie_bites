//! Canonical browser enrollment surface. Never reads another service's database.
use super::{correlation_id_for_response, CurrentUser};
use crate::application::dto::facade_dto::{HighRiskActionDto, SessionDecisionDto};
use crate::application::enrollment_policy::*;
use crate::config::{AppState, RuntimeMode};
use crate::domain::admission::RouteClass;
use axum::{
    extract::{DefaultBodyLimit, Request, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use chrono::{SecondsFormat, Utc};
use ring::rand::{SecureRandom, SystemRandom};
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

pub const COOKIE: &str = "__Secure-mcr-enrollment";
pub const DEV_COOKIE: &str = "mcr-enrollment-dev";
#[derive(Clone, Copy)]
pub struct PolicyDenial;
#[derive(Clone, Copy)]
struct Protected;
pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/auth/enrollment/policy", get(policy))
        .route("/api/auth/enrollment/attempts", post(begin))
        .route("/api/auth/enrollment/signup", post(signup))
        .route("/api/auth/enrollment/verify", post(verify))
        .route("/api/auth/enrollment/resend", post(resend))
        .route("/api/auth/enrollment/admission", get(admission))
        .route("/api/auth/enrollment/consent", post(consent))
        .layer(DefaultBodyLimit::max(12288))
        .layer(middleware::from_fn(protect_errors))
        .with_state(state)
}
fn cookie_name(state: &AppState) -> &'static str {
    if matches!(state.runtime_mode, RuntimeMode::Production) || state.refresh_cookie_secure {
        COOKIE
    } else {
        DEV_COOKIE
    }
}
fn binding(headers: &HeaderMap, name: &str) -> Result<Option<String>, EnrollmentFailure> {
    let mut found = None;
    for header in headers.get_all(header::COOKIE) {
        let text = header.to_str().map_err(|_| EnrollmentFailure {
            status: 404,
            reason: "ATTEMPT_INVALID",
        })?;
        for (key, value) in text
            .split(';')
            .filter_map(|part| part.trim().split_once('='))
        {
            if key != name {
                continue;
            }
            if found.is_some()
                || value.len() != 64
                || !value
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err(EnrollmentFailure {
                    status: 404,
                    reason: "ATTEMPT_INVALID",
                });
            }
            found = Some(value.to_owned());
        }
    }
    Ok(found)
}
fn response(operation: EnrollmentOperation, result: Result<Value, EnrollmentFailure>) -> Response {
    match result {
        Ok(value) => (
            StatusCode::from_u16(operation.success_status()).expect("fixed status"),
            Json(value),
        )
            .into_response(),
        Err(error) => failure(error),
    }
}
pub fn failure(error: EnrollmentFailure) -> Response {
    let mut response=(StatusCode::from_u16(error.status).unwrap_or(StatusCode::SERVICE_UNAVAILABLE),Json(json!({
        "code":format!("GATEWAY_{}",error.status),"message":"Enrollment request could not be completed",
        "traceId":correlation_id_for_response(),"timestamp":Utc::now().to_rfc3339_opts(SecondsFormat::Millis,true),
        "details":if error.reason=="RATE_LIMITED" {json!({"reason":error.reason,"retry_after_ms":1000})} else {json!({"reason":error.reason})}
    }))).into_response();
    response.extensions_mut().insert(Protected);
    if error.status == 429 {
        response
            .headers_mut()
            .insert(header::RETRY_AFTER, HeaderValue::from_static("1"));
    }
    if matches!(error.reason, "CONSENT_REQUIRED" | "POLICY_UNAVAILABLE") {
        response.extensions_mut().insert(PolicyDenial);
    }
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}
async fn policy(State(state): State<Arc<AppState>>) -> Response {
    response(
        EnrollmentOperation::Policy,
        state
            .enrollment_policy
            .execute(EnrollmentOperation::Policy, None, None, Value::Null)
            .await,
    )
}
async fn begin(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let name = cookie_name(&state);
    let binding = match binding(&headers, name) {
        Ok(Some(value)) => value,
        Ok(None) => {
            let mut bytes = [0u8; 32];
            if SystemRandom::new().fill(&mut bytes).is_err() {
                return failure(EnrollmentFailure::UNAVAILABLE);
            }
            bytes.iter().map(|b| format!("{b:02x}")).collect()
        }
        Err(error) => return failure(error),
    };
    if let Err(error) = super::admit_anonymous_identity(&state, RouteClass::Signup, &binding).await
    {
        return error.into_response();
    }
    let mut reply = response(
        EnrollmentOperation::Begin,
        state
            .enrollment_policy
            .execute(EnrollmentOperation::Begin, Some(&binding), None, body)
            .await,
    );
    if reply.status() == StatusCode::CREATED {
        let secure = if name == COOKIE { "; Secure" } else { "" };
        let value=format!("{name}={binding}; HttpOnly; SameSite=Strict; Path=/api/auth/enrollment; Max-Age=1800{secure}");
        reply.headers_mut().append(
            header::SET_COOKIE,
            HeaderValue::from_str(&value).expect("fixed hex cookie"),
        );
    }
    reply
}
async fn anonymous(
    state: Arc<AppState>,
    headers: HeaderMap,
    body: Value,
    operation: EnrollmentOperation,
) -> Response {
    let binding = match binding(&headers, cookie_name(&state)) {
        Ok(Some(value)) => value,
        _ => {
            return failure(EnrollmentFailure {
                status: 404,
                reason: "ATTEMPT_INVALID",
            })
        }
    };
    let class = if operation == EnrollmentOperation::Resend {
        RouteClass::VerificationResend
    } else {
        RouteClass::Signup
    };
    let identity = if operation == EnrollmentOperation::Signup {
        body.get("email")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_lowercase()
    } else {
        binding.clone()
    };
    if let Err(error) = super::admit_anonymous_identity(&state, class, &identity).await {
        return error.into_response();
    }
    response(
        operation,
        state
            .enrollment_policy
            .execute(operation, Some(&binding), None, body)
            .await,
    )
}
async fn signup(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    anonymous(state, headers, body, EnrollmentOperation::Signup).await
}
async fn verify(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    anonymous(state, headers, body, EnrollmentOperation::Verify).await
}
async fn resend(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    anonymous(state, headers, body, EnrollmentOperation::Resend).await
}
async fn authenticated(
    state: Arc<AppState>,
    user: CurrentUser,
    body: Value,
    operation: EnrollmentOperation,
) -> Response {
    let Some(session) = user.session_id.as_deref() else {
        return failure(EnrollmentFailure {
            status: 401,
            reason: "AUTHENTICATION_REQUIRED",
        });
    };
    match state
        .high_risk_sessions
        .check(&user.user_id, session, HighRiskActionDto::AccountSecurity)
        .await
    {
        Ok(verdict) if verdict.decision == SessionDecisionDto::Allowed => {}
        Ok(_) => {
            return failure(EnrollmentFailure {
                status: 401,
                reason: "AUTHENTICATION_REQUIRED",
            })
        }
        Err(_) => return failure(EnrollmentFailure::UNAVAILABLE),
    }
    let actor = match Uuid::parse_str(&user.user_id) {
        Ok(id) if !id.is_nil() => id,
        _ => {
            return failure(EnrollmentFailure {
                status: 401,
                reason: "AUTHENTICATION_REQUIRED",
            })
        }
    };
    response(
        operation,
        state
            .enrollment_policy
            .execute(operation, None, Some(actor), body)
            .await,
    )
}
async fn admission(State(state): State<Arc<AppState>>, user: CurrentUser) -> Response {
    authenticated(state, user, Value::Null, EnrollmentOperation::Admission).await
}
async fn consent(
    State(state): State<Arc<AppState>>,
    user: CurrentUser,
    Json(body): Json<Value>,
) -> Response {
    authenticated(state, user, body, EnrollmentOperation::Consent).await
}
async fn protect_errors(request: Request, next: Next) -> Response {
    let response = next.run(request).await;
    if response.extensions().get::<Protected>().is_some()
        || (!response.status().is_client_error() && !response.status().is_server_error())
    {
        return response;
    }
    let status = response.status();
    let reason = match status.as_u16() {
        400 | 422 => "INVALID_INPUT",
        401 => "AUTHENTICATION_REQUIRED",
        403 => "PERMISSION_DENIED",
        404 => "RESOURCE_UNAVAILABLE",
        413 => "PAYLOAD_TOO_LARGE",
        415 => "INVALID_MEDIA_TYPE",
        429 => "RATE_LIMITED",
        504 => "DEADLINE_EXCEEDED",
        _ => "DEPENDENCY_UNAVAILABLE",
    };
    let retry = response.headers().get(header::RETRY_AFTER).cloned();
    let mut result = failure(EnrollmentFailure {
        status: status.as_u16(),
        reason,
    });
    if let Some(retry) = retry {
        result.headers_mut().insert(header::RETRY_AFTER, retry);
    }
    result
}
