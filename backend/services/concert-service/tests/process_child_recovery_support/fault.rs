//! Negative-only transport fault around the actual guarded Auth owner server.
//! Every method except exact Auth /Complete delegates to the original service.
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tonic::server::NamedService;
use tower::Service;

#[derive(Clone)]
pub(super) struct NegativeComplete<S> {
    inner: S,
    enabled: bool,
}
impl<S> NegativeComplete<S> {
    pub(super) fn new(inner: S, enabled: bool) -> Self {
        Self { inner, enabled }
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
    R: Default + Send + 'static,
{
    type Response = axum::http::Response<R>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
    fn call(&mut self, request: axum::http::Request<B>) -> Self::Future {
        if self.enabled && request.uri().path() == format!("/{}/Complete", S::NAME) {
            return Box::pin(async {
                Ok(
                    tonic::Status::unavailable("CR01 injected Auth Complete failure")
                        .into_http()
                        .map(|_| R::default()),
                )
            });
        }
        Box::pin(self.inner.call(request))
    }
}
