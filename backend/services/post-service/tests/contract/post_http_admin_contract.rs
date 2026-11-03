use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use post_service::adapters::out::persistence::event_outbox::PgEventOutbox;
use post_service::adapters::r#in::http;
use post_service::application::dto::post_dto::CreatePostDto;
use post_service::config::RuntimeMode;
use tower::ServiceExt;
use uuid::Uuid;

#[tokio::test]
async fn direct_http_replay_admin_route_is_not_registered_in_production_mode() {
    let app = crate::support::start_postgres_redis_app(true)
        .await
        .expect("app");
    let router = http::router(app.state.clone(), RuntimeMode::Production);

    let response = router
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/events/replay")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn metrics_endpoint_reports_outbox_backlog_and_dead_letters_in_production_mode() {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("app");
    let dead_lettered = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: "22222222-2222-2222-2222-222222222222".into(),
            content: "Dead letter metrics probe".into(),
            visibility: "PUBLIC".into(),
        })
        .await
        .expect("dead letter post");
    let _pending = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: "22222222-2222-2222-2222-222222222222".into(),
            content: "Pending metrics probe".into(),
            visibility: "PUBLIC".into(),
        })
        .await
        .expect("pending post");
    let dead_event_id: Uuid =
        sqlx::query_scalar("select event_id from post_event_outbox where aggregate_id = $1")
            .bind(Uuid::parse_str(&dead_lettered.id).expect("dead letter id"))
            .fetch_one(&app.pool)
            .await
            .expect("dead event id");
    PgEventOutbox::new(app.pool.clone())
        .record_publish_failure(dead_event_id, "notification unavailable", 1)
        .await
        .expect("dead letter event");

    let response = http::metrics_router(app.state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/metrics")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = String::from_utf8(
        response
            .into_body()
            .collect()
            .await
            .expect("metrics body")
            .to_bytes()
            .to_vec(),
    )
    .expect("utf8 metrics");
    assert!(body.contains("post_outbox_unpublished_total 1"));
    assert!(body.contains("post_outbox_dead_lettered_total 1"));
    assert!(body.contains("post_outbox_oldest_unpublished_age_seconds"));
}
