use crate::application::dto::auth_dto::OAuthLinkRequestDto;
use crate::config::AppState;
use crate::domain::user::IdentityProvider;
use axum::extract::{Form, Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use std::collections::HashMap;
use std::sync::Arc;

use super::{map_application_error as map_bad_request, AppError};

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/auth/oauth/:provider/authorize", get(authorize))
        .route(
            "/auth/oauth/:provider/callback",
            get(callback).post(callback_form_post),
        )
        .route("/auth/oauth/:provider/link", post(link))
        .with_state(state)
}

async fn authorize(
    State(state): State<Arc<AppState>>,
    Path(provider): Path<String>,
) -> Result<(StatusCode, HeaderMap), AppError> {
    let response = state
        .oauth_login
        .authorize(parse_provider(&provider)?)
        .await
        .map_err(map_bad_request)?;
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::LOCATION,
        HeaderValue::from_str(&response.authorization_uri)
            .map_err(|error| AppError::Internal(error.into()))?,
    );
    Ok((StatusCode::FOUND, headers))
}

async fn callback(
    State(state): State<Arc<AppState>>,
    Path(provider): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    complete_callback(state, provider, params, headers).await
}

/// Accept the cross-site form post a provider may send instead of a redirect.
///
/// Apple's `response_mode=form_post` makes the identity provider POST the
/// authorization result to this URL as `application/x-www-form-urlencoded`, not
/// append it as query parameters. The route previously accepted only `GET`, so
/// that POST was answered with a method-not-allowed rejection and Apple sign-in
/// could never complete. The parameters are read from the form body here and then
/// handled identically to the redirect case, so state binding and the one-time
/// authorization code are enforced the same way in both paths.
async fn callback_form_post(
    State(state): State<Arc<AppState>>,
    Path(provider): Path<String>,
    headers: HeaderMap,
    Form(params): Form<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, AppError> {
    complete_callback(state, provider, params, headers).await
}

/// Exchange an authorization result for a session, however it arrived.
async fn complete_callback(
    state: Arc<AppState>,
    provider: String,
    params: HashMap<String, String>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let response = state
        .oauth_login
        .oauth_callback(
            parse_provider(&provider)?,
            params.get("code").map(String::as_str).unwrap_or_default(),
            params.get("state").map(String::as_str).unwrap_or_default(),
            request_id(&headers),
            client_ip(&headers),
            header_to_string(&headers, "user-agent"),
        )
        .await
        .map_err(map_bad_request)?;
    Ok(Json(serde_json::to_value(response)?))
}

async fn link(
    State(state): State<Arc<AppState>>,
    Path(provider): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    Json(request): Json<OAuthLinkRequestDto>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    reject_query_access_token(&params)?;
    let token = resolve_access_token(&headers)
        .ok_or_else(|| AppError::BadRequest("Authorization token required".into()))?;
    let user_id = state
        .oauth_link
        .decode_access_token(&token)
        .map_err(map_bad_request)?;
    let response = state
        .oauth_link
        .link_provider(
            user_id,
            parse_provider(&provider)?,
            &request.code,
            &request.state,
            request_id(&headers),
            client_ip(&headers),
            header_to_string(&headers, "user-agent"),
        )
        .await
        .map_err(map_bad_request)?;
    Ok((StatusCode::OK, Json(serde_json::to_value(response)?)))
}

fn parse_provider(value: &str) -> Result<IdentityProvider, AppError> {
    match value.trim().to_ascii_uppercase().as_str() {
        "GOOGLE" => Ok(IdentityProvider::Google),
        "APPLE" => Ok(IdentityProvider::Apple),
        "X" | "TWITTER" => Ok(IdentityProvider::X),
        "EMAIL" => Err(AppError::BadRequest("OAuth provider required".into())),
        _ => Err(AppError::BadRequest("Identity provider is required".into())),
    }
}

fn reject_query_access_token(params: &HashMap<String, String>) -> Result<(), AppError> {
    if params
        .keys()
        .any(|key| key.eq_ignore_ascii_case("access_token"))
    {
        return Err(AppError::BadRequest(
            "Authorization token must be sent in the Authorization header".into(),
        ));
    }

    Ok(())
}

fn resolve_access_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(parse_bearer_token)
        .map(ToOwned::to_owned)
}

fn parse_bearer_token(value: &str) -> Option<&str> {
    let (scheme, token) = value.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    let token = token.trim();
    if token.is_empty() {
        return None;
    }
    Some(token)
}

fn request_id(headers: &HeaderMap) -> Option<String> {
    header_to_string(headers, "x-request-id")
        .or_else(|| header_to_string(headers, "x-correlation-id"))
}

fn client_ip(headers: &HeaderMap) -> Option<String> {
    header_to_string(headers, "x-forwarded-for")
        .and_then(|value| value.split(',').next().map(|value| value.trim().to_owned()))
        .filter(|value| !value.is_empty())
}

fn header_to_string(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_access_token_reads_bearer_authorization_header() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_static("Bearer auth-token"),
        );

        assert_eq!(
            resolve_access_token(&headers).as_deref(),
            Some("auth-token")
        );
    }

    #[test]
    fn resolve_access_token_rejects_empty_bearer_header() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", HeaderValue::from_static("Bearer "));

        assert_eq!(resolve_access_token(&headers), None);
    }

    #[test]
    fn resolve_access_token_accepts_case_insensitive_scheme_and_trims_token() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_static("bEaReR auth-token  "),
        );

        assert_eq!(
            resolve_access_token(&headers).as_deref(),
            Some("auth-token")
        );
    }

    #[test]
    fn reject_query_access_token_blocks_url_token_transport() {
        let params = HashMap::from([("access_token".to_owned(), "leaky-token".to_owned())]);

        let error = reject_query_access_token(&params).expect_err("query token must be rejected");

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
