//! Retain an admitted producer scope through actual response-body consumption.
//! A dropped/error body never certifies settlement. Completion may only query
//! and seal existing work; it cannot renew the original content authority.
use futures::future::BoxFuture;
use http_body::{Body, Frame, SizeHint};
use std::{
    error::Error,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tonic::Status;

type BoxError = Box<dyn Error + Send + Sync>;
/// Implemented by the canonical scope. Registration in a response is not proof
/// of completion; only the body layer invokes this after the serializer ends.
pub trait ResponseScope: Send + Sync + 'static {
    fn finish_response(self: Arc<Self>) -> BoxFuture<'static, Result<(), Status>>;
}
#[derive(Clone)]
pub struct ResponseScopeToken(Arc<dyn ResponseScope>);
impl ResponseScopeToken {
    pub fn new(scope: Arc<dyn ResponseScope>) -> Self {
        Self(scope)
    }
}
#[derive(Clone, Copy, Default)]
pub struct RetainedScopeLayer;
impl<S> tower::Layer<S> for RetainedScopeLayer {
    type Service = RetainedScopeService<S>;
    fn layer(&self, inner: S) -> Self::Service {
        RetainedScopeService { inner }
    }
}
#[derive(Clone)]
pub struct RetainedScopeService<S> {
    inner: S,
}
impl<S, R, B> tower::Service<R> for RetainedScopeService<S>
where
    S: tower::Service<R, Response = http::Response<B>> + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Send + 'static,
    R: Send + 'static,
    B: Body + Send + 'static,
    B::Error: Into<BoxError>,
{
    type Response = http::Response<ScopeBody<B>>;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
    fn call(&mut self, request: R) -> Self::Future {
        let future = self.inner.call(request);
        Box::pin(async move {
            let response = future.await?;
            let (mut parts, body) = response.into_parts();
            let scope = parts
                .extensions
                .remove::<ResponseScopeToken>()
                .map(|token| token.0);
            Ok(http::Response::from_parts(
                parts,
                ScopeBody::new(body, scope),
            ))
        })
    }
}
pub struct ScopeBody<B> {
    inner: Pin<Box<B>>,
    scope: Option<Arc<dyn ResponseScope>>,
    completion: Option<BoxFuture<'static, Result<(), Status>>>,
    trailers: Option<http::HeaderMap>,
    ended: bool,
    failed: bool,
}
impl<B> ScopeBody<B> {
    fn new(inner: B, scope: Option<Arc<dyn ResponseScope>>) -> Self {
        Self {
            inner: Box::pin(inner),
            scope,
            completion: None,
            trailers: None,
            ended: false,
            failed: false,
        }
    }
}
impl<B> Body for ScopeBody<B>
where
    B: Body,
    B::Error: Into<BoxError>,
{
    type Data = B::Data;
    type Error = BoxError;
    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = self.get_mut();
        if this.failed {
            return Poll::Ready(None);
        }
        if this.scope.is_none() && this.completion.is_none() && !this.ended {
            return this
                .inner
                .as_mut()
                .poll_frame(cx)
                .map(|value| value.map(|result| result.map_err(Into::into)));
        }
        loop {
            if let Some(future) = this.completion.as_mut() {
                match future.as_mut().poll(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(Err(error)) => {
                        this.failed = true;
                        this.completion = None;
                        return Poll::Ready(Some(Err(Box::new(error))));
                    }
                    Poll::Ready(Ok(())) => {
                        this.completion = None;
                        this.ended = true;
                    }
                }
            }
            if this.ended {
                return Poll::Ready(
                    this.trailers
                        .take()
                        .map(|trailers| Ok(Frame::trailers(trailers))),
                );
            }
            match this.inner.as_mut().poll_frame(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Some(Err(error))) => {
                    this.failed = true;
                    return Poll::Ready(Some(Err(error.into())));
                }
                Poll::Ready(Some(Ok(frame))) => match frame.into_trailers() {
                    Ok(trailers) => {
                        this.trailers = Some(trailers);
                    }
                    Err(frame) => return Poll::Ready(Some(Ok(frame))),
                },
                Poll::Ready(None) => {
                    this.completion =
                        Some(this.scope.take().expect("retained scope").finish_response());
                }
            }
        }
    }
    fn is_end_stream(&self) -> bool {
        if self.failed {
            return true;
        }
        if self.scope.is_none() && self.completion.is_none() && !self.ended {
            return self.inner.is_end_stream();
        }
        self.ended && self.trailers.is_none()
    }
    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::{BodyExt, Full, StreamBody};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tonic::codegen::Bytes;
    struct Scope(AtomicUsize);
    impl ResponseScope for Scope {
        fn finish_response(self: Arc<Self>) -> BoxFuture<'static, Result<(), Status>> {
            Box::pin(async move {
                self.0.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
        }
    }
    #[tokio::test]
    async fn successful_body_settles_once_after_serialization_and_before_final_trailers() {
        let scope = Arc::new(Scope(AtomicUsize::new(0)));
        let mut trailers = http::HeaderMap::new();
        trailers.insert("grpc-status", http::HeaderValue::from_static("0"));
        let frames = vec![
            Ok::<_, std::convert::Infallible>(Frame::data(Bytes::from_static(b"data"))),
            Ok(Frame::trailers(trailers)),
        ];
        let mut body = ScopeBody::new(
            StreamBody::new(futures::stream::iter(frames)),
            Some(scope.clone()),
        );
        assert!(!body.is_end_stream());
        assert!(body.frame().await.unwrap().unwrap().is_data());
        assert_eq!(scope.0.load(Ordering::SeqCst), 0);
        assert!(body.frame().await.unwrap().unwrap().is_trailers());
        assert_eq!(scope.0.load(Ordering::SeqCst), 1);
        assert!(body.frame().await.is_none());
    }
    #[tokio::test]
    async fn cancellation_and_body_error_never_certify_completion() {
        let scope = Arc::new(Scope(AtomicUsize::new(0)));
        drop(ScopeBody::new(
            Full::new(Bytes::from_static(b"data")),
            Some(scope.clone()),
        ));
        let frames = vec![Err::<Frame<Bytes>, _>(std::io::Error::other(
            "serializer failure",
        ))];
        let mut body = ScopeBody::new(
            StreamBody::new(futures::stream::iter(frames)),
            Some(scope.clone()),
        );
        assert!(body.frame().await.unwrap().is_err());
        assert!(body.frame().await.is_none());
        assert_eq!(scope.0.load(Ordering::SeqCst), 0);
    }
}
