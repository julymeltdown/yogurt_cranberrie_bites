pub mod admin_routes;
pub mod aggregation_routes;
pub mod api_docs_routes;
pub mod auth_routes;
pub mod auth_shadow_routes;
pub mod browser_auth;
pub mod client_identity;
pub mod concert_access;
pub mod concert_world;
pub mod concert_bootstrap;
pub mod concert_product;
pub mod concert_participant;
pub(crate) mod response_disclosure;

pub mod friend_routes;
pub mod moderation_decision_worker;
pub mod moderation_routes;
pub mod notification_routes;
pub mod oauth_routes;
pub mod operation_routes;
pub mod operation_worker;
pub mod owner_receipts;
pub mod post_routes;
pub mod profile_routes;
pub mod refresh_routes;
pub mod request_journal;
pub mod server;
pub mod verification_routes;

mod extractor_rejections;
pub use extractor_rejections::normalize_extractor_rejections;

use crate::application::dto::auth_dto::{AuthResponse, ErrorResponse};
use crate::application::dto::facade_dto::IdentityProviderDto as IdentityProvider;
use crate::application::dto::facade_dto::SessionDecisionDto as SessionDecision;
use crate::application::dto::legacy_audit_dto::{LegacyAuditCommandDto, LegacyAuditOutcomeDto};
use crate::config::{AppState, GatewayMetrics};
use crate::domain::access_token::{TokenRejection, VerifiedAccessToken};
use crate::domain::admission::{AdmissionMode, AdmissionScope, RouteClass};
use crate::domain::high_risk;
use async_trait::async_trait;
use axum::extract::{FromRef, FromRequestParts, MatchedPath, State};
use axum::http::header::{AUTHORIZATION, CONTENT_TYPE, COOKIE, SET_COOKIE, USER_AGENT};
use axum::http::{HeaderMap, HeaderValue, Request, StatusCode};
use axum::middleware::{from_fn_with_state, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

pub const REFRESH_COOKIE_NAME: &str = "refresh_token";

/// Absolute request budget shared with route composition. Durable dispatch may
/// shorten it to the caller's journal wait; presentation must not restart it.
#[derive(Clone, Copy)]
pub struct RequestDeadline(pub std::time::Instant);

impl RequestDeadline {
    pub fn projection_budget(self) -> std::time::Duration {
        self.0
            .saturating_duration_since(std::time::Instant::now())
            .saturating_sub(std::time::Duration::from_millis(100))
    }
}

tokio::task_local! {
    /// Correlation identifier for the request being handled right now.
    ///
    /// The error type's response converter has no access to request headers, so
    /// without this an error body could only mint its own unrelated identifier and
    /// support could not join a client report to the server logs.
    pub static CORRELATION_ID: String;
}

/// The current request's correlation identifier, when one is in scope.
pub fn current_correlation_id() -> Option<String> {
    CORRELATION_ID.try_with(Clone::clone).ok()
}

/// The identifier an error response should report.
///
/// Prefers the identifier the transport layer already assigned so the body and
/// the header agree, which is what lets support join a client report to a log
/// line. Falls back to a fresh identifier so the field is always populated and,
/// critically, the response header is set from the same value rather than a
/// different random one.
pub fn correlation_id_for_response() -> String {
    current_correlation_id().unwrap_or_else(|| Uuid::new_v4().to_string())
}

#[derive(Debug, thiserror::Error)]
pub enum GatewayError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Unauthorized(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    TooManyRequests(String),
    #[error("{0}")]
    GatewayTimeout(String),
    #[error("friend service unavailable")]
    ServiceUnavailable(#[source] anyhow::Error),
    /// The owning service refused work because it is saturated. Unlike an
    /// outage, the caller should retry shortly, so the response carries a hint.
    #[error("service busy")]
    ServiceBusy,
    /// The Gateway's route-class concurrency ceiling is saturated.
    #[error("Gateway admission capacity is busy")]
    AdmissionSaturated,
    #[error("downstream request timed out")]
    DownstreamTimeout(#[source] anyhow::Error),
    #[error("internal error")]
    Internal(#[from] anyhow::Error),
}

impl GatewayError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::BadRequest(message.into())
    }
    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::Unauthorized(message.into())
    }
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::Forbidden(message.into())
    }
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound(message.into())
    }
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::Conflict(message.into())
    }
    pub fn too_many_requests(message: impl Into<String>) -> Self {
        Self::TooManyRequests(message.into())
    }
    pub fn gateway_timeout(message: impl Into<String>) -> Self {
        Self::GatewayTimeout(message.into())
    }
}

/// Seconds a client should wait before retrying a saturated request.
///
/// A saturated password-hashing class drains within about a second at the
/// default bounds; a small fixed hint spreads retries without stalling a user.
pub const BUSY_RETRY_AFTER_SECONDS: u64 = 1;

impl IntoResponse for GatewayError {
    fn into_response(self) -> Response {
        let retry_after = matches!(self, Self::ServiceBusy | Self::AdmissionSaturated)
            .then_some(BUSY_RETRY_AFTER_SECONDS);
        let (status, message) = match self {
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
            Self::Unauthorized(message) => (StatusCode::UNAUTHORIZED, message),
            Self::Forbidden(message) => (StatusCode::FORBIDDEN, message),
            Self::NotFound(message) => (StatusCode::NOT_FOUND, message),
            Self::Conflict(message) => (StatusCode::CONFLICT, message),
            Self::TooManyRequests(message) => (StatusCode::TOO_MANY_REQUESTS, message),
            Self::GatewayTimeout(message) => (StatusCode::GATEWAY_TIMEOUT, message),
            Self::ServiceUnavailable(error) => {
                tracing::error!(error = ?error, "downstream service unavailable");
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "Service unavailable".into(),
                )
            }
            Self::ServiceBusy => {
                tracing::warn!(code = "AUTH_BUSY", "downstream service busy");
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "Service busy; retry later".into(),
                )
            }
            Self::AdmissionSaturated => {
                tracing::warn!(code = "GATEWAY_ADMISSION_BUSY", "route class at capacity");
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "Service busy; retry later".into(),
                )
            }
            Self::DownstreamTimeout(error) => {
                tracing::error!(error = ?error, "downstream request timed out");
                (
                    StatusCode::GATEWAY_TIMEOUT,
                    "Downstream request timed out".into(),
                )
            }
            Self::Internal(error) => {
                tracing::error!(error = ?error, "gateway error");
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal error".into())
            }
        };
        // One identifier for both the body and the header. Previously the body
        // carried a random value unrelated to the header, so a client could not
        // join its report to the server logs.
        let correlation_id = correlation_id_for_response();
        let mut response = (
            status,
            axum::Json(ErrorResponse {
                code: format!("GATEWAY_{}", status.as_str()),
                message,
                trace_id: Some(correlation_id.clone()),
                timestamp: Some(Utc::now()),
                details: None,
            }),
        )
            .into_response();
        if let Ok(value) = HeaderValue::from_str(&correlation_id) {
            response.headers_mut().insert("x-correlation-id", value);
        }
        if let Some(seconds) = retry_after {
            response
                .headers_mut()
                .insert(axum::http::header::RETRY_AFTER, HeaderValue::from(seconds));
        }
        response
    }
}

pub fn router(state: Arc<AppState>) -> Router {
    // Session confirmation runs inside the admin audit, so a refused admin
    // request is audited like any other.
    let admin_router = admin_routes::router(state.clone())
        .layer(from_fn_with_state(
            state.clone(),
            high_risk_session_middleware,
        ))
        .layer(from_fn_with_state(state.clone(), admin_audit_middleware));
    let participant = state.concert_participant_client.as_ref().map(|client| {
        concert_participant::router(state.clone(), client.clone())
    }).unwrap_or_default();
    let router = Router::new()
        .merge(api_docs_routes::router(
            state.runtime_mode,
            state.developer_portal_links.clone(),
        ))
        .merge(browser_auth::router(state.clone()))
        .merge(concert_bootstrap::router(state.clone()))
        .merge(participant)
        .merge(concert_product::router(state.clone()))
        .merge(concert_world::router_with_boundary(state.clone()))
        .merge(profile_routes::router(state.clone()))
        .merge(friend_routes::router(state.clone()))
        .merge(moderation_routes::router(state.clone()))
        .merge(operation_routes::router(state.clone()))
        .merge(notification_routes::router(state.clone()))
        .merge(post_routes::router(state.clone()))
        .merge(aggregation_routes::router(state.clone()))
        // A layer covers only the routes present when it is added; the admin
        // routes carry their own, and parity routes are never high-risk.
        .layer(from_fn_with_state(
            state.clone(),
            high_risk_session_middleware,
        ))
        .merge(admin_router);
    // Rate admission precedes durable intent; the external request deadline
    // remains outside all writes. Production transport enables durable dispatch.
    router
        .layer(from_fn_with_state(state.clone(), mutation_audit_middleware))
        .layer(from_fn_with_state(state.clone(), rate_limit_middleware))
        .layer(from_fn_with_state(state.clone(), telemetry_middleware))
        .layer(axum::middleware::from_fn(normalize_extractor_rejections))
        .layer(from_fn_with_state(state.clone(), timeout_middleware))
        .layer(axum::middleware::from_fn(
            client_identity::normalize_forwarded_for,
        ))
        .layer(axum::middleware::from_fn(
            verification_routes::protect_response,
        ))
        .layer(axum::middleware::from_fn(
            oauth_routes::protect_form_post_response,
        ))
        .layer(from_fn_with_state(
            state.browser_auth_origins.clone(),
            browser_auth::boundary,
        ))
        .layer(axum::middleware::from_fn(concert_bootstrap::protect_response))
        .layer(axum::middleware::from_fn(concert_product::protect_response))
}

pub fn metrics_router(metrics_state: GatewayMetrics) -> Router {
    Router::new()
        .route("/metrics", get(metrics))
        .with_state(metrics_state)
}

async fn metrics(State(metrics): State<GatewayMetrics>) -> Response {
    (
        [(
            CONTENT_TYPE,
            HeaderValue::from_static("text/plain; version=0.0.4; charset=utf-8"),
        )],
        metrics.render_prometheus(),
    )
        .into_response()
}

#[derive(Clone)]
pub struct CurrentUser {
    pub user_id: String,
    pub roles: Vec<String>,
    /// Session family named by the token, when it carries one.
    pub session_id: Option<String>,
}

impl std::fmt::Debug for CurrentUser {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CurrentUser")
            .field("user_id", &"[REDACTED]")
            .field("roles", &"[REDACTED]")
            .field("session_id", &"[REDACTED]")
            .finish()
    }
}

impl CurrentUser {
    pub fn ensure_admin(&self) -> Result<(), GatewayError> {
        if self.roles.iter().any(|role| role == "ROLE_ADMIN") {
            Ok(())
        } else {
            Err(GatewayError::forbidden("Admin role required"))
        }
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for CurrentUser
where
    Arc<AppState>: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = GatewayError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let state = Arc::<AppState>::from_ref(state);
        let claims = if let Some(claims) = parts.extensions.get::<VerifiedAccessToken>() {
            claims.clone()
        } else {
            let authorization = parts
                .headers
                .get(AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| {
                    state
                        .metrics
                        .inc_jwt_verification_failure("missing_bearer_token");
                    GatewayError::unauthorized("Missing bearer token")
                })?;
            let token = parse_bearer_token(authorization).ok_or_else(|| {
                state
                    .metrics
                    .inc_jwt_verification_failure("invalid_bearer_token");
                GatewayError::unauthorized("Invalid bearer token")
            })?;
            verify_bearer_token(&state, token)
                .await
                .map_err(|rejection| {
                    state
                        .metrics
                        .inc_jwt_verification_failure(rejection.reason());
                    GatewayError::unauthorized("Invalid bearer token")
                })?
        };
        Ok(CurrentUser {
            user_id: claims.subject,
            roles: claims.roles,
            session_id: claims.session_id,
        })
    }
}

/// Verify a bearer token, refreshing the issuer's keys once if it names a key
/// this Gateway does not hold.
///
/// An unknown key is usually one the issuer published after the last scheduled
/// refresh. The refresher rate-limits these early refreshes, so tokens with
/// made-up key ids cost at most one fetch per interval and are otherwise
/// answered from the installed keys.
pub(crate) async fn verify_bearer_token(
    state: &AppState,
    token: &str,
) -> Result<VerifiedAccessToken, TokenRejection> {
    match state.jwt_verifier.verify_access_token(token) {
        Err(TokenRejection::UnknownKid) => match &state.signing_keys {
            Some(refresher) if refresher.refresh_for_unknown_kid().await => {
                state.jwt_verifier.verify_access_token(token)
            }
            _ => Err(TokenRejection::UnknownKid),
        },
        verified => verified,
    }
}

/// Confirm a high-risk request with the issuer before its handler runs.
///
/// An access token is trusted until it expires, which is too long for the
/// actions [`high_risk::classify`] lists: a session that has logged out, been
/// revoked, or lost the admin role must stop within the configured freshness
/// window instead. Such a request is refused when its token names no session,
/// when the session has ended, when the required role is gone, and when the
/// issuer cannot answer.
///
/// A request without a usable bearer token passes through; its handler refuses
/// it exactly as it refuses one on any other protected route.
async fn high_risk_session_middleware(
    State(state): State<Arc<AppState>>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let Some(action) = high_risk::classify(request.method().as_str(), matched_route(&request))
    else {
        return next.run(request).await;
    };
    let token = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_bearer_token)
        .map(ToOwned::to_owned);
    let Some(token) = token else {
        return next.run(request).await;
    };
    let claims = match request.extensions().get::<VerifiedAccessToken>().cloned() {
        Some(claims) => claims,
        None => match verify_bearer_token(&state, &token).await {
            Ok(claims) => claims,
            Err(_) => return next.run(request).await,
        },
    };
    let label = action.as_str();
    let Some(session_id) = claims.session_id.as_deref() else {
        // Issued before tokens named their session. It cannot be checked, so
        // it cannot be trusted with this action; signing in again issues one
        // that can.
        state
            .metrics
            .inc_high_risk_session_check(label, "no_session_claim");
        return GatewayError::unauthorized("Sign in again to continue").into_response();
    };
    match state
        .high_risk_sessions
        .check(&claims.subject, session_id, action.into())
        .await
    {
        Ok(verdict) => match verdict.decision {
            SessionDecision::Allowed => {
                let outcome = if verdict.from_cache {
                    "allowed_cached"
                } else {
                    "allowed"
                };
                state.metrics.inc_high_risk_session_check(label, outcome);
                next.run(request).await
            }
            SessionDecision::SessionEnded => {
                state
                    .metrics
                    .inc_high_risk_session_check(label, "session_ended");
                GatewayError::unauthorized("Session has ended; sign in again").into_response()
            }
            SessionDecision::RoleRevoked => {
                state
                    .metrics
                    .inc_high_risk_session_check(label, "role_revoked");
                GatewayError::forbidden("Admin role required").into_response()
            }
        },
        Err(error) => {
            state
                .metrics
                .inc_high_risk_session_check(label, "unavailable");
            GatewayError::ServiceUnavailable(
                anyhow::Error::new(error)
                    .context("high-risk session check could not reach the issuer"),
            )
            .into_response()
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct OptionalCurrentUser(pub Option<CurrentUser>);

#[async_trait]
impl<S> FromRequestParts<S> for OptionalCurrentUser
where
    Arc<AppState>: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = GatewayError;
    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        if parts.headers.get(AUTHORIZATION).is_none() {
            return Ok(Self(None));
        }
        CurrentUser::from_request_parts(parts, state)
            .await
            .map(|user| Self(Some(user)))
    }
}

pub fn strip_refresh_token(response: AuthResponse) -> AuthResponse {
    AuthResponse {
        refresh_token: String::new(),
        ..response
    }
}

pub fn build_refresh_cookie(state: &AppState, refresh_token: &str) -> String {
    let mut cookie = format!(
        "{}={}; HttpOnly; Path=/; SameSite=Lax; Max-Age={}",
        REFRESH_COOKIE_NAME, refresh_token, state.refresh_cookie_max_age_seconds
    );
    if state.refresh_cookie_secure {
        cookie.push_str("; Secure");
    }
    if !state.refresh_cookie_domain.is_empty() {
        cookie.push_str("; Domain=");
        cookie.push_str(&state.refresh_cookie_domain);
    }
    cookie
}

pub fn clear_refresh_cookie(state: &AppState) -> String {
    let mut cookie = format!("{REFRESH_COOKIE_NAME}=; HttpOnly; Path=/; SameSite=Lax; Max-Age=0");
    if state.refresh_cookie_secure {
        cookie.push_str("; Secure");
    }
    if !state.refresh_cookie_domain.is_empty() {
        cookie.push_str("; Domain=");
        cookie.push_str(&state.refresh_cookie_domain);
    }
    cookie
}

pub fn oauth_state_cookie_name(provider: IdentityProvider) -> String {
    format!("oauth_state_{}", provider_name(provider))
}

pub fn build_oauth_state_cookie(
    state: &AppState,
    provider: IdentityProvider,
    value: &str,
) -> String {
    let mut cookie = format!(
        "{}={}; HttpOnly; Path=/; SameSite=Lax; Max-Age={}",
        oauth_state_cookie_name(provider),
        value,
        600
    );
    if state.refresh_cookie_secure {
        cookie.push_str("; Secure");
    }
    if !state.refresh_cookie_domain.is_empty() {
        cookie.push_str("; Domain=");
        cookie.push_str(&state.refresh_cookie_domain);
    }
    cookie
}

pub fn clear_oauth_state_cookie(state: &AppState, provider: IdentityProvider) -> String {
    let mut cookie = format!(
        "{}=; HttpOnly; Path=/; SameSite=Lax; Max-Age=0",
        oauth_state_cookie_name(provider)
    );
    if state.refresh_cookie_secure {
        cookie.push_str("; Secure");
    }
    if !state.refresh_cookie_domain.is_empty() {
        cookie.push_str("; Domain=");
        cookie.push_str(&state.refresh_cookie_domain);
    }
    cookie
}

pub fn resolve_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .filter_map(|segment| segment.trim().split_once('='))
        .find(|(key, _)| *key == name)
        .map(|(_, value)| value.to_owned())
}

const MAX_QUERY_STRING_BYTES: usize = 8 * 1024;

pub fn extract_query_param(uri: &http::Uri, key: &str) -> Option<String> {
    let query = uri.query()?;
    if query.len() > MAX_QUERY_STRING_BYTES {
        return None;
    }
    for pair in query.split('&') {
        // A pair without an equals sign is a bare flag, not a reason to stop
        // scanning. The previous early return abandoned the whole query at the
        // first such pair, so a valid parameter after it was silently lost.
        let (field, value) = match pair.split_once('=') {
            Some(parts) => parts,
            None => (pair, ""),
        };
        // Compare the decoded field so an encoded key still matches.
        if percent_decode(field) == key {
            return Some(percent_decode(value));
        }
    }
    None
}

/// Decode a percent-encoded query component, treating plus as a space.
///
/// Invalid escapes are left as written rather than rejected: a malformed escape
/// is a malformed value, and the caller's own validation is the place to reject
/// it, not a silent None from the parser.
fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                let high = (bytes[index + 1] as char).to_digit(16);
                let low = (bytes[index + 2] as char).to_digit(16);
                match (high, low) {
                    (Some(high), Some(low)) => {
                        out.push((high * 16 + low) as u8);
                        index += 3;
                    }
                    _ => {
                        out.push(bytes[index]);
                        index += 1;
                    }
                }
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn parse_provider(value: &str) -> Result<IdentityProvider, GatewayError> {
    match value.to_ascii_uppercase().as_str() {
        "EMAIL" => Ok(IdentityProvider::Email),
        "GOOGLE" => Ok(IdentityProvider::Google),
        "APPLE" => Ok(IdentityProvider::Apple),
        "X" | "TWITTER" => Ok(IdentityProvider::X),
        _ => Err(GatewayError::bad_request("Unsupported identity provider")),
    }
}

pub fn header_value(value: &str) -> Result<HeaderValue, GatewayError> {
    HeaderValue::from_str(value).map_err(|error| GatewayError::Internal(error.into()))
}

fn matched_route<B>(request: &Request<B>) -> &str {
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map(MatchedPath::as_str)
        .unwrap_or("unmatched");
    // Reuse the owning Auth admission/session policies at the new C03 ingress.
    if route.starts_with("/api/auth/") {
        route.strip_prefix("/api").unwrap_or(route)
    } else {
        route
    }
}

async fn rate_limit_middleware(
    State(state): State<Arc<AppState>>,
    mut request: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, GatewayError> {
    let route = matched_route(&request).to_owned();
    let class = RouteClass::classify(request.method().as_str(), &route);
    let _in_flight_permit = state
        .rate_limiter
        .try_acquire_in_flight(class)
        .map_err(|_| {
            state.metrics.inc_admission_in_flight_rejection(class);
            GatewayError::AdmissionSaturated
        })?;
    let key = rate_limit_key_with_prefix(
        request.headers(),
        state.rate_limiter.policies().v6_prefix_length,
    );
    let admission_started = std::time::Instant::now();
    let (outcome, lock_wait) = state.rate_limiter.admit_observed(class, &key).await;
    if let Some(lock_wait) = lock_wait {
        state
            .metrics
            .observe_admission_lock_wait(class, AdmissionScope::Network, lock_wait);
    }
    state.metrics.observe_admission_duration(
        class,
        AdmissionScope::Network,
        admission_started.elapsed(),
    );
    state
        .metrics
        .inc_admission_decision(class.as_str(), AdmissionScope::Network, outcome.as_str());
    if outcome.is_store_failure() {
        return Err(GatewayError::ServiceUnavailable(anyhow::anyhow!(
            "admission store unavailable for {class} requests"
        )));
    }
    if !outcome.is_admitted() {
        if state.rate_limiter.policies().mode == AdmissionMode::Shadow
            && outcome.is_shadowable_rejection()
        {
            state.metrics.inc_admission_shadow_pass(
                class.as_str(),
                AdmissionScope::Network,
                outcome.as_str(),
            );
        } else {
            state.metrics.inc_rate_limit_rejection(&route);
            return Err(GatewayError::too_many_requests("Rate limit exceeded"));
        }
    }

    // Principal identity is trusted only after the issuer's signature, issuer,
    // audience, expiry, and access-token type have been verified. Invalid or
    // missing tokens do not choose a budget key; protected handlers retain
    // responsibility for returning their normal authentication error.
    if state.rate_limiter.policies().principal_budget(class) > 0 {
        let verified = request
            .headers()
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(parse_bearer_token)
            .map(ToOwned::to_owned);
        if let Some(token) = verified {
            if let Ok(claims) = verify_bearer_token(&state, &token).await {
                if !(32..=4096).contains(&state.admission_hmac_key.len()) {
                    return Err(GatewayError::ServiceUnavailable(anyhow::anyhow!(
                        "authenticated principal admission key is unavailable"
                    )));
                }
                let digest = client_identity::keyed_admission_principal(
                    state.admission_hmac_key.as_ref(),
                    &claims.subject,
                );
                let principal_started = std::time::Instant::now();
                let (outcome, lock_wait) = state
                    .rate_limiter
                    .admit_principal_observed(class, &digest)
                    .await;
                if let Some(lock_wait) = lock_wait {
                    state.metrics.observe_admission_lock_wait(
                        class,
                        AdmissionScope::Principal,
                        lock_wait,
                    );
                }
                state.metrics.observe_admission_duration(
                    class,
                    AdmissionScope::Principal,
                    principal_started.elapsed(),
                );
                state.metrics.inc_admission_decision(
                    class.as_str(),
                    AdmissionScope::Principal,
                    outcome.as_str(),
                );
                if outcome.is_store_failure() {
                    return Err(GatewayError::ServiceUnavailable(anyhow::anyhow!(
                        "admission store unavailable for {class} principal requests"
                    )));
                }
                if !outcome.is_admitted() {
                    if state.rate_limiter.policies().mode == AdmissionMode::Shadow
                        && outcome.is_shadowable_rejection()
                    {
                        state.metrics.inc_admission_shadow_pass(
                            class.as_str(),
                            AdmissionScope::Principal,
                            outcome.as_str(),
                        );
                    } else {
                        state
                            .metrics
                            .inc_rate_limit_rejection(&format!("principal:{}", class.as_str()));
                        return Err(GatewayError::too_many_requests("Rate limit exceeded"));
                    }
                }
                request.extensions_mut().insert(claims);
            }
        }
    }
    Ok(next.run(request).await)
}

/// Spend an optional second budget keyed by a pseudonymous account identity.
/// The original network budget has already been applied by the outer middleware.
pub(crate) async fn admit_anonymous_identity(
    state: &AppState,
    class: RouteClass,
    identifier: &str,
) -> Result<(), GatewayError> {
    if state.rate_limiter.policies().identity_budget(class) == 0 {
        return Ok(());
    }
    if !(32..=4096).contains(&state.admission_hmac_key.len()) {
        return Err(GatewayError::ServiceUnavailable(anyhow::anyhow!(
            "anonymous identity admission key is unavailable"
        )));
    }
    let digest =
        client_identity::keyed_admission_identity(state.admission_hmac_key.as_ref(), identifier);
    let admission_started = std::time::Instant::now();
    let (outcome, lock_wait) = state
        .rate_limiter
        .admit_identity_observed(class, &digest)
        .await;
    if let Some(lock_wait) = lock_wait {
        state
            .metrics
            .observe_admission_lock_wait(class, AdmissionScope::Identity, lock_wait);
    }
    state.metrics.observe_admission_duration(
        class,
        AdmissionScope::Identity,
        admission_started.elapsed(),
    );
    state.metrics.inc_admission_decision(
        class.as_str(),
        AdmissionScope::Identity,
        outcome.as_str(),
    );
    if outcome.is_store_failure() {
        return Err(GatewayError::ServiceUnavailable(anyhow::anyhow!(
            "admission store unavailable for {class} identity requests"
        )));
    }
    if !outcome.is_admitted() {
        if state.rate_limiter.policies().mode == AdmissionMode::Shadow
            && outcome.is_shadowable_rejection()
        {
            state.metrics.inc_admission_shadow_pass(
                class.as_str(),
                AdmissionScope::Identity,
                outcome.as_str(),
            );
            return Ok(());
        }
        state
            .metrics
            .inc_rate_limit_rejection(&format!("identity:{}", class.as_str()));
        return Err(GatewayError::too_many_requests("Rate limit exceeded"));
    }
    Ok(())
}

#[doc(hidden)]
pub async fn timeout_middleware(
    State(state): State<Arc<AppState>>,
    mut request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let route = matched_route(&request).to_owned();
    let deadline = std::time::Instant::now() + state.request_timeout;
    request.extensions_mut().insert(RequestDeadline(deadline));
    // Internal calls made for this request send only the time the client still
    // has, so each owner can bound its own downstream work by it.
    match tokio::time::timeout(
        state.request_timeout,
        platform::call_deadline::scope(deadline, next.run(request)),
    )
    .await
    {
        Ok(response) => response,
        Err(_) => {
            state
                .metrics
                .inc_request_timeout(&route, downstream_for_route(&route));
            GatewayError::gateway_timeout("Request timed out").into_response()
        }
    }
}

fn downstream_for_route(route: &str) -> &'static str {
    if route.starts_with("/api/profile") {
        "profile-service"
    } else if route.starts_with("/api/friends") {
        "friend-service"
    } else if route.starts_with("/api/notifications") {
        "notification-service"
    } else if route.starts_with("/api/posts") || route.starts_with("/api/feed") {
        "post-service"
    } else if route.starts_with("/auth") {
        "auth-service"
    } else {
        "gateway"
    }
}

/// Key one client address, aggregating IPv6 to the configured prefix.
///
/// IPv6 is aggregated to a /64 by default
/// (`GATEWAY_RATE_LIMIT_V6_PREFIX_LENGTH`). A single host is routinely given a
/// whole /64, so keying on the full address let one attacker cycle through the
/// prefix and receive a fresh quota per address. The client address is the one
/// `client_identity` resolved from the peer and trusted proxies only.
///
/// A value that is not an IP address is used verbatim so a malformed forwarded
/// header still produces a stable, distinct key rather than collapsing every
/// such caller into the unknown bucket.
fn rate_limit_key_with_prefix(headers: &HeaderMap, v6_prefix: u8) -> String {
    let raw = headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("unknown");
    match raw.trim().parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(address)) => address.to_string(),
        Ok(std::net::IpAddr::V6(address)) => {
            // IPv4-mapped addresses behave as their IPv4 form, so a client cannot
            // obtain a second quota by presenting the same host in mapped form.
            match address.to_ipv4_mapped() {
                Some(mapped) => mapped.to_string(),
                None => ipv6_prefix_key(address, v6_prefix),
            }
        }
        Err(_) => raw.trim().to_owned(),
    }
}

/// Render the network part of an IPv6 address at the given prefix length.
fn ipv6_prefix_key(address: std::net::Ipv6Addr, prefix: u8) -> String {
    let bits = u128::from(address);
    // A prefix of 0 covers every address, which would make one bucket gate the
    // internet; the caller's validation keeps this within 1..=128.
    let mask = if prefix >= 128 {
        u128::MAX
    } else {
        u128::MAX << (128 - prefix)
    };
    format!("{}/{}", std::net::Ipv6Addr::from(bits & mask), prefix)
}

async fn mutation_audit_middleware(
    State(state): State<Arc<AppState>>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    audit_http_request(state, request, next, false).await
}

async fn admin_audit_middleware(
    State(state): State<Arc<AppState>>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    audit_http_request(state, request, next, true).await
}

async fn audit_http_request(
    state: Arc<AppState>,
    request: Request<axum::body::Body>,
    next: Next,
    audit_all_methods: bool,
) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    // C04 media commands are recovered by Concert. Avoid a second replay owner
    // and never persist private upload bytes in Gateway's retained journal.
    if concert_product::owns_receipt(&method, &path) {
        return next.run(request).await;
    }
    if path == "/metrics"
        || (!audit_all_methods
            && (!is_mutation_method(method.as_str()) || path.starts_with("/admin")))
    {
        return next.run(request).await;
    }
    if request
        .extensions()
        .get::<server::DurableJournalEnabled>()
        .is_some()
    {
        return request_journal::dispatch(state, request, next).await;
    }
    // Legacy router-only harnesses use this path. The executable always uses
    // with_transport_layers, which enables durable admission and completion.
    let metric_action = format!("{method} {}", matched_route(&request));
    let request_id = header_to_string(request.headers(), "x-request-id")
        .or_else(|| header_to_string(request.headers(), "x-correlation-id"));
    let ip_address = header_to_string(request.headers(), "x-forwarded-for");
    let user_agent = header_to_string(request.headers(), USER_AGENT.as_str());
    let bearer_token = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_bearer_token)
        .map(str::to_owned);
    let actor_id = match bearer_token {
        Some(token) => verify_bearer_token(&state, &token)
            .await
            .ok()
            .map(|claims| claims.subject),
        None => None,
    };
    let response = next.run(request).await;
    let outcome = if response.status().is_success() {
        LegacyAuditOutcomeDto::Success
    } else {
        LegacyAuditOutcomeDto::Failure
    };
    let action = format!("{method} {path}");
    if state
        .legacy_audit_repository
        .record(LegacyAuditCommandDto {
            event_id: Uuid::new_v4(),
            actor_id,
            action,
            resource_path: path,
            outcome,
            request_id,
            ip_address,
            user_agent,
        })
        .await
        .is_ok()
    {
        state
            .metrics
            .inc_admin_audit_event(&metric_action, outcome.as_str());
    }
    response
}

fn is_mutation_method(method: &str) -> bool {
    !matches!(method, "GET" | "HEAD" | "OPTIONS")
}

fn parse_bearer_token(value: &str) -> Option<&str> {
    let (scheme, token) = value.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    let token = token.trim();
    if token.is_empty() {
        None
    } else {
        Some(token)
    }
}

fn header_to_string(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
}

pub async fn ensure_mutation_allowed(state: &AppState, user_id: &str) -> Result<(), GatewayError> {
    if state
        .moderation_service
        .is_user_restricted(user_id)
        .await
        .map_err(map_service_error)?
    {
        return Err(GatewayError::forbidden("User is restricted"));
    }
    Ok(())
}

pub fn map_service_error(error: impl Into<anyhow::Error>) -> GatewayError {
    let error: anyhow::Error = error.into();
    use crate::application::client_error::GatewayClientError as ClientFailure;
    if let Some(failure) = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<ClientFailure>())
        .cloned()
    {
        return match failure {
            ClientFailure::InvalidArgument(message) => GatewayError::bad_request(message.as_str()),
            ClientFailure::Unauthenticated => GatewayError::unauthorized("Invalid credentials"),
            ClientFailure::PermissionDenied => GatewayError::forbidden("Forbidden"),
            ClientFailure::NotFound => GatewayError::not_found("Not found"),
            ClientFailure::AlreadyExists => GatewayError::conflict("Resource already exists"),
            ClientFailure::OperationOutcomeUnknown => GatewayError::conflict(
                "Operation outcome is unknown; reconcile using the original operation ID and key",
            ),
            ClientFailure::OperationReplayExpired => GatewayError::conflict(
                "Operation replay window expired; reconcile before issuing another command",
            ),
            ClientFailure::OperationConflict => GatewayError::conflict(
                "Operation identity or request conflicts with its durable receipt",
            ),
            ClientFailure::Aborted => GatewayError::conflict("Operation conflict; retry"),
            ClientFailure::RateLimited => GatewayError::too_many_requests("Too many requests"),
            ClientFailure::FailedPrecondition => {
                GatewayError::bad_request("Request is not valid in the current state")
            }
            ClientFailure::Busy => GatewayError::ServiceBusy,
            ClientFailure::Unavailable => GatewayError::ServiceUnavailable(error),
            ClientFailure::DeadlineExceeded => GatewayError::DownstreamTimeout(error),
            ClientFailure::Unexpected => GatewayError::Internal(error),
        };
    }
    use crate::application::moderation_repository_error::ModerationRepositoryError as Failure;
    if let Some(failure) = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<Failure>())
        .copied()
    {
        return match failure {
            Failure::ReportMissing => GatewayError::not_found("Report not found"),
            Failure::ReceiptMissing | Failure::ReceiptExpired | Failure::ReceiptConflict => GatewayError::conflict(
                "Report operation receipt is unavailable or expired; reconcile before retrying",
            ),
            Failure::TargetUnavailable => GatewayError::conflict("Moderation target is no longer available"),
            Failure::DecisionInProgress => GatewayError::conflict("Moderation decision is still being processed"),
            Failure::DecisionConflict => GatewayError::conflict("Moderation report already has a different decision"),
            Failure::ResolutionNoteRequired => GatewayError::bad_request(
                "A non-empty note is required to dismiss a report with an unavailable content target",
            ),
            Failure::ReplayConflict => GatewayError::conflict(
                "Moderation content decision is not available for the requested replay",
            ),
            Failure::Storage | Failure::Unexpected => GatewayError::Internal(error),
        };
    }
    if let Some(input) = error.chain().find_map(|source| {
        source.downcast_ref::<crate::application::moderation_input::ModerationInputError>()
    }) {
        return GatewayError::bad_request(input.message());
    }
    if error.chain().any(|source| {
        source
            .downcast_ref::<crate::application::search_cursor::InvalidSearchCursor>()
            .is_some()
    }) {
        return GatewayError::bad_request("Invalid search cursor");
    }
    if error.chain().any(|source| {
        source
            .downcast_ref::<crate::domain::moderation::ModerationResolutionNoteRequired>()
            .is_some()
    }) {
        return GatewayError::bad_request(
            "A non-empty note is required to dismiss a report with an unavailable content target",
        );
    }
    if error.chain().any(|source| {
        source
            .downcast_ref::<crate::domain::moderation::InvalidContentModerationDecisionCursor>()
            .is_some()
    }) {
        return GatewayError::bad_request(
            "Both afterCreatedAt and afterReportId are required together",
        );
    }
    if error.chain().any(|source| {
        source
            .downcast_ref::<crate::domain::profile::ProfileNotVisible>()
            .is_some()
    }) {
        return GatewayError::not_found("Not found");
    }
    if error.chain().any(|source| {
        source
            .downcast_ref::<crate::domain::moderation::ModerationTargetUnavailable>()
            .is_some()
    }) {
        return GatewayError::conflict("Moderation target is no longer available");
    }
    if error.chain().any(|source| {
        source
            .downcast_ref::<crate::domain::moderation::ModerationDecisionInProgress>()
            .is_some()
    }) {
        return GatewayError::conflict("Moderation decision is still being processed");
    }
    if error.chain().any(|source| {
        source
            .downcast_ref::<crate::domain::moderation::ModerationDecisionConflict>()
            .is_some()
    }) {
        return GatewayError::conflict("Moderation report already has a different decision");
    }
    if error.chain().any(|source| {
        source
            .downcast_ref::<crate::domain::moderation::ContentDecisionReplayConflict>()
            .is_some()
    }) {
        return GatewayError::conflict(
            "Moderation content decision is not available for the requested replay",
        );
    }
    if let Some(status) = error
        .chain()
        .find_map(|source| source.downcast_ref::<tonic::Status>())
    {
        return match status.code() {
            // An `InvalidArgument` message is a deliberate client-facing string
            // built by the owning service from a domain error, so it is forwarded
            // after bounds are enforced. Credential failures use one fixed message
            // so the response cannot disclose account state.
            tonic::Code::InvalidArgument => {
                GatewayError::bad_request(sanitize_client_message(status.message()))
            }
            tonic::Code::Unauthenticated => GatewayError::unauthorized("Invalid credentials"),
            tonic::Code::PermissionDenied => GatewayError::forbidden("Forbidden"),
            tonic::Code::NotFound => GatewayError::not_found("Not found"),
            tonic::Code::AlreadyExists => GatewayError::conflict(match status.metadata().get(platform::mutation_context::ERROR_CODE).and_then(|value| value.to_str().ok()) {
                Some("unknown") => "Operation outcome is unknown; reconcile using the original operation ID and key",
                Some("expired") => "Operation replay window expired; reconcile before issuing another command",
                Some("conflict") => "Operation identity or request conflicts with its durable receipt",
                _ => "Resource already exists",
            }),
            tonic::Code::Aborted => GatewayError::conflict("Operation conflict; retry"),
            tonic::Code::ResourceExhausted => GatewayError::too_many_requests("Too many requests"),
            tonic::Code::FailedPrecondition => {
                GatewayError::bad_request("Request is not valid in the current state")
            }
            // A fixed machine identifier from the auth service, matched exactly:
            // saturation is retryable, an outage is not something to hammer.
            tonic::Code::Unavailable if status.message() == "AUTH_BUSY" => {
                GatewayError::ServiceBusy
            }
            tonic::Code::Unavailable => GatewayError::ServiceUnavailable(error),
            tonic::Code::DeadlineExceeded => GatewayError::DownstreamTimeout(error),
            _ => GatewayError::Internal(error),
        };
    }
    // No transport status and no recognized public code: this is a defect or an
    // internal failure, not a bad request. Never infer a client error from the
    // text of an internal error, which can contain SQL, pool, or provider detail.
    GatewayError::Internal(error)
}

/// Bound and normalize a downstream client-facing message.
///
/// Only `InvalidArgument` reaches this helper, so the text is already meant for
/// a caller. Trimming, dropping control characters, and capping the length keeps
/// an unexpected upstream string from becoming an unbounded or multi-line public
/// body. Empty input falls back to a generic client message.
pub const ERROR_RESPONSE_MAX_MESSAGE_CHARS: usize = 256;

fn sanitize_client_message(message: &str) -> String {
    let cleaned: String = message
        .chars()
        .filter(|character| !character.is_control())
        .take(ERROR_RESPONSE_MAX_MESSAGE_CHARS)
        .collect();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        "Invalid request".to_owned()
    } else {
        cleaned.to_owned()
    }
}

pub fn client_ip(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned)
}

pub fn user_agent(headers: &HeaderMap) -> Option<String> {
    headers
        .get(USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned)
}

fn provider_name(provider: IdentityProvider) -> &'static str {
    match provider {
        IdentityProvider::Email => "email",
        IdentityProvider::Google => "google",
        IdentityProvider::Apple => "apple",
        IdentityProvider::X => "x",
    }
}

async fn telemetry_middleware(
    State(state): State<Arc<AppState>>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let correlation_id = request
        .headers()
        .get("x-correlation-id")
        .and_then(|value| value.to_str().ok())
        // Bounded and control-character free, because the value is caller-supplied
        // and would otherwise reach log lines verbatim.
        .and_then(platform::correlation::normalize)
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let route_key = matched_route(&request).to_owned();
    // The x-client-id header is caller-controlled and is not tied to any
    // credential, so it is not recorded as a client identity.
    //
    // Checking the value against the client registry would not fix this: it
    // would reject unknown ids and still accept any known one, so a caller could
    // attribute its traffic to whichever client it liked by naming that client.
    // Preventing impersonation requires binding the identity to a credential the
    // caller must prove it holds, which an unauthenticated header cannot do.
    //
    // Per-client attribution therefore stays absent rather than wrong, and is
    // recorded once an authenticated client identity is available.
    if let Some(value) = request.headers().get("x-client-id") {
        tracing::debug!(
            presented = value.to_str().unwrap_or("<unprintable>"),
            "ignoring the unauthenticated x-client-id header"
        );
    }
    let client_profile_id: Option<String> = None;
    let started_at = std::time::Instant::now();
    // Everything below runs inside the scope, so every internal call the Gateway
    // makes while handling this request carries the same identifier. That is what
    // lets one identifier join the edge log line to the services it reached.
    let mut response =
        platform::correlation::scope(correlation_id.clone(), next.run(request)).await;
    let latency_ms = started_at.elapsed().as_millis() as i64;
    let status = response.status();
    let outcome = if status.is_server_error() {
        "ERROR"
    } else {
        "SUCCESS"
    };
    let error_code = if status.is_server_error() {
        Some(format!("GATEWAY_{}", status.as_str()))
    } else {
        None
    };
    response.headers_mut().insert(
        "x-correlation-id",
        header_value(&correlation_id)
            .unwrap_or_else(|_| HeaderValue::from_static("invalid-correlation-id")),
    );
    // Queued, not awaited. Recording used to run to completion before the
    // response was returned, which made a database insert part of the request: a
    // slow or locked telemetry table added its latency to every response, and the
    // request timeout had to cover a write that only observes the request. The
    // background writer now owns that latency, and the request path pays for a
    // channel send.
    //
    // A full queue sheds the record instead of failing a request that already
    // succeeded, and the drop is counted, because telemetry that quietly stops
    // being recorded is indistinguishable from a system under no load.
    if !state.telemetry_service.admit(
        correlation_id,
        route_key,
        client_profile_id,
        outcome.to_owned(),
        latency_ms,
        error_code,
        HashMap::new(),
    ) {
        state.metrics.inc_telemetry_dropped();
    }
    response
}

impl From<serde_json::Error> for GatewayError {
    fn from(error: serde_json::Error) -> Self {
        Self::Internal(error.into())
    }
}
impl From<std::io::Error> for GatewayError {
    fn from(error: std::io::Error) -> Self {
        Self::Internal(error.into())
    }
}
impl From<axum::extract::multipart::MultipartError> for GatewayError {
    fn from(error: axum::extract::multipart::MultipartError) -> Self {
        Self::BadRequest(error.to_string())
    }
}
impl From<axum::http::header::InvalidHeaderValue> for GatewayError {
    fn from(error: axum::http::header::InvalidHeaderValue) -> Self {
        Self::Internal(error.into())
    }
}

pub fn append_set_cookie(headers: &mut HeaderMap, value: String) -> Result<(), GatewayError> {
    headers.append(SET_COOKIE, header_value(&value)?);
    Ok(())
}

#[cfg(test)]
mod correlation_tests {
    use super::*;
    use proptest::prelude::*;

    fn headers_with_client(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", value.parse().expect("header value"));
        headers
    }

    #[test]
    fn ipv4_clients_are_keyed_individually() {
        assert_eq!(
            rate_limit_key_with_prefix(&headers_with_client("203.0.113.7"), 64),
            "203.0.113.7"
        );
    }

    #[test]
    fn ipv6_clients_sharing_a_prefix_share_one_key() {
        // One host is routinely given a whole /64, so keying on the full address
        // would hand an attacker a fresh quota for every address it tried.
        let first = rate_limit_key_with_prefix(&headers_with_client("2001:db8::1"), 64);
        let second = rate_limit_key_with_prefix(&headers_with_client("2001:db8::ffff"), 64);
        assert_eq!(first, second);
        assert_eq!(first, "2001:db8::/64");

        // A different prefix must remain a different key.
        let other = rate_limit_key_with_prefix(&headers_with_client("2001:db9::1"), 64);
        assert_ne!(first, other);
    }

    #[test]
    fn ipv4_mapped_clients_cannot_obtain_a_second_quota() {
        // The same host in mapped form must not look like a different client.
        assert_eq!(
            rate_limit_key_with_prefix(&headers_with_client("::ffff:203.0.113.7"), 64),
            rate_limit_key_with_prefix(&headers_with_client("203.0.113.7"), 64)
        );
    }

    #[test]
    fn a_narrower_prefix_separates_addresses_that_a_wider_one_merges() {
        let wide = rate_limit_key_with_prefix(&headers_with_client("2001:db8::1"), 64);
        let narrow = rate_limit_key_with_prefix(&headers_with_client("2001:db8::1"), 128);
        assert_ne!(wide, narrow);
        assert_eq!(narrow, "2001:db8::1/128");
    }

    #[test]
    fn a_malformed_client_value_keeps_a_distinct_stable_key() {
        // A forwarded value that is not an address must not collapse into the
        // unknown bucket, or one malformed client could exhaust it for everyone.
        let key = rate_limit_key_with_prefix(&headers_with_client("not-an-address"), 64);
        assert_eq!(key, "not-an-address");
    }

    #[test]
    fn query_parameters_are_decoded_and_not_truncated_by_a_bare_flag() {
        let uri: http::Uri = "/callback?flag&state=a%20b&code=xyz".parse().expect("uri");
        // A pair without '=' must not abandon the rest of the query, which is
        // exactly what the previous early return did.
        assert_eq!(extract_query_param(&uri, "state"), Some("a b".to_owned()));
        assert_eq!(extract_query_param(&uri, "code"), Some("xyz".to_owned()));
        assert_eq!(extract_query_param(&uri, "flag"), Some(String::new()));
        assert_eq!(extract_query_param(&uri, "missing"), None);
    }

    #[test]
    fn percent_escapes_decode_and_plus_becomes_a_space() {
        let uri: http::Uri = "/cb?redirect=https%3A%2F%2Fexample.com%2Fa+b"
            .parse()
            .expect("uri");
        assert_eq!(
            extract_query_param(&uri, "redirect"),
            Some("https://example.com/a b".to_owned())
        );
    }

    #[test]
    fn query_parameter_parser_accepts_the_limit_and_rejects_one_byte_over() {
        let prefix = "state=";
        let at_limit_value = "a".repeat(MAX_QUERY_STRING_BYTES - prefix.len());
        let at_limit_uri = format!("/callback?{prefix}{at_limit_value}")
            .parse::<http::Uri>()
            .expect("URI remains syntactically valid");
        assert_eq!(
            extract_query_param(&at_limit_uri, "state").as_deref(),
            Some(at_limit_value.as_str())
        );

        let over_limit_uri = format!("/callback?{prefix}{at_limit_value}a")
            .parse::<http::Uri>()
            .expect("URI remains syntactically valid");
        assert_eq!(extract_query_param(&over_limit_uri, "state"), None);
    }

    proptest! {
        #![proptest_config(ProptestConfig {
            failure_persistence: Some(Box::new(proptest::test_runner::FileFailurePersistence::WithSource("proptest-regressions"))),
            ..ProptestConfig::default()
        })]

        #[test]
        fn percent_decoder_is_total_with_bounded_utf8_expansion(input in prop::collection::vec(any::<u8>(), 0..=2048)) {
            let candidate = String::from_utf8_lossy(&input);
            let decoded = percent_decode(&candidate);
            prop_assert!(decoded.len() <= candidate.len().saturating_mul(3));
            prop_assert_eq!(percent_decode(&candidate), decoded);
        }

        #[test]
        fn valid_percent_byte_sequences_decode_to_the_standard_lossy_utf8(input in prop::collection::vec(any::<u8>(), 0..=1024)) {
            let encoded = input.iter().map(|byte| format!("%{byte:02X}")).collect::<String>();
            let expected = String::from_utf8_lossy(&input).into_owned();
            prop_assert_eq!(percent_decode(&encoded), expected);
        }
    }

    #[tokio::test]
    async fn an_error_response_reports_the_current_correlation_id() {
        // Inside the scope the error body must reuse the request identifier so a
        // client report can be joined to the server logs.
        let response = CORRELATION_ID
            .scope("corr-123".to_owned(), async {
                GatewayError::bad_request("nope").into_response()
            })
            .await;
        // Read the header before consuming the body.
        let header = response.headers()["x-correlation-id"]
            .to_str()
            .expect("utf8")
            .to_owned();
        let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
            .await
            .expect("body");
        let json: serde_json::Value = serde_json::from_slice(&bytes).expect("json body");
        assert_eq!(json["traceId"], "corr-123");
        assert_eq!(json["code"], "GATEWAY_400");
        // The header must carry the same value as the body, which is the whole
        // point: a client reporting an error can find the matching log line.
        assert_eq!(header, "corr-123");
    }

    #[tokio::test]
    async fn an_error_response_outside_any_request_scope_still_reports_one_identifier() {
        // A route mounted without the transport layers still returns a usable
        // identifier, and the body and header must not disagree.
        let response = GatewayError::not_found("missing").into_response();
        let header = response.headers()["x-correlation-id"]
            .to_str()
            .expect("utf8")
            .to_owned();
        let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
            .await
            .expect("body");
        let json: serde_json::Value = serde_json::from_slice(&bytes).expect("json body");
        assert_eq!(json["traceId"].as_str(), Some(header.as_str()));
    }
}

#[cfg(test)]
mod identity_debug_tests {
    use super::{CurrentUser, VerifiedAccessToken};

    #[test]
    fn verified_access_token_debug_redacts_subject_roles_and_session() {
        let claims = VerifiedAccessToken {
            subject: "private-user-subject".into(),
            roles: vec!["ROLE_ADMIN_PRIVATE".into()],
            session_id: Some("private-session-family".into()),
        };

        let rendered = format!("{claims:?}");

        for sensitive in [
            "private-user-subject",
            "ROLE_ADMIN_PRIVATE",
            "private-session-family",
        ] {
            assert!(
                !rendered.contains(sensitive),
                "Debug leaked {sensitive}: {rendered}"
            );
        }
    }

    #[test]
    fn current_user_debug_redacts_identity_and_authorization_claims() {
        let current_user = CurrentUser {
            user_id: "private-user-subject".into(),
            roles: vec!["ROLE_ADMIN_PRIVATE".into()],
            session_id: Some("private-session-family".into()),
        };

        let rendered = format!("{current_user:?}");

        for sensitive in [
            "private-user-subject",
            "ROLE_ADMIN_PRIVATE",
            "private-session-family",
        ] {
            assert!(
                !rendered.contains(sensitive),
                "Debug leaked {sensitive}: {rendered}"
            );
        }
    }
}

#[cfg(test)]
mod timeout_metric_label_tests {
    use super::downstream_for_route;

    #[test]
    fn request_timeout_downstream_labels_cover_gateway_service_route_families() {
        for (route, expected) in [
            ("/auth/login", "auth-service"),
            ("/api/profile/{user_id}", "profile-service"),
            ("/api/friends/following", "friend-service"),
            ("/api/notifications", "notification-service"),
            ("/api/posts/{post_id}", "post-service"),
            ("/api/feed", "post-service"),
        ] {
            assert_eq!(downstream_for_route(route), expected, "route {route}");
        }

        // Non-service routes keep a stable fallback label rather than putting
        // arbitrary request paths into the Prometheus label set.
        assert_eq!(downstream_for_route("/metrics"), "gateway");
    }
}
