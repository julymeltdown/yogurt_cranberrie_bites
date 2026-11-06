//! Chromium fixture coverage of actual Gateway transport and signed cookies.
//! Recording Auth proves no Auth persistence, private issuance, provider
//! exchange, canonical refresh, frontend admission or external sign-in.
#[allow(dead_code)]
#[path = "unit/http_support.rs"]
mod http_support;

use api_gateway::adapters::r#in::http::browser_auth::{self, BrowserBoundaryState};
use api_gateway::config::{OAuthExposurePolicy, RuntimeMode};
use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::{Request, StatusCode};
use axum::middleware::{from_fn_with_state, Next};
use axum::response::{Html, Response};
use axum::routing::get;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

#[tokio::test]
async fn mounted_recording_fixture_rejects_unbound_state_before_auth() -> anyhow::Result<()> {
    let origin = "http://127.0.0.1:18080";
    let mut harness = http_support::RouteHarness::new().await;
    let state = Arc::get_mut(&mut harness.state).expect("exclusive fixture state");
    state.oauth_exposure = OAuthExposurePolicy::from_settings(
        RuntimeMode::Parity, "", "apple",
        Some("fixture-only-independent-signing-key-32bytes".into()), Some(origin.into()),
    )?;
    state.refresh_cookie_secure = true;
    state.browser_auth_origins = vec![origin.into()];
    let router = browser_auth::router(harness.state.clone()).layer(from_fn_with_state(
        BrowserBoundaryState { origins: vec![origin.into()], oauth: harness.state.oauth_exposure.clone() },
        browser_auth::boundary,
    ));
    let authorized = router.clone().oneshot(Request::builder()
        .uri("/auth/oauth/apple/authorize?returnTo=%2Ffeed").body(Body::empty())?).await?;
    anyhow::ensure!(authorized.status() == StatusCode::FOUND, "configured route missing");
    let cookie = authorized.headers().get("set-cookie").expect("signed state cookie")
        .to_str()?.split(';').next().expect("cookie pair").to_owned();
    let altered = format!("{cookie}x");
    let cases = [
        ("code=never-exchange&state=apple-state", None),
        ("code=never-exchange&state=", Some(cookie.as_str())),
        ("code=never-exchange&state=wrong-state", Some(cookie.as_str())),
        ("code=never-exchange&state=apple-state&state=apple-state", Some(cookie.as_str())),
        ("code=never-exchange&state=apple-state", Some(altered.as_str())),
        ("code=never-exchange&state=apple-state&error=access_denied", Some(cookie.as_str())),
        ("code=never-exchange&state=apple-state&access_token=forbidden", Some(cookie.as_str())),
        ("code=%QQ&state=apple-state", Some(cookie.as_str())),
    ];
    for (query, supplied) in cases {
        let mut request = Request::builder().uri(format!("/auth/oauth/apple/callback?{query}"));
        if let Some(value) = supplied { request = request.header("cookie", value); }
        let response = router.clone().oneshot(request.body(Body::empty())?).await?;
        anyhow::ensure!(response.status() == StatusCode::BAD_REQUEST, "invalid fixture callback accepted");
        anyhow::ensure!(!response.headers().contains_key("location") && !response.headers().contains_key("set-cookie"), "invalid callback changed browser credentials");
    }
    for separate_headers in [false, true] {
        let mut request = Request::builder().uri("/auth/oauth/apple/callback?code=never-exchange&state=apple-state");
        request = if separate_headers { request.header("cookie", &cookie).header("cookie", &cookie) }
            else { request.header("cookie", format!("{cookie}; {cookie}")) };
        let response = router.clone().oneshot(request.body(Body::empty())?).await?;
        anyhow::ensure!(response.status() == StatusCode::BAD_REQUEST, "duplicate state cookie accepted");
    }
    let denial = router.oneshot(Request::builder()
        .uri("/auth/oauth/apple/callback?state=apple-state&error=access_denied&error_description=untrusted&returnTo=https%3A%2F%2Fexample.invalid")
        .header("cookie", &cookie).body(Body::empty())?).await?;
    anyhow::ensure!(denial.status() == StatusCode::SEE_OTHER, "bound denial not redirected");
    anyhow::ensure!(denial.headers().get("location").and_then(|value| value.to_str().ok()) == Some("/auth/callback?status=denied&returnTo=%2Ffeed"), "denial return override or leak");
    anyhow::ensure!(denial.headers().get_all("set-cookie").iter().count() == 1
        && denial.headers()["set-cookie"].to_str()?.contains("Max-Age=0"), "denial cookie policy");
    anyhow::ensure!(harness.clients.oauth_callback_calls.lock().unwrap().is_empty(), "unbound or denied fixture called Auth");
    anyhow::ensure!(harness.clients.auth_calls.lock().unwrap().as_slice() == ["authorize"], "unexpected Auth port call");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actual_browser_cross_site_post_restores_only_the_bound_cookie() -> anyhow::Result<()> {
    let node = std::env::var("OAUTH_BROWSER_NODE")?;
    for name in ["OAUTH_BROWSER_PLAYWRIGHT", "OAUTH_BROWSER_CHROME"] {
        anyhow::ensure!(std::path::Path::new(&std::env::var(name)?).is_absolute(), "absolute fixture tool required");
    }
    // Exercise the actual composition decision, rather than unconditionally
    // mounting oauth_routes and bypassing the default-off policy.
    let default = http_support::RouteHarness::new().await;
    let disabled = browser_auth::router(default.state.clone())
        .oneshot(Request::builder().uri("/auth/oauth/apple/authorize").body(Body::empty())?)
        .await?;
    anyhow::ensure!(disabled.status() == StatusCode::NOT_FOUND, "default OAuth mounted");
    anyhow::ensure!(default.clients.oauth_callback_calls.lock().unwrap().is_empty(), "default reached Auth");

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let origin = format!("http://127.0.0.1:{port}");
    let mut harness = http_support::RouteHarness::new().await;
    let state = Arc::get_mut(&mut harness.state).expect("exclusive fixture state");
    state.oauth_exposure = OAuthExposurePolicy::from_settings(
        RuntimeMode::Parity, "", "apple",
        Some("fixture-only-independent-signing-key-32bytes".into()), Some(origin.clone()),
    )?;
    state.refresh_cookie_secure = true;
    state.browser_auth_origins = vec![origin.clone()];
    let observations = Arc::new(Mutex::new(Vec::<serde_json::Value>::new()));
    let app = browser_auth::router(harness.state.clone())
        .route("/fixture/provider", get(move |Query(query): Query<HashMap<String, String>>| async move {
            let credential = if query.get("denial").is_some_and(|value| value == "1") {
                "<input name=\"error\" value=\"access_denied\" type=\"hidden\">"
            } else { "<input name=\"code\" value=\"fixture-code\" type=\"hidden\">" };
            Html(format!("<!doctype html><title>Fixture provider</title><form method=\"post\" action=\"http://127.0.0.1:{port}/auth/oauth/apple/callback\">{credential}<input name=\"state\" value=\"apple-state\" type=\"hidden\"><button>Authorize fixture</button></form>"))
        }))
        .route("/auth/callback", get(|| async { Html("<!doctype html><title>Fixture handoff</title>Fixture handoff") }))
        .layer(from_fn_with_state(
            BrowserBoundaryState { origins: vec![origin], oauth: harness.state.oauth_exposure.clone() },
            browser_auth::boundary,
        ))
        .layer(from_fn_with_state(observations.clone(), record));
    let (shutdown, stopped) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(async move {
        axum::serve(listener, app).with_graceful_shutdown(async { let _ = stopped.await; }).await
    });
    // A fresh allocating controller supplies installed tools. No installation
    // or personal browser profile is used by this fixture.
    let output = tokio::process::Command::new(node)
        .arg(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/testing/oauth-form-post-browser.cjs"))
        .arg(port.to_string())
        .kill_on_drop(true)
        .output()
        .await;
    let _ = shutdown.send(());
    server.await??;
    let output = output?;
    anyhow::ensure!(output.status.success(), "browser fixture failed: {}", String::from_utf8_lossy(&output.stderr));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    anyhow::ensure!(report["scope"] == "gateway-recording-auth-browser-fixture" && report["cases"] == 7, "wrong fixture report");
    let entries = observations.lock().unwrap();
    let staged: Vec<_> = entries.iter().filter(|entry| entry["method"] == "POST" && entry["path"] == "/auth/oauth/apple/callback").collect();
    let completed: Vec<_> = entries.iter().filter(|entry| entry["path"] == "/auth/oauth/apple/callback/complete" && entry["method"] == "POST").collect();
    anyhow::ensure!(staged.len() == 4 && staged.iter().all(|entry| entry["stateCookies"] == 0), "cross-site Lax fixture boundary failed");
    anyhow::ensure!(completed.iter().any(|entry| entry["stateCookies"] == 0) && completed.iter().any(|entry| entry["stateCookies"] == 2), "missing negative cookie cases");
    anyhow::ensure!(harness.clients.oauth_callback_calls.lock().unwrap().len() == 2, "unbound/denied browser reached recording Auth");
    eprintln!("OAUTH_BROWSER_FIXTURE {}", report);
    Ok(())
}

async fn record(
    State(observations): State<Arc<Mutex<Vec<serde_json::Value>>>>,
    request: Request<Body>, next: Next,
) -> Response {
    // Never persist signed state, authorization codes or refresh credentials.
    let state_cookies = request.headers().get_all("cookie").iter()
        .filter_map(|value| value.to_str().ok()).flat_map(|value| value.split(';'))
        .filter(|value| value.trim().split_once('=').is_some_and(|(name, _)| name == "oauth_state_apple"))
        .count();
    observations.lock().unwrap().push(serde_json::json!({
        "method": request.method().as_str(), "path": request.uri().path(), "stateCookies": state_cookies,
        "fetchSite": request.headers().get("sec-fetch-site").and_then(|value| value.to_str().ok()),
    }));
    next.run(request).await
}
