//! Actual canonical RPC and owning-store effects; no mocked positive receipt.
use friend_service as owner;
use owner::adapters::r#in::grpc::{
    method_policy::FRIEND_METHOD_POLICY as POLICY,
    server::{proto, FriendGrpcServer as GrpcServer},
};
use owner::config::{build_state, AppConfig};
type Service = proto::friend_service_server::FriendServiceServer<GrpcServer>;
type Client = proto::friend_service_client::FriendServiceClient<
    tonic::service::interceptor::InterceptedService<
        tonic::transport::Channel,
        platform::grpc_auth::ServiceIdentityClientInterceptor,
    >,
>;
const OWNER: &str = "friend";
const ROLE: platform::grpc_auth::Workload = platform::grpc_auth::Workload::Friend;
#[path = "../../../scripts/mcr/legacy_deletion_owners/owned.rs"]
mod owned;
#[path = "../../../scripts/mcr/legacy_deletion_owners/seed.rs"]
mod seed;
include!("../../../scripts/mcr/legacy_deletion_owners/rpc_fixture.rs");
