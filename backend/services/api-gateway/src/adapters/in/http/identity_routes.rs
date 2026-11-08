//! The identity verification callback route.
//!
//! The four identity operations are split across two transports, because one of
//! them is not like the others:
//!
//! | operation | transport | why |
//! | --- | --- | --- |
//! | `getOwnIdentityVerification` | profile product | authenticated owner read |
//! | `startIdentityVerification` | profile product | authenticated owner mutation |
//! | `disconnectIdentityVerification` | profile product | authenticated owner mutation |
//! | `completeIdentityVerification` | **this module** | anonymous, answers `303` |
//!
//! The callback arrives from the provider, in a top-level navigation, with no
//! session cookie that can be trusted and no `X-MCR-Intent` header (a browser
//! navigation cannot set one). It therefore cannot pass the browser boundary or
//! the profile product's envelope, and it must not be made to: relaxing either
//! for every route would be the actual vulnerability. Instead this route is
//! registered **outside** both, and carries its own, narrower discipline:
//!
//! * it is `GET` only. The provider redirects with a `GET`; a `POST` here would
//!   be a cross-site form submission, which is exactly what must not be possible;
//! * it accepts exactly three query parameters and refuses anything else, so a
//!   caller cannot smuggle a return URL or a user identity through it;
//! * it is rate limited per state and per client address bucket, before the
//!   profile service is touched, so a flood of guessed states cannot be used as
//!   an oracle or as a load amplifier;
//! * it answers `303 See Other` with a `Location` that is a constant path from
//!   the start allowlist plus one of eight constant result markers. The reply
//!   from the profile service is validated against both closed sets, so a
//!   compromised producer cannot inject a redirect;
//! * every response is `no-store` and carries no claim value, no provider code
//!   and no correlation of one state to another.
use super::{correlation_id_for_response, GatewayError};
use crate::config::AppState;
use crate::domain::identity_verification::{CallbackReply, CallbackReplyError};
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{header, HeaderName, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use chrono::{SecondsFormat, Utc};
use serde_json::json;
use std::sync::Arc;


/// The one callback path. `{provider}` is a closed enum with the single value
/// `naver`; any other provider is not a route at all.
pub const PATH: &str = "/api/identity/naver/callback";

/// The query parameters the provider may send. Anything else is refused.
const ALLOWED_QUERY: [&str; 3] = ["code", "state", "error"];

/// Longest accepted `state`. The contract's `IdentityCallbackQuery.state` bound.
const MAX_STATE: usize = 512;
/// Longest accepted `code`. The contract's `IdentityCallbackQuery.code` bound.
const MAX_CODE: usize = 512;
/// Longest accepted provider `error`. A provider error code is short.
const MAX_PROVIDER_ERROR: usize = 128;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route(PATH, get(callback))
        .with_state(state)
}

/// Whether a path is the callback. Used by the composition to keep this route
/// out of the browser boundary and the profile product's response protection.
pub fn is_callback(path: &str) -> bool {
    path == PATH
}

/// The parsed provider query.
#[derive(Debug, PartialEq, Eq)]
pub struct CallbackQuery {
    pub state: String,
    pub code: Option<String>,
    pub provider_error: Option<String>,
}

/// Parse the provider's query string.
///
/// Rules, all of them refusals rather than coercions:
///
/// * `state` is required and bounded;
/// * every parameter may appear once. A repeated parameter is ambiguous, and an
///   ambiguous state is a request-smuggling primitive;
/// * only the three documented names are accepted;
/// * `code` and `error` are mutually exclusive: the provider sends one or the
///   other, and a request carrying both is not a provider redirect.
pub fn parse_query(raw: Option<&str>) -> Result<CallbackQuery, &'static str> {
    let mut state: Option<String> = None;
    let mut code: Option<String> = None;
    let mut provider_error: Option<String> = None;
    for (name, value) in parse_form(raw.unwrap_or_default()) {
        if !ALLOWED_QUERY.contains(&name.as_ref()) {
            return Err("Unexpected callback parameter");
        }
        let target = match name.as_ref() {
            "state" => &mut state,
            "code" => &mut code,
            _ => &mut provider_error,
        };
        if target.replace(value).is_some() {
            return Err("Repeated callback parameter");
        }
    }
    let state = state.ok_or("Missing callback state")?;
    if state.is_empty() || state.len() > MAX_STATE || !state.bytes().all(is_state_byte) {
        return Err("Invalid callback state");
    }
    if code
        .as_deref()
        .is_some_and(|code| code.is_empty() || code.len() > MAX_CODE)
    {
        return Err("Invalid callback code");
    }
    if provider_error
        .as_deref()
        .is_some_and(|error| error.is_empty() || error.len() > MAX_PROVIDER_ERROR)
    {
        return Err("Invalid callback error");
    }
    if code.is_some() && provider_error.is_some() {
        return Err("Conflicting callback parameters");
    }
    Ok(CallbackQuery {
        state,
        code,
        provider_error,
    })
}

/// Percent-decode a query string into `(name, value)` pairs.
///
/// `+` is decoded as a space, which is what a browser form encoding means and
/// what the provider's own parameters never rely on. A malformed escape is
/// preserved literally rather than dropped, so a value that cannot be decoded
/// then fails the grammar check instead of silently becoming a different value.
fn parse_form(raw: &str) -> Vec<(String, String)> {
    raw.split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| match pair.split_once('=') {
            Some((name, value)) => (percent_decode(name), percent_decode(value)),
            None => (percent_decode(pair), String::new()),
        })
        .collect()
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                decoded.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                match u8::from_str_radix(&value[index + 1..index + 3], 16) {
                    Ok(byte) => {
                        decoded.push(byte);
                        index += 3;
                    }
                    Err(_) => {
                        decoded.push(bytes[index]);
                        index += 1;
                    }
                }
            }
            byte => {
                decoded.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

/// The `state` grammar: the contract's `IdentityCallbackQuery` admits the
/// unreserved URL characters, and this service issues unpadded base64url.
fn is_state_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~')
}

/// The callback handler.
async fn callback(State(state): State<Arc<AppState>>, request: Request<Body>) -> Response {
    // The method is `GET` by routing; this is the belt-and-braces check that a
    // future `any` registration cannot turn this into a form target.
    if request.method() != Method::GET {
        return closed(StatusCode::METHOD_NOT_ALLOWED, "METHOD_NOT_ALLOWED");
    }
    let query = match parse_query(request.uri().query()) {
        Ok(query) => query,
        Err(_) => {
            // A malformed callback is a `400` that reveals nothing: not which
            // parameter was wrong, not whether the state exists.
            return closed(StatusCode::BAD_REQUEST, "INVALID_INPUT");
        }
    };
    if let Err(error) = admit(&state, &query.state).await {
        return error.into_response();
    }
    let Some(client) = state.identity_callback_client.as_ref() else {
        return closed(StatusCode::SERVICE_UNAVAILABLE, "DEPENDENCY_UNAVAILABLE");
    };
    let reply = match client
        .complete(
            &query.state,
            query.code.as_deref(),
            query.provider_error.as_deref(),
        )
        .await
    {
        Ok(reply) => reply,
        Err(CallbackReplyError::InvalidState) => {
            return closed(StatusCode::BAD_REQUEST, "INVALID_INPUT")
        }
        Err(CallbackReplyError::Unavailable) => {
            return closed(StatusCode::SERVICE_UNAVAILABLE, "DEPENDENCY_UNAVAILABLE")
        }
    };
    redirect(&reply)
}

/// A `303` to the validated return path.
///
/// `303` rather than `302`: the browser must follow with a `GET`, so the
/// landing page is a read and can never re-submit the work the user was doing
/// before they started verification.
fn redirect(reply: &CallbackReply) -> Response {
    let location = reply.location();
    let mut response = Response::new(Body::empty());
    *response.status_mut() = StatusCode::SEE_OTHER;
    response.headers_mut().insert(
        header::LOCATION,
        HeaderValue::from_str(&location).expect("closed location grammar"),
    );
    protect(&mut response);
    response
}

/// A closed error answer.
///
/// The body is the gateway's standard envelope with a fixed reason. It never
/// names the state, the provider's error code, or which of "unknown", "expired"
/// and "already used" applied.
fn closed(status: StatusCode, reason: &'static str) -> Response {
    let correlation = correlation_id_for_response();
    let mut response = (
        status,
        axum::Json(json!({
            "code": format!("GATEWAY_{}", status.as_str()),
            "message": "Identity verification could not be completed",
            "traceId": correlation,
            "timestamp": Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
            "details": {"reason": reason},
        })),
    )
        .into_response();
    protect(&mut response);
    response
}

/// Every callback response is uncacheable and carries no referrer.
fn protect(response: &mut Response) {
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    headers.insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    // The callback URL contains a one-use state. A `Referer` on the landing page
    // would hand that state to whatever the landing page loads next.
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers.remove(header::ETAG);
    headers.remove(header::LAST_MODIFIED);
}

/// The callback's own admission, before any profile-service work.
///
/// Two independent budgets, both required:
///
/// * **per state**: a state may be presented at most once per window from this
///   gateway. A second presentation is refused locally, so guessing states costs
///   the attacker a full round trip per attempt and never reaches the store;
/// * **per client address bucket**: the ordinary admission class for this route,
///   so one address cannot use the callback as a load amplifier.
///
/// The state is hashed with the admission key before it becomes a budget key:
/// the raw state must not appear in a metrics label or an admission store row.
pub async fn admit(state: &AppState, state_token: &str) -> Result<(), GatewayError> {
    if state.admission_hmac_key.len() < 32 {
        return Err(GatewayError::ServiceUnavailable(anyhow::anyhow!(
            "callback admission key is unavailable"
        )));
    }
    let digest =
        super::client_identity::keyed_admission_identity(state.admission_hmac_key.as_ref(), state_token);
    let (outcome, _lock_wait) = state
        .rate_limiter
        .admit_identity_observed(
            crate::domain::admission::RouteClass::VerificationCallback,
            &digest,
        )
        .await;
    if outcome.is_store_failure() {
        return Err(GatewayError::ServiceUnavailable(anyhow::anyhow!(
            "callback admission store unavailable"
        )));
    }
    if !outcome.is_admitted() {
        return Err(GatewayError::rate_limited(
            "Rate limit exceeded",
            state.rate_limiter.window_seconds(),
        ));
    }
    // The per-address bucket is the ordinary network budget for this class,
    // spent by the global admission layer before this handler runs. Nothing here
    // can widen it.
    Ok(())
}
