//! One request identifier that follows a call across service boundaries.
//!
//! Each service already correlates its own logs, but nothing connected them: the
//! Gateway logged a request under one identifier and the services it called logged
//! under others, so an operator holding a support report could follow a request up
//! to the edge and no further. The identifier travels in the same metadata as the
//! workload identity, so it needs no extra transport configuration.
//!
//! It is deliberately not a security control. The value is caller-influenced, so it
//! must never be used for authorization or as a deduplication key; it exists to join
//! log lines.

use tonic::metadata::MetadataValue;
use tonic::{Request, Status};

/// Metadata key carrying the identifier between services.
pub const CORRELATION_ID_HEADER: &str = "x-correlation-id";

/// Longest accepted identifier.
///
/// A caller controls this value, so it is bounded before it reaches a log line or is
/// echoed back. Without a bound, one request could add arbitrarily much text to every
/// log entry written while handling it.
pub const MAX_CORRELATION_ID_LENGTH: usize = 128;

tokio::task_local! {
    /// The identifier for the work currently running on this task.
    static CURRENT: String;
}

/// Run a future with this identifier as the current request identifier.
///
/// Task-local rather than a parameter because the identifier has to reach every
/// outbound call made while handling one request, including calls inside layers that
/// do not know about it. Threading it through every signature would spread the
/// concern through the whole call graph for a value no caller decides.
pub async fn scope<F>(correlation_id: String, future: F) -> F::Output
where
    F: std::future::Future,
{
    CURRENT.scope(correlation_id, future).await
}

/// The identifier for the current task, if one was established.
pub fn current() -> Option<String> {
    CURRENT.try_with(|value| value.clone()).ok()
}

/// Normalize an incoming identifier.
///
/// Returns None for a value that cannot be used, so the caller generates a fresh
/// identifier rather than propagating something unusable. Control characters are
/// rejected outright: they could forge line breaks in a log.
pub fn normalize(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > MAX_CORRELATION_ID_LENGTH {
        return None;
    }
    if trimmed.chars().any(char::is_control) {
        return None;
    }
    Some(trimmed.to_owned())
}

/// Attach the current identifier to an outbound request.
///
/// A request made outside any scope carries no identifier rather than a fabricated
/// one. Inventing a value per hop would produce identifiers that look meaningful and
/// join nothing.
// `tonic::Status` is a large error value and is the type an interceptor must
// return, so the lint suggestion of boxing does not apply at this boundary.
#[allow(clippy::result_large_err)]
pub fn attach<T>(request: &mut Request<T>) -> Result<(), Status> {
    // Normalized here as well as on the reading side, so an unusable value is not
    // transmitted at all. Relying only on the reader would put the value on the
    // wire and leave correctness to whoever consumes it.
    let Some(correlation_id) = current().as_deref().and_then(normalize) else {
        return Ok(());
    };
    let value: MetadataValue<tonic::metadata::Ascii> = correlation_id
        .parse()
        .map_err(|_| Status::internal("correlation id is not valid metadata"))?;
    request.metadata_mut().insert(CORRELATION_ID_HEADER, value);
    Ok(())
}

/// Read an incoming identifier from a request.
pub fn from_metadata<T>(request: &Request<T>) -> Option<String> {
    request
        .metadata()
        .get(CORRELATION_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(normalize)
}

/// A tower layer that runs each incoming request inside its correlation scope.
///
/// Applied to a gRPC server, it means a service handling an internal call continues
/// the identifier rather than starting a new one, so a downstream call made while
/// serving that request carries it too. Without this the chain would stop at the
/// first hop.
#[derive(Clone, Copy, Debug, Default)]
pub struct CorrelationLayer;

impl<S> tower::Layer<S> for CorrelationLayer {
    type Service = CorrelationService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        CorrelationService { inner }
    }
}

/// The service produced by [`CorrelationLayer`].
#[derive(Clone, Copy, Debug)]
pub struct CorrelationService<S> {
    inner: S,
}

impl<S, Body> tower::Service<http::Request<Body>> for CorrelationService<S>
where
    S: tower::Service<http::Request<Body>>,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future =
        std::pin::Pin<Box<dyn std::future::Future<Output = Result<S::Response, S::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        context: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(context)
    }

    fn call(&mut self, request: http::Request<Body>) -> Self::Future {
        // The identifier is taken from the headers and the request is passed on
        // unchanged, so this is transparent to the service underneath.
        let correlation_id = request
            .headers()
            .get(CORRELATION_ID_HEADER)
            .and_then(|value| value.to_str().ok())
            .and_then(normalize);
        let future = self.inner.call(request);
        Box::pin(async move {
            match correlation_id {
                Some(correlation_id) => scope(correlation_id, future).await,
                // An internal caller that sent none is not given one: a fabricated
                // value would look meaningful while joining nothing.
                None => future.await,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_oversized_or_control_bearing_identifier_is_refused() {
        assert_eq!(normalize("abc-123"), Some("abc-123".to_owned()));
        assert_eq!(normalize("  abc-123  "), Some("abc-123".to_owned()));
        assert_eq!(normalize(""), None);
        assert_eq!(normalize("   "), None);
        assert_eq!(normalize(&"a".repeat(MAX_CORRELATION_ID_LENGTH + 1)), None);
        // A newline could forge an extra line in a log entry.
        assert_eq!(normalize("abc\ndef"), None);
    }

    #[tokio::test]
    async fn a_scoped_identifier_is_visible_inside_and_absent_outside() {
        assert_eq!(current(), None);
        let observed = scope("corr-1".to_owned(), async { current() }).await;
        assert_eq!(observed, Some("corr-1".to_owned()));
        // The scope does not leak past the future it wrapped.
        assert_eq!(current(), None);
    }

    #[tokio::test]
    async fn an_outbound_request_carries_the_scoped_identifier() {
        let mut request = Request::new(());
        // Outside a scope nothing is attached, rather than a fabricated value.
        attach(&mut request).expect("attach");
        assert!(request.metadata().get(CORRELATION_ID_HEADER).is_none());

        scope("corr-2".to_owned(), async {
            let mut request = Request::new(());
            attach(&mut request).expect("attach");
            assert_eq!(from_metadata(&request), Some("corr-2".to_owned()));
        })
        .await;
    }
}
