//! Friend-owned system-consumer replay. Signals invalidate; they grant no authority.
//! Wrapper shape follows Auth d46e19cc; Friend's journal/budget remain independent.
use super::concert_relationship::proto;
use crate::concert_invalidation::{HeadQuery, PageQuery, PgJournal, ReadBudget, ReadCancellation};
use platform::{
    grpc_auth::{CallerRegistry, VerifiedCaller, Workload},
    grpc_method_policy::{guard_without_delegation, GuardedService, MethodPolicy},
};
use prost::Message;
use std::{
    convert::Infallible,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::{Duration, Instant},
};
use tonic::{
    body::Body,
    codegen::{http, Service},
    server::NamedService,
    Request, Response, Status,
};

pub const MAX_OPERATION: Duration = Duration::from_secs(1);
pub const MAX_REQUEST_BYTES: usize = 4096;
pub const MAX_RESPONSE_BYTES: usize = 128 * 1024;
pub const POLICY: MethodPolicy = MethodPolicy {
    service: "relationship.v1.FriendConcertInvalidationService",
    workload: Some(Workload::Friend),
    methods: &[
        ("ReadHead", &[Workload::Concert]),
        ("ReadAfter", &[Workload::Concert]),
    ],
    actor_fields: &[],
};
pub struct InvalidationServer {
    journal: PgJournal,
}
impl InvalidationServer {
    pub fn from_pool(pool: sqlx::PgPool) -> Self {
        Self {
            journal: PgJournal::new(pool),
        }
    }
}
type GeneratedServer =
    proto::friend_concert_invalidation_service_server::FriendConcertInvalidationServiceServer<
        InvalidationServer,
    >;
/// Compose only on the retained mutually authenticated TLS listener. Startup
/// omits this service when TLS is absent; consumer IDs and actor JWTs do not authenticate it.
pub fn guarded_server(
    pool: sqlx::PgPool,
    registry: CallerRegistry,
) -> anyhow::Result<OperationDeadline<GuardedService<GeneratedServer>>> {
    Ok(OperationDeadline(guard_without_delegation(
        &POLICY,
        GeneratedServer::new(InvalidationServer::from_pool(pool))
            .max_decoding_message_size(MAX_REQUEST_BYTES)
            .max_encoding_message_size(MAX_RESPONSE_BYTES),
        registry,
    )?))
}
/// Starts before caller/method guards and protobuf frame decoding. Inherits the
/// ingress deadline; timeout/cancel drops the handler and its owner read together.
#[derive(Clone)]
pub struct OperationDeadline<S>(S);
impl<S: NamedService> NamedService for OperationDeadline<S> {
    const NAME: &'static str = S::NAME;
}
impl<S, B> Service<http::Request<Body>> for OperationDeadline<S>
where
    S: Service<http::Request<Body>, Response = http::Response<B>, Error = Infallible>,
    S::Future: Send + 'static,
    B: tonic::codegen::Body<Data = tonic::codegen::Bytes> + Send + 'static,
    B::Error: Into<tonic::codegen::StdError>,
{
    type Response = http::Response<Body>;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.0.poll_ready(cx)
    }
    fn call(&mut self, request: http::Request<Body>) -> Self::Future {
        let now = Instant::now();
        let mut stop = platform::call_deadline::current().map_or(now + MAX_OPERATION, |parent| {
            parent.min(now + MAX_OPERATION)
        });
        if let Some(remaining) = platform::call_deadline::received_timeout(request.headers()) {
            if let Some(parent) = now.checked_add(remaining) {
                stop = stop.min(parent);
            }
        }
        let future = self.0.call(request);
        Box::pin(platform::call_deadline::scope(stop, async move {
            match tokio::time::timeout_at(stop.into(), future).await {
                Ok(result) => result.map(|response| response.map(Body::new)),
                Err(_) => Ok(elapsed().into_http()),
            }
        }))
    }
}
fn elapsed() -> Status {
    Status::deadline_exceeded("Friend invalidation deadline elapsed")
}
fn caller<T>(request: &Request<T>) -> Result<(), Status> {
    match request.extensions().get::<VerifiedCaller>() {
        Some(VerifiedCaller(Workload::Concert)) => Ok(()),
        Some(_) => Err(Status::permission_denied("Concert consumer required")),
        None => Err(Status::unauthenticated("Verified Concert caller required")),
    }
}
fn original_stop() -> Result<Instant, Status> {
    let stop = platform::call_deadline::current()
        .ok_or_else(|| Status::unavailable("Original Friend invalidation budget required"))?;
    if Instant::now() >= stop {
        return Err(elapsed());
    }
    if stop.saturating_duration_since(Instant::now()) > MAX_OPERATION {
        return Err(Status::unavailable(
            "Original Friend invalidation budget required",
        ));
    }
    Ok(stop)
}
struct CancelOnDrop(ReadCancellation);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
}
fn response<T: Message>(message: T, budget: &ReadBudget) -> Result<Response<T>, Status> {
    if message.encoded_len() > MAX_RESPONSE_BYTES {
        return Err(Status::resource_exhausted(
            "Friend invalidation response too large",
        ));
    }
    budget.check().map_err(Status::from)?;
    let mut result = Response::new(message);
    result.metadata_mut().insert(
        "cache-control",
        tonic::metadata::MetadataValue::from_static("no-store"),
    );
    Ok(result)
}
#[tonic::async_trait]
impl proto::friend_concert_invalidation_service_server::FriendConcertInvalidationService
    for InvalidationServer
{
    async fn read_head(
        &self,
        request: Request<proto::ReadInvalidationHeadRequest>,
    ) -> Result<Response<proto::ReadInvalidationHeadResponse>, Status> {
        caller(&request)?;
        let stop = original_stop()?;
        let message = request.into_inner();
        if message.encoded_len() > MAX_REQUEST_BYTES {
            return Err(Status::invalid_argument(
                "Invalid Friend invalidation query",
            ));
        }
        let query = HeadQuery::try_from(message.clone()).map_err(Status::from)?;
        let cancellation = ReadCancellation::default();
        let _cancel = CancelOnDrop(cancellation.clone());
        let budget = ReadBudget::new(stop, cancellation).map_err(Status::from)?;
        let result = self
            .journal
            .read_head(query, &budget)
            .await
            .map_err(Status::from)?
            .to_wire()
            .map_err(Status::from)?;
        if result.query.as_ref() != Some(&message) {
            return Err(Status::unavailable("Friend invalidation query mismatch"));
        }
        response(result, &budget)
    }
    async fn read_after(
        &self,
        request: Request<proto::ReadInvalidationsRequest>,
    ) -> Result<Response<proto::ReadInvalidationsResponse>, Status> {
        caller(&request)?;
        let stop = original_stop()?;
        let message = request.into_inner();
        if message.encoded_len() > MAX_REQUEST_BYTES {
            return Err(Status::invalid_argument(
                "Invalid Friend invalidation query",
            ));
        }
        let query = PageQuery::try_from(message.clone()).map_err(Status::from)?;
        let cancellation = ReadCancellation::default();
        let _cancel = CancelOnDrop(cancellation.clone());
        let budget = ReadBudget::new(stop, cancellation).map_err(Status::from)?;
        let result = self
            .journal
            .read_after(query, &budget)
            .await
            .map_err(Status::from)?
            .to_wire()
            .map_err(Status::from)?;
        if result.query.as_ref() != Some(&message) {
            return Err(Status::unavailable("Friend invalidation query mismatch"));
        }
        response(result, &budget)
    }
}
