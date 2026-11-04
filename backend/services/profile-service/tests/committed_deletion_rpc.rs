//! Actual canonical RPC and owning-store effects; no mocked positive receipt.
use owner::adapters::r#in::grpc::{
    method_policy::PROFILE_METHOD_POLICY as POLICY,
    server::{proto, ProfileGrpcServer as GrpcServer},
};
use owner::config::{build_state, AppConfig};
use profile_service as owner;
type Service = proto::profile_service_server::ProfileServiceServer<GrpcServer>;
type Client = proto::profile_service_client::ProfileServiceClient<
    tonic::service::interceptor::InterceptedService<
        tonic::transport::Channel,
        platform::grpc_auth::ServiceIdentityClientInterceptor,
    >,
>;
const OWNER: &str = "profile";
const ROLE: platform::grpc_auth::Workload = platform::grpc_auth::Workload::Profile;
#[path = "../../../scripts/mcr/legacy_deletion_owners/owned.rs"]
mod owned;
#[path = "../../../scripts/mcr/legacy_deletion_owners/seed.rs"]
mod seed;
include!("../../../scripts/mcr/legacy_deletion_owners/rpc_fixture.rs");
