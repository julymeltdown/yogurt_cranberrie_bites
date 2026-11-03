use anyhow::Context;
use axum::Router;
use std::net::SocketAddr;
use tokio::net::TcpListener;

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
    let application = async {
        axum::serve(listeners.application, application_router)
            .with_graceful_shutdown(platform::shutdown::global().cancelled())
            .await
            .context("application HTTP server failed")
    };
    let metrics = async {
        axum::serve(listeners.metrics, metrics_router)
            .with_graceful_shutdown(platform::shutdown::global().cancelled())
            .await
            .context("metrics HTTP server failed")
    };
    tokio::try_join!(application, metrics)?;
    Ok(())
}
