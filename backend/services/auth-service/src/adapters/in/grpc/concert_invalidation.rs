//! Auth-owned system-consumer replay. Hints invalidate; they never grant authority.
use super::concert_relationship::proto;
use crate::concert_invalidation::{
    InvalidationKind, InvalidationSignal, PgInvalidationJournal, ReplayCursor, ReplayError,
    MAX_CURSOR,
};
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
use tonic::{body::Body, codegen::Service, server::NamedService, Request, Response, Status};
use uuid::Uuid;

pub const MAX_OPERATION: Duration = Duration::from_secs(1);
pub const MAX_REQUEST_BYTES: usize = 4096;
pub const MAX_RESPONSE_BYTES: usize = 128 * 1024;
pub const POLICY: MethodPolicy = MethodPolicy {
    service: "relationship.v1.AuthConcertInvalidationService",
    workload: Some(Workload::Auth),
    methods: &[
        ("ReadHead", &[Workload::Concert]),
        ("ReadAfter", &[Workload::Concert]),
    ],
    actor_fields: &[],
};
pub struct InvalidationServer {
    pool: sqlx::PgPool,
}
impl InvalidationServer {
    pub fn from_pool(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}
type GeneratedServer =
    proto::auth_concert_invalidation_service_server::AuthConcertInvalidationServiceServer<
        InvalidationServer,
    >;

/// Register with retained workload credentials/method policy. The caller must
/// configure retained mutual TLS; main omits this service on plaintext listeners.
/// No actor delegation, JWT or consumer ID authenticates this system read.
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
/// One original budget starts before guard/decode. Dropping this future on
/// HTTP/2 cancellation/timeout drops the read future and queues transaction
/// rollback; any in-flight SQL retains its original bounded statement timer.
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
        let mut stop = platform::call_deadline::current().map_or(now + MAX_OPERATION, |original| {
            original.min(now + MAX_OPERATION)
        });
        if let Some(remaining) = platform::call_deadline::received_timeout(request.headers()) {
            if let Some(original) = now.checked_add(remaining) {
                stop = stop.min(original);
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
    Status::deadline_exceeded("Auth invalidation deadline elapsed")
}
fn unavailable() -> Status {
    Status::unavailable("Auth invalidation owner unavailable")
}
fn invalid() -> Status {
    Status::invalid_argument("Invalid Auth invalidation query")
}
fn canonical(value: &str) -> Result<Uuid, Status> {
    let id = Uuid::parse_str(value).map_err(|_| invalid())?;
    if id.is_nil() || id.to_string() != value {
        return Err(invalid());
    }
    Ok(id)
}
fn caller<T>(request: &Request<T>) -> Result<(), Status> {
    match request.extensions().get::<VerifiedCaller>() {
        Some(VerifiedCaller(Workload::Concert)) => Ok(()),
        Some(_) => Err(Status::permission_denied("Concert consumer required")),
        None => Err(Status::unauthenticated("Verified Concert caller required")),
    }
}
fn check_time(stop: Instant) -> Result<(), Status> {
    if Instant::now() >= stop {
        Err(elapsed())
    } else {
        Ok(())
    }
}
fn original_stop() -> Result<Instant, Status> {
    // Registration starts this before decode. Direct handler calls cannot start
    // a fresh budget after pool readiness or query parsing.
    let stop = platform::call_deadline::current().ok_or_else(unavailable)?;
    check_time(stop)?;
    Ok(stop.min(Instant::now() + MAX_OPERATION))
}
fn journal(pool: &sqlx::PgPool, stop: Instant) -> Result<PgInvalidationJournal, Status> {
    check_time(stop)?;
    // The retained journal floors statement_timeout to whole milliseconds. Round
    // its SQL timer up so it cannot obscure the operation deadline as an early
    // Unavailable. Both outer timeout_at calls still enforce the ORIGINAL stop.
    let remaining = stop.saturating_duration_since(Instant::now());
    PgInvalidationJournal::new(
        pool.clone(),
        Duration::from_millis(remaining.as_millis() as u64 + 1),
    )
    .map_err(|_| elapsed())
}
fn owner_error(error: ReplayError, stop: Instant) -> Status {
    if Instant::now() >= stop {
        return elapsed();
    }
    match error {
        // Structural/range input was checked before I/O; this InvalidCursor is
        // a future position in the owner's consistent committed snapshot.
        ReplayError::InvalidCursor | ReplayError::OwnerEpochMismatch => {
            Status::failed_precondition("Auth invalidation checkpoint mismatch")
        }
        ReplayError::InvalidBatchSize => invalid(),
        ReplayError::Unavailable => unavailable(),
    }
}
fn wire_cursor(value: ReplayCursor) -> proto::InvalidationCursor {
    proto::InvalidationCursor {
        owner_epoch: value.owner_epoch.to_string(),
        sequence: value.after,
    }
}
fn wire_signal(value: InvalidationSignal) -> Result<proto::InvalidationSignal, Status> {
    use proto::invalidation_signal::Scope;
    let user = || {
        Scope::User(proto::InvalidationUserScope {
            user_id: value.user_id.to_string(),
        })
    };
    let (reason, scope) = match (value.kind, value.family_id) {
        (InvalidationKind::SessionChanged, Some(family)) if !family.is_nil() => (
            proto::InvalidationReason::AuthSessionChanged,
            Scope::Family(proto::InvalidationFamilyScope {
                user_id: value.user_id.to_string(),
                session_family_id: family.to_string(),
            }),
        ),
        (InvalidationKind::UserAuthorityChanged, None) => {
            (proto::InvalidationReason::AuthUserAuthorityChanged, user())
        }
        (InvalidationKind::CredentialsChanged, None) => {
            (proto::InvalidationReason::AuthCredentialsChanged, user())
        }
        (InvalidationKind::UserRemoved, None) => {
            (proto::InvalidationReason::AuthUserRemoved, user())
        }
        _ => return Err(unavailable()),
    };
    if value.user_id.is_nil() || value.signal_id.is_nil() {
        return Err(unavailable());
    }
    ReplayCursor::new(value.cursor.owner_epoch, value.cursor.after).map_err(|_| unavailable())?;
    if value.cursor.after == 0 {
        return Err(unavailable());
    }
    Ok(proto::InvalidationSignal {
        cursor: Some(wire_cursor(value.cursor)),
        signal_id: value.signal_id.to_string(),
        reason: reason as i32,
        scope: Some(scope),
    })
}
fn response<T: Message>(message: T, stop: Instant) -> Result<Response<T>, Status> {
    if message.encoded_len() > MAX_RESPONSE_BYTES {
        return Err(Status::resource_exhausted(
            "Auth invalidation response too large",
        ));
    }
    check_time(stop)?;
    let mut result = Response::new(message);
    result.metadata_mut().insert(
        "cache-control",
        tonic::metadata::MetadataValue::from_static("no-store"),
    );
    Ok(result)
}
#[tonic::async_trait]
impl proto::auth_concert_invalidation_service_server::AuthConcertInvalidationService
    for InvalidationServer
{
    async fn read_head(
        &self,
        request: Request<proto::ReadInvalidationHeadRequest>,
    ) -> Result<Response<proto::ReadInvalidationHeadResponse>, Status> {
        caller(&request)?;
        let stop = original_stop()?;
        let query = request.into_inner();
        canonical(&query.consumer_id)?;
        canonical(&query.request_id)?;
        if query.encoded_len() > MAX_REQUEST_BYTES {
            return Err(invalid());
        }
        let head = tokio::time::timeout_at(stop.into(), journal(&self.pool, stop)?.checkpoint())
            .await
            .map_err(|_| elapsed())?
            .map_err(|e| owner_error(e, stop))?;
        response(
            proto::ReadInvalidationHeadResponse {
                query: Some(query),
                head: Some(wire_cursor(head)),
            },
            stop,
        )
    }
    async fn read_after(
        &self,
        request: Request<proto::ReadInvalidationsRequest>,
    ) -> Result<Response<proto::ReadInvalidationsResponse>, Status> {
        caller(&request)?;
        let stop = original_stop()?;
        let query = request.into_inner();
        canonical(&query.consumer_id)?;
        canonical(&query.request_id)?;
        let after = query.after.as_ref().ok_or_else(invalid)?;
        if !(0..=MAX_CURSOR).contains(&after.sequence)
            || !(1..=256).contains(&query.limit)
            || query.encoded_len() > MAX_REQUEST_BYTES
        {
            return Err(invalid());
        }
        let after = ReplayCursor::new(canonical(&after.owner_epoch)?, after.sequence)
            .map_err(|_| invalid())?;
        let page = tokio::time::timeout_at(
            stop.into(),
            journal(&self.pool, stop)?.replay(after, query.limit as u16),
        )
        .await
        .map_err(|_| elapsed())?
        .map_err(|e| owner_error(e, stop))?;
        let mut expected = after.after;
        if page.high_water.owner_epoch != after.owner_epoch
            || page.high_water.after < after.after
            || page.high_water.after > MAX_CURSOR
            || page.signals.len() > query.limit as usize
        {
            return Err(unavailable());
        }
        let mut seen = std::collections::HashSet::new();
        let mut signals = Vec::with_capacity(page.signals.len());
        for signal in page.signals {
            expected += 1;
            if signal.cursor.owner_epoch != after.owner_epoch
                || signal.cursor.after != expected
                || !seen.insert(signal.signal_id)
            {
                return Err(unavailable());
            }
            signals.push(wire_signal(signal)?);
        }
        if page.next
            != (ReplayCursor {
                owner_epoch: after.owner_epoch,
                after: expected,
            })
            || expected > page.high_water.after
            || (signals.len() < query.limit as usize && expected != page.high_water.after)
            || page.has_more != (expected < page.high_water.after)
        {
            return Err(unavailable());
        }
        response(
            proto::ReadInvalidationsResponse {
                query: Some(query),
                next: Some(wire_cursor(page.next)),
                head: Some(wire_cursor(page.high_water)),
                signals,
            },
            stop,
        )
    }
}
