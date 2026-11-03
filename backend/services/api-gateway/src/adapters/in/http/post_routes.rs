use super::{
    client_ip, ensure_mutation_allowed, map_service_error, user_agent, CurrentUser, GatewayError,
};
use crate::application::dto::post_dto::{
    CommentCreateRequest, FeedRecommendationEventRequest, PostCreateRequest,
};
use crate::config::AppState;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
struct FeedQuery {
    cursor: Option<String>,
    #[serde(default = "default_limit")]
    limit: i32,
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
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
        .route("/api/feed", get(get_feed))
        .route("/api/feed/events", post(record_feed_recommendation_event))
        .route("/api/posts", post(create_post))
        .route("/api/posts/search", get(search_posts))
        .route("/api/posts/trending", get(get_trending))
        .route("/api/posts/author/:author_id", get(get_author_posts))
        .route("/api/posts/:post_id", get(get_post).delete(delete_post))
        .route("/api/posts/:post_id/comments", post(create_comment))
        .route(
            "/api/posts/:post_id/likes",
            post(like_post).delete(unlike_post),
        )
        .with_state(state)
}

async fn get_feed(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    headers: HeaderMap,
    Query(query): Query<FeedQuery>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    let request_id = request_id(&headers).unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    Ok(Json(serde_json::to_value(
        state
            .post_service
            .get_ranked_feed(
                current_user.user_id,
                query.cursor,
                query.limit,
                request_id,
                None,
            )
            .await
            .map_err(map_service_error)?,
    )?))
}

async fn record_feed_recommendation_event(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    headers: HeaderMap,
    Json(event): Json<FeedRecommendationEventRequest>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    let request_id = request_id(&headers).unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    Ok(Json(serde_json::to_value(
        state
            .post_service
            .record_feed_recommendation_event(current_user.user_id, request_id, event)
            .await
            .map_err(map_service_error)?,
    )?))
}

async fn get_author_posts(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(author_id): Path<String>,
    Query(query): Query<FeedQuery>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    Ok(Json(serde_json::to_value(
        state
            .post_service
            .get_author_posts(author_id, current_user.user_id, query.cursor, query.limit)
            .await
            .map_err(map_service_error)?,
    )?))
}

async fn search_posts(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Query(query): Query<SearchQuery>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    Ok(Json(serde_json::to_value(
        state
            .post_service
            .search_posts(current_user.user_id, query.query, query.cursor, query.limit)
            .await
            .map_err(map_service_error)?,
    )?))
}

async fn get_trending(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Query(query): Query<FeedQuery>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    Ok(Json(serde_json::to_value(
        state
            .post_service
            .get_trending(current_user.user_id, query.cursor, query.limit)
            .await
            .map_err(map_service_error)?,
    )?))
}

async fn get_post(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(post_id): Path<String>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    Ok(Json(serde_json::to_value(
        state
            .post_service
            .get_post(current_user.user_id, post_id)
            .await
            .map_err(map_service_error)?,
    )?))
}

async fn create_post(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Json(request): Json<PostCreateRequest>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    ensure_mutation_allowed(&state, &current_user.user_id).await?;
    Ok(Json(serde_json::to_value(
        state
            .post_service
            .create_post(current_user.user_id, request)
            .await
            .map_err(map_service_error)?,
    )?))
}

async fn delete_post(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(post_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, GatewayError> {
    Ok(Json(serde_json::to_value(
        state
            .post_service
            .delete_post(
                current_user.user_id,
                post_id,
                request_id(&headers),
                client_ip(&headers),
                user_agent(&headers),
            )
            .await
            .map_err(map_service_error)?,
    )?))
}

async fn create_comment(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(post_id): Path<String>,
    Json(request): Json<CommentCreateRequest>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    ensure_mutation_allowed(&state, &current_user.user_id).await?;
    Ok(Json(serde_json::to_value(
        state
            .post_service
            .create_comment(current_user.user_id, post_id, request)
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

async fn like_post(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(post_id): Path<String>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    ensure_mutation_allowed(&state, &current_user.user_id).await?;
    Ok(Json(serde_json::to_value(
        state
            .post_service
            .like_post(current_user.user_id, post_id)
            .await
            .map_err(map_service_error)?,
    )?))
}

async fn unlike_post(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(post_id): Path<String>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    Ok(Json(serde_json::to_value(
        state
            .post_service
            .unlike_post(current_user.user_id, post_id)
            .await
            .map_err(map_service_error)?,
    )?))
}
