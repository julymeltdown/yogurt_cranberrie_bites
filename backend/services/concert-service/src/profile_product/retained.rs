//! A named wrapper so the retained-scope layer can be added to a tonic server.
//!
//! This is the concert-hosted copy of
//! `profile_service::profile_product::NamedRetainedScope`. The layer itself
//! (`platform::actor_delegation::response_scope::RetainedScopeLayer`) is shared
//! platform code; only the service *name* differs, because this transport is
//! `concert.profile_product.v1` rather than `profile.product.v1`.
use platform::actor_delegation::response_scope::RetainedScopeService;

pub struct NamedRetainedScope<S, B> {
    inner: RetainedScopeService<S>,
    marker: std::marker::PhantomData<fn() -> B>,
}

impl<S, B> NamedRetainedScope<S, B> {
    pub fn new(service: S) -> Self {
        use tower::Layer;
        Self {
            inner: platform::actor_delegation::response_scope::RetainedScopeLayer.layer(service),
            marker: std::marker::PhantomData,
        }
    }
}

impl<S: Clone, B> Clone for NamedRetainedScope<S, B> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            marker: std::marker::PhantomData,
        }
    }
}

impl<S, B> tonic::server::NamedService for NamedRetainedScope<S, B> {
    const NAME: &'static str = "concert.profile_product.v1.ProfileProductService";
}

impl<S, B> tower::Service<http::Request<tonic::body::Body>> for NamedRetainedScope<S, B>
where
    S: tower::Service<http::Request<tonic::body::Body>, Response = http::Response<B>>
        + Send
        + 'static,
    S::Future: Send + 'static,
    S::Error: Send + 'static,
    B: http_body::Body + Send + 'static,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    type Response = http::Response<platform::actor_delegation::response_scope::ScopeBody<B>>;
    type Error = S::Error;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;
    fn poll_ready(
        &mut self,
        context: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(context)
    }
    fn call(&mut self, request: http::Request<tonic::body::Body>) -> Self::Future {
        Box::pin(self.inner.call(request))
    }
}
