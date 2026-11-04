//! Auth-only account deletion transport. Owning adapters determine completion;
//! canonical Auth provenance is independently checked around their committed ACK.
use crate::world::account_deletion::{
    check_deadline, DeletionAcknowledgement, DeletionAuthority, DeletionFailure, TicketDeletion,
    WorldDeletionCommand, WorldDeletionTarget,
};
use platform::{
    grpc_auth::{CallerRegistry, VerifiedCaller, Workload},
    grpc_method_policy::{guard_without_delegation, GuardedService, MethodPolicy},
};
use std::{
    convert::Infallible,
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::{Duration, Instant},
};
use tonic::{
    body::Body,
    codegen::{http, Service},
    server::NamedService,
    Request, Response, Status,
};
pub mod proto {
    include!(concat!(
        env!("OUT_DIR"),
        "/account_deletion/concert.account_deletion.v1.rs"
    ));
}
pub const POLICY: MethodPolicy = MethodPolicy {
    service: "concert.account_deletion.v1.AccountDeletionService",
    workload: Some(Workload::Concert),
    methods: &[("Apply", &[Workload::Auth])],
    actor_fields: &[],
};
#[async_trait::async_trait]
pub trait DeletionOwner: Send + Sync {
    async fn apply(
        &self,
        command: WorldDeletionCommand,
        original_deadline: tokio::time::Instant,
    ) -> Result<DeletionAcknowledgement, DeletionFailure>;
}
#[async_trait::async_trait]
impl DeletionOwner for TicketDeletion {
    async fn apply(
        &self,
        command: WorldDeletionCommand,
        original_deadline: tokio::time::Instant,
    ) -> Result<DeletionAcknowledgement, DeletionFailure> {
        TicketDeletion::apply(self, command, original_deadline).await
    }
}
#[async_trait::async_trait]
impl DeletionOwner for crate::social_live::account_deletion::SharingDeletion {
    async fn apply(
        &self,
        command: WorldDeletionCommand,
        original_deadline: tokio::time::Instant,
    ) -> Result<DeletionAcknowledgement, DeletionFailure> {
        crate::social_live::account_deletion::SharingDeletion::apply(
            self,
            command,
            original_deadline,
        )
        .await
    }
}
/// Missing owners are unavailable. Injection is not permission to substitute
/// a positive fixture for an unimplemented production owner or drain publisher.
#[derive(Clone, Default)]
pub struct DeletionOwners {
    pub tickets: Option<Arc<dyn DeletionOwner>>,
    pub sessions: Option<Arc<dyn DeletionOwner>>,
    pub sharing: Option<Arc<dyn DeletionOwner>>,
    pub private_data: Option<Arc<dyn DeletionOwner>>,
}
impl DeletionOwners {
    fn owner(&self, target: WorldDeletionTarget) -> Result<Arc<dyn DeletionOwner>, Status> {
        match target {
            WorldDeletionTarget::Tickets => &self.tickets,
            WorldDeletionTarget::Sessions => &self.sessions,
            WorldDeletionTarget::Sharing => &self.sharing,
            WorldDeletionTarget::PrivateData => &self.private_data,
        }
        .clone()
        .ok_or_else(|| Status::unavailable("Concert deletion owner unavailable"))
    }
}
pub struct ApplyServer {
    owners: DeletionOwners,
    authority: Arc<dyn DeletionAuthority>,
}
type Generated = proto::account_deletion_service_server::AccountDeletionServiceServer<ApplyServer>;
pub fn guarded_server(
    owners: DeletionOwners,
    authority: Arc<dyn DeletionAuthority>,
    registry: CallerRegistry,
) -> anyhow::Result<Deadline<GuardedService<Generated>>> {
    Ok(Deadline(guard_without_delegation(
        &POLICY,
        Generated::new(ApplyServer { owners, authority })
            .max_decoding_message_size(2048)
            .max_encoding_message_size(2048),
        registry,
    )?))
}
fn invalid() -> Status {
    Status::invalid_argument("Invalid Concert deletion command")
}
fn expired() -> Status {
    Status::deadline_exceeded("Concert deletion deadline expired")
}
fn failure(e: DeletionFailure) -> Status {
    match e {
        DeletionFailure::InvalidCommand => invalid(),
        DeletionFailure::Incomplete => Status::failed_precondition("Concert deletion incomplete"),
        DeletionFailure::Unavailable => Status::unavailable("Concert deletion unavailable"),
    }
}
fn id(s: &str) -> Result<uuid::Uuid, Status> {
    uuid::Uuid::parse_str(s)
        .ok()
        .filter(|u| !u.is_nil() && u.to_string() == s)
        .ok_or_else(invalid)
}
/// Shape conversion only: a raw decoded command never constitutes authority.
pub fn decode_command(c: &proto::Command) -> Result<WorldDeletionCommand, Status> {
    if !(0..1_000_000_000).contains(&c.retention_nanos) {
        return Err(invalid());
    }
    Ok(WorldDeletionCommand {
        saga_id: id(&c.saga_id)?,
        user_id: id(&c.user_id)?,
        actor_id: id(&c.actor_id)?,
        target: match c.target {
            1 => WorldDeletionTarget::Tickets,
            2 => WorldDeletionTarget::Sessions,
            3 => WorldDeletionTarget::Sharing,
            4 => WorldDeletionTarget::PrivateData,
            _ => return Err(invalid()),
        },
        retention_until: chrono::DateTime::from_timestamp(
            c.retention_seconds,
            c.retention_nanos as u32,
        )
        .ok_or_else(invalid)?,
    })
}
// Deadline wrapper derives from the qualified Auth source bridge at50a3e054.
// The clock starts before protobuf decode and is never recreated after await.
#[derive(Clone)]
pub struct Deadline<S>(S);
impl<S: NamedService> NamedService for Deadline<S> {
    const NAME: &'static str = S::NAME;
}
impl<S, B> Service<http::Request<Body>> for Deadline<S>
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
    fn call(&mut self, q: http::Request<Body>) -> Self::Future {
        let now = Instant::now();
        let Some(left) = platform::call_deadline::received_timeout(q.headers())
            .filter(|d| !d.is_zero() && *d <= Duration::from_secs(5))
        else {
            return Box::pin(async { Ok(expired().into_http()) });
        };
        let stop = platform::call_deadline::current().map_or(now + left, |p| p.min(now + left));
        let f = self.0.call(q);
        Box::pin(platform::call_deadline::scope(stop, async move {
            let mut r = match tokio::time::timeout_at(stop.into(), f).await {
                Ok(v) => v?.map(Body::new),
                Err(_) => expired().into_http(),
            };
            r.headers_mut().insert(
                http::header::CACHE_CONTROL,
                http::HeaderValue::from_static("no-store"),
            );
            Ok(r)
        }))
    }
}
#[tonic::async_trait]
impl proto::account_deletion_service_server::AccountDeletionService for ApplyServer {
    async fn apply(
        &self,
        q: Request<proto::ApplyRequest>,
    ) -> Result<Response<proto::ApplyResponse>, Status> {
        if !matches!(
            q.extensions().get::<VerifiedCaller>(),
            Some(VerifiedCaller(Workload::Auth))
        ) || q.peer_certs().map_or(true, |c| c.is_empty())
        {
            return Err(Status::unauthenticated(
                "Verified mTLS Auth workload required",
            ));
        }
        let original = platform::call_deadline::current().ok_or_else(expired)?;
        let q = q.into_inner();
        let wire = q.command.as_ref().ok_or_else(invalid)?;
        let command = decode_command(wire)?;
        if q.request_nonce.len() != 16 || q.request_nonce.iter().all(|b| *b == 0) {
            return Err(invalid());
        }
        let left = q
            .expires_unix_micros
            .checked_sub(chrono::Utc::now().timestamp_micros())
            .ok_or_else(expired)?;
        if left <= 0 {
            return Err(expired());
        }
        if left > 5_000_000 {
            return Err(invalid());
        }
        let original = original.min(Instant::now() + Duration::from_micros(left as u64));
        let stop = tokio::time::Instant::from_std(original);
        check_deadline(stop).map_err(failure)?;
        let owner = self.owners.owner(command.target)?;
        tokio::time::timeout_at(
            stop,
            platform::call_deadline::scope(original, async {
                let source = self
                    .authority
                    .verify(&command, stop)
                    .await
                    .map_err(failure)?;
                source.check(&command, stop).map_err(failure)?;
                let ack = owner.apply(command.clone(), stop).await.map_err(failure)?;
                if ack.command != command || ack.fence_revision == 0 {
                    return Err(Status::unavailable("Concert deletion receipt mismatch"));
                }
                let again = self
                    .authority
                    .verify(&command, stop)
                    .await
                    .map_err(failure)?;
                again.check(&command, stop).map_err(failure)?;
                if source != again {
                    return Err(Status::unavailable("Concert deletion source changed"));
                }
                check_deadline(stop).map_err(failure)?;
                if chrono::Utc::now().timestamp_micros() >= q.expires_unix_micros {
                    return Err(expired());
                }
                let mut r = Response::new(proto::ApplyResponse {
                    command: Some(wire.clone()),
                    fence_revision: ack.fence_revision,
                    source_binding: again.source_binding().to_vec(),
                    request_nonce: q.request_nonce.clone(),
                    expires_unix_micros: q.expires_unix_micros,
                });
                r.metadata_mut().insert(
                    "cache-control",
                    "no-store".parse().expect("static metadata"),
                );
                Ok(r)
            }),
        )
        .await
        .map_err(|_| expired())?
    }
}
