use axum::body::Body;
use axum::http::{Method, Request};
use serde_json::json;
use tower::util::ServiceExt;

#[tokio::test]
async fn oauth_parity_fixture_matches_authorize_and_callback_summary() {
    let fixture: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../scripts/parity/auth-service/oauth-shadow.json"),
        )
        .expect("fixture"),
    )
    .expect("json");
    let app = crate::support::start_postgres_app(true).await.expect("app");

    let authorize_request = Request::builder()
        .method(Method::GET)
        .uri("/auth/oauth/google/authorize")
        .body(Body::empty())
        .expect("request");
    let authorize_response = app
        .router
        .clone()
        .oneshot(authorize_request)
        .await
        .expect("response");
    let location = authorize_response
        .headers()
        .get("location")
        .and_then(|value| value.to_str().ok())
        .expect("location")
        .to_owned();
    let state = extract_query_param(&location, "state").expect("state");

    let callback_request = Request::builder()
        .method(Method::GET)
        .uri(format!(
            "/auth/oauth/google/callback?code=google-hana-code&state={state}"
        ))
        .body(Body::empty())
        .expect("request");
    let (callback_status, callback_body, _) =
        crate::support::json_response(app.router, callback_request)
            .await
            .expect("callback");

    let actual = json!({
        "authorize": {
            "status": authorize_response.status().as_u16(),
            "locationContainsState": location.contains("state=")
        },
        "callback": {
            "status": callback_status.as_u16(),
            "userId": callback_body["userId"],
            "linkedProviders": callback_body["linkedProviders"]
        }
    });

    assert_eq!(actual, fixture);
}

fn extract_query_param(uri: &str, key: &str) -> Option<String> {
    let query = uri.split('?').nth(1)?;
    for pair in query.split('&') {
        let (name, value) = pair.split_once('=')?;
        if name == key {
            return Some(value.to_owned());
        }
    }
    None
}
