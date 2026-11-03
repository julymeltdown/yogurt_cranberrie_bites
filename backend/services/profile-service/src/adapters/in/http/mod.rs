use crate::config::ProfileMetrics;
use anyhow::Context;
use axum::extract::State;
use axum::http::header::CONTENT_TYPE;
use axum::http::HeaderValue;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use std::future::{Future, IntoFuture};
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::watch;

pub fn router() -> Router {
    Router::new().layer(platform::trace_context::TraceContextLayer)
}

pub fn metrics_router(metrics_state: Arc<ProfileMetrics>) -> Router {
    Router::new()
        .route("/metrics", get(metrics))
        .with_state(metrics_state)
}

pub async fn bind_listeners(
    application_addr: SocketAddr,
    metrics_addr: SocketAddr,
) -> anyhow::Result<(TcpListener, TcpListener)> {
    let application = TcpListener::bind(application_addr)
        .await
        .with_context(|| format!("failed to bind application listener at {application_addr}"))?;
    let metrics = TcpListener::bind(metrics_addr)
        .await
        .with_context(|| format!("failed to bind metrics listener at {metrics_addr}"))?;
    Ok((application, metrics))
}

pub async fn serve_until<F>(
    application_listener: TcpListener,
    application_router: Router,
    metrics_listener: TcpListener,
    metrics_router: Router,
    shutdown: F,
) -> io::Result<()>
where
    F: Future<Output = ()> + Send,
{
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let application_server = axum::serve(application_listener, application_router)
        .with_graceful_shutdown(shutdown_requested(shutdown_rx.clone()))
        .into_future();
    let metrics_server = axum::serve(metrics_listener, metrics_router)
        .with_graceful_shutdown(shutdown_requested(shutdown_rx))
        .into_future();
    tokio::pin!(application_server, metrics_server, shutdown);

    tokio::select! {
        result = &mut application_server => result,
        result = &mut metrics_server => result,
        () = &mut shutdown => {
            shutdown_tx.send(true).map_err(|_| io::Error::other("HTTP listeners stopped before shutdown"))?;
            let (application_result, metrics_result) = tokio::join!(&mut application_server, &mut metrics_server);
            application_result?;
            metrics_result
        }
    }
}

async fn shutdown_requested(mut shutdown: watch::Receiver<bool>) {
    if *shutdown.borrow() {
        return;
    }
    while shutdown.changed().await.is_ok() {
        if *shutdown.borrow_and_update() {
            return;
        }
    }
}

async fn metrics(State(metrics): State<Arc<ProfileMetrics>>) -> Response {
    (
        [(
            CONTENT_TYPE,
            HeaderValue::from_static("text/plain; version=0.0.4; charset=utf-8"),
        )],
        metrics.render_prometheus(),
    )
        .into_response()
}

pub fn route_count() -> usize {
    0
}
