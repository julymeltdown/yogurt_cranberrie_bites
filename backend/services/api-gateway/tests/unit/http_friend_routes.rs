use crate::http_support::{
    response_json, PeopleRecommendationCall, PeopleRecommendationEventCall, RelationshipCall,
    RouteHarness, OTHER_USER_ID, USER_ID,
};
use api_gateway::adapters::r#in::http::friend_routes;
use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use serde_json::{json, Value};
use tower::ServiceExt;

async fn request(
    app: &Router,
    harness: &RouteHarness,
    method: Method,
    uri: &str,
    body: Option<Value>,
    request_id: Option<(&str, &str)>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::AUTHORIZATION, harness.bearer_token(USER_ID));
    if body.is_some() {
        builder = builder.header(header::CONTENT_TYPE, "application/json");
    }
    if let Some((name, value)) = request_id {
        builder = builder.header(name, value);
    }
    let response = app
        .clone()
        .oneshot(
            builder
                .body(body.map_or_else(Body::empty, |value| Body::from(value.to_string())))
                .expect("request"),
        )
        .await
        .expect("response");
    let status = response.status();
    (status, response_json(response).await)
}

#[tokio::test]
async fn follow_request_routes_preserve_actor_cursor_and_default_limit_contracts() {
    let harness = RouteHarness::new().await;
    let app = friend_routes::router(harness.state.clone());

    let (status, accepted) = request(
        &app,
        &harness,
        Method::POST,
        "/api/follows/requests/request-accept/accept",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(accepted["requestId"], "request-accept");
    assert_eq!(accepted["following"], true);

    let (status, rejected) = request(
        &app,
        &harness,
        Method::POST,
        "/api/follows/requests/request-reject/reject",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(rejected["id"], "request-reject");
    assert_eq!(rejected["status"], "REJECTED");

    let (status, cancelled) = request(
        &app,
        &harness,
        Method::DELETE,
        "/api/follows/requests/request-cancel",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cancelled["id"], "request-cancel");
    assert_eq!(cancelled["status"], "CANCELLED");

    let (status, incoming) = request(
        &app,
        &harness,
        Method::GET,
        "/api/follows/requests/incoming?cursor=incoming-next&limit=7",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(incoming["requests"][0]["id"], "incoming-1");
    assert_eq!(incoming["nextCursor"], "incoming-next");

    let (status, outgoing) = request(
        &app,
        &harness,
        Method::GET,
        "/api/follows/requests/outgoing",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(outgoing["requests"][0]["id"], "outgoing-1");
    assert_eq!(outgoing["nextCursor"], Value::Null);
}

#[tokio::test]
async fn relationship_and_people_routes_preserve_viewer_and_request_context() {
    let harness = RouteHarness::new().await;
    let app = friend_routes::router(harness.state.clone());

    for (method, path, field, expected) in [
        (
            Method::DELETE,
            format!("/api/relationships/blocks/{OTHER_USER_ID}"),
            "blockedByViewer",
            false,
        ),
        (
            Method::POST,
            format!("/api/relationships/mutes/{OTHER_USER_ID}"),
            "mutedByViewer",
            true,
        ),
        (
            Method::DELETE,
            format!("/api/relationships/mutes/{OTHER_USER_ID}"),
            "mutedByViewer",
            false,
        ),
        (
            Method::GET,
            format!("/api/relationships/{OTHER_USER_ID}"),
            "following",
            true,
        ),
    ] {
        let (status, body) = request(&app, &harness, method, &path, None, None).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert_eq!(body[field], expected, "{path}");
    }

    let (status, recommendations) = request(
        &app,
        &harness,
        Method::GET,
        "/api/recommendations/people?cursor=people-next&limit=4",
        None,
        Some(("x-correlation-id", "people-request-1")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(recommendations["requestId"], "people-request-1");
    assert_eq!(recommendations["nextCursor"], "people-next");
    assert_eq!(recommendations["items"][0]["userId"], OTHER_USER_ID);
    assert_eq!(
        *harness.clients.people_recommendation_calls.lock().unwrap(),
        vec![PeopleRecommendationCall {
            viewer_id: USER_ID.into(),
            cursor: Some("people-next".into()),
            limit: 4,
            request_id: "people-request-1".into(),
        }]
    );

    let (status, event) = request(
        &app,
        &harness,
        Method::POST,
        "/api/recommendations/people/events",
        Some(json!({
            "candidateUserId": OTHER_USER_ID,
            "impressionId": "33333333-3333-3333-3333-333333333333",
            "action": "FOLLOW",
            "source": "SOCIAL_GRAPH",
            "modelVersion": "people-v1",
            "position": 2,
            "propensityMicros": 750000,
            "dwellMs": 1250,
            "metadata": {"surface": "discover"}
        })),
        Some(("x-request-id", "people-event-1")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(event["accepted"], true);
    assert_eq!(
        *harness
            .clients
            .people_recommendation_event_calls
            .lock()
            .unwrap(),
        vec![PeopleRecommendationEventCall {
            viewer_id: USER_ID.into(),
            request_id: "people-event-1".into(),
            candidate_user_id: OTHER_USER_ID.into(),
            action: "FOLLOW".into(),
        }]
    );
    assert_eq!(
        *harness.clients.relationship_calls.lock().unwrap(),
        vec![
            RelationshipCall {
                operation: "unblock".into(),
                viewer_id: USER_ID.into(),
                target_id: OTHER_USER_ID.into(),
            },
            RelationshipCall {
                operation: "mute".into(),
                viewer_id: USER_ID.into(),
                target_id: OTHER_USER_ID.into(),
            },
            RelationshipCall {
                operation: "unmute".into(),
                viewer_id: USER_ID.into(),
                target_id: OTHER_USER_ID.into(),
            },
            RelationshipCall {
                operation: "get".into(),
                viewer_id: USER_ID.into(),
                target_id: OTHER_USER_ID.into(),
            },
        ]
    );
}
