mod contract;

use crate::config::{DeveloperPortalLinks, RuntimeMode};
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use std::fmt::Write;

pub fn router(mode: RuntimeMode, links: DeveloperPortalLinks) -> Router {
    Router::new()
        .route("/docs", get(move || portal(mode, links.clone())))
        .route("/openapi.json", get(openapi_contract))
}

async fn openapi_contract() -> Response {
    match contract::bundled_contract() {
        Ok(document) => (
            [
                (CONTENT_TYPE, HeaderValue::from_static("application/json")),
                (CACHE_CONTROL, HeaderValue::from_static("no-cache")),
            ],
            document,
        )
            .into_response(),
        Err(error) => {
            tracing::error!(error, "embedded OpenAPI contract is invalid");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Serve the portal appropriate to the runtime mode, with only validated
/// deployment links supplied by Gateway configuration.
async fn portal(mode: RuntimeMode, links: DeveloperPortalLinks) -> Html<String> {
    let (title, intro) = if mode.allows_parity_routes() {
        (
            "Port Rust Developer Portal",
            "The API Gateway publishes its checked-in OpenAPI 3.1 contract. Developer tools are linked according to this runtime's configuration.",
        )
    } else {
        (
            "Port Rust API",
            "This Gateway publishes its OpenAPI 3.1 contract for clients and integrators. Operational dashboards are linked only when configured for this deployment.",
        )
    };

    let mut tool_links = String::new();
    append_link(
        &mut tool_links,
        links.swagger(),
        "Swagger UI",
        "Interactive REST reference",
    );
    append_link(
        &mut tool_links,
        links.grafana(),
        "Grafana",
        "Metrics dashboards",
    );
    append_link(
        &mut tool_links,
        links.profiles(),
        "Profiles Drilldown",
        "Query-less flame graph exploration",
    );
    append_link(
        &mut tool_links,
        links.prometheus(),
        "Prometheus",
        "Targets, PromQL, and alerts",
    );
    append_link(
        &mut tool_links,
        links.pyroscope(),
        "Pyroscope",
        "Continuous CPU profile backend",
    );

    Html(format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1">
  <title>{title}</title>
  <style>
    :root {{ color-scheme: dark; font-family: ui-sans-serif, system-ui, sans-serif; }}
    body {{ max-width: 920px; margin: 0 auto; padding: 64px 24px; background: #111; color: #eee; }}
    h1 {{ font-size: 2.25rem; margin-bottom: 8px; }}
    p {{ color: #bbb; line-height: 1.6; }}
    .grid {{ display: grid; grid-template-columns: repeat(auto-fit,minmax(240px,1fr)); gap: 16px; margin-top: 32px; }}
    a {{ display: block; border: 1px solid #444; padding: 20px; color: #fff; text-decoration: none; background: #191919; }}
    a:hover {{ border-color: #999; }}
    code {{ color: #9ee493; }}
  </style>
</head>
<body>
  <h1>{title}</h1>
  <p>{intro}</p>
  <div class="grid">
    <a href="/openapi.json"><strong>Raw OpenAPI contract</strong><p><code>/openapi.json</code></p></a>
    {tool_links}
  </div>
</body>
</html>"#
    ))
}

fn append_link(html: &mut String, url: Option<&str>, title: &str, description: &str) {
    let Some(url) = url else {
        return;
    };
    let href = escape_html_attribute(url);
    let _ = write!(
        html,
        "\n    <a href=\"{href}\" target=\"_blank\" rel=\"noopener noreferrer\"><strong>{title}</strong><p>{description}</p></a>"
    );
}

fn escape_html_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}
