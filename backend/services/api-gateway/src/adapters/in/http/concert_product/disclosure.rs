//! Check the actual root-wide ceiling at the HTTP data-frame boundary.
//! Completion after EOF may settle existing work; expiry never releases it.
use axum::body::{Body, Bytes};
use http_body::{Frame, SizeHint};
use platform::actor_delegation::authority_deadline::AuthorityRpcDeadline;
use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};
use tonic::Status;

pub(super) struct DisclosureBody {
    inner: Pin<Box<Body>>,
    deadline: AuthorityRpcDeadline,
    ended: bool,
}
impl DisclosureBody {
    pub(super) fn new(
        body: Body,
        until: i64,
        original: AuthorityRpcDeadline,
    ) -> Result<Self, Status> {
        if until <= 0 || until > original.expires_micros() {
            return Err(Status::permission_denied(
                "Invalid product disclosure ceiling",
            ));
        }
        let deadline = AuthorityRpcDeadline::within_original(
            until,
            original.stop().into_std(),
            Duration::from_secs(5),
        )?;
        Ok(Self {
            inner: Box::pin(body),
            deadline,
            ended: false,
        })
    }
}
impl http_body::Body for DisclosureBody {
    type Data = Bytes;
    type Error = Status;
    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Status>>> {
        let this = self.get_mut();
        if this.ended {
            return Poll::Ready(None);
        }
        match this.inner.as_mut().poll_frame(cx) {
            Poll::Ready(Some(Ok(frame))) => {
                // Check AFTER the inner poll and immediately BEFORE handing a
                // data frame to the server. Headers/DTO validity are insufficient.
                if frame.is_data()
                    && this
                        .deadline
                        .remaining(Duration::from_secs(5), this.deadline.expires_micros())
                        .is_err()
                {
                    this.ended = true;
                    return Poll::Ready(Some(Err(Status::deadline_exceeded(
                        "Product disclosure expired",
                    ))));
                }
                Poll::Ready(Some(Ok(frame)))
            }
            Poll::Ready(Some(Err(_))) => {
                this.ended = true;
                Poll::Ready(Some(Err(Status::unavailable("Product body unavailable"))))
            }
            Poll::Ready(None) => {
                this.ended = true;
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }
    fn is_end_stream(&self) -> bool {
        self.ended
    }
    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::BodyExt;
    use platform::actor_delegation::response_scope::{
        ResponseScope, ResponseScopeToken, RetainedScopeLayer,
    };
    use std::{
        future::Future,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
    };
    use tower::{Layer, ServiceExt};

    struct Terminal(Arc<AtomicUsize>);
    impl ResponseScope for Terminal {
        fn finish_response(
            self: Arc<Self>,
        ) -> Pin<Box<dyn Future<Output = Result<(), Status>> + Send>> {
            Box::pin(async move {
                self.0.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
        }
    }
    fn original() -> AuthorityRpcDeadline {
        AuthorityRpcDeadline::new(
            chrono::Utc::now().timestamp_micros() + 4_000_000,
            Duration::from_secs(4),
        )
        .unwrap()
    }
    async fn response() -> (impl http_body::Body<Data = Bytes>, Arc<AtomicUsize>) {
        let deadline = original();
        let body = DisclosureBody::new(
            Body::from("private-code"),
            deadline.expires_micros(),
            deadline,
        )
        .unwrap();
        let count = Arc::new(AtomicUsize::new(0));
        let token = ResponseScopeToken::new(Arc::new(Terminal(count.clone())));
        let service = tower::service_fn(move |()| {
            let mut response = http::Response::new(body);
            response.extensions_mut().insert(token);
            async move { Ok::<_, std::convert::Infallible>(response) }
        });
        (
            RetainedScopeLayer
                .layer(service)
                .oneshot(())
                .await
                .unwrap()
                .into_body(),
            count,
        )
    }
    #[tokio::test(start_paused = true)]
    async fn elapsed_monotonic_stop_withholds_bytes_and_does_not_complete_scope() {
        let (body, count) = response().await;
        tokio::time::advance(Duration::from_secs(5)).await;
        assert!(body.collect().await.is_err());
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }
    #[tokio::test(start_paused = true)]
    async fn terminal_eof_may_settle_after_previously_valid_data() {
        let (body, count) = response().await;
        let mut body = Box::pin(body);
        let frame = body.frame().await.unwrap().unwrap();
        assert_eq!(
            frame.into_data().unwrap(),
            Bytes::from_static(b"private-code")
        );
        assert_eq!(count.load(Ordering::SeqCst), 0);
        tokio::time::advance(Duration::from_secs(5)).await;
        assert!(body.frame().await.is_none());
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }
    #[tokio::test]
    async fn absent_or_extended_expiry_is_not_disclosure_authority() {
        let original = original();
        for until in [0, -1, original.expires_micros() + 1] {
            assert!(DisclosureBody::new(Body::empty(), until, original).is_err());
        }
    }
}
