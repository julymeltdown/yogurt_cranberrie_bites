//! Actual Chromium HTTP navigation across localhost and 127.0.0.1 sites.
//! Auth is a recording double here; this does not prove Auth persistence.
#[allow(dead_code)]
#[path = "unit/http_support.rs"]
mod http_support;

use api_gateway::adapters::r#in::http::oauth_routes;
use axum::extract::State;
use axum::http::Request;
use axum::middleware::{from_fn_with_state, Next};
use axum::response::{Html, Response};
use axum::routing::get;
use std::sync::{Arc, Mutex};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actual_browser_cross_site_post_restores_original_lax_cookie_only_on_completion(
) -> anyhow::Result<()> {
    let harness = http_support::RouteHarness::new().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let observations = Arc::new(Mutex::new(Vec::<serde_json::Value>::new()));
    let app = oauth_routes::router(harness.state.clone())
        .route("/fixture/provider", get(move || async move {
            Html(format!("<!doctype html><title>Fixture provider</title><form method=\"post\" action=\"http://127.0.0.1:{port}/auth/oauth/apple/callback\"><input name=\"code\" value=\"fixture-code\" type=\"hidden\"><input name=\"state\" value=\"apple-state\" type=\"hidden\"><button>Authorize fixture</button></form>"))
        }))
        .layer(from_fn_with_state(observations.clone(), record));
    let (shutdown, stopped) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = stopped.await;
            })
            .await
    });
    let output = tokio::process::Command::new(
        std::env::var("OAUTH_BROWSER_NODE").unwrap_or_else(|_| "node".into()),
    )
    .arg(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../scripts/testing/oauth-form-post-browser.cjs"),
    )
    .arg(port.to_string())
    .output()
    .await;
    let _ = shutdown.send(());
    server.await??;
    let output = output?;
    anyhow::ensure!(
        output.status.success(),
        "browser failed: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    eprintln!("OAUTH_BROWSER {}", String::from_utf8_lossy(&output.stdout));
    let observations = observations.lock().unwrap();
    let staged: Vec<_> = observations
        .iter()
        .filter(|entry| entry["method"] == "POST" && entry["path"] == "/auth/oauth/apple/callback")
        .collect();
    let completed: Vec<_> = observations
        .iter()
        .filter(|entry| entry["path"] == "/auth/oauth/apple/callback/complete")
        .collect();
    anyhow::ensure!(
        staged.len() == 3 && completed.len() == 3,
        "missing browser submissions: {observations:?}"
    );
    anyhow::ensure!(
        staged.iter().all(|entry| entry["cookie"].is_null()),
        "Lax cookie leaked on cross-site POST"
    );
    anyhow::ensure!(
        completed[0]["cookie"] == "oauth_state_apple=apple-state",
        "original cookie missing"
    );
    anyhow::ensure!(
        completed[1]["cookie"].is_null(),
        "other browser gained state cookie"
    );
    anyhow::ensure!(
        completed[2]["cookie"] == "oauth_state_apple=apple-state",
        "manual completion missing original cookie"
    );
    anyhow::ensure!(
        harness.clients.oauth_callback_calls.lock().unwrap().len() == 2,
        "unbound browser reached Auth"
    );
    eprintln!(
        "OAUTH_BROWSER_OBSERVATIONS {}",
        serde_json::to_string(&*observations)?
    );
    Ok(())
}

async fn record(
    State(observations): State<Arc<Mutex<Vec<serde_json::Value>>>>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    observations.lock().unwrap().push(serde_json::json!({
        "method": request.method().as_str(), "path": request.uri().path(),
        "cookie": request.headers().get("cookie").and_then(|value| value.to_str().ok()),
        "fetchSite": request.headers().get("sec-fetch-site").and_then(|value| value.to_str().ok())
    }));
    next.run(request).await
}
