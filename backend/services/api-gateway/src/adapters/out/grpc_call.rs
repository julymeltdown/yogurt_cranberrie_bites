use crate::config::GatewayMetrics;
use platform::edge::Edge;
use platform::grpc_deadline;
use std::future::Future;
use std::time::Duration;
use tonic::Status;

/// Request for an owner write that acts for `actor`. It attaches the owner
/// operation context and asks the delegation client to sign the call. When an
/// operation context is active it must name the same user, so the Gateway
/// never signs one user's authority onto another user's command.
pub(crate) fn owner_request<T>(message: T, actor: &str) -> anyhow::Result<tonic::Request<T>> {
    if let Some(context) = platform::mutation_context::current() {
        anyhow::ensure!(
            uuid::Uuid::parse_str(actor).ok() == Some(context.actor_id),
            "operation actor does not match the command actor"
        );
    }
    Ok(platform::actor_delegation::on_behalf_of(
        platform::mutation_context::attach(message)?,
        actor,
    )?)
}

/// Owner replay or lookup request, acting for the operation's actor.
pub(crate) fn replay_request<T>(message: T) -> anyhow::Result<tonic::Request<T>> {
    let context = platform::mutation_context::current()
        .ok_or_else(|| anyhow::anyhow!("Operation context required for lookup"))?;
    owner_request(message, &context.actor_id.to_string())
}

/// Request that acts for `actor` without an owner operation context.
pub(crate) fn delegated<T>(message: T, actor: &str) -> anyhow::Result<tonic::Request<T>> {
    Ok(platform::actor_delegation::request_for(message, actor)?)
}

/// Bounds the entire generated-client future, including channel readiness.
/// Endpoint timeouts remain in place as a transport-level safeguard.
pub(crate) trait BoundedGrpcCallExt<T>: Future<Output = Result<T, Status>> + Sized {
    fn with_edge_deadline(
        self,
        edge: Edge,
        metrics: GatewayMetrics,
    ) -> impl Future<Output = Result<T, Status>> + Send
    where
        Self: Send,
        T: Send,
    {
        with_deadline(local_deadline(edge), edge, metrics, self)
    }
}

impl<T, F> BoundedGrpcCallExt<T> for F where F: Future<Output = Result<T, Status>> + Sized {}

/// A read listed in `platform::grpc_retry::REPLAY_SAFE_READS`. Every attempt
/// builds its request again with `build` (a delegation is single-use) and is
/// bounded by the edge deadline; UNAVAILABLE may be retried once within the
/// caller's remaining time and the edge's retry tokens.
pub(crate) async fn replay_safe_read<M, R, B, C, Fut>(
    edge: Edge,
    method: &'static str,
    metrics: &GatewayMetrics,
    build: B,
    call: C,
) -> anyhow::Result<tonic::Response<R>>
where
    B: FnMut() -> anyhow::Result<tonic::Request<M>>,
    C: Fn(tonic::Request<M>) -> Fut,
    Fut: Future<Output = Result<tonic::Response<R>, Status>> + Send,
    R: Send,
{
    let deadlines = metrics.edge_deadline_metrics();
    let (shared, call) = (&deadlines, &call);
    let mut build = build;
    platform::grpc_retry::read(edge, method, &deadlines, move || {
        let request = build();
        async move {
            let request = request?;
            anyhow::Ok(
                grpc_deadline::with_edge_deadline(edge, shared.clone(), call(request)).await?,
            )
        }
    })
    .await
}

fn local_deadline(edge: Edge) -> Duration {
    grpc_deadline::local_deadline(edge)
}

async fn with_deadline<T, F>(
    deadline: Duration,
    edge: Edge,
    metrics: GatewayMetrics,
    call: F,
) -> Result<T, Status>
where
    T: Send,
    F: Future<Output = Result<T, Status>> + Send,
{
    grpc_deadline::with_deadline(deadline, edge, metrics.edge_deadline_metrics(), call).await
}

#[cfg(test)]
mod tests {
    use super::{local_deadline, with_deadline};
    use crate::config::GatewayMetrics;
    use platform::edge::Edge;
    use platform::grpc_deadline;
    use std::time::Duration;
    use tonic::{Code, Status};

    #[test]
    fn every_gateway_local_deadline_precedes_the_endpoint_timeout() {
        for edge in [
            Edge::GatewayAuth,
            Edge::GatewayProfile,
            Edge::GatewayPost,
            Edge::GatewayNotification,
            Edge::GatewayFriend,
        ] {
            let endpoint_deadline = edge.budget().deadline;
            let local = local_deadline(edge);

            assert!(local < endpoint_deadline, "{edge:?} local timeout must win");
            assert_eq!(
                endpoint_deadline - local,
                grpc_deadline::LOCAL_TIMEOUT_MARGIN
            );
        }
    }

    #[tokio::test]
    async fn local_expiry_is_a_deadline_exceeded_status() {
        let metrics = GatewayMetrics::default();
        let edges = [
            Edge::GatewayAuth,
            Edge::GatewayProfile,
            Edge::GatewayPost,
            Edge::GatewayNotification,
            Edge::GatewayFriend,
        ];
        for edge in edges {
            let status = with_deadline(
                Duration::from_millis(1),
                edge,
                metrics.clone(),
                std::future::pending::<Result<(), Status>>(),
            )
            .await
            .expect_err("pending call should expire");

            assert_eq!(status.code(), Code::DeadlineExceeded);
            assert_eq!(status.message(), "Request timed out");
        }
        let rendered = metrics.render_prometheus();
        for edge in edges {
            assert!(rendered.contains(&format!(
                "grpc_edge_deadline_exhaustions_total{{edge=\"{}\"}} 1",
                edge.budget().edge
            )));
        }
        assert_eq!(
            rendered
                .lines()
                .filter(|line| line.starts_with("grpc_edge_deadline_exhaustions_total"))
                .count(),
            edges.len()
        );
    }

    #[tokio::test]
    async fn completed_call_keeps_its_original_result() {
        let metrics = GatewayMetrics::default();
        let result = with_deadline(
            Duration::from_secs(1),
            Edge::GatewayFriend,
            metrics.clone(),
            async { Ok::<_, Status>(42) },
        )
        .await;

        assert_eq!(result.expect("call should complete"), 42);
        assert!(!metrics
            .render_prometheus()
            .contains("grpc_edge_deadline_exhaustions_total"));
    }

    #[tokio::test]
    async fn non_deadline_rpc_errors_do_not_count_as_budget_exhaustion() {
        let metrics = GatewayMetrics::default();
        let error = with_deadline(
            Duration::from_secs(1),
            Edge::GatewayFriend,
            metrics.clone(),
            async { Err::<(), _>(Status::unavailable("peer unavailable")) },
        )
        .await
        .expect_err("the original RPC error should be preserved");

        assert_eq!(error.code(), Code::Unavailable);
        assert!(!metrics
            .render_prometheus()
            .contains("grpc_edge_deadline_exhaustions_total"));
    }

    #[tokio::test]
    async fn deadline_exceeded_rpc_status_counts_once() {
        let metrics = GatewayMetrics::default();
        let error = with_deadline(
            Duration::from_secs(1),
            Edge::GatewayFriend,
            metrics.clone(),
            async { Err::<(), _>(Status::deadline_exceeded("peer deadline")) },
        )
        .await
        .expect_err("the RPC deadline status should be preserved");

        assert_eq!(error.code(), Code::DeadlineExceeded);
        assert!(metrics
            .render_prometheus()
            .contains("grpc_edge_deadline_exhaustions_total{edge=\"gateway->friend\"} 1"));
    }
}
