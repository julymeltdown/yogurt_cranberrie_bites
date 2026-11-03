//! Bounded retries for replay-safe internal reads.
//!
//! Only methods in [REPLAY_SAFE_READS] are ever attempted twice. Their owners
//! read state without a business effect and persist no evidence, so a second
//! attempt repeats only the read. When a Post read needs Gateway-issued child
//! authority, each attempt asks for a fresh child (delegations are
//! single-use), which also spends one more of the parent's bounded child
//! issuances. Reads that persist impressions or authority witnesses, or whose
//! owner itself calls the exchange, are in [SINGLE_ATTEMPT_READS] and run once.
//! A failure of the exchange is marked [NotRetried] in process and with
//! [NO_RETRY_METADATA] on the wire, and ends any retry loop that sees it.
//! Every other call runs exactly once, and an
//! unknown mutation outcome stays with the operation journal and owner
//! receipts. A retry is made only for UNAVAILABLE, only while the caller's
//! remaining time covers the backoff plus the edge's expected latency, and only
//! while the edge's retry tokens allow it, so a dependency outage cannot turn
//! every request into several.

use crate::edge::Edge;
use crate::edge_metrics::EdgeDeadlineMetrics;
use std::future::Future;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;
use tonic::{Code, Status};

/// Internal reads whose repetition has no effect. Each entry is a full gRPC
/// path; a test checks that every one exists in Proto and is a read.
pub const REPLAY_SAFE_READS: &[&str] = &[
    "/authority.v1.ProfileAuthorAuthorityService/CheckAuthorProfile",
    "/friend.v1.FriendService/BatchGetRelationships",
    "/friend.v1.FriendService/CheckFollow",
    "/friend.v1.FriendService/GetFollowCounts",
    "/friend.v1.FriendService/GetFollowStats",
    "/friend.v1.FriendService/GetRelationship",
    "/friend.v1.FriendService/ListFollowers",
    "/friend.v1.FriendService/ListFollowing",
    "/notification.v1.NotificationService/ListNotifications",
    "/post.v1.PostService/GetAuthorPostCounts",
    "/profile.v1.ProfileService/GetAvatar",
    "/profile.v1.ProfileService/GetProfile",
    "/profile.v1.ProfileService/GetProfileByUsername",
    "/profile.v1.ProfileService/GetProfilePrivacy",
    "/profile.v1.ProfileService/GetProfiles",
    "/profile.v1.ProfileService/SearchProfiles",
];

/// Reads that must run once, with the reason:
/// - GetPeopleRecommendations and GetFeed (ranked) persist impressions, so a
///   second attempt would log a second impression set for one response.
/// - GetPost, GetTrending and SearchPosts publish authority-witness rows with
///   a fresh stage per call; a retry would publish more rows.
/// - GetAuthorPosts filters by relationship through the authority exchange, so
///   a retry would repeat exchanges whose failure Post reports as UNAVAILABLE.
pub const SINGLE_ATTEMPT_READS: &[&str] = &[
    "/friend.v1.FriendService/GetPeopleRecommendations",
    "/post.v1.PostService/GetAuthorPosts",
    "/post.v1.PostService/GetFeed",
    "/post.v1.PostService/GetPost",
    "/post.v1.PostService/GetTrending",
    "/post.v1.PostService/SearchPosts",
];

/// Response metadata an owner sets on a status that must not be retried by
/// its caller, such as Post reporting a failed authority exchange.
pub const NO_RETRY_METADATA: &str = "x-retry-disposition";
const NO_RETRY_VALUE: &str = "never";

/// Mark `status` so a caller's retry loop returns it after one attempt.
pub fn mark_not_retryable(status: &mut Status) {
    status
        .metadata_mut()
        .insert(NO_RETRY_METADATA, NO_RETRY_VALUE.parse().expect("static"));
}

/// Whether `status` is an UNAVAILABLE its owner allows the caller to retry.
fn retryable(status: &Status) -> bool {
    status.code() == Code::Unavailable
        && status
            .metadata()
            .get(NO_RETRY_METADATA)
            .is_none_or(|value| value != NO_RETRY_VALUE)
}

/// Attempt count and backoff for one read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Total attempts, including the first.
    pub max_attempts: u32,
    /// Ceiling of the first backoff; each later one doubles up to `max_backoff`.
    pub base_backoff: Duration,
    pub max_backoff: Duration,
}

/// Request-path reads: at most one retry after a short jittered pause.
/// Provisional until the D5 latency targets are frozen.
pub const READ_RETRY: RetryPolicy = RetryPolicy {
    max_attempts: 2,
    base_backoff: Duration::from_millis(25),
    max_backoff: Duration::from_millis(100),
};

impl RetryPolicy {
    /// Full-jitter backoff before attempt `attempt + 1`, from a uniform
    /// fraction `unit` in [0, 1).
    pub fn backoff(&self, attempt: u32, unit: f64) -> Duration {
        let ceiling = self
            .base_backoff
            .saturating_mul(1u32 << attempt.saturating_sub(1).min(16))
            .min(self.max_backoff);
        ceiling.mul_f64(unit.clamp(0.0, 1.0))
    }
}

/// Per-edge retry tokens, after the gRPC retry-throttling scheme: each
/// retryable failure costs one token, each success returns a tenth, and
/// retries stop while fewer than half the tokens remain.
pub struct RetryThrottle {
    milli_tokens: AtomicI64,
}

const MAX_MILLI_TOKENS: i64 = 10_000;
const FAILURE_COST: i64 = 1_000;
const SUCCESS_CREDIT: i64 = 100;

impl Default for RetryThrottle {
    fn default() -> Self {
        Self {
            milli_tokens: AtomicI64::new(MAX_MILLI_TOKENS),
        }
    }
}

impl RetryThrottle {
    fn record_success(&self) {
        let _ = self
            .milli_tokens
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |tokens| {
                Some((tokens + SUCCESS_CREDIT).min(MAX_MILLI_TOKENS))
            });
    }

    /// Record a retryable failure and report whether a retry is still allowed.
    fn record_failure(&self) -> bool {
        let previous = self
            .milli_tokens
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |tokens| {
                Some((tokens - FAILURE_COST).max(0))
            })
            .expect("closure always returns a value");
        (previous - FAILURE_COST).max(0) > MAX_MILLI_TOKENS / 2
    }
}

/// What a retry loop needs to know about an attempt's error.
pub trait RetrySignal {
    /// The gRPC status the attempt ended with, if it ended with one.
    fn grpc_status(&self) -> Option<&Status>;
}

/// Context marking an error that ends the retry loop whatever status it
/// carries, for example a failed authority exchange before the owner read.
/// The original error stays in the chain, so callers that map a nested
/// `tonic::Status` still see it.
#[derive(Debug, Clone, Copy)]
pub struct NotRetried;

impl std::fmt::Display for NotRetried {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("dependency call failed before the read; not retried")
    }
}

/// Wrap `error` so a retry loop returns it after this attempt.
pub fn not_retried(error: impl Into<anyhow::Error>) -> anyhow::Error {
    error.into().context(NotRetried)
}

impl RetrySignal for Status {
    fn grpc_status(&self) -> Option<&Status> {
        Some(self)
    }
}

impl RetrySignal for anyhow::Error {
    fn grpc_status(&self) -> Option<&Status> {
        if self.downcast_ref::<NotRetried>().is_some() {
            return None;
        }
        self.downcast_ref::<Status>()
    }
}

fn jitter_unit() -> f64 {
    use ring::rand::SecureRandom;
    let mut bytes = [0u8; 8];
    if ring::rand::SystemRandom::new().fill(&mut bytes).is_err() {
        return 0.5;
    }
    (u64::from_le_bytes(bytes) >> 11) as f64 / (1u64 << 53) as f64
}

/// Run a read, retrying UNAVAILABLE within the policy, the edge's retry
/// tokens held by `metrics`, and the caller budget. `attempt` must build a
/// fresh request each time (a delegation is single-use) and apply its own
/// edge deadline.
pub async fn read<T, E, F, Fut>(
    edge: Edge,
    method: &'static str,
    metrics: &EdgeDeadlineMetrics,
    attempt: F,
) -> Result<T, E>
where
    E: RetrySignal,
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
{
    read_with(
        &READ_RETRY,
        metrics.retry_throttle(edge),
        jitter_unit,
        edge,
        method,
        metrics,
        attempt,
    )
    .await
}

/// [read] with an explicit policy, throttle and jitter source.
pub async fn read_with<T, E, F, Fut>(
    policy: &RetryPolicy,
    throttle: &RetryThrottle,
    mut unit: impl FnMut() -> f64,
    edge: Edge,
    method: &'static str,
    metrics: &EdgeDeadlineMetrics,
    mut attempt: F,
) -> Result<T, E>
where
    E: RetrySignal,
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
{
    let replay_safe = REPLAY_SAFE_READS.contains(&method);
    let mut made = 1;
    loop {
        let error = match attempt().await {
            Ok(value) => {
                throttle.record_success();
                return Ok(value);
            }
            Err(error) => error,
        };
        let unavailable = error.grpc_status().is_some_and(retryable);
        if !unavailable {
            return Err(error);
        }
        let allowed = throttle.record_failure();
        if !replay_safe || made >= policy.max_attempts || !allowed {
            return Err(error);
        }
        let pause = policy.backoff(made, unit());
        if crate::call_deadline::remaining()
            .is_some_and(|left| left <= pause + edge.budget().expected_latency)
        {
            return Err(error);
        }
        metrics.record_read_retry(edge);
        tokio::time::sleep(pause).await;
        made += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU32;
    use std::sync::Arc;

    const READ: &str = "/friend.v1.FriendService/ListFollowing";

    async fn run(
        throttle: &RetryThrottle,
        method: &'static str,
        outcomes: Vec<Result<u8, Status>>,
    ) -> (Result<u8, Status>, u32, String) {
        let metrics = EdgeDeadlineMetrics::default();
        let calls = Arc::new(AtomicU32::new(0));
        let outcomes = Arc::new(std::sync::Mutex::new(outcomes.into_iter()));
        let counter = calls.clone();
        let result = read_with(
            &READ_RETRY,
            throttle,
            || 0.0,
            Edge::PostFriend,
            method,
            &metrics,
            || {
                counter.fetch_add(1, Ordering::SeqCst);
                let next = outcomes.lock().unwrap().next().expect("outcome");
                async move { next }
            },
        )
        .await;
        (
            result,
            calls.load(Ordering::SeqCst),
            metrics.render_prometheus(),
        )
    }

    #[tokio::test]
    async fn an_unavailable_read_is_retried_once_and_counted() {
        let throttle = RetryThrottle::default();
        let (result, calls, rendered) = run(
            &throttle,
            READ,
            vec![Err(Status::unavailable("down")), Ok(7)],
        )
        .await;
        assert_eq!(result.unwrap(), 7);
        assert_eq!(calls, 2);
        assert!(rendered.contains("grpc_edge_read_retries_total{edge=\"post->friend\"} 1"));
    }

    #[tokio::test]
    async fn attempts_stop_at_the_policy_limit() {
        let throttle = RetryThrottle::default();
        let (result, calls, _) = run(
            &throttle,
            READ,
            vec![Err(Status::unavailable("a")), Err(Status::unavailable("b"))],
        )
        .await;
        assert_eq!(result.unwrap_err().message(), "b");
        assert_eq!(calls, READ_RETRY.max_attempts);
    }

    #[tokio::test]
    async fn an_unlisted_method_runs_once_even_when_unavailable() {
        let throttle = RetryThrottle::default();
        let (result, calls, rendered) = run(
            &throttle,
            "/post.v1.PostService/CreatePost",
            vec![Err(Status::unavailable("down"))],
        )
        .await;
        assert_eq!(result.unwrap_err().code(), Code::Unavailable);
        assert_eq!(calls, 1);
        assert!(rendered.is_empty());
    }

    #[tokio::test]
    async fn deadline_and_refusal_statuses_are_never_retried() {
        for status in [
            Status::deadline_exceeded("unknown outcome"),
            Status::resource_exhausted("in-flight limit"),
            Status::permission_denied("denied"),
            Status::internal("internal"),
        ] {
            let throttle = RetryThrottle::default();
            let code = status.code();
            let (result, calls, _) = run(&throttle, READ, vec![Err(status)]).await;
            assert_eq!(result.unwrap_err().code(), code);
            assert_eq!(calls, 1, "{code:?}");
        }
    }

    #[tokio::test]
    async fn repeated_failures_exhaust_the_retry_tokens() {
        let throttle = RetryThrottle::default();
        let mut retried = 0;
        for _ in 0..10 {
            let (_, calls, _) = run(
                &throttle,
                READ,
                vec![Err(Status::unavailable("a")), Err(Status::unavailable("b"))],
            )
            .await;
            retried += calls - 1;
        }
        // Ten tokens, two spent per failing read, retries stop at half: the
        // first two reads retry, every later one gives up after one attempt.
        assert_eq!(retried, 2);
        for _ in 0..70 {
            throttle.record_success();
        }
        let (_, calls, _) = run(&throttle, READ, vec![Err(Status::unavailable("a")), Ok(1)]).await;
        assert_eq!(calls, 2, "successes restore the tokens");
    }

    #[tokio::test]
    async fn a_retry_that_would_outlive_the_caller_budget_is_not_made() {
        let throttle = RetryThrottle::default();
        let deadline = std::time::Instant::now() + Duration::from_millis(20);
        let (result, calls, _) = crate::call_deadline::scope(deadline, async {
            let metrics = EdgeDeadlineMetrics::default();
            let calls = Arc::new(AtomicU32::new(0));
            let counter = calls.clone();
            let result = read_with(
                &READ_RETRY,
                &throttle,
                || 0.99,
                Edge::PostFriend,
                READ,
                &metrics,
                || {
                    counter.fetch_add(1, Ordering::SeqCst);
                    async { Err::<u8, _>(Status::unavailable("down")) }
                },
            )
            .await;
            (result, calls.load(Ordering::SeqCst), ())
        })
        .await;
        assert!(result.is_err());
        assert_eq!(calls, 1);
    }

    #[tokio::test]
    async fn anyhow_errors_carrying_a_status_are_classified() {
        let throttle = RetryThrottle::default();
        let metrics = EdgeDeadlineMetrics::default();
        let mut outcomes = vec![
            Ok(3u8),
            Err(anyhow::Error::from(Status::unavailable("down"))),
        ];
        let result = read_with(
            &READ_RETRY,
            &throttle,
            || 0.0,
            Edge::PostFriend,
            READ,
            &metrics,
            || {
                let next = outcomes.pop().unwrap();
                async move { next }
            },
        )
        .await;
        assert_eq!(result.unwrap(), 3);
        let plain = read_with(
            &READ_RETRY,
            &throttle,
            || 0.0,
            Edge::PostFriend,
            READ,
            &metrics,
            || async { Err::<u8, _>(anyhow::anyhow!("not a status")) },
        )
        .await;
        assert!(plain.is_err());
    }

    #[tokio::test]
    async fn a_marked_failure_ends_the_loop_but_keeps_its_status() {
        let throttle = RetryThrottle::default();
        let metrics = EdgeDeadlineMetrics::default();
        let calls = AtomicU32::new(0);
        let error = read_with(
            &READ_RETRY,
            &throttle,
            || 0.0,
            Edge::PostFriend,
            READ,
            &metrics,
            || {
                calls.fetch_add(1, Ordering::SeqCst);
                async { Err::<u8, _>(not_retried(Status::unavailable("exchange down"))) }
            },
        )
        .await
        .expect_err("marked failure");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            error.downcast_ref::<Status>().map(Status::code),
            Some(Code::Unavailable),
            "the original status stays reachable"
        );
        assert!(error.chain().any(|cause| cause.is::<Status>()));
    }

    #[test]
    fn backoff_is_full_jitter_and_capped() {
        assert_eq!(READ_RETRY.backoff(1, 0.0), Duration::ZERO);
        assert_eq!(READ_RETRY.backoff(1, 1.0), Duration::from_millis(25));
        assert_eq!(READ_RETRY.backoff(2, 1.0), Duration::from_millis(50));
        assert_eq!(READ_RETRY.backoff(9, 1.0), Duration::from_millis(100));
        let unit = jitter_unit();
        assert!((0.0..1.0).contains(&unit));
    }

    #[test]
    fn every_replay_safe_read_is_a_proto_read_method() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../proto");
        for path in REPLAY_SAFE_READS.iter().chain(SINGLE_ATTEMPT_READS) {
            let (service, method) = path[1..].split_once('/').expect("full path");
            let (package, service_name) = service.rsplit_once('.').expect("package");
            let directory = package.split('.').collect::<Vec<_>>().join("/");
            let found = std::fs::read_dir(root.join(&directory))
                .unwrap_or_else(|_| panic!("{path}: no proto directory {directory}"))
                .filter_map(|entry| std::fs::read_to_string(entry.ok()?.path()).ok())
                .any(|source| {
                    source.contains(&format!("service {service_name} {{"))
                        && source.contains(&format!("rpc {method}("))
                });
            assert!(found, "{path} is not a served Proto method");
            assert!(
                ["Get", "List", "Check", "BatchGet", "Search"]
                    .iter()
                    .any(|verb| method.starts_with(verb)),
                "{path} does not name a read"
            );
            assert_ne!(
                REPLAY_SAFE_READS.contains(path),
                SINGLE_ATTEMPT_READS.contains(path),
                "{path} must be on exactly one list"
            );
        }
    }

    #[tokio::test]
    async fn an_owner_marked_unavailable_is_returned_after_one_attempt() {
        let throttle = RetryThrottle::default();
        let metrics = EdgeDeadlineMetrics::default();
        let calls = AtomicU32::new(0);
        let status = read_with(
            &READ_RETRY,
            &throttle,
            || 0.0,
            Edge::GatewayFriend,
            READ,
            &metrics,
            || {
                calls.fetch_add(1, Ordering::SeqCst);
                let mut status = Status::unavailable("owner exchange failed");
                mark_not_retryable(&mut status);
                async move { Err::<u8, _>(status) }
            },
        )
        .await
        .expect_err("marked");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(status.code(), Code::Unavailable);
        // The same marker inside an anyhow chain is honoured too.
        let calls = AtomicU32::new(0);
        let _ = read_with(
            &READ_RETRY,
            &throttle,
            || 0.0,
            Edge::GatewayFriend,
            READ,
            &metrics,
            || {
                calls.fetch_add(1, Ordering::SeqCst);
                let mut status = Status::unavailable("owner exchange failed");
                mark_not_retryable(&mut status);
                async move { Err::<u8, _>(anyhow::Error::from(status)) }
            },
        )
        .await;
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
