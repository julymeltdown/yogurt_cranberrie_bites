//! Minimal author projection served to Post behind Gateway-issued child proof.
use platform::actor_delegation::VerifiedActor;
use platform::grpc_auth::{authorize_call, RpcAction, Workload};
use platform::grpc_method_policy::{ForwardedMethod, MethodPolicy};
use profile_application::author_existence::{
    AuthorProfileLookupPort, AuthorProfileQueryDto, CheckAuthorProfileUseCase,
};
use profile_application::errors::ProfileError;
use std::sync::Arc;
use tonic::{Request, Response, Status};
use uuid::Uuid;

pub mod proto {
    tonic::include_proto!("authority.v1");
}
pub static PROFILE_AUTHOR_POLICY: MethodPolicy = MethodPolicy {
    service: "authority.v1.ProfileAuthorAuthorityService",
    workload: Some(Workload::Profile),
    methods: &[("CheckAuthorProfile", &[Workload::Post])],
    actor_fields: &[],
};
pub static PROFILE_AUTHOR_FORWARDING: &[ForwardedMethod] = &[ForwardedMethod {
    method: "CheckAuthorProfile",
    caller: Workload::Post,
    actor_fields: &[&[1]],
}];

#[derive(Clone)]
pub struct ProfileAuthorAuthorityGrpcServer(CheckAuthorProfileUseCase);
impl ProfileAuthorAuthorityGrpcServer {
    pub fn new(repository: Arc<dyn AuthorProfileLookupPort>) -> Self {
        Self(CheckAuthorProfileUseCase::new(repository))
    }
}
#[tonic::async_trait]
impl proto::profile_author_authority_service_server::ProfileAuthorAuthorityService
    for ProfileAuthorAuthorityGrpcServer
{
    async fn check_author_profile(
        &self,
        request: Request<proto::CheckAuthorProfileRequest>,
    ) -> Result<Response<proto::CheckAuthorProfileResponse>, Status> {
        if authorize_call(&request, RpcAction::Read)? != Workload::Post {
            return Err(Status::permission_denied("Post presenter required"));
        }
        let actor = request
            .extensions()
            .get::<VerifiedActor>()
            .ok_or_else(|| Status::unauthenticated("Verified child authority required"))?
            .0;
        let user_id = request
            .get_ref()
            .user_id
            .parse::<Uuid>()
            .map_err(|_| Status::invalid_argument("Author ID must be a UUID"))?;
        if actor != user_id {
            return Err(Status::permission_denied("Author binding denied"));
        }
        let query = AuthorProfileQueryDto::new(user_id)
            .map_err(|_| Status::invalid_argument("Author ID must be non-nil"))?;
        let profile = self
            .0
            .check_author(query)
            .await
            .map_err(|error| match error {
                ProfileError::ProfileNotFound => Status::not_found("Profile not found"),
                ProfileError::Busy => Status::unavailable("Profile unavailable"),
                _ => Status::internal("Profile lookup failed"),
            })?;
        Ok(Response::new(proto::CheckAuthorProfileResponse {
            user_id: profile.user_id().to_string(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn projection_policy_is_post_only_and_requires_exact_author_child() {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../proto/authority/v1/authority.proto"
        ));
        let service = source
            .split("service ProfileAuthorAuthorityService")
            .nth(1)
            .unwrap()
            .split('}')
            .next()
            .unwrap();
        assert_eq!(
            platform::grpc_method_policy::proto_rpc_names(service),
            vec!["CheckAuthorProfile"]
        );
        assert_eq!(
            PROFILE_AUTHOR_POLICY.method_names().collect::<Vec<_>>(),
            vec!["CheckAuthorProfile"]
        );
        for caller in Workload::ALL {
            assert_eq!(
                PROFILE_AUTHOR_POLICY.allows(
                    "/authority.v1.ProfileAuthorAuthorityService/CheckAuthorProfile",
                    caller
                ),
                caller == Workload::Post
            );
        }
        assert_eq!(PROFILE_AUTHOR_FORWARDING[0].actor_fields, &[&[1][..]][..]);
    }
    use async_trait::async_trait;
    use profile_application::author_existence::ExistingAuthorProfileDto;
    use profile_application::errors::ProfileResult;
    struct WrongOwner;
    #[async_trait]
    impl AuthorProfileLookupPort for WrongOwner {
        async fn find_existing_author(
            &self,
            _: AuthorProfileQueryDto,
        ) -> ProfileResult<Option<ExistingAuthorProfileDto>> {
            Ok(Some(ExistingAuthorProfileDto::new(Uuid::from_u128(2))?))
        }
    }
    #[tokio::test]
    async fn mismatched_owner_identity_cannot_satisfy_existence() {
        let use_case = CheckAuthorProfileUseCase::new(Arc::new(WrongOwner));
        assert!(matches!(
            use_case
                .check_author(AuthorProfileQueryDto::new(Uuid::from_u128(1)).unwrap())
                .await,
            Err(ProfileError::Persistence)
        ));
        assert!(AuthorProfileQueryDto::new(Uuid::nil()).is_err());
    }
}
