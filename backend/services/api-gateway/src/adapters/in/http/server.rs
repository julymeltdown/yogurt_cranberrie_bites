use super::client_identity::TrustedProxies;
use super::{matched_route, GatewayError};
use crate::config::{build_cors_layer, AppConfig};
use anyhow::Context;
use axum::body::Body;
use axum::http::header::{HeaderName, AUTHORIZATION, CONTENT_TYPE};
use axum::http::{HeaderValue, Method, Request};
use axum::middleware::{from_fn, from_fn_with_state, Next};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Router};
use std::net::SocketAddr;
use tokio::net::TcpListener;

#[derive(Clone, Copy, Debug)]
pub struct DurableJournalEnabled;

#[derive(Debug)]
pub struct HttpListeners {
    application: TcpListener,
    metrics: TcpListener,
}

impl HttpListeners {
    pub fn application_addr(&self) -> std::io::Result<SocketAddr> {
        self.application.local_addr()
    }

    pub fn metrics_addr(&self) -> std::io::Result<SocketAddr> {
        self.metrics.local_addr()
    }
}

/// The executable and transport regression tests share this composition.
/// Bare route-only harnesses do not assert runtime transport coverage.
pub fn with_transport_layers(
    application_router: Router,
    config: &AppConfig,
    trusted_proxies: TrustedProxies,
) -> Router {
    let cors = build_cors_layer(config)
        .allow_headers([
            AUTHORIZATION,
            CONTENT_TYPE,
            HeaderName::from_static("x-request-id"),
            HeaderName::from_static("x-correlation-id"),
            HeaderName::from_static("x-client-id"),
            HeaderName::from_static("x-mcr-intent"),
            HeaderName::from_static("idempotency-key"),
            HeaderName::from_static("traceparent"),
            HeaderName::from_static("tracestate"),
        ])
        .expose_headers([
            HeaderName::from_static("x-correlation-id"),
            HeaderName::from_static("x-operation-id"),
            HeaderName::from_static("idempotency-replayed"),
            HeaderName::from_static("retry-after"),
        ]);
    application_router
        .layer(from_fn(enforce_idempotency_scope))
        .layer(cors)
        .layer(Extension(DurableJournalEnabled))
        .layer(Extension(trusted_proxies))
        .layer(platform::trace_context::TraceContextLayer)
        .layer(from_fn(super::oauth_routes::protect_form_post_response))
        // Also cover transport rejections/preflights before the application router.
        .layer(from_fn_with_state(
            super::browser_auth::BrowserBoundaryState {
                origins: config.cors_allowed_origins.clone(),
                oauth: config.oauth_exposure.clone(),
            },
            super::browser_auth::boundary,
        ))
        .layer(from_fn(super::concert_bootstrap::protect_response))
        .layer(from_fn(super::concert_product::protect_response))
        .layer(from_fn(super::concert_participant::protect_response))
        // Outermost body boundary: inner middleware may transform response data.
        // Preserve canonical product scopes until actual terminal HTTP framing.
        .layer(platform::actor_delegation::response_scope::RetainedScopeLayer)
}

// Restrict replay to command results, never read-by-POST profile batches or
// privileged/admin/auth responses. Expanding this list requires a separate
// authorization/privacy review for the proposed response shape.
pub(super) fn supports_idempotency(method: &Method, route: &str) -> bool {
    idempotency_owner(method, route).is_some()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MutationOwner {
    Post,
    Friend,
    Profile,
    Gateway,
}

impl MutationOwner {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Post => "post-service",
            Self::Friend => "friend-service",
            Self::Profile => "profile-service",
            Self::Gateway => "gateway-service",
        }
    }
}

pub(super) fn idempotency_owner(method: &Method, route: &str) -> Option<MutationOwner> {
    matches!(
        (method.as_str(), route),
        ("POST", "/api/posts")
            | ("DELETE", "/api/posts/:post_id")
            | ("POST", "/api/posts/:post_id/comments")
            | ("POST", "/api/posts/:post_id/likes")
            | ("DELETE", "/api/posts/:post_id/likes")
    )
    .then_some(MutationOwner::Post)
    .or_else(|| {
        matches!(
            (method.as_str(), route),
            ("POST", "/api/follows") | ("DELETE", "/api/follows/:target_user_id")
        )
        .then_some(MutationOwner::Friend)
    })
    .or_else(|| {
        matches!(
            (method.as_str(), route),
            ("PUT", "/api/profile/privacy") | ("PUT", "/api/profile/:user_id")
        )
        .then_some(MutationOwner::Profile)
    })
    .or_else(|| {
        matches!(
            (method.as_str(), route),
            ("POST", "/api/moderation/reports")
        )
        .then_some(MutationOwner::Gateway)
    })
}

async fn enforce_idempotency_scope(request: Request<Body>, next: Next) -> Response {
    if request.headers().contains_key("idempotency-key")
        && !supports_idempotency(request.method(), matched_route(&request))
        && !super::concert_product::owns_receipt(request.method(), request.uri().path())
    {
        let mut response = GatewayError::bad_request(
            "Idempotency-Key is supported only for post, follow/unfollow, profile edits/privacy, and moderation report creation",
        )
        .into_response();
        response.headers_mut().insert(
            "x-correlation-id",
            HeaderValue::from_str(&uuid::Uuid::new_v4().to_string()).expect("UUID header"),
        );
        return response;
    }
    let mut response = next.run(request).await;
    if response.headers().contains_key("idempotency-replayed") {
        response.headers_mut().insert(
            axum::http::header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        );
    }
    response
}

pub async fn bind_http_listeners(
    application_addr: SocketAddr,
    metrics_addr: SocketAddr,
) -> anyhow::Result<HttpListeners> {
    let application = TcpListener::bind(application_addr).await.with_context(|| {
        format!("failed to bind application HTTP listener at {application_addr}")
    })?;
    let metrics = TcpListener::bind(metrics_addr)
        .await
        .with_context(|| format!("failed to bind metrics HTTP listener at {metrics_addr}"))?;
    Ok(HttpListeners {
        application,
        metrics,
    })
}

pub async fn serve_http_listeners(
    listeners: HttpListeners,
    application_router: Router,
    metrics_router: Router,
) -> anyhow::Result<()> {
    let application = async move {
        axum::serve(
            listeners.application,
            application_router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(platform::shutdown::global().cancelled())
        .await
        .context("application HTTP server failed")
    };
    let metrics = async move {
        axum::serve(listeners.metrics, metrics_router)
            .with_graceful_shutdown(platform::shutdown::global().cancelled())
            .await
            .context("metrics HTTP server failed")
    };
    tokio::try_join!(application, metrics)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_scope_excludes_reads_auth_and_administration() {
        assert!(supports_idempotency(&Method::POST, "/api/posts"));
        assert!(supports_idempotency(&Method::DELETE, "/api/posts/:post_id"));
        assert!(supports_idempotency(
            &Method::POST,
            "/api/posts/:post_id/comments"
        ));
        assert!(supports_idempotency(
            &Method::DELETE,
            "/api/posts/:post_id/likes"
        ));
        assert!(supports_idempotency(&Method::POST, "/api/follows"));
        assert!(supports_idempotency(
            &Method::DELETE,
            "/api/follows/:target_user_id"
        ));
        assert!(supports_idempotency(&Method::PUT, "/api/profile/privacy"));
        assert_eq!(
            idempotency_owner(&Method::PUT, "/api/profile/privacy"),
            Some(MutationOwner::Profile)
        );
        assert!(supports_idempotency(&Method::PUT, "/api/profile/:user_id"));
        assert_eq!(
            idempotency_owner(&Method::PUT, "/api/profile/:user_id"),
            Some(MutationOwner::Profile)
        );
        assert_eq!(MutationOwner::Profile.as_str(), "profile-service");
        assert!(supports_idempotency(
            &Method::POST,
            "/api/moderation/reports"
        ));
        assert_eq!(
            idempotency_owner(&Method::POST, "/api/moderation/reports"),
            Some(MutationOwner::Gateway)
        );
        assert!(!supports_idempotency(&Method::POST, "/api/profile/batch"));
        assert!(!supports_idempotency(&Method::GET, "/api/posts/:post_id"));
        assert!(!supports_idempotency(&Method::POST, "/auth/login"));
        assert!(!supports_idempotency(&Method::POST, "/admin/moderation"));
        assert!(!supports_idempotency(
            &Method::DELETE,
            "/api/moderation/reports"
        ));
        assert!(!supports_idempotency(&Method::POST, "unmatched"));
    }
}
