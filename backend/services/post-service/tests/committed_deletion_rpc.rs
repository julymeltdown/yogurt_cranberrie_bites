//! Actual canonical RPC and owning-store effects; no mocked positive receipt.
use owner::adapters::r#in::grpc::{
    method_policy::POST_METHOD_POLICY as POLICY,
    server::{proto, PostGrpcServer as GrpcServer},
};
use owner::config::{build_state, AppConfig};
use post_service as owner;
type Service = proto::post_service_server::PostServiceServer<GrpcServer>;
type Client = proto::post_service_client::PostServiceClient<
    tonic::service::interceptor::InterceptedService<
        tonic::transport::Channel,
        platform::grpc_auth::ServiceIdentityClientInterceptor,
    >,
>;
const OWNER: &str = "post";
const ROLE: platform::grpc_auth::Workload = platform::grpc_auth::Workload::Post;
#[path = "../../../scripts/mcr/legacy_deletion_owners/owned.rs"]
mod owned;
#[path = "../../../scripts/mcr/legacy_deletion_owners/seed.rs"]
mod seed;
include!("../../../scripts/mcr/legacy_deletion_owners/rpc_fixture.rs");
