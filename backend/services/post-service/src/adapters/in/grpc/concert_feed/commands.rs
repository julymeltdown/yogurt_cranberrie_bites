//! Dedicated whole-envelope command server. Legacy workload-only RPCs stay off.
use super::proto;
use crate::concert_feed::{
    command_admission::CommandAdmission, command_runtime::QualifiedCommandRuntime,
};
use platform::{
    actor_delegation::read_authority::VerifiedReadAuthority,
    concert_feed_command::FeedCommandVerifier,
    grpc_auth::{CallerRegistry, VerifiedCaller, Workload},
    grpc_method_policy::{self, GuardedService, MethodPolicy},
};
use tonic::{Request, Response, Status};

pub const POLICY: MethodPolicy = MethodPolicy {
    service: "post.concert.v1.PostConcertFeedCommandService",
    workload: Some(Workload::Post),
    methods: &[
        ("ExecuteCommand", &[Workload::Concert]),
        ("GetReceipt", &[Workload::Concert]),
        ("PreviewCommand", &[Workload::Concert]),
    ],
    actor_fields: &[
        ("ExecuteCommand", &[&[1, 1, 1], &[1, 2, 2], &[2, 1, 1, 1]]),
        ("GetReceipt", &[&[1, 1, 1], &[1, 2, 2], &[2, 1, 1, 1]]),
        ("PreviewCommand", &[&[1, 1, 1], &[1, 2, 2], &[2, 1, 1, 1]]),
    ],
};
#[derive(Clone)]
pub struct CommandServer {
    runtime: QualifiedCommandRuntime,
}
pub type Service = GuardedService<
    proto::post_concert_feed_command_service_server::PostConcertFeedCommandServiceServer<
        CommandServer,
    >,
>;
pub fn service(
    runtime: QualifiedCommandRuntime,
    callers: CallerRegistry,
    verifier: FeedCommandVerifier,
) -> anyhow::Result<Service> {
    let server =
        proto::post_concert_feed_command_service_server::PostConcertFeedCommandServiceServer::new(
            CommandServer { runtime },
        )
        .max_decoding_message_size(131072)
        .max_encoding_message_size(1048576);
    grpc_method_policy::guard(&POLICY, server, callers, verifier.into_inner())
}
fn verified<T>(r: &Request<T>) -> Result<(VerifiedCaller, VerifiedReadAuthority), Status> {
    let caller = *r
        .extensions()
        .get::<VerifiedCaller>()
        .ok_or_else(|| Status::unauthenticated("Verified caller required"))?;
    if caller.0 != Workload::Concert {
        return Err(Status::permission_denied("Feed command caller denied"));
    }
    let proof = *r
        .extensions()
        .get::<VerifiedReadAuthority>()
        .ok_or_else(|| Status::unauthenticated("Original command deadline required"))?;
    Ok((caller, proof))
}
fn response<T>(value: T) -> Response<T> {
    let mut r = Response::new(value);
    r.metadata_mut().insert(
        "cache-control",
        "no-store".parse().expect("static metadata"),
    );
    r
}
#[tonic::async_trait]
impl proto::post_concert_feed_command_service_server::PostConcertFeedCommandService
    for CommandServer
{
    async fn execute_command(
        &self,
        r: Request<proto::QualifiedExecuteCommandRequest>,
    ) -> Result<Response<proto::Receipt>, Status> {
        let (caller, proof) = verified(&r)?;
        let r = r.into_inner();
        let query = r
            .query
            .ok_or_else(|| Status::invalid_argument("Qualified command required"))?;
        let a = CommandAdmission::from_verified(caller, proof, query, r.authority)
            .map_err(super::read_wire::status)?;
        Ok(response(
            self.runtime
                .run(a)
                .await
                .map_err(super::read_wire::status)?,
        ))
    }
    async fn get_receipt(
        &self,
        r: Request<proto::QualifiedGetReceiptRequest>,
    ) -> Result<Response<proto::GetReceiptResponse>, Status> {
        let (caller, proof) = verified(&r)?;
        let r = r.into_inner();
        let query = r
            .query
            .ok_or_else(|| Status::invalid_argument("Qualified receipt required"))?;
        let a = CommandAdmission::from_verified(
            caller,
            proof,
            proto::ExecuteCommandRequest {
                context: query.context,
                command: query.command,
            },
            r.authority,
        )
        .map_err(super::read_wire::status)?;
        Ok(response(proto::GetReceiptResponse {
            receipt: self
                .runtime
                .receipt(a)
                .await
                .map_err(super::read_wire::status)?,
        }))
    }
    async fn preview_command(
        &self,
        r: Request<proto::QualifiedPreviewCommandRequest>,
    ) -> Result<Response<proto::PreviewCommandResponse>, Status> {
        let (caller, proof) = verified(&r)?;
        let r = r.into_inner();
        let query = r
            .query
            .ok_or_else(|| Status::invalid_argument("Qualified preview required"))?;
        let a = CommandAdmission::from_verified(caller, proof, query, r.authority)
            .map_err(super::read_wire::status)?;
        let (post, comment) = self
            .runtime
            .preview(a)
            .await
            .map_err(super::read_wire::status)?;
        Ok(response(proto::PreviewCommandResponse {
            post: post.map(super::read_wire::post),
            comment: comment.map(super::read_wire::comment),
        }))
    }
}
