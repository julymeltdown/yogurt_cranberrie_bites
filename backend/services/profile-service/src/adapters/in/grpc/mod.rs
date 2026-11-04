pub fn service_name() -> &'static str {
    "profile-grpc"
}
pub mod method_policy;
mod concert_identity;
pub mod server;

pub mod author_authority;
pub mod world_identity;
pub mod concert_public_profile;
