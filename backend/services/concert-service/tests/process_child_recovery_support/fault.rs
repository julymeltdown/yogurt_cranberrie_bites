//! Negative-only transport fault around actual guarded owner servers.
//! Every method except exact /Complete delegates to the original service.
use std::{
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    task::{Context, Poll},
};
use tonic::server::NamedService;
use tower::Service;

#[derive(Clone)]
pub(super) struct NegativeComplete<S> {
    inner: S,
    enabled: Arc<AtomicBool>,
    failures: Arc<AtomicUsize>,
}
impl<S> NegativeComplete<S> {
    pub(super) fn new(inner: S, enabled: bool) -> Self {
        Self::controlled(
            inner,
            Arc::new(AtomicBool::new(enabled)),
            Arc::new(AtomicUsize::new(0)),
        )
    }
    pub(super) fn controlled(
        inner: S,
        enabled: Arc<AtomicBool>,
        failures: Arc<AtomicUsize>,
    ) -> Self {
        Self {
            inner,
            enabled,
            failures,
        }
    }
}
impl<S: NamedService> NamedService for NegativeComplete<S> {
    const NAME: &'static str = S::NAME;
}
impl<S, B, R> Service<axum::http::Request<B>> for NegativeComplete<S>
where
    S: NamedService
        + Service<axum::http::Request<B>, Response = axum::http::Response<R>>
        + Send
        + 'static,
    S::Future: Send + 'static,
    S::Error: Send + 'static,
    R: axum::body::HttpBody<Data = tonic::codegen::Bytes> + Send + 'static,
    R::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    type Response = axum::http::Response<tonic::body::Body>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
    fn call(&mut self, request: axum::http::Request<B>) -> Self::Future {
        if self.enabled.load(Ordering::SeqCst)
            && request.uri().path() == format!("/{}/Complete", S::NAME)
        {
            self.failures.fetch_add(1, Ordering::SeqCst);
            return Box::pin(async {
                Ok(
                    tonic::Status::unavailable("CR01 injected Complete unavailability")
                        .into_http::<tonic::body::Body>(),
                )
            });
        }
        let future = self.inner.call(request);
        Box::pin(async move {
            future
                .await
                .map(|response| response.map(tonic::body::Body::new))
        })
    }
}
