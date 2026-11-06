use super::{
    client_ip, ensure_mutation_allowed, map_service_error, user_agent, CurrentUser, GatewayError,
    OptionalCurrentUser,
};
use crate::application::dto::profile_dto::{
    AvatarUploadResponse, ProfileBatchRequest, ProfilePrivacyUpdateRequest, ProfileUpdateRequest,
};
use crate::config::AppState;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::{header::CACHE_CONTROL, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use std::sync::Arc;

// Keep this at the public REST boundary as well as in Profile's domain model:
// oversized uploads should be rejected before the gateway accumulates the full
// field or forwards it to the Profile service.
const MAX_AVATAR_UPLOAD_BYTES: usize = 1_048_576;

#[derive(Debug, Deserialize)]
struct ProfileSearchQuery {
    #[serde(rename = "q")]
    query: String,
    cursor: Option<String>,
    #[serde(default = "default_limit")]
    limit: i32,
}

fn default_limit() -> i32 {
    10
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/profile/batch", post(get_profiles))
        .route("/api/profile/search", get(search_profiles))
        .route("/api/profile/avatar", post(upload_avatar))
        .route("/api/profile/privacy", axum::routing::put(update_privacy))
        .route("/api/profile/avatar/:user_id", get(get_avatar))
        .route(
            "/api/profile/username/:username",
            get(get_profile_by_username),
        )
        .route(
            "/api/profile/:user_id",
            get(get_profile).put(update_profile).delete(delete_profile),
        )
        .with_state(state)
}

async fn get_profile(
    State(state): State<Arc<AppState>>,
    OptionalCurrentUser(current_user): OptionalCurrentUser,
    Path(user_id): Path<String>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    Ok(Json(serde_json::to_value(
        state
            .profile_service
            .get_profile_for_viewer(current_user.map(|user| user.user_id), user_id)
            .await
            .map_err(map_service_error)?,
    )?))
}

async fn get_profile_by_username(
    State(state): State<Arc<AppState>>,
    OptionalCurrentUser(current_user): OptionalCurrentUser,
    Path(username): Path<String>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    Ok(Json(serde_json::to_value(
        state
            .profile_service
            .get_profile_by_username_for_viewer(current_user.map(|user| user.user_id), username)
            .await
            .map_err(map_service_error)?,
    )?))
}

async fn get_profiles(
    State(state): State<Arc<AppState>>,
    OptionalCurrentUser(current_user): OptionalCurrentUser,
    Json(request): Json<ProfileBatchRequest>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    Ok(Json(serde_json::to_value(
        state
            .profile_service
            .get_profiles_for_viewer(current_user.map(|user| user.user_id), request.user_ids)
            .await
            .map_err(map_service_error)?,
    )?))
}

async fn search_profiles(
    State(state): State<Arc<AppState>>,
    OptionalCurrentUser(current_user): OptionalCurrentUser,
    Query(query): Query<ProfileSearchQuery>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    Ok(Json(serde_json::to_value(
        state
            .profile_service
            .search_profiles_for_viewer(
                current_user.map(|user| user.user_id),
                query.query,
                query.cursor,
                query.limit,
            )
            .await
            .map_err(map_service_error)?,
    )?))
}

async fn upload_avatar(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    mut multipart: Multipart,
) -> Result<Json<AvatarUploadResponse>, GatewayError> {
    ensure_mutation_allowed(&state, &current_user.user_id).await?;
    let field = multipart
        .next_field()
        .await?
        .ok_or_else(|| GatewayError::bad_request("Avatar image is required"))?;
    let file_name = field.file_name().unwrap_or_default().to_owned();
    let content_type = field
        .content_type()
        .unwrap_or("application/octet-stream")
        .to_owned();
    let bytes = collect_avatar_bytes(field).await?;
    if bytes.is_empty() {
        return Err(GatewayError::bad_request("Avatar image is required"));
    }
    let avatar_url = state
        .profile_service
        .upload_avatar(current_user.user_id, bytes, content_type, file_name)
        .await
        .map_err(map_service_error)?;
    Ok(Json(AvatarUploadResponse { avatar_url }))
}

async fn collect_avatar_bytes(
    mut field: axum::extract::multipart::Field<'_>,
) -> Result<Vec<u8>, GatewayError> {
    let mut bytes = Vec::new();
    while let Some(chunk) = field.chunk().await? {
        if chunk.len() > MAX_AVATAR_UPLOAD_BYTES.saturating_sub(bytes.len()) {
            return Err(GatewayError::payload_too_large(format!(
                "Avatar image must be <= {MAX_AVATAR_UPLOAD_BYTES} bytes"
            )));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

async fn get_avatar(
    State(state): State<Arc<AppState>>,
    OptionalCurrentUser(current_user): OptionalCurrentUser,
    Path(user_id): Path<String>,
) -> Result<Response, GatewayError> {
    state
        .profile_service
        .get_profile_for_viewer(current_user.map(|user| user.user_id), user_id.clone())
        .await
        .map_err(map_service_error)?;
    let avatar = state
        .profile_service
        .get_avatar(user_id)
        .await
        .map_err(map_service_error)?;
    let mut headers = HeaderMap::new();
    // The read above is viewer-dependent: a block or a privacy change can
    // withdraw access for a specific viewer. A shared cache would keep serving
    // the image to viewers who are no longer allowed to see it, and a replaced or
    // deleted avatar would stay visible for the whole max-age window.
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("private, no-store"));
    headers.insert(
        axum::http::header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_str(&avatar.content_type)?,
    );
    Ok((StatusCode::OK, headers, avatar.content).into_response())
}

async fn update_profile(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<String>,
    current_user: CurrentUser,
    Json(request): Json<ProfileUpdateRequest>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    ensure_mutation_allowed(&state, &current_user.user_id).await?;
    if current_user.user_id != user_id {
        return Err(GatewayError::forbidden("Not allowed"));
    }
    Ok(Json(serde_json::to_value(
        state
            .profile_service
            .update_profile(user_id, request)
            .await
            .map_err(map_service_error)?,
    )?))
}

async fn update_privacy(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Json(request): Json<ProfilePrivacyUpdateRequest>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    ensure_mutation_allowed(&state, &current_user.user_id).await?;
    Ok(Json(serde_json::to_value(
        state
            .profile_service
            .update_profile_privacy(current_user.user_id, request)
            .await
            .map_err(map_service_error)?,
    )?))
}

async fn delete_profile(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<String>,
    current_user: CurrentUser,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, GatewayError> {
    if current_user.user_id != user_id {
        return Err(GatewayError::forbidden("Not allowed"));
    }
    Ok(Json(serde_json::to_value(
        state
            .profile_service
            .delete_profile(
                user_id,
                request_id(&headers),
                client_ip(&headers),
                user_agent(&headers),
            )
            .await
            .map_err(map_service_error)?,
    )?))
}

fn request_id(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-request-id")
        .or_else(|| headers.get("x-correlation-id"))
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::extract::FromRequest;
    use axum::http::{header::CONTENT_TYPE, Request};

    async fn read_avatar_payload(payload: &[u8]) -> Result<Vec<u8>, GatewayError> {
        let boundary = "avatar-reader-test-boundary";
        let mut body = format!(
            "--{boundary}\r\n\
             Content-Disposition: form-data; name=\"file\"; filename=\"avatar.png\"\r\n\
             Content-Type: image/png\r\n\r\n\
             "
        )
        .into_bytes();
        body.extend_from_slice(payload);
        body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

        let request = Request::builder()
            .header(
                CONTENT_TYPE,
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(body))
            .expect("multipart request");
        let mut multipart = Multipart::from_request(request, &())
            .await
            .expect("multipart body");
        let field = multipart
            .next_field()
            .await
            .expect("multipart field")
            .expect("avatar field");
        collect_avatar_bytes(field).await
    }

    #[tokio::test]
    async fn avatar_field_reader_accepts_limit_and_rejects_one_byte_over() {
        let max_size_payload = vec![0x5a; MAX_AVATAR_UPLOAD_BYTES];
        let accepted = read_avatar_payload(&max_size_payload)
            .await
            .expect("the documented avatar size must be accepted");
        assert_eq!(accepted, max_size_payload);

        let oversized_payload = vec![0x5a; MAX_AVATAR_UPLOAD_BYTES + 1];
        let error = read_avatar_payload(&oversized_payload)
            .await
            .expect_err("the first byte beyond the limit must be rejected");
        assert_eq!(
            error.to_string(),
            format!("Avatar image must be <= {MAX_AVATAR_UPLOAD_BYTES} bytes")
        );
        // The upload bound is a body-size refusal, not a malformed request.
        let response = axum::response::IntoResponse::into_response(error);
        assert_eq!(response.status(), axum::http::StatusCode::PAYLOAD_TOO_LARGE);
    }
}
