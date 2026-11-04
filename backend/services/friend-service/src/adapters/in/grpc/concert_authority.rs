use crate::concert_authority::{
    proto, OperationBudget, PgOwnerAuthority, MAX_COMPLETION, MAX_OPERATION,
};
use platform::{
    actor_delegation::{
        read_authority::VerifiedReadAuthority, relationship_read::RelationshipReadVerifier,
        VerifiedActor,
    },
    grpc_auth::{CallerRegistry, VerifiedCaller, Workload},
    grpc_method_policy::{GuardedService, MethodPolicy},
};
use std::{
    convert::Infallible,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Instant,
};
use tonic::{
    body::Body,
    codegen::{http, Service},
    server::NamedService,
    Request, Response, Status,
};

pub const POLICY: MethodPolicy = MethodPolicy {
    service: "authority.lease.v1.FriendOwnerAuthorityService",
    workload: Some(Workload::Friend),
    methods: &[
        ("Acquire", &[Workload::Concert]),
        (
            "Check",
            &[Workload::Concert, Workload::Post, Workload::Profile],
        ),
        (
            "Enroll",
            &[Workload::Concert, Workload::Post, Workload::Profile],
        ),
        (
            "Track",
            &[Workload::Concert, Workload::Post, Workload::Profile],
        ),
        ("Complete", &[Workload::Concert]),
    ],
    actor_fields: &[("Acquire", &[&[1, 1]])],
};
pub struct AuthorityServer {
    store: PgOwnerAuthority,
}
pub type OwnerServer = OperationDeadline<
    GuardedService<
        proto::friend_owner_authority_service_server::FriendOwnerAuthorityServiceServer<
            AuthorityServer,
        >,
    >,
>;
pub fn guarded_server(
    store: PgOwnerAuthority,
    registry: CallerRegistry,
    verifier: RelationshipReadVerifier,
) -> anyhow::Result<OwnerServer> {
    Ok(OperationDeadline(platform::grpc_method_policy::guard(
        &POLICY,
        proto::friend_owner_authority_service_server::FriendOwnerAuthorityServiceServer::new(
            AuthorityServer { store },
        )
        .max_decoding_message_size(64 * 1024)
        .max_encoding_message_size(64 * 1024),
        registry,
        verifier.into_inner(),
    )?))
}
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
        let ceiling = if request.uri().path().ends_with("/Complete") {
            MAX_COMPLETION
        } else {
            MAX_OPERATION
        };
        let mut stop =
            platform::call_deadline::current().map_or(now + ceiling, |v| v.min(now + ceiling));
        if let Some(remaining) = platform::call_deadline::received_timeout(request.headers()) {
            if let Some(original) = now.checked_add(remaining) {
                stop = stop.min(original);
            }
        }
        let future = self.0.call(request);
        Box::pin(platform::call_deadline::scope(stop, async move {
            match tokio::time::timeout_at(stop.into(), future).await {
                Ok(result) => result.map(|r| r.map(Body::new)),
                Err(_) => Ok(Status::deadline_exceeded("Owner operation expired").into_http()),
            }
        }))
    }
}
fn caller<T>(request: &Request<T>, root: bool) -> Result<Workload, Status> {
    let who = request
        .extensions()
        .get::<VerifiedCaller>()
        .ok_or_else(|| Status::unauthenticated("Verified owner caller required"))?
        .0;
    if if root {
        who != Workload::Concert
    } else {
        !matches!(who, Workload::Concert | Workload::Post | Workload::Profile)
    } {
        return Err(Status::permission_denied("Owner caller denied"));
    }
    Ok(who)
}
fn budget(binding: &proto::GuardBinding, cleanup: bool) -> Result<OperationBudget, Status> {
    let original = platform::call_deadline::current()
        .ok_or_else(|| Status::invalid_argument("Original owner deadline required"))?;
    let ceiling = if cleanup {
        MAX_COMPLETION
    } else {
        MAX_OPERATION
    };
    let mut stop = original.min(Instant::now() + ceiling);
    if !cleanup {
        let remaining = binding
            .expires_unix_micros
            .saturating_sub(chrono::Utc::now().timestamp_micros());
        if remaining <= 0 {
            return Err(Status::deadline_exceeded("Owner guard expired"));
        }
        stop = stop.min(Instant::now() + std::time::Duration::from_micros(remaining as u64));
    }
    OperationBudget::new(stop)
}
fn reply<T>(value: T) -> Response<T> {
    let mut r = Response::new(value);
    r.metadata_mut()
        .insert("cache-control", "no-store".parse().unwrap());
    r
}
#[tonic::async_trait]
impl proto::friend_owner_authority_service_server::FriendOwnerAuthorityService for AuthorityServer {
    async fn acquire(
        &self,
        request: Request<proto::AcquireRequest>,
    ) -> Result<Response<proto::AcquireResponse>, Status> {
        caller(&request, true)?;
        let actor = request
            .extensions()
            .get::<VerifiedActor>()
            .ok_or_else(|| Status::unauthenticated("Verified owner actor required"))?
            .0;
        let proof = request
            .extensions()
            .get::<VerifiedReadAuthority>()
            .copied()
            .ok_or_else(|| Status::unauthenticated("Verified owner deadline required"))?;
        let message = request.into_inner();
        let binding = message
            .binding
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Owner binding required"))?;
        let original = proof.deadline(MAX_OPERATION)?;
        if binding.actor_id != actor.to_string()
            || actor != proof.actor()
            || binding.expires_unix_micros > original.expires_micros()
        {
            return Err(Status::permission_denied("Owner signed binding denied"));
        }
        let clock = budget(binding, false)?;
        let clock = OperationBudget::new(clock.stop().min(original.stop().into_std()))?;
        Ok(reply(self.store.acquire(message, &clock).await?))
    }
    async fn check(
        &self,
        request: Request<proto::CheckRequest>,
    ) -> Result<Response<proto::CheckResponse>, Status> {
        caller(&request, false)?;
        let message = request.into_inner();
        let clock = budget(
            message
                .binding
                .as_ref()
                .ok_or_else(|| Status::invalid_argument("Owner binding required"))?,
            false,
        )?;
        Ok(reply(self.store.check(message, &clock).await?))
    }
    async fn enroll(
        &self,
        request: Request<proto::EnrollRequest>,
    ) -> Result<Response<proto::CheckResponse>, Status> {
        caller(&request, false)?;
        let message = request.into_inner();
        let clock = budget(
            message
                .binding
                .as_ref()
                .ok_or_else(|| Status::invalid_argument("Owner binding required"))?,
            false,
        )?;
        Ok(reply(self.store.enroll(message, &clock).await?))
    }
    async fn track(
        &self,
        request: Request<proto::TrackRequest>,
    ) -> Result<Response<proto::TrackResponse>, Status> {
        let who = caller(&request, false)?;
        let message = request.into_inner();
        let clock = budget(
            message
                .binding
                .as_ref()
                .ok_or_else(|| Status::invalid_argument("Owner binding required"))?,
            false,
        )?;
        Ok(reply(self.store.track(message, who, &clock).await?))
    }
    async fn complete(
        &self,
        request: Request<proto::CompleteRequest>,
    ) -> Result<Response<proto::CompleteResponse>, Status> {
        caller(&request, true)?;
        let message = request.into_inner();
        let clock = budget(
            message
                .binding
                .as_ref()
                .ok_or_else(|| Status::invalid_argument("Owner binding required"))?,
            true,
        )?;
        Ok(reply(self.store.complete(message, &clock).await?))
    }
}
