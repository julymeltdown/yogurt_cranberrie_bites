use crate::edge::Edge;
use crate::grpc_retry::RetryThrottle;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

/// Process-local counters for internal gRPC edge events: deadline
/// exhaustion, local in-flight refusals and read retries.
///
/// The only label values come from the closed `Edge` enum, so the series set
/// cannot grow with request or peer data.
///
/// The handle also owns each edge's read-retry tokens. A service passes one
/// handle to all of its clients, so the tokens are shared per process and
/// per edge; an independent handle (as in a test) has independent tokens.
#[derive(Clone)]
pub struct EdgeDeadlineMetrics {
    counters: Arc<Mutex<BTreeMap<(&'static str, &'static str), u64>>>,
    throttles: Arc<HashMap<Edge, RetryThrottle>>,
}

impl Default for EdgeDeadlineMetrics {
    fn default() -> Self {
        Self {
            counters: Arc::default(),
            throttles: Arc::new(
                Edge::ALL
                    .into_iter()
                    .map(|edge| (edge, RetryThrottle::default()))
                    .collect(),
            ),
        }
    }
}

impl EdgeDeadlineMetrics {
    pub const METRIC_NAME: &'static str = "grpc_edge_deadline_exhaustions_total";
    /// Calls refused before dispatch because the edge was at its in-flight limit.
    pub const IN_FLIGHT_REJECTIONS: &'static str = "grpc_edge_in_flight_rejections_total";
    /// Additional attempts made for replay-safe reads.
    pub const READ_RETRIES: &'static str = "grpc_edge_read_retries_total";

    fn record(&self, metric: &'static str, edge: Edge) {
        let mut counters = self.counters.lock().expect("gRPC edge metrics");
        let count = counters.entry((metric, edge.budget().edge)).or_default();
        *count = count.saturating_add(1);
    }

    pub fn record_deadline_exhausted(&self, edge: Edge) {
        self.record(Self::METRIC_NAME, edge);
    }

    pub fn record_in_flight_rejected(&self, edge: Edge) {
        self.record(Self::IN_FLIGHT_REJECTIONS, edge);
    }

    pub fn record_read_retry(&self, edge: Edge) {
        self.record(Self::READ_RETRIES, edge);
    }

    /// The read-retry tokens for `edge` held by this handle.
    pub fn retry_throttle(&self, edge: Edge) -> &RetryThrottle {
        &self.throttles[&edge]
    }

    /// Render only edges observed by this process; absent edges are implicitly
    /// zero and do not create a second fixed set of unused series.
    pub fn render_prometheus(&self) -> String {
        let counters = self.counters.lock().expect("gRPC edge metrics");
        let mut output = String::new();
        for ((metric, edge), count) in counters.iter() {
            output.push_str(metric);
            output.push_str("{edge=\"");
            output.push_str(edge);
            output.push_str("\"} ");
            output.push_str(&count.to_string());
            output.push('\n');
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::EdgeDeadlineMetrics;
    use crate::edge::Edge;

    #[test]
    fn renders_only_bounded_edge_labels_and_accumulates_counts() {
        let metrics = EdgeDeadlineMetrics::default();
        metrics.record_deadline_exhausted(Edge::PostProfile);
        metrics.record_deadline_exhausted(Edge::PostProfile);
        metrics.record_deadline_exhausted(Edge::AuthFriend);

        let rendered = metrics.render_prometheus();
        assert!(
            rendered.contains("grpc_edge_deadline_exhaustions_total{edge=\"post->profile\"} 2\n")
        );
        assert!(
            rendered.contains("grpc_edge_deadline_exhaustions_total{edge=\"auth->friend\"} 1\n")
        );
        assert_eq!(rendered.lines().count(), 2);
        assert!(!rendered.contains("request"));
    }

    #[test]
    fn refusals_and_retries_are_separate_series() {
        let metrics = EdgeDeadlineMetrics::default();
        metrics.record_in_flight_rejected(Edge::PostFriend);
        metrics.record_read_retry(Edge::PostFriend);
        metrics.record_read_retry(Edge::PostFriend);
        let rendered = metrics.render_prometheus();
        assert!(
            rendered.contains("grpc_edge_in_flight_rejections_total{edge=\"post->friend\"} 1\n")
        );
        assert!(rendered.contains("grpc_edge_read_retries_total{edge=\"post->friend\"} 2\n"));
        assert!(!rendered.contains("grpc_edge_deadline_exhaustions_total"));
    }
}
