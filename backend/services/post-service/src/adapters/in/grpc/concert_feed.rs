//! Isolated transport candidate. Workload authentication does NOT grant actor,
//! relationship, media or linked-object authority. The current platform cannot
//! verify Concert-forwarded user authority or fence remote writes, so all public
//! handlers deliberately return UNAVAILABLE before touching owner persistence.
//! Enabling these handlers requires a separately qualified authority protocol;
//! neither caller-supplied context/digests nor a configuration flag can enable it.
use platform::grpc_auth::{VerifiedCaller, Workload};
use platform::grpc_method_policy::MethodPolicy;
use tonic::{Request, Response, Status};

pub mod proto {
    tonic::include_proto!("post.concert.v1");
}
const CONCERT: &[Workload] = &[Workload::Concert];
pub static POST_CONCERT_FEED_POLICY: MethodPolicy = MethodPolicy {
    service: "post.concert.v1.PostConcertFeedService",
    workload: Some(Workload::Post),
    methods: &[
        ("ExecuteCommand", CONCERT),
        ("GetReceipt", CONCERT),
        ("GetPost", CONCERT),
        ("ListPosts", CONCERT),
        ("ListComments", CONCERT),
        ("GetComment", CONCERT),
    ],
    // No unverified actor field is admitted here. Every handler stops before
    // any read/effect. Adding actor forwarding must use its qualified guard.
    actor_fields: &[],
};
#[derive(Clone, Default)]
pub struct PostConcertFeedGrpcServer;
fn admission<T>(request: &Request<T>) -> Status {
    match request.extensions().get::<VerifiedCaller>() {
        None => Status::unauthenticated("verified service identity required"),
        Some(VerifiedCaller(Workload::Concert)) => {
            Status::unavailable("Concert Post owner authority is not qualified")
        }
        Some(_) => Status::permission_denied("Concert Post owner caller denied"),
    }
}
#[tonic::async_trait]
impl proto::post_concert_feed_service_server::PostConcertFeedService for PostConcertFeedGrpcServer {
    async fn execute_command(
        &self,
        r: Request<proto::ExecuteCommandRequest>,
    ) -> Result<Response<proto::Receipt>, Status> {
        Err(admission(&r))
    }
    async fn get_receipt(
        &self,
        r: Request<proto::GetReceiptRequest>,
    ) -> Result<Response<proto::GetReceiptResponse>, Status> {
        Err(admission(&r))
    }
    async fn get_post(
        &self,
        r: Request<proto::GetPostRequest>,
    ) -> Result<Response<proto::GetPostResponse>, Status> {
        Err(admission(&r))
    }
    async fn list_posts(
        &self,
        r: Request<proto::ListPostsRequest>,
    ) -> Result<Response<proto::ListPostsResponse>, Status> {
        Err(admission(&r))
    }
    async fn get_comment(
        &self,
        r: Request<proto::GetCommentRequest>,
    ) -> Result<Response<proto::GetCommentResponse>, Status> {
        Err(admission(&r))
    }
    async fn list_comments(
        &self,
        r: Request<proto::ListCommentsRequest>,
    ) -> Result<Response<proto::ListCommentsResponse>, Status> {
        Err(admission(&r))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use platform::grpc_method_policy::proto_rpc_names;
    use std::collections::BTreeSet;
    #[test]
    fn isolated_method_table_matches_exact_proto_and_only_concert() {
        let declared: BTreeSet<_> = proto_rpc_names(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../proto/post/v1/concert_feed.proto"
        )))
        .into_iter()
        .collect();
        let listed: BTreeSet<_> = POST_CONCERT_FEED_POLICY
            .method_names()
            .map(str::to_owned)
            .collect();
        assert_eq!(declared, listed);
        for method in POST_CONCERT_FEED_POLICY.method_names() {
            for caller in Workload::ALL {
                assert_eq!(
                    POST_CONCERT_FEED_POLICY.allows(
                        &format!("/post.concert.v1.PostConcertFeedService/{method}"),
                        caller
                    ),
                    caller == Workload::Concert
                );
            }
        }
    }
    #[test]
    fn caller_context_cannot_activate_production_authority() {
        let mut r = Request::new(proto::ExecuteCommandRequest::default());
        assert_eq!(admission(&r).code(), tonic::Code::Unauthenticated);
        r.extensions_mut().insert(VerifiedCaller(Workload::Gateway));
        assert_eq!(admission(&r).code(), tonic::Code::PermissionDenied);
        r.extensions_mut().insert(VerifiedCaller(Workload::Concert));
        assert_eq!(admission(&r).code(), tonic::Code::Unavailable);
    }
}
