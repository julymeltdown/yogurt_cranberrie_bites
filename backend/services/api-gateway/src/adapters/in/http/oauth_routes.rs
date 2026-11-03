use super::{
    append_set_cookie, build_oauth_state_cookie, build_refresh_cookie, clear_oauth_state_cookie,
    client_ip, extract_query_param, map_service_error, parse_provider, resolve_cookie,
    strip_refresh_token, user_agent, CurrentUser, GatewayError,
};
use crate::application::dto::auth_dto::OAuthLinkRequest;
use crate::config::AppState;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
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

#[derive(Deserialize)]
struct OAuthCallbackQuery {
    code: String,
    state: String,
}

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
        .route("/auth/oauth/:provider/link", post(link))
        .with_state(state)
}

async fn authorize(
    State(state): State<Arc<AppState>>,
    Path(provider): Path<String>,
) -> Result<Response, GatewayError> {
    let provider = parse_provider(&provider)?;
    let authorization = state
        .auth_service
        .authorize(provider)
        .await
        .map_err(map_service_error)?;
    let location: http::Uri = authorization
        .authorization_uri
        .parse()
        .map_err(|error: http::uri::InvalidUri| map_service_error(error))?;
    let oauth_state = extract_query_param(&location, "state")
        .ok_or_else(|| GatewayError::bad_request("OAuth state missing from authorization URI"))?;
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::LOCATION,
        super::header_value(&location.to_string())?,
    );
    append_set_cookie(
        &mut headers,
        build_oauth_state_cookie(&state, provider, &oauth_state),
    )?;
    Ok((StatusCode::FOUND, headers).into_response())
}

async fn callback(
    State(state): State<Arc<AppState>>,
    Path(provider): Path<String>,
    Query(query): Query<OAuthCallbackQuery>,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    let provider = parse_provider(&provider)?;
    verify_oauth_state(&state, provider, &query.state, &headers)?;
    let response = state
        .auth_service
        .oauth_callback(
            provider,
            query.code,
            query.state,
            client_ip(&headers),
            user_agent(&headers),
        )
        .await
        .map_err(map_service_error)?;
    let mut response_headers = HeaderMap::new();
    append_set_cookie(
        &mut response_headers,
        build_refresh_cookie(&state, &response.refresh_token),
    )?;
    append_set_cookie(
        &mut response_headers,
        clear_oauth_state_cookie(&state, provider),
    )?;
    Ok((
        StatusCode::OK,
        response_headers,
        Json(strip_refresh_token(response)),
    )
        .into_response())
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
    _state: &AppState,
    provider: crate::application::dto::facade_dto::IdentityProviderDto,
    state_value: &str,
    headers: &HeaderMap,
) -> Result<(), GatewayError> {
    if state_value.trim().is_empty() {
        return Err(GatewayError::bad_request("OAuth state is required"));
    }
    let cookie_name = super::oauth_state_cookie_name(provider);
    let Some(cookie_value) = resolve_cookie(headers, &cookie_name) else {
        return Err(GatewayError::bad_request("Invalid OAuth state"));
    };
    if cookie_value != state_value {
        return Err(GatewayError::bad_request("Invalid OAuth state"));
    }
    Ok(())
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
