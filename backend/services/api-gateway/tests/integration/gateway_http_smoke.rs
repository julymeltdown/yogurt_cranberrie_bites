use crate::support::{assembled_gateway_router, bearer_token, FakeFriendClient};
use reqwest::{Client, StatusCode};
use serde_json::Value;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

const AUTHENTICATED_USER_ID: &str = "11111111-1111-1111-1111-111111111111";
const REQUESTED_USER_ID: &str = "22222222-2222-2222-2222-222222222222";

struct RunningGateway {
    addr: SocketAddr,
    task: Option<JoinHandle<std::io::Result<()>>>,
    friend_client: Arc<FakeFriendClient>,
}

impl RunningGateway {
    async fn start() -> Self {
        let (router, friend_client) = assembled_gateway_router().await.expect("assembled router");
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("ephemeral gateway listener");
        let addr = listener.local_addr().expect("gateway address");
        let task = tokio::spawn(async move { axum::serve(listener, router).await });
        Self {
            addr,
            task: Some(task),
            friend_client,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.addr)
    }

    async fn shutdown(mut self) {
        let task = self.task.take().expect("running server task");
        task.abort();
        let cancelled = task.await.expect_err("server task must be cancelled");
        assert!(cancelled.is_cancelled());
        let listener = TcpListener::bind(self.addr)
            .await
            .expect("gateway listener released");
        drop(listener);
        println!("gateway_http_smoke cleanup_rebind={}", self.addr);
    }
}

impl Drop for RunningGateway {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

fn client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("HTTP client")
}

async fn ready_openapi(gateway: &RunningGateway, client: &Client) -> Value {
    let response = client
        .get(gateway.url("/openapi.json"))
        .send()
        .await
        .expect("OpenAPI readiness response");
    assert_eq!(response.status(), StatusCode::OK);
    println!("gateway_http_smoke readiness_status={}", response.status());
    response.json().await.expect("OpenAPI JSON")
}

fn assert_openapi_contract(openapi: &Value) {
    assert!(
        openapi["paths"].get("/metrics").is_none(),
        "the application listener OpenAPI must not advertise dedicated metrics"
    );
    for (path, method, expected_ref) in [
        (
            "/api/follows/requests/{request_id}/accept",
            "post",
            "#/components/schemas/FollowActionResponse",
        ),
        (
            "/api/follows/requests/{request_id}/reject",
            "post",
            "#/components/schemas/FollowRequestResponse",
        ),
        (
            "/api/posts/author/{author_id}",
            "get",
            "#/components/schemas/FeedResponse",
        ),
        (
            "/api/posts/search",
            "get",
            "#/components/schemas/FeedResponse",
        ),
        (
            "/api/posts/trending",
            "get",
            "#/components/schemas/FeedResponse",
        ),
    ] {
        assert_eq!(
            openapi["paths"][path][method]["responses"]["200"]["content"]["application/json"]
                ["schema"]["$ref"],
            expected_ref,
            "{method} {path} success response ref"
        );
    }
    assert_eq!(
        openapi["paths"]["/auth/refresh"]["post"]["security"],
        serde_json::json!([{"RefreshCookie": []}])
    );
    assert_eq!(
        openapi["paths"]["/auth/oauth/{provider}/callback"]["get"]["security"],
        serde_json::json!([{"OAuthStateCookie": []}])
    );
    assert_eq!(
        openapi["components"]["securitySchemes"]["OAuthStateCookie"]["x-provider-cookie-names"],
        serde_json::json!({
            "google": "oauth_state_google",
            "apple": "oauth_state_apple",
            "x": "oauth_state_x"
        })
    );
    for (path, method, has_json_request_body) in [
        ("/auth/login", "post", true),
        ("/api/follows/counts", "get", false),
    ] {
        for status in ["400", "415", "422"] {
            let response = &openapi["paths"][path][method]["responses"][status];
            if status != "400" && !has_json_request_body {
                assert!(
                    response.is_null(),
                    "{method} {path} must not declare JSON-body extractor response {status}"
                );
                continue;
            }
            assert_eq!(
                response["content"]["application/json"]["schema"]["$ref"],
                "#/components/schemas/ErrorResponse",
                "{method} {path} response {status}"
            );
        }
    }
}

#[tokio::test]
async fn gateway_http_smoke_serves_docs_spec_and_explicit_count_override() {
    // Given: the fully assembled production gateway router on a real ephemeral socket.
    let gateway = RunningGateway::start().await;
    let client = client();
    let openapi = ready_openapi(&gateway, &client).await;

    // When: clients fetch docs and counts for a user other than the authenticated user.
    let docs = client
        .get(gateway.url("/docs"))
        .send()
        .await
        .expect("docs response");
    let counts = client
        .get(gateway.url(&format!("/api/follows/counts?user_id={REQUESTED_USER_ID}")))
        .bearer_auth(bearer_token(AUTHENTICATED_USER_ID, &["ROLE_USER"]))
        .send()
        .await
        .expect("counts response");
    let counts_status = counts.status();
    let counts_json: Value = counts.json().await.expect("counts JSON");

    // Then: runtime docs/spec are available and the explicit UUID reaches the fake downstream.
    assert_eq!(docs.status(), StatusCode::OK);
    assert_eq!(docs.headers()["content-type"], "text/html; charset=utf-8");
    assert_eq!(openapi["openapi"], "3.1.0");
    assert_openapi_contract(&openapi);
    assert_eq!(counts_status, StatusCode::OK);
    assert_eq!(counts_json["followerCount"], 3);
    assert_eq!(counts_json["followingCount"], 2);
    assert_eq!(gateway.friend_client.count_calls(), vec![REQUESTED_USER_ID]);
    println!(
        "gateway_http_smoke docs_status={} counts_status={} count_calls={:?}",
        docs.status(),
        counts_status,
        gateway.friend_client.count_calls()
    );
    gateway.shutdown().await;
}

#[tokio::test]
async fn gateway_http_smoke_returns_safe_envelopes_and_hides_metrics() {
    // Given: the fully assembled gateway and a valid user token.
    let gateway = RunningGateway::start().await;
    let client = client();
    ready_openapi(&gateway, &client).await;
    let token = bearer_token(AUTHENTICATED_USER_ID, &["ROLE_USER"]);

    // When: malformed JSON, an invalid counts UUID, and the removed metrics path are requested.
    let malformed = client
        .post(gateway.url("/auth/login"))
        .header("content-type", "application/json")
        .body(r#"{"email":"#)
        .send()
        .await
        .expect("malformed response");
    let malformed_status = malformed.status();
    let malformed_content_type = malformed.headers()["content-type"].clone();
    let malformed_json: Value = malformed.json().await.expect("malformed JSON envelope");
    let invalid_uuid = client
        .get(gateway.url("/api/follows/counts?user_id=not-a-uuid"))
        .bearer_auth(token)
        .send()
        .await
        .expect("invalid UUID response");
    let invalid_uuid_status = invalid_uuid.status();
    let invalid_uuid_content_type = invalid_uuid.headers()["content-type"].clone();
    let invalid_uuid_json: Value = invalid_uuid.json().await.expect("invalid UUID envelope");
    let metrics = client
        .get(gateway.url("/metrics"))
        .send()
        .await
        .expect("metrics response");

    // Then: both invalid requests use the approved envelope and metrics stays off this router.
    assert_eq!(malformed_status, StatusCode::BAD_REQUEST);
    assert_eq!(malformed_content_type, "application/json");
    assert_eq!(malformed_json["code"], "GATEWAY_400");
    assert_eq!(malformed_json["message"], "Malformed JSON request body");
    assert!(malformed_json["traceId"].is_string());
    assert!(malformed_json["timestamp"].is_string());
    assert_eq!(invalid_uuid_status, StatusCode::BAD_REQUEST);
    assert_eq!(invalid_uuid_content_type, "application/json");
    assert_eq!(invalid_uuid_json["code"], "GATEWAY_400");
    assert_eq!(invalid_uuid_json["message"], "Invalid query parameters");
    assert!(invalid_uuid_json["traceId"].is_string());
    assert!(invalid_uuid_json["timestamp"].is_string());
    assert_eq!(metrics.status(), StatusCode::NOT_FOUND);
    println!(
        "gateway_http_smoke malformed_status={} malformed_body={} invalid_uuid_status={} invalid_uuid_body={} metrics_status={}",
        malformed_status,
        malformed_json,
        invalid_uuid_status,
        invalid_uuid_json,
        metrics.status()
    );
    gateway.shutdown().await;
}
