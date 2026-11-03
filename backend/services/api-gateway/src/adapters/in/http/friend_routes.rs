use super::{ensure_mutation_allowed, map_service_error, CurrentUser, GatewayError};
use crate::application::dto::friend_dto::{FollowRequest, PeopleRecommendationEventRequest};
use crate::config::AppState;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
struct FollowListQuery {
    user_id: Option<String>,
    cursor: Option<String>,
    limit: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct FollowUserQuery {
    user_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
struct FollowStatusQuery {
    target_user_id: String,
}

#[derive(Debug, Deserialize)]
struct FollowRequestListQuery {
    cursor: Option<String>,
    #[serde(default = "default_limit")]
    limit: i32,
}

#[derive(Debug, Deserialize)]
struct PeopleRecommendationQuery {
    cursor: Option<String>,
    #[serde(default = "default_limit")]
    limit: i32,
}

fn default_limit() -> i32 {
    20
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/follows", post(follow))
        .route("/api/follows/followers", get(list_followers))
        .route("/api/follows/following", get(list_following))
        .route("/api/follows/counts", get(counts))
        .route("/api/follows/status", get(status))
        .route("/api/follows/:target_user_id", delete(unfollow))
        .route(
            "/api/follows/requests/:request_id/accept",
            post(accept_follow_request),
        )
        .route(
            "/api/follows/requests/:request_id/reject",
            post(reject_follow_request),
        )
        .route(
            "/api/follows/requests/:request_id",
            delete(cancel_follow_request),
        )
        .route(
            "/api/follows/requests/incoming",
            get(list_incoming_follow_requests),
        )
        .route(
            "/api/follows/requests/outgoing",
            get(list_outgoing_follow_requests),
        )
        .route(
            "/api/relationships/blocks/:target_user_id",
            post(block_user).delete(unblock_user),
        )
        .route(
            "/api/relationships/mutes/:target_user_id",
            post(mute_user).delete(unmute_user),
        )
        .route("/api/relationships/:target_user_id", get(get_relationship))
        .route(
            "/api/recommendations/people",
            get(get_people_recommendations),
        )
        .route(
            "/api/recommendations/people/events",
            post(record_people_recommendation_event),
        )
        .with_state(state)
}

async fn follow(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Json(request): Json<FollowRequest>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    ensure_mutation_allowed(&state, &current_user.user_id).await?;
    to_json(
        state
            .friend_service
            .follow(current_user.user_id, request.target_user_id)
            .await,
    )
}

async fn unfollow(
    State(state): State<Arc<AppState>>,
    Path(target_user_id): Path<String>,
    current_user: CurrentUser,
) -> Result<Json<serde_json::Value>, GatewayError> {
    ensure_mutation_allowed(&state, &current_user.user_id).await?;
    to_json(
        state
            .friend_service
            .unfollow(current_user.user_id, target_user_id)
            .await,
    )
}

async fn list_followers(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Query(query): Query<FollowListQuery>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    let user_id = query.user_id.unwrap_or(current_user.user_id);
    to_json(
        state
            .friend_service
            .list_followers(user_id, query.cursor, query.limit)
            .await,
    )
}

async fn list_following(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Query(query): Query<FollowListQuery>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    let user_id = query.user_id.unwrap_or(current_user.user_id);
    to_json(
        state
            .friend_service
            .list_following(user_id, query.cursor, query.limit)
            .await,
    )
}

async fn counts(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Query(query): Query<FollowUserQuery>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    let user_id = query
        .user_id
        .map(|user_id| user_id.to_string())
        .unwrap_or(current_user.user_id);
    to_json(state.friend_service.counts(user_id).await)
}

async fn status(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Query(query): Query<FollowStatusQuery>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    to_json(
        state
            .friend_service
            .status(current_user.user_id, query.target_user_id)
            .await,
    )
}

async fn accept_follow_request(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(request_id): Path<String>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    ensure_mutation_allowed(&state, &current_user.user_id).await?;
    to_json(
        state
            .friend_service
            .accept_follow_request(request_id, current_user.user_id)
            .await,
    )
}

async fn reject_follow_request(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(request_id): Path<String>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    to_json(
        state
            .friend_service
            .reject_follow_request(request_id, current_user.user_id)
            .await,
    )
}

async fn cancel_follow_request(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(request_id): Path<String>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    to_json(
        state
            .friend_service
            .cancel_follow_request(request_id, current_user.user_id)
            .await,
    )
}

async fn list_incoming_follow_requests(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Query(query): Query<FollowRequestListQuery>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    to_json(
        state
            .friend_service
            .list_incoming_follow_requests(current_user.user_id, query.cursor, query.limit)
            .await,
    )
}

async fn list_outgoing_follow_requests(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Query(query): Query<FollowRequestListQuery>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    to_json(
        state
            .friend_service
            .list_outgoing_follow_requests(current_user.user_id, query.cursor, query.limit)
            .await,
    )
}

async fn block_user(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(target_user_id): Path<String>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    to_json(
        state
            .friend_service
            .block(current_user.user_id, target_user_id)
            .await,
    )
}

async fn unblock_user(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(target_user_id): Path<String>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    to_json(
        state
            .friend_service
            .unblock(current_user.user_id, target_user_id)
            .await,
    )
}

async fn mute_user(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(target_user_id): Path<String>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    to_json(
        state
            .friend_service
            .mute(current_user.user_id, target_user_id)
            .await,
    )
}

async fn unmute_user(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(target_user_id): Path<String>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    to_json(
        state
            .friend_service
            .unmute(current_user.user_id, target_user_id)
            .await,
    )
}

async fn get_relationship(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(target_user_id): Path<String>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    to_json(
        state
            .friend_service
            .relationship(current_user.user_id, target_user_id)
            .await,
    )
}

async fn get_people_recommendations(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    headers: HeaderMap,
    Query(query): Query<PeopleRecommendationQuery>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    let request_id = request_id(&headers).unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    to_json(
        state
            .friend_service
            .people_recommendations(current_user.user_id, query.cursor, query.limit, request_id)
            .await,
    )
}

async fn record_people_recommendation_event(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    headers: HeaderMap,
    Json(event): Json<PeopleRecommendationEventRequest>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    let request_id = request_id(&headers).unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    to_json(
        state
            .friend_service
            .record_people_recommendation_event(current_user.user_id, request_id, event)
            .await,
    )
}

fn request_id(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-request-id")
        .or_else(|| headers.get("x-correlation-id"))
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn to_json<T: serde::Serialize, E: Into<anyhow::Error>>(
    result: Result<T, E>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    Ok(Json(serde_json::to_value(
        result.map_err(map_service_error)?,
    )?))
}
