//! Test-only observation of actual server futures, including cancellation/drop.
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};

#[derive(Default)]
pub struct Activity {
    pub active: AtomicUsize,
    pub started: AtomicUsize,
    pub finished: AtomicUsize,
}
pub struct Flight(Arc<Activity>);
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Snapshot {
    pub active: usize,
    pub started: usize,
    pub finished: usize,
}
impl Snapshot {
    pub fn quiescent(self) -> bool {
        self.active == 0 && self.started == self.finished
    }
}
impl Activity {
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            started: self.started.load(Ordering::SeqCst),
            active: self.active.load(Ordering::SeqCst),
            finished: self.finished.load(Ordering::SeqCst),
        }
    }
    pub fn enter(self: &Arc<Self>) -> Flight {
        self.started.fetch_add(1, Ordering::SeqCst);
        self.active.fetch_add(1, Ordering::SeqCst);
        Flight(self.clone())
    }
}
impl Drop for Flight {
    fn drop(&mut self) {
        self.0.finished.fetch_add(1, Ordering::SeqCst);
        self.0.active.fetch_sub(1, Ordering::SeqCst);
    }
}

#[derive(Clone)]
pub struct RpcActivity<S> {
    pub inner: S,
    pub activity: Arc<Activity>,
}
impl<S, B> tower::Service<http::Request<B>> for RpcActivity<S>
where
    S: tower::Service<http::Request<B>>,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
    fn call(&mut self, request: http::Request<B>) -> Self::Future {
        let flight = (request.uri().path() == "/auth.v1.AuthService/OAuthCallback")
            .then(|| self.activity.enter());
        let future = self.inner.call(request);
        Box::pin(async move {
            let _flight = flight;
            future.await
        })
    }
}
