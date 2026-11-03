use crate::edge::Edge;
use crate::edge_metrics::EdgeDeadlineMetrics;
use std::future::Future;
use std::time::Duration;
use tonic::{Code, Status};

/// Leave time for the local deadline to win before the transport timeout.
pub const LOCAL_TIMEOUT_MARGIN: Duration = Duration::from_millis(100);

/// Keep the caller-side deadline ahead of the transport-level edge deadline.
pub fn local_deadline(edge: Edge) -> Duration {
    edge.budget().deadline.saturating_sub(LOCAL_TIMEOUT_MARGIN)
}

/// Bound a generated-client future, including channel readiness, and count
/// local or peer-reported `DEADLINE_EXCEEDED` exactly once.
pub async fn with_edge_deadline<T, F>(
    edge: Edge,
    metrics: EdgeDeadlineMetrics,
    call: F,
) -> Result<T, Status>
where
    T: Send,
    F: Future<Output = Result<T, Status>> + Send,
{
    with_deadline(local_deadline(edge), edge, metrics, call).await
}

/// Apply a shorter caller-supplied deadline while retaining the shared edge
/// accounting. This is useful when a route has less time than its edge budget.
/// The caller's remaining time, when one is in force, shortens it further.
/// The call also holds one of the edge's in-flight permits until it ends.
pub async fn with_deadline<T, F>(
    deadline: Duration,
    edge: Edge,
    metrics: EdgeDeadlineMetrics,
    call: F,
) -> Result<T, Status>
where
    T: Send,
    F: Future<Output = Result<T, Status>> + Send,
{
    with_deadline_on(
        crate::edge_concurrency::EdgeLimiter::global(),
        deadline,
        edge,
        metrics,
        call,
    )
    .await
}

/// [with_deadline] against an explicit limiter.
pub async fn with_deadline_on<T, F>(
    limiter: &crate::edge_concurrency::EdgeLimiter,
    deadline: Duration,
    edge: Edge,
    metrics: EdgeDeadlineMetrics,
    call: F,
) -> Result<T, Status>
where
    T: Send,
    F: Future<Output = Result<T, Status>> + Send,
{
    // A caller with no time left gets no dispatch: reject before the call is
    // polled, so nothing is sent and the exhaustion is counted once.
    if crate::call_deadline::remaining() == Some(Duration::ZERO) {
        metrics.record_deadline_exhausted(edge);
        return Err(Status::deadline_exceeded("Request timed out"));
    }
    // A full edge refuses before dispatch. The permit lives as long as this
    // future, so completion, timeout and caller cancellation all release it.
    let Some(_permit) = limiter.try_acquire(edge) else {
        metrics.record_in_flight_rejected(edge);
        return Err(Status::resource_exhausted(
            "Dependency is at its concurrency limit",
        ));
    };
    let deadline = crate::call_deadline::bounded(deadline);
    match tokio::time::timeout(deadline, call).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(status)) => {
            if status.code() == Code::DeadlineExceeded {
                metrics.record_deadline_exhausted(edge);
            }
            Err(status)
        }
        Err(_) => {
            metrics.record_deadline_exhausted(edge);
            Err(Status::deadline_exceeded("Request timed out"))
        }
    }
}

/// Extension method for the generated tonic client futures.
/// [with_edge_deadline] for a call made while serving another workload's
/// in-flight request on the same edge; it draws on [EdgeLimiter::callbacks]
/// so the outer calls cannot exhaust the capacity it needs.
///
/// [EdgeLimiter::callbacks]: crate::edge_concurrency::EdgeLimiter::callbacks
pub async fn with_callback_edge_deadline<T, F>(
    edge: Edge,
    metrics: EdgeDeadlineMetrics,
    call: F,
) -> Result<T, Status>
where
    T: Send,
    F: Future<Output = Result<T, Status>> + Send,
{
    with_deadline_on(
        crate::edge_concurrency::EdgeLimiter::callbacks(),
        local_deadline(edge),
        edge,
        metrics,
        call,
    )
    .await
}

/// Extension method for the generated tonic client futures.
pub trait EdgeDeadlineCallExt<T>: Future<Output = Result<T, Status>> + Sized {
    fn with_edge_deadline(
        self,
        edge: Edge,
        metrics: EdgeDeadlineMetrics,
    ) -> impl Future<Output = Result<T, Status>> + Send
    where
        Self: Send,
        T: Send,
    {
        with_edge_deadline(edge, metrics, self)
    }
}

impl<T, F> EdgeDeadlineCallExt<T> for F where F: Future<Output = Result<T, Status>> + Sized {}

#[cfg(test)]
mod tests {
    use super::{local_deadline, with_edge_deadline, LOCAL_TIMEOUT_MARGIN};
    use crate::edge::Edge;
    use crate::edge_metrics::EdgeDeadlineMetrics;
    use tonic::{Code, Status};

    #[tokio::test]
    async fn callbacks_are_admitted_while_outer_calls_hold_every_permit() {
        use crate::edge_concurrency::EdgeLimiter;
        let edge = Edge::GatewayPost;
        let held: Vec<_> = std::iter::from_fn(|| EdgeLimiter::global().try_acquire(edge)).collect();
        assert_eq!(held.len(), edge.max_in_flight());
        let metrics = EdgeDeadlineMetrics::default();
        let outer = with_edge_deadline(edge, metrics.clone(), async { Ok::<_, Status>(()) })
            .await
            .expect_err("the outer pool is full");
        assert_eq!(outer.code(), Code::ResourceExhausted);
        let value = super::with_callback_edge_deadline(edge, metrics, async { Ok::<_, Status>(7) })
            .await
            .expect("the callback pool is separate");
        assert_eq!(value, 7);
        drop(held);
        assert_eq!(EdgeLimiter::global().available(edge), edge.max_in_flight());
    }

    #[tokio::test]
    async fn a_full_edge_refuses_before_dispatch_and_cancellation_frees_the_permit() {
        use crate::edge_concurrency::EdgeLimiter;
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;
        use std::time::Duration;
        let limiter = Arc::new(EdgeLimiter::new(|_| 1));
        let metrics = EdgeDeadlineMetrics::default();
        let (entered_tx, entered_rx) = tokio::sync::oneshot::channel::<()>();
        let holder = {
            let limiter = limiter.clone();
            let metrics = metrics.clone();
            tokio::spawn(async move {
                super::with_deadline_on(
                    &limiter,
                    Duration::from_secs(30),
                    Edge::PostFriend,
                    metrics,
                    async move {
                        let _ = entered_tx.send(());
                        std::future::pending::<Result<(), Status>>().await
                    },
                )
                .await
            })
        };
        entered_rx.await.expect("holder entered");
        let polled = Arc::new(AtomicBool::new(false));
        let flag = polled.clone();
        let refused = super::with_deadline_on(
            &limiter,
            Duration::from_secs(1),
            Edge::PostFriend,
            metrics.clone(),
            async move {
                flag.store(true, Ordering::SeqCst);
                Ok::<(), Status>(())
            },
        )
        .await
        .expect_err("the edge is full");
        assert_eq!(refused.code(), Code::ResourceExhausted);
        assert!(
            !polled.load(Ordering::SeqCst),
            "a refused call was dispatched"
        );
        assert!(metrics
            .render_prometheus()
            .contains("grpc_edge_in_flight_rejections_total{edge=\"post->friend\"} 1"));
        holder.abort();
        let _ = holder.await;
        assert_eq!(
            limiter.available(Edge::PostFriend),
            1,
            "cancellation released it"
        );
        super::with_deadline_on(
            &limiter,
            Duration::from_secs(1),
            Edge::PostFriend,
            metrics,
            async { Ok::<(), Status>(()) },
        )
        .await
        .expect("capacity is reusable");
        assert_eq!(limiter.available(Edge::PostFriend), 1);
    }

    #[tokio::test]
    async fn an_exhausted_caller_budget_is_rejected_before_the_call_is_polled() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;
        let metrics = EdgeDeadlineMetrics::default();
        let polled = Arc::new(AtomicBool::new(false));
        let flag = polled.clone();
        let status = crate::call_deadline::scope(std::time::Instant::now(), async {
            with_edge_deadline(Edge::PostFriend, metrics.clone(), async move {
                flag.store(true, Ordering::SeqCst);
                Ok::<(), Status>(())
            })
            .await
        })
        .await
        .expect_err("no time left must not dispatch");
        assert_eq!(status.code(), Code::DeadlineExceeded);
        assert!(!polled.load(Ordering::SeqCst), "the call was polled");
        assert!(metrics
            .render_prometheus()
            .contains("grpc_edge_deadline_exhaustions_total{edge=\"post->friend\"} 1"));
    }

    #[test]
    fn local_deadline_precedes_every_declared_transport_deadline() {
        for edge in Edge::ALL {
            let transport_deadline = edge.budget().deadline;
            let local = local_deadline(edge);

            assert!(
                local < transport_deadline,
                "{edge:?} local timeout must win"
            );
            assert_eq!(transport_deadline - local, LOCAL_TIMEOUT_MARGIN);
        }
    }

    #[tokio::test]
    async fn local_expiry_returns_deadline_exceeded_and_counts_the_edge() {
        let metrics = EdgeDeadlineMetrics::default();
        let status = with_edge_deadline(
            Edge::FriendProfile,
            metrics.clone(),
            std::future::pending::<Result<(), Status>>(),
        )
        .await
        .expect_err("pending call should expire");

        assert_eq!(status.code(), Code::DeadlineExceeded);
        assert!(metrics
            .render_prometheus()
            .contains("grpc_edge_deadline_exhaustions_total{edge=\"friend->profile\"} 1"));
    }

    #[tokio::test]
    async fn peer_deadline_is_counted_but_other_statuses_and_success_are_not() {
        let metrics = EdgeDeadlineMetrics::default();
        let deadline = with_edge_deadline(Edge::AuthNotification, metrics.clone(), async {
            Err::<(), _>(Status::deadline_exceeded("peer deadline"))
        })
        .await
        .expect_err("peer deadline should be preserved");
        assert_eq!(deadline.code(), Code::DeadlineExceeded);

        let unavailable = with_edge_deadline(Edge::PostFriend, metrics.clone(), async {
            Err::<(), _>(Status::unavailable("peer unavailable"))
        })
        .await
        .expect_err("ordinary peer error should be preserved");
        assert_eq!(unavailable.code(), Code::Unavailable);

        assert_eq!(
            with_edge_deadline(Edge::PostNotification, metrics.clone(), async { Ok(42) })
                .await
                .expect("success should be preserved"),
            42
        );

        let rendered = metrics.render_prometheus();
        assert!(rendered
            .contains("grpc_edge_deadline_exhaustions_total{edge=\"auth->notification\"} 1"));
        assert_eq!(rendered.lines().count(), 1);
    }
}
