use super::{
    append_set_cookie, build_bound_oauth_state_cookie, build_refresh_cookie, clear_oauth_state_cookie,
    client_ip, extract_query_param, map_service_error, parse_provider,
    user_agent, CurrentUser, GatewayError,
};
use crate::application::dto::auth_dto::OAuthLinkRequest;
use crate::config::AppState;
use axum::extract::{Path, Query, RawQuery, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
#[cfg(test)]
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;

mod form_post;

/// Protect even errors returned by outer admission, deadline or transport
/// middleware. The executable installs this outside its transport layers too.
pub(super) async fn protect_form_post_response(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let path: Vec<_> = request.uri().path().split('/').collect();
    let protect = request.method() == axum::http::Method::POST
        && matches!(
            path.as_slice(),
            ["", "auth", "oauth", _, "callback"] | ["", "auth", "oauth", _, "callback", "complete"]
        );
    let mut response = next.run(request).await;
    if protect {
        form_post::protect_response(&mut response);
    }
    response
}

#[cfg(test)]
#[derive(Deserialize)]
struct OAuthCallbackQuery {
    code: String,
    state: String,
}

#[cfg(test)]
impl std::fmt::Debug for OAuthCallbackQuery {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OAuthCallbackQuery")
            .field("code", &"[REDACTED]")
            .field("state", &"[REDACTED]")
            .finish()
    }
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/auth/oauth/:provider/authorize", get(authorize))
        .route(
            "/auth/oauth/:provider/callback",
            get(callback).post(form_post::receive),
        )
        .route(
            "/auth/oauth/:provider/callback/complete",
            post(form_post::complete),
        )
        // Account linking has a separate, still-unqualified binding contract.
        .with_state(state)
}

async fn authorize(
    State(state): State<Arc<AppState>>,
    Path(provider): Path<String>,
    RawQuery(raw): RawQuery,
    Query(params): Query<Vec<(String, String)>>,
) -> Result<Response, GatewayError> {
    // An unknown provider is an input error, while a known provider whose
    // transport is not explicitly enabled stays indistinguishable from a
    // missing route. Production leaves every provider disabled, so both the
    // composed router and this gate hide it.
    let provider = parse_provider(&provider)?;
    require_transport(&state, super::provider_name(provider))?;
    if raw.as_ref().is_some_and(|value| value.len() > 8 * 1024) || params.len() > 8 {
        return Err(GatewayError::bad_request("OAuth query exceeds its limit"));
    }
    form_post::validate_encoding(raw.as_deref().unwrap_or_default().as_bytes())?;
    if params.iter().any(|(key, _)| key != "returnTo") {
        return Err(GatewayError::bad_request("Unknown OAuth authorization field"));
    }
    let target = super::oauth_return_target(if params.len() == 1 {
        Some(params[0].1.as_str())
    } else { None });
    let authorization = state
        .auth_service
        .authorize(provider)
        .await
        .map_err(map_service_error)?;
    let location: http::Uri = authorization
        .authorization_uri
        .parse()
        .map_err(|error: http::uri::InvalidUri| map_service_error(error))?;
    if location.authority().is_none()
        || !matches!(location.scheme_str(), Some("https" | "http"))
        || location.authority().is_some_and(|value| value.as_str().contains('@'))
        || location.query().unwrap_or_default().split('&').filter(|value| value.starts_with("state=")).count() != 1
    { return Err(GatewayError::bad_request("Invalid provider authorization URI")); }
    let oauth_state = extract_query_param(&location, "state")
        .ok_or_else(|| GatewayError::bad_request("OAuth state missing from authorization URI"))?;
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::LOCATION,
        super::header_value(&location.to_string())?,
    );
    append_set_cookie(
        &mut headers,
        build_bound_oauth_state_cookie(&state, provider, &oauth_state, target)?,
    )?;
    Ok((StatusCode::FOUND, headers).into_response())
}

async fn callback(
    State(state): State<Arc<AppState>>,
    Path(provider): Path<String>,
    RawQuery(raw): RawQuery,
    Query(params): Query<Vec<(String, String)>>,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    require_transport(&state, &provider)?;
    if raw.as_ref().is_some_and(|value| value.len() > 8 * 1024) || params.len() > 8 {
        return Err(GatewayError::bad_request("OAuth query exceeds its limit"));
    }
    form_post::validate_encoding(raw.as_deref().unwrap_or_default().as_bytes())?;
    let mut query = HashMap::new();
    for (key, value) in params {
        if !matches!(key.as_str(), "state" | "code" | "error" | "error_description" | "error_uri" | "returnTo")
            || query.insert(key, value).is_some()
        { return Err(GatewayError::bad_request("Unknown or duplicate OAuth callback field")); }
    }
    let provider = parse_provider(&provider)?;
    let query_state = query.remove("state").unwrap_or_default();
    let target = super::verify_bound_oauth_state(&state, provider, &query_state, &headers)?;
    if let Some(error) = query.remove("error") {
        if query.contains_key("code") || error.is_empty() {
            return Err(GatewayError::bad_request("Ambiguous OAuth callback"));
        }
        return browser_handoff(&state, provider, &target,
            if error == "access_denied" { "denied" } else { "error" }, None);
    }
    let code = query.remove("code").unwrap_or_default();
    if code.trim().is_empty() || code.len() > 4096 || code.chars().any(char::is_control)
        || query.contains_key("error_description") || query.contains_key("error_uri")
    { return Err(GatewayError::bad_request("Invalid OAuth callback code")); }
    // callback returnTo is deliberately unused: only signed state owns target.
    let response = state
        .auth_service
        .oauth_callback(
            provider,
            code,
            query_state,
            client_ip(&headers),
            user_agent(&headers),
        )
        .await;
    match response {
        Ok(response) => browser_handoff(&state, provider, &target, "success", Some(&response.refresh_token)),
        Err(_) => browser_handoff(&state, provider, &target, "error", None),
    }
}

pub(super) fn require_transport(state: &AppState, provider: &str) -> Result<(), GatewayError> {
    if !state.oauth_exposure.transport_enabled(provider) {
        return Err(GatewayError::not_found("OAuth provider unavailable"));
    }
    Ok(())
}

pub(super) fn browser_handoff(
    state: &AppState,
    provider: crate::application::dto::facade_dto::IdentityProviderDto,
    target: &str, status: &str, refresh: Option<&str>,
) -> Result<Response, GatewayError> {
    if !matches!(status, "success" | "denied" | "error")
        || super::oauth_return_target(Some(target)) != target
        || (status == "success") != refresh.is_some()
    { return Err(GatewayError::Internal(anyhow::anyhow!("Invalid OAuth handoff"))); }
    let mut headers = HeaderMap::new();
    if let Some(refresh) = refresh {
        append_set_cookie(&mut headers, build_refresh_cookie(state, refresh))?;
    }
    append_set_cookie(&mut headers, super::clear_bound_oauth_state_cookie(provider))?;
    // The fixed finite target set needs no URL normalization or arbitrary
    // escaping. Encode slashes for a literal, credential-free query value.
    let location = format!("/auth/callback?status={status}&returnTo={}", target.replace('/', "%2F"));
    headers.insert(axum::http::header::LOCATION, super::header_value(&location)?);
    let mut response = (StatusCode::SEE_OTHER, headers).into_response();
    form_post::protect_response(&mut response);
    Ok(response)
}

async fn link(
    State(state): State<Arc<AppState>>,
    Path(provider): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    current_user: CurrentUser,
    headers: HeaderMap,
    Json(request): Json<OAuthLinkRequest>,
) -> Result<Response, GatewayError> {
    reject_query_access_token(&params)?;
    let provider = parse_provider(&provider)?;
    verify_oauth_state(&state, provider, &request.state, &headers)?;
    let response = state
        .auth_service
        .link_provider(provider, request, current_user.user_id)
        .await
        .map_err(map_service_error)?;
    let mut response_headers = HeaderMap::new();
    append_set_cookie(
        &mut response_headers,
        clear_oauth_state_cookie(&state, provider),
    )?;
    Ok((StatusCode::OK, response_headers, Json(response)).into_response())
}

fn verify_oauth_state(
    state: &AppState,
    provider: crate::application::dto::facade_dto::IdentityProviderDto,
    state_value: &str,
    headers: &HeaderMap,
) -> Result<(), GatewayError> {
    super::verify_bound_oauth_state(state, provider, state_value, headers).map(|_| ())
}

fn reject_query_access_token(params: &HashMap<String, String>) -> Result<(), GatewayError> {
    if params
        .keys()
        .any(|key| key.eq_ignore_ascii_case("access_token"))
    {
        return Err(GatewayError::bad_request(
            "Authorization token must be sent in the Authorization header",
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oauth_callback_query_debug_redacts_code_and_state() {
        let query = OAuthCallbackQuery {
            code: "oauth-code-secret".into(),
            state: "oauth-state-secret".into(),
        };

        let rendered = format!("{query:?}");

        assert!(!rendered.contains("oauth-code-secret"), "{rendered}");
        assert!(!rendered.contains("oauth-state-secret"), "{rendered}");
        assert!(rendered.contains("[REDACTED]"));
    }

    #[test]
    fn reject_query_access_token_blocks_url_token_transport() {
        let params = HashMap::from([("access_token".to_owned(), "leaky-token".to_owned())]);

        let error = reject_query_access_token(&params).expect_err("query token must be rejected");

        assert!(matches!(error, GatewayError::BadRequest(_)));
        assert_eq!(
            error.to_string(),
            "Authorization token must be sent in the Authorization header"
        );
    }

    #[test]
    fn reject_query_access_token_is_case_insensitive() {
        let params = HashMap::from([("ACCESS_TOKEN".to_owned(), "leaky-token".to_owned())]);

        assert!(reject_query_access_token(&params).is_err());
    }

    #[test]
    fn reject_query_access_token_allows_unrelated_query_parameters() {
        let params = HashMap::from([("locale".to_owned(), "en".to_owned())]);

        reject_query_access_token(&params).expect("unrelated query parameters should be ignored");
    }
}
