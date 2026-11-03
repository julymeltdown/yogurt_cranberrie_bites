//! The caller's remaining time, carried across service hops.
//!
//! Each edge declares its own budget, but a fixed per-edge budget cannot say how
//! much time the original caller still has. A service three hops from the edge
//! could otherwise spend its full edge budget after the client has already given
//! up. The remaining time therefore travels with each call in the
//! x-call-deadline-remaining header, in the grpc-timeout value format. A
//! receiver turns that duration into a deadline on its own monotonic clock when
//! the request arrives, so no wall-clock or monotonic timestamp is ever shared
//! between processes.
//!
//! The standard grpc-timeout header is deliberately not used for this: Tonic
//! servers enforce it and drop the handler at expiry, which could interrupt a
//! handler finishing durable work after its caller left. Propagation bounds the
//! receiver's own downstream work; it does not cancel the receiver. An explicit
//! request timeout set by a caller still travels as grpc-timeout and is still
//! enforced, and the receiver also counts it as a deadline.
//!
//! The deadline only shortens limits. It never extends an edge budget, and a
//! request without a caller deadline keeps its edge budget unchanged.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

/// The standard gRPC request timeout header, enforced by Tonic servers.
pub const GRPC_TIMEOUT_HEADER: &str = "grpc-timeout";
/// Header carrying the sender's remaining time without server enforcement.
pub const REMAINING_HEADER: &str = "x-call-deadline-remaining";
/// The protocol allows at most eight digits in the value.
const MAX_TIMEOUT_DIGITS: usize = 8;
const MAX_TIMEOUT_VALUE: u128 = 99_999_999;

tokio::task_local! {
    static DEADLINE: Instant;
}

/// Run a future under a deadline. A nested scope can only shorten the deadline
/// already in force.
pub async fn scope<F>(deadline: Instant, future: F) -> F::Output
where
    F: Future,
{
    let effective = current().map_or(deadline, |outer| outer.min(deadline));
    DEADLINE.scope(effective, future).await
}

/// The deadline in force for this task, if any.
pub fn current() -> Option<Instant> {
    DEADLINE.try_with(|deadline| *deadline).ok()
}

/// Time left before the caller's deadline, if one is in force.
pub fn remaining() -> Option<Duration> {
    current().map(|deadline| deadline.saturating_duration_since(Instant::now()))
}

/// The shorter of a local limit and the caller's remaining time.
pub fn bounded(limit: Duration) -> Duration {
    remaining().map_or(limit, |remaining| remaining.min(limit))
}

/// Parse a gRPC TimeoutValue/TimeoutUnit header value. Malformed values are
/// ignored rather than trusted, as the protocol requires.
pub fn parse_grpc_timeout(value: &str) -> Option<Duration> {
    let split = value.len().checked_sub(1)?;
    if !value.is_char_boundary(split) {
        return None;
    }
    let (digits, unit) = value.split_at(split);
    if digits.is_empty()
        || digits.len() > MAX_TIMEOUT_DIGITS
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let amount: u64 = digits.parse().ok()?;
    Some(match unit {
        "H" => Duration::from_secs(amount.checked_mul(3600)?),
        "M" => Duration::from_secs(amount.checked_mul(60)?),
        "S" => Duration::from_secs(amount),
        "m" => Duration::from_millis(amount),
        "u" => Duration::from_micros(amount),
        "n" => Duration::from_nanos(amount),
        _ => return None,
    })
}

/// Encode a duration with the finest unit that fits eight digits. Each unit
/// truncates, so the receiver is never given more time than the sender had.
pub fn encode_grpc_timeout(duration: Duration) -> String {
    for (unit, value) in [
        ("n", duration.as_nanos()),
        ("u", duration.as_micros()),
        ("m", duration.as_millis()),
        ("S", u128::from(duration.as_secs())),
        ("M", u128::from(duration.as_secs() / 60)),
    ] {
        if value <= MAX_TIMEOUT_VALUE {
            return format!("{value}{unit}");
        }
    }
    format!(
        "{}H",
        (duration.as_secs() / 3600).min(MAX_TIMEOUT_VALUE as u64)
    )
}

/// Send the caller's remaining time with an outbound request. Any value already
/// present is replaced, so a forged or stale header is never forwarded; outside
/// a deadline scope the header is removed. grpc-timeout is left untouched.
pub fn apply_to_headers(headers: &mut http::HeaderMap) {
    headers.remove(REMAINING_HEADER);
    let Some(remaining) = remaining() else {
        return;
    };
    if let Ok(value) = http::HeaderValue::from_str(&encode_grpc_timeout(remaining)) {
        headers.insert(REMAINING_HEADER, value);
    }
}

fn header_duration(headers: &http::HeaderMap, name: &str) -> Option<Duration> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_grpc_timeout)
}

/// The shortest caller deadline a request carries, from the propagated
/// remaining time or an explicit request timeout.
pub fn received_timeout(headers: &http::HeaderMap) -> Option<Duration> {
    match (
        header_duration(headers, REMAINING_HEADER),
        header_duration(headers, GRPC_TIMEOUT_HEADER),
    ) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (left, right) => left.or(right),
    }
}

/// Server layer: start the caller's deadline on this process's clock when the
/// request arrives and keep it in force while the request is handled.
#[derive(Clone, Copy, Debug, Default)]
pub struct CallDeadlineLayer;

impl<S> tower::Layer<S> for CallDeadlineLayer {
    type Service = CallDeadlineService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        CallDeadlineService { inner }
    }
}

/// The service produced by [CallDeadlineLayer].
#[derive(Clone, Copy, Debug)]
pub struct CallDeadlineService<S> {
    inner: S,
}

impl<S, Body> tower::Service<http::Request<Body>> for CallDeadlineService<S>
where
    S: tower::Service<http::Request<Body>>,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<S::Response, S::Error>> + Send>>;

    fn poll_ready(&mut self, context: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(context)
    }

    fn call(&mut self, request: http::Request<Body>) -> Self::Future {
        let received = Instant::now();
        let timeout = received_timeout(request.headers());
        let future = self.inner.call(request);
        match timeout.and_then(|timeout| received.checked_add(timeout)) {
            Some(deadline) => Box::pin(scope(deadline, future)),
            None => Box::pin(future),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grpc_timeout_values_round_trip_without_gaining_time() {
        assert_eq!(parse_grpc_timeout("250m"), Some(Duration::from_millis(250)));
        assert_eq!(parse_grpc_timeout("3S"), Some(Duration::from_secs(3)));
        assert_eq!(parse_grpc_timeout("2M"), Some(Duration::from_secs(120)));
        assert_eq!(parse_grpc_timeout("1H"), Some(Duration::from_secs(3600)));
        assert_eq!(parse_grpc_timeout("15u"), Some(Duration::from_micros(15)));
        assert_eq!(parse_grpc_timeout("9n"), Some(Duration::from_nanos(9)));
        for invalid in ["", "m", "123", "123456789m", "-1m", "1.5S", "10x", "１m"] {
            assert_eq!(parse_grpc_timeout(invalid), None, "{invalid:?}");
        }
        for duration in [
            Duration::ZERO,
            Duration::from_nanos(1),
            Duration::from_micros(799_999),
            Duration::from_millis(950),
            Duration::from_secs(42),
            Duration::from_secs(400_000),
            Duration::from_secs(u64::MAX / 2),
        ] {
            let encoded = encode_grpc_timeout(duration);
            let digits = &encoded[..encoded.len() - 1];
            assert!(digits.len() <= MAX_TIMEOUT_DIGITS, "{encoded}");
            let decoded = parse_grpc_timeout(&encoded).expect("valid encoding");
            assert!(decoded <= duration, "{duration:?} encoded as {encoded}");
        }
        assert_eq!(encode_grpc_timeout(Duration::from_millis(950)), "950000u");
    }

    #[tokio::test]
    async fn nested_scopes_only_shorten_and_bound_local_limits() {
        assert_eq!(remaining(), None);
        assert_eq!(bounded(Duration::from_secs(1)), Duration::from_secs(1));
        let outer = Instant::now() + Duration::from_millis(300);
        scope(outer, async {
            assert!(bounded(Duration::from_secs(1)) <= Duration::from_millis(300));
            assert_eq!(
                bounded(Duration::from_millis(10)),
                Duration::from_millis(10)
            );
            scope(Instant::now() + Duration::from_secs(60), async {
                assert_eq!(current(), Some(outer), "a later scope cannot extend");
            })
            .await;
            let inner = Instant::now() + Duration::from_millis(50);
            scope(inner, async {
                assert_eq!(current(), Some(inner));
            })
            .await;
        })
        .await;
    }

    #[tokio::test]
    async fn outbound_headers_carry_remaining_time_without_touching_grpc_timeout() {
        let mut headers = http::HeaderMap::new();
        headers.insert(REMAINING_HEADER, "9S".parse().unwrap());
        apply_to_headers(&mut headers);
        assert!(
            headers.get(REMAINING_HEADER).is_none(),
            "a forged header is not forwarded outside a deadline scope"
        );
        scope(Instant::now() + Duration::from_millis(200), async {
            let mut headers = http::HeaderMap::new();
            headers.insert(GRPC_TIMEOUT_HEADER, "5S".parse().unwrap());
            headers.insert(REMAINING_HEADER, "9S".parse().unwrap());
            apply_to_headers(&mut headers);
            assert_eq!(headers[GRPC_TIMEOUT_HEADER], "5S");
            let sent = parse_grpc_timeout(headers[REMAINING_HEADER].to_str().unwrap()).unwrap();
            assert!(sent <= Duration::from_millis(200));
            assert!(sent > Duration::from_millis(100));
            let mut stricter = http::HeaderMap::new();
            stricter.insert(GRPC_TIMEOUT_HEADER, "20m".parse().unwrap());
            apply_to_headers(&mut stricter);
            assert_eq!(received_timeout(&stricter), Some(Duration::from_millis(20)));
        })
        .await;
    }

    #[tokio::test]
    async fn server_layer_starts_the_deadline_on_the_receiver_clock() {
        use tower::{Layer, Service, ServiceExt};
        let mut service =
            CallDeadlineLayer.layer(tower::service_fn(
                |_request: http::Request<()>| async move {
                    Ok::<_, std::convert::Infallible>(remaining())
                },
            ));
        for (name, other) in [
            (REMAINING_HEADER, None),
            (GRPC_TIMEOUT_HEADER, None),
            (REMAINING_HEADER, Some("9S")),
        ] {
            let mut builder = http::Request::builder().header(name, "400m");
            if let Some(other) = other {
                builder = builder.header(GRPC_TIMEOUT_HEADER, other);
            }
            let seen = service
                .ready()
                .await
                .unwrap()
                .call(builder.body(()).unwrap())
                .await
                .unwrap()
                .expect("deadline in scope");
            assert!(seen <= Duration::from_millis(400) && seen > Duration::from_millis(300));
        }
        for header in [None, Some("garbage")] {
            let mut builder = http::Request::builder();
            if let Some(header) = header {
                builder = builder.header(REMAINING_HEADER, header);
            }
            let seen = service
                .ready()
                .await
                .unwrap()
                .call(builder.body(()).unwrap())
                .await
                .unwrap();
            assert_eq!(seen, None, "{header:?}");
        }
    }
}
