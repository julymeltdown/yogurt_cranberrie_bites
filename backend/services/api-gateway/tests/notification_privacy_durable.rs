//! Real Gateway journal and executable transport/request middleware. Owner
//! notification and authorization ports are deterministic timing doubles.
#[allow(dead_code)]
#[path = "unit/http_support.rs"]
mod http_support;
mod support;

use api_gateway::adapters::r#in::http::client_identity::TrustedProxies;
use api_gateway::adapters::r#in::http::{self, server};
use api_gateway::application::dto::notification_dto::NotificationResponse;
use api_gateway::application::service::notification_gateway_service::NotificationGatewayService;
use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use serde_json::json;
use std::sync::{atomic::Ordering, Arc};
use std::time::Duration;
use tower::ServiceExt;
use uuid::Uuid;

#[tokio::test]
async fn committed_mark_read_projection_respects_shorter_durable_journal_wait() -> anyhow::Result<()>
{
    let app = support::start_gateway_app(true).await?;
    let result = async {
        let clients = Arc::new(http_support::TestClients::default());
        let notification_id = Uuid::new_v4().to_string();
        *clients.notification_items.lock().unwrap() = Some(vec![NotificationResponse {
            id: notification_id.clone(), user_id: http_support::USER_ID.into(), notification_type: "POST_COMMENTED".into(),
            payload_json: json!({"postId": Uuid::new_v4(), "content": "STORED_PRIVATE_COMMENT_CANARY", "preview": {"text": "NESTED_SECRET"}}).to_string(),
            created_at: "2026-10-11T00:00:00Z".into(), read_at: String::new(),
        }]);
        *clients.notification_mark_read_delay.lock().unwrap() = Duration::from_millis(150);
        *clients.notification_authorization_delay.lock().unwrap() = Duration::from_millis(250);
        let mut state = (*app.state).clone();
        state.request_timeout = Duration::from_millis(1000);
        state.journal_client_timeout = Duration::from_millis(300);
        state.notification_service = NotificationGatewayService::new(clients.clone(), clients.clone(), Arc::new(api_gateway::adapters::out::runtime::SystemGatewayRuntime));
        let mut config = app.config.clone();
        config.request_timeout_ms = 1000;
        config.journal_client_timeout_ms = 300;
        let router = server::with_transport_layers(http::router(Arc::new(state)), &config, TrustedProxies::default());
        let response = router.oneshot(Request::builder().method("PATCH")
            .uri(format!("/api/notifications/{notification_id}/read"))
            .header(header::AUTHORIZATION, format!("Bearer {}", support::bearer_token(http_support::USER_ID, &["ROLE_USER"])))
            .body(Body::empty())?).await?;
        assert_eq!(response.status(), StatusCode::OK, "the committed read must finish presentation before the 300ms journal wait");
        let operation_id = Uuid::parse_str(response.headers().get("x-operation-id").expect("durable operation id").to_str()?)?;
        let body = http_support::response_json(response).await;
        assert_eq!(body["payloadJson"], "{}");
        assert_eq!(body["readAt"], "2026-10-11T00:00:00Z");
        assert_eq!(clients.notification_mark_read_calls.lock().unwrap().len(), 1);
        assert_eq!(clients.notification_authorization_batches.load(Ordering::SeqCst), 1);
        assert!(clients.notification_post_checks.lock().unwrap().is_empty());
        let recorded: (String, String, i32, Option<Vec<u8>>) = sqlx::query_as("select state, outcome, response_status, response_body from gateway_request_journal where id = $1")
            .bind(operation_id).fetch_one(&app.pool).await?;
        assert_eq!((recorded.0.as_str(), recorded.1.as_str(), recorded.2), ("COMPLETED", "COMMITTED", 200));
        assert!(recorded.3.is_none(), "unkeyed notification writes must not retain a replay body");
        Ok::<(), anyhow::Error>(())
    }.await;
    app.shutdown().await?;
    result
}
