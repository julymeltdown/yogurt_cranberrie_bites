// Authoring core: e34b90dd8616bc2c51a66d1acaf3521d4dd142c7; source-only draft, compile/runtime NOT_RUN.
//! Negative ReadCurrent transport faults around the actual guarded Profile service.
//! Derived from the assigned starting draft; no successful owner/content fake.
use std::{
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    task::{Context, Poll},
    time::Duration,
};
use tokio::sync::Notify;
use tonic::server::NamedService;
use tower::Service;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Mode {
    None,
    LostReply,
    BeforeDispatch,
}
// No Debug: observations are private, and are not printed by fixture failures.
pub(super) struct State {
    pub mode: Mode,
    pub observed: AtomicUsize,
    pub handler_calls: AtomicUsize,
    pub complete_calls: AtomicUsize,
    pub read_future_dropped: AtomicUsize,
    pub fallback_elapsed: AtomicUsize,
    pub entered: Notify,
}
impl State {
    pub fn new(mode: Mode) -> Self {
        Self {
            mode,
            observed: AtomicUsize::new(0),
            handler_calls: AtomicUsize::new(0),
            complete_calls: AtomicUsize::new(0),
            read_future_dropped: AtomicUsize::new(0),
            fallback_elapsed: AtomicUsize::new(0),
            entered: Notify::new(),
        }
    }
}
#[derive(Clone)]
pub(super) struct ReadFault<S> {
    inner: S,
    state: Arc<State>,
}
impl<S> ReadFault<S> {
    pub fn new(inner: S, state: Arc<State>) -> Self {
        Self { inner, state }
    }
}
impl<S: NamedService> NamedService for ReadFault<S> {
    const NAME: &'static str = S::NAME;
}
impl<S, B, R> Service<axum::http::Request<B>> for ReadFault<S>
where
    S: NamedService
        + Service<axum::http::Request<B>, Response = axum::http::Response<R>>
        + Send
        + 'static,
    S::Future: Send + 'static,
    S::Error: Send + 'static,
    R: axum::body::HttpBody<Data = tonic::codegen::Bytes> + Send + 'static,
    R::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    B: Send + 'static,
{
    type Response = axum::http::Response<tonic::body::Body>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
    fn call(&mut self, request: axum::http::Request<B>) -> Self::Future {
        if request.uri().path() == format!("/{}/Complete", S::NAME) {
            self.state.complete_calls.fetch_add(1, Ordering::SeqCst);
        }
        let target = request.uri().path() == format!("/{}/ReadCurrent", S::NAME);
        if target && self.state.mode == Mode::BeforeDispatch {
            self.state.observed.fetch_add(1, Ordering::SeqCst);
            // Signal only after the actual SDK request reaches transport. The
            // actual Profile handler's Service::call/future is NEVER constructed.
            self.state.entered.notify_one();
            let state = self.state.clone();
            return Box::pin(async move {
                // Bounded negative fallback; the producer must drop its SDK read
                // before this can return. A fallback cannot pass CR05 assertions.
                // Keep the inbound request alive until cancellation/fallback so
                // dropping its unread body cannot race the Notify handshake.
                let held_request = request;
                tokio::time::sleep(Duration::from_millis(100)).await;
                drop(held_request);
                state.fallback_elapsed.fetch_add(1, Ordering::SeqCst);
                Ok(tonic::Status::unavailable("CR05 bounded undispatched read")
                    .into_http::<tonic::body::Body>())
            });
        }
        if target {
            self.state.handler_calls.fetch_add(1, Ordering::SeqCst);
        }
        let future = self.inner.call(request);
        if target && self.state.mode == Mode::LostReply {
            let state = self.state.clone();
            return Box::pin(async move {
                // Actual guarded ReadCurrent has reserved its receipt and prepared
                // its native snapshot before its actual reply is discarded.
                let actual = future.await?;
                drop(actual);
                state.observed.fetch_add(1, Ordering::SeqCst);
                Ok(
                    tonic::Status::unavailable("CR02 actual acquisition reply lost")
                        .into_http::<tonic::body::Body>(),
                )
            });
        }
        Box::pin(async move {
            future
                .await
                .map(|response| response.map(tonic::body::Body::new))
        })
    }
}
