//! Per-RPC client tracing for Tonic channels.
//!
//! The client span encloses the transport request and response-body lifecycle,
//! rather than the synchronous metadata interceptor. Its bounded method path
//! is the only RPC label; request/response content and peer error text are not
//! recorded.

use crate::trace_context::{self, TraceContext, TRACEPARENT_HEADER, TRACESTATE_HEADER};
use http::{HeaderMap, HeaderValue, Request, Response};
use http_body::Body;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Instant;
use tonic::body::Body as TonicBody;
use tonic::codegen::Bytes;
use tracing::{field, Instrument, Span};

/// Tower service to wrap a Tonic channel inside `InterceptedService`.
#[derive(Clone, Debug)]
pub struct GrpcClientTraceService<S> {
    inner: S,
}

impl<S> GrpcClientTraceService<S> {
    pub fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<S, ResponseBody> tower::Service<Request<TonicBody>> for GrpcClientTraceService<S>
where
    S: tower::Service<Request<TonicBody>, Response = Response<ResponseBody>> + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Send + 'static,
    ResponseBody: Body<Data = Bytes> + Send + Unpin + 'static,
    ResponseBody::Error: Send + 'static,
{
    type Response = Response<GrpcClientTraceBody<ResponseBody>>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, context: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(context)
    }

    fn call(&mut self, mut request: Request<TonicBody>) -> Self::Future {
        let rpc_method = bounded_rpc_method(request.uri().path());
        let mut context = trace_context::current().map(|parent| parent.child());
        let span = tracing::info_span!(
            parent: None,
            "grpc_client_call",
            rpc_method = %rpc_method,
            trace_id = field::Empty,
            span_id = field::Empty,
            parent_span_id = field::Empty,
            sampled = field::Empty,
            duration_ms = field::Empty,
            outcome = field::Empty,
        );
        if let Some(context) = context.as_mut() {
            context.bind_to_span(&span);
            context.record_on_span(&span);
        }
        replace_trace_headers(request.headers_mut(), context.as_ref());
        // Send no more time than the caller this task serves still has.
        crate::call_deadline::apply_to_headers(request.headers_mut());
        let started_at = Instant::now();
        let future = self.inner.call(request);
        let future_context = context.clone();

        Box::pin(async move {
            let result = match future_context {
                Some(context) => {
                    trace_context::scope(context, future.instrument(span.clone())).await
                }
                None => future.instrument(span.clone()).await,
            };

            match result {
                Ok(response) => Ok(response.map(|inner| GrpcClientTraceBody {
                    inner,
                    span,
                    rpc_method,
                    started_at,
                    finished: false,
                })),
                Err(error) => {
                    record_completion(&span, &rpc_method, started_at, "transport_error");
                    Err(error)
                }
            }
        })
    }
}

/// Response wrapper keeps the client span open through unary decoding and
/// streaming response consumption, and records cancellation if the body drops
/// before it reaches the end.
#[doc(hidden)]
pub struct GrpcClientTraceBody<B>
where
    B: Body<Data = Bytes> + Unpin,
{
    inner: B,
    span: Span,
    rpc_method: String,
    started_at: Instant,
    finished: bool,
}

impl<B> Default for GrpcClientTraceBody<B>
where
    B: Body<Data = Bytes> + Unpin + Default,
{
    fn default() -> Self {
        Self {
            inner: B::default(),
            span: Span::none(),
            rpc_method: "unknown".to_owned(),
            started_at: Instant::now(),
            finished: true,
        }
    }
}

impl<B> GrpcClientTraceBody<B>
where
    B: Body<Data = Bytes> + Unpin,
{
    fn finish(&mut self, outcome: &'static str) {
        if self.finished {
            return;
        }
        self.finished = true;
        record_completion(&self.span, &self.rpc_method, self.started_at, outcome);
    }
}

impl<B> Body for GrpcClientTraceBody<B>
where
    B: Body<Data = Bytes> + Unpin,
{
    type Data = Bytes;
    type Error = B::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        let this = self.get_mut();
        let span = this.span.clone();
        let poll = span.in_scope(|| Pin::new(&mut this.inner).poll_frame(context));

        match &poll {
            Poll::Ready(None) => this.finish("response_body_complete"),
            Poll::Ready(Some(Err(_))) => this.finish("response_body_error"),
            Poll::Ready(Some(Ok(_))) | Poll::Pending => {}
        }
        poll
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> http_body::SizeHint {
        self.inner.size_hint()
    }
}

impl<B> Drop for GrpcClientTraceBody<B>
where
    B: Body<Data = Bytes> + Unpin,
{
    fn drop(&mut self) {
        if !self.finished {
            let outcome = if self.inner.is_end_stream() {
                "response_body_complete"
            } else {
                "response_body_dropped"
            };
            self.finish(outcome);
        }
    }
}

fn replace_trace_headers(headers: &mut HeaderMap, context: Option<&TraceContext>) {
    headers.remove(TRACEPARENT_HEADER);
    headers.remove(TRACESTATE_HEADER);
    let Some(context) = context else {
        return;
    };

    let traceparent = HeaderValue::from_str(&context.traceparent())
        .expect("normalized traceparent is a valid HTTP header value");
    headers.insert(TRACEPARENT_HEADER, traceparent);
    if let Some(tracestate) = context.tracestate() {
        if let Ok(tracestate) = HeaderValue::from_str(tracestate) {
            headers.insert(TRACESTATE_HEADER, tracestate);
        }
    }
}

fn bounded_rpc_method(path: &str) -> String {
    if path.len() > 128 {
        return "unknown".to_owned();
    }
    let Some(relative) = path.strip_prefix('/') else {
        return "unknown".to_owned();
    };
    let Some((service, method)) = relative.split_once('/') else {
        return "unknown".to_owned();
    };
    if service.is_empty()
        || method.is_empty()
        || method.contains('/')
        || !service
            .bytes()
            .chain(method.bytes())
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'.')
    {
        return "unknown".to_owned();
    }
    path.to_owned()
}

fn record_completion(span: &Span, rpc_method: &str, started_at: Instant, outcome: &'static str) {
    let duration_ms = u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX);
    span.record("duration_ms", duration_ms);
    span.record("outcome", outcome);
    tracing::debug!(
        parent: span,
        rpc_method,
        duration_ms,
        outcome,
        "gRPC client call completed"
    );
}

#[cfg(test)]
mod tests {
    use super::{bounded_rpc_method, GrpcClientTraceService};
    use crate::grpc_auth::{ServiceIdentityClientInterceptor, Workload};
    use crate::trace_context::{
        self, TraceContext, TraceContextLayer, TRACEPARENT_HEADER, TRACESTATE_HEADER,
    };
    use http::{HeaderMap, HeaderValue, Request, Response};
    use http_body::Body as _;
    use opentelemetry::trace::TracerProvider as _;
    use opentelemetry_sdk::trace::{InMemorySpanExporter, Sampler, SdkTracerProvider};
    use std::convert::Infallible;
    use std::sync::{Arc, Mutex};
    use tonic::body::Body;
    use tonic::service::interceptor::InterceptedService;
    use tower::Layer;
    use tower::ServiceExt;
    use tracing::instrument::WithSubscriber;
    use tracing_subscriber::prelude::*;

    #[tokio::test]
    async fn client_rpc_uses_a_distinct_validated_child_and_consumes_the_full_body() {
        const PARENT: &str = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
        let seen_headers = Arc::new(Mutex::new(None));
        let capture = seen_headers.clone();
        let service = tower::service_fn(move |request: Request<Body>| {
            *capture.lock().expect("capture lock") = Some(request.headers().clone());
            async { Ok::<_, Infallible>(Response::new(Body::empty())) }
        });
        let client = InterceptedService::new(
            GrpcClientTraceService::new(service),
            ServiceIdentityClientInterceptor::new(Workload::Gateway, "gateway-test-token")
                .expect("identity interceptor"),
        );
        let mut parent_headers = HeaderMap::new();
        parent_headers.insert(TRACEPARENT_HEADER, HeaderValue::from_static(PARENT));
        parent_headers.insert(TRACESTATE_HEADER, HeaderValue::from_static("vendor=opaque"));
        let parent = TraceContext::from_headers(&parent_headers);
        let request = Request::builder()
            .uri("/profile.v1.ProfileService/GetProfile")
            .body(Body::empty())
            .expect("request");

        let response = trace_context::scope(parent.clone(), async {
            client.oneshot(request).await.expect("client response")
        })
        .await;
        let mut body = Box::pin(response.into_body());
        let final_frame = futures::future::poll_fn(|context| {
            std::pin::Pin::as_mut(&mut body).poll_frame(context)
        })
        .await;

        assert!(final_frame.is_none());
        let headers = seen_headers
            .lock()
            .expect("capture lock")
            .clone()
            .expect("downstream request headers");
        let traceparent = headers
            .get(TRACEPARENT_HEADER)
            .expect("client child traceparent")
            .to_str()
            .expect("ASCII traceparent");
        assert_eq!(&traceparent[3..35], parent.trace_id());
        assert_ne!(&traceparent[36..52], parent.span_id());
        assert_eq!(
            headers
                .get(TRACESTATE_HEADER)
                .and_then(|value| value.to_str().ok()),
            Some("vendor=opaque")
        );
    }

    #[tokio::test]
    async fn inbound_http_context_flows_into_an_outbound_grpc_child() {
        const PARENT: &str = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
        let exporter = InMemorySpanExporter::default();
        let provider = SdkTracerProvider::builder()
            .with_simple_exporter(exporter.clone())
            .with_sampler(Sampler::AlwaysOn)
            .build();
        let subscriber = tracing_subscriber::registry().with(
            tracing_opentelemetry::layer()
                .with_tracer(provider.tracer("platform-test"))
                .with_filter(crate::telemetry::SafeSpanFilter),
        );
        // tracing-core's single-dispatcher cache uses the registering thread's
        // default subscriber. Other test threads may register this shared
        // callsite without our scoped capture subscriber. Retain two registered
        // dispatchers so registration considers the capture subscriber too.
        let _registration_peer = tracing::Dispatch::new(tracing_subscriber::registry());
        let dispatch = tracing::Dispatch::new(subscriber);
        // Exercise registration/cache rebuilding from a thread with no scoped
        // subscriber, rather than relying on a favorable test interleaving.
        std::thread::spawn(|| {
            tracing::callsite::rebuild_interest_cache();
            let mut client =
                GrpcClientTraceService::new(tower::service_fn(|_request: Request<Body>| async {
                    Ok::<_, Infallible>(Response::new(Body::empty()))
                }));
            drop(tower::Service::call(
                &mut client,
                Request::builder()
                    .uri("/profile.v1.ProfileService/GetProfile")
                    .body(Body::empty())
                    .unwrap(),
            ));
        })
        .join()
        .expect("unscoped callsite registration thread");
        let downstream_observation = Arc::new(Mutex::new(None));
        let capture = downstream_observation.clone();
        let grpc_service =
            TraceContextLayer.layer(tower::service_fn(move |request: Request<Body>| {
                let headers = request.headers().clone();
                let capture = capture.clone();
                async move {
                    let downstream_context =
                        trace_context::current().expect("downstream server trace context");
                    *capture.lock().expect("capture lock") = Some((headers, downstream_context));
                    Ok::<_, Infallible>(Response::new(Body::empty()))
                }
            }));
        let grpc_service = grpc_service.clone();
        let inbound = TraceContextLayer.layer(tower::service_fn(move |_request: Request<()>| {
            let grpc_service = grpc_service.clone();
            async move {
                let inbound_context = trace_context::current().expect("HTTP request trace context");
                let client = InterceptedService::new(
                    GrpcClientTraceService::new(grpc_service),
                    ServiceIdentityClientInterceptor::new(Workload::Gateway, "gateway-test-token")
                        .expect("identity interceptor"),
                );
                let rpc = Request::builder()
                    .uri("/profile.v1.ProfileService/GetProfile")
                    .body(Body::empty())
                    .expect("RPC request");
                let response = client.oneshot(rpc).await.expect("downstream RPC");
                Ok::<_, Infallible>((inbound_context, response))
            }
        }));
        let request = Request::builder()
            .method("GET")
            .uri("/api/profile/trace-test")
            .header(TRACEPARENT_HEADER, PARENT)
            .header(TRACESTATE_HEADER, "vendor=private-state-marker")
            .body(())
            .expect("inbound HTTP request");

        // The client span encloses response consumption as well as dispatch.
        // Keep the test subscriber installed for that entire lifecycle, rather
        // than dropping its scope before the client body completes/closes.
        let inbound_context = async {
            let (inbound_context, response) = inbound.oneshot(request).await.unwrap();
            let mut body = Box::pin(response.into_body());
            let final_frame = futures::future::poll_fn(|context| {
                std::pin::Pin::as_mut(&mut body).poll_frame(context)
            })
            .await;
            assert!(final_frame.is_none());
            drop(body);
            inbound_context
        }
        .with_subscriber(dispatch)
        .await;

        assert_eq!(
            inbound_context.trace_id(),
            "4bf92f3577b34da6a3ce929d0e0e4736"
        );
        assert_eq!(inbound_context.parent_span_id(), Some("00f067aa0ba902b7"));
        let (headers, downstream_context) = downstream_observation
            .lock()
            .expect("capture lock")
            .clone()
            .expect("downstream server observation");
        let child = headers
            .get(TRACEPARENT_HEADER)
            .expect("gRPC child traceparent")
            .to_str()
            .expect("ASCII traceparent");
        assert_eq!(&child[3..35], inbound_context.trace_id());
        assert_ne!(&child[36..52], inbound_context.span_id());
        assert_eq!(downstream_context.parent_span_id(), Some(&child[36..52]));
        assert_eq!(downstream_context.trace_id(), inbound_context.trace_id());
        assert_eq!(
            downstream_context.tracestate(),
            Some("vendor=private-state-marker")
        );
        assert!(headers.contains_key(TRACESTATE_HEADER));

        provider.force_flush().expect("flush in-memory spans");
        let spans = exporter.get_finished_spans().expect("read exported spans");
        let exported_http = spans
            .iter()
            .find(|span| {
                span.name == "request"
                    && format!("{:016x}", span.span_context.span_id()) == inbound_context.span_id()
            })
            .expect("exported inbound HTTP span");
        let exported_client = spans
            .iter()
            .find(|span| span.name == "grpc_client_call")
            .unwrap_or_else(|| {
                panic!(
                    "exported gRPC client span missing; completed spans: {:?}",
                    spans
                        .iter()
                        .map(|span| span.name.as_ref())
                        .collect::<Vec<_>>()
                )
            });
        let exported_server = spans
            .iter()
            .find(|span| {
                span.name == "request"
                    && format!("{:016x}", span.span_context.span_id())
                        == downstream_context.span_id()
            })
            .expect("exported downstream server span");

        assert_eq!(
            format!("{:032x}", exported_http.span_context.trace_id()),
            "4bf92f3577b34da6a3ce929d0e0e4736"
        );
        assert_eq!(
            format!("{:016x}", exported_http.parent_span_id),
            "00f067aa0ba902b7"
        );
        assert_eq!(
            format!("{:016x}", exported_client.parent_span_id),
            inbound_context.span_id()
        );
        assert_eq!(
            format!("{:016x}", exported_server.parent_span_id),
            format!("{:016x}", exported_client.span_context.span_id())
        );
        assert!(exported_http.span_context.trace_state().header().is_empty());
        assert!(exported_client
            .span_context
            .trace_state()
            .header()
            .is_empty());
        assert!(exported_server
            .span_context
            .trace_state()
            .header()
            .is_empty());
        assert!(exported_http.events.is_empty());
        assert!(exported_client.events.is_empty());
        assert!(exported_server.events.is_empty());
        assert!(spans.iter().all(|span| {
            span.attributes.iter().all(|attribute| {
                let value = format!("{:?}", attribute.value);
                !value.contains("gateway-test-token") && !value.contains("private-state-marker")
            })
        }));
    }

    #[tokio::test]
    async fn client_without_a_scoped_context_drops_caller_trace_headers() {
        let seen_headers = Arc::new(Mutex::new(None));
        let capture = seen_headers.clone();
        let service = tower::service_fn(move |request: Request<Body>| {
            *capture.lock().expect("capture lock") = Some(request.headers().clone());
            async { Ok::<_, Infallible>(Response::new(Body::empty())) }
        });
        let request = Request::builder()
            .uri("/profile.v1.ProfileService/GetProfile")
            .header(TRACEPARENT_HEADER, "00-invalid-caller-context")
            .header(TRACESTATE_HEADER, "vendor=caller-controlled")
            .body(Body::empty())
            .expect("request");

        let response = GrpcClientTraceService::new(service)
            .oneshot(request)
            .await
            .expect("client response");
        let mut body = Box::pin(response.into_body());
        let final_frame = futures::future::poll_fn(|context| {
            std::pin::Pin::as_mut(&mut body).poll_frame(context)
        })
        .await;

        assert!(final_frame.is_none());
        let headers = seen_headers
            .lock()
            .expect("capture lock")
            .clone()
            .expect("downstream request headers");
        assert!(!headers.contains_key(TRACEPARENT_HEADER));
        assert!(!headers.contains_key(TRACESTATE_HEADER));
    }

    #[test]
    fn rpc_method_label_accepts_only_bounded_grpc_paths() {
        assert_eq!(
            bounded_rpc_method("/profile.v1.ProfileService/GetProfile"),
            "/profile.v1.ProfileService/GetProfile"
        );
        assert_eq!(bounded_rpc_method("/service/method/with-extra"), "unknown");
        assert_eq!(bounded_rpc_method("/service/"), "unknown");
        assert_eq!(
            bounded_rpc_method(&format!("/service/{}", "x".repeat(128))),
            "unknown"
        );
    }
}
