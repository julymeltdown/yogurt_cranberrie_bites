use crate::application::dto::mutation_dto::MutationContextDto;
use crate::application::dto::profile_dto::{
    ApplyUserDeletionDto, DeleteProfileDto, UpdateProfileDto, UpdateProfilePrivacyDto,
};
use crate::application::errors::ProfileError;
use crate::application::presentation::{
    present_profile_error, ProfileFailureKind, ProfileMutationReason, ProfilePresentationMode,
};
use crate::config::AppState;
use platform::grpc_auth::{authorize_call, RpcAction};
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub mod proto {
    tonic::include_proto!("profile.v1");
}

#[derive(Clone)]
pub struct ProfileGrpcServer {
    state: Arc<AppState>,
}

impl ProfileGrpcServer {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }
}

#[tonic::async_trait]
impl proto::profile_service_server::ProfileService for ProfileGrpcServer {
    async fn get_mutation_receipt_status(
        &self,
        request: Request<proto::GetMutationReceiptStatusRequest>,
    ) -> Result<Response<proto::GetMutationReceiptStatusResponse>, Status> {
        authorize_call(&request, RpcAction::MutationReceiptLookup)?;
        let operation_id = uuid::Uuid::parse_str(&request.into_inner().operation_id)
            .map_err(|_| Status::invalid_argument("Operation ID must be a UUID"))?;
        let receipt = self
            .state
            .update_privacy
            .mutation_receipt_status(operation_id)
            .await
            .map_err(to_mutation_status)?
            .ok_or_else(|| Status::not_found("OWNER_RECEIPT_NOT_FOUND"))?;
        use proto::get_mutation_receipt_status_response::State;
        let state = match receipt.state {
            crate::application::dto::mutation_dto::MutationReceiptStateDto::Replayable => {
                State::Replayable
            }
            crate::application::dto::mutation_dto::MutationReceiptStateDto::Expired => {
                State::Expired
            }
            crate::application::dto::mutation_dto::MutationReceiptStateDto::Incomplete => {
                State::Incomplete
            }
        };
        Ok(Response::new(proto::GetMutationReceiptStatusResponse {
            operation_id: receipt.operation_id.to_string(),
            actor_id: receipt.actor_id.to_string(),
            scope: receipt.scope,
            committed_at: receipt.committed_at.to_rfc3339(),
            replay_until: receipt.replay_until.to_rfc3339(),
            state: state as i32,
        }))
    }

    async fn replay_mutation(
        &self,
        request: Request<proto::ReplayMutationRequest>,
    ) -> Result<Response<proto::ReplayMutationResponse>, Status> {
        authorize_call(&request, RpcAction::UserMutation)?;
        if platform::mutation_context::from_request(&request)?.is_none() {
            return Err(Status::invalid_argument(
                "Operation context required for lookup",
            ));
        }
        let (metadata, mut extensions, request) = request.into_parts();
        extensions.insert(LookupOnly);
        use proto::replay_mutation_request::Command;
        use proto::replay_mutation_response::Outcome;
        let outcome = match request
            .command
            .ok_or_else(|| Status::invalid_argument("Mutation command required"))?
        {
            Command::UpdateProfilePrivacy(command) => Outcome::UpdateProfilePrivacy(
                self.update_profile_privacy(Request::from_parts(metadata, extensions, command))
                    .await?
                    .into_inner(),
            ),
            Command::UpdateProfile(command) => Outcome::UpdateProfile(
                self.update_profile(Request::from_parts(metadata, extensions, command))
                    .await?
                    .into_inner(),
            ),
        };
        Ok(Response::new(proto::ReplayMutationResponse {
            outcome: Some(outcome),
        }))
    }

    async fn get_profile(
        &self,
        request: Request<proto::GetProfileRequest>,
    ) -> Result<Response<proto::GetProfileResponse>, Status> {
        let user_id = request
            .into_inner()
            .user_id
            .parse()
            .map_err(invalid_user_id)?;
        let profile = self
            .state
            .get_profile
            .execute(user_id)
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::GetProfileResponse {
            profile: Some(to_proto_profile(profile)),
        }))
    }

    async fn get_profile_by_username(
        &self,
        request: Request<proto::GetProfileByUsernameRequest>,
    ) -> Result<Response<proto::GetProfileResponse>, Status> {
        let profile = self
            .state
            .get_profile_by_username
            .execute(&request.into_inner().username)
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::GetProfileResponse {
            profile: Some(to_proto_profile(profile)),
        }))
    }

    async fn get_profiles(
        &self,
        request: Request<proto::GetProfilesRequest>,
    ) -> Result<Response<proto::GetProfilesResponse>, Status> {
        let user_ids = request
            .into_inner()
            .user_ids
            .into_iter()
            .map(|value| value.parse().map_err(invalid_user_id))
            .collect::<Result<Vec<_>, _>>()?;
        let profiles = self
            .state
            .get_profiles_batch
            .execute(&user_ids)
            .await
            .map_err(to_invalid)?;
        Ok(Response::new(proto::GetProfilesResponse {
            profiles: profiles.into_iter().map(to_proto_profile).collect(),
        }))
    }

    async fn update_profile(
        &self,
        request: Request<proto::UpdateProfileRequest>,
    ) -> Result<Response<proto::UpdateProfileResponse>, Status> {
        authorize_call(&request, RpcAction::UserMutation)?;
        let context = mutation_context(&request)?;
        // The Gateway creates operation context for the route even when the
        // caller did not opt into idempotency. Avoid retaining an outcome body
        // for that ordinary path, but preserve lookup-only semantics if the
        // dedicated replay RPC supplied the context.
        let context =
            context.filter(|context| context.idempotency_key.is_some() || context.lookup_only);
        let request = request.into_inner();
        if context.as_ref().is_some_and(|context| {
            uuid::Uuid::parse_str(&request.user_id).ok() != Some(context.actor_id)
        }) {
            return Err(Status::permission_denied(
                "Operation actor does not match profile owner",
            ));
        }
        let command = UpdateProfileDto {
            user_id: request.user_id,
            username: request.username,
            nickname: request.nickname,
            avatar_url: request.avatar_url,
            bio: request.bio,
        };
        let profile = if let Some(context) = context {
            self.state
                .update_profile
                .execute_with_operation(command, Some(context))
                .await
                .map_err(to_mutation_status)?
        } else {
            self.state
                .update_profile
                .execute(command)
                .await
                .map_err(to_invalid)?
        };
        Ok(Response::new(proto::UpdateProfileResponse {
            profile: Some(to_proto_profile(profile)),
        }))
    }

    async fn update_profile_privacy(
        &self,
        request: Request<proto::UpdateProfilePrivacyRequest>,
    ) -> Result<Response<proto::UpdateProfilePrivacyResponse>, Status> {
        authorize_call(&request, RpcAction::UserMutation)?;
        let operation = mutation_context(&request)?;
        let request = request.into_inner();
        if operation.as_ref().is_some_and(|context| {
            uuid::Uuid::parse_str(&request.user_id).ok() != Some(context.actor_id)
                || uuid::Uuid::parse_str(&request.actor_id).ok() != Some(context.actor_id)
        }) {
            return Err(Status::permission_denied(
                "Operation actor does not match profile owner",
            ));
        }
        let profile = self
            .state
            .update_privacy
            .execute_with_operation(
                UpdateProfilePrivacyDto {
                    user_id: request.user_id,
                    actor_id: request.actor_id,
                    account_visibility: request.account_visibility,
                },
                operation,
            )
            .await
            .map_err(to_mutation_status)?;
        Ok(Response::new(proto::UpdateProfilePrivacyResponse {
            profile: Some(to_proto_profile(profile)),
        }))
    }

    async fn get_profile_privacy(
        &self,
        request: Request<proto::GetProfilePrivacyRequest>,
    ) -> Result<Response<proto::GetProfilePrivacyResponse>, Status> {
        let user_id = request
            .into_inner()
            .user_id
            .parse()
            .map_err(invalid_user_id)?;
        let profile = self
            .state
            .get_profile
            .execute(user_id)
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::GetProfilePrivacyResponse {
            user_id: profile.user_id,
            account_visibility: profile.account_visibility,
            lifecycle_status: profile.lifecycle_status,
        }))
    }

    async fn delete_profile(
        &self,
        request: Request<proto::DeleteProfileRequest>,
    ) -> Result<Response<proto::DeleteProfileResponse>, Status> {
        // Hiding a profile is a user-facing mutation: only the edge may request it.
        authorize_call(&request, RpcAction::UserMutation)?;
        let request = request.into_inner();
        let deleted = self
            .state
            .delete_profile
            .execute(DeleteProfileDto {
                user_id: request.user_id,
                actor_id: request.actor_id,
                request_id: empty_string_to_option(request.request_id),
                ip_address: empty_string_to_option(request.ip_address),
                user_agent: empty_string_to_option(request.user_agent),
            })
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::DeleteProfileResponse {
            user_id: deleted.user_id,
            deleted: deleted.deleted,
            account_deleted: deleted.account_deleted,
        }))
    }

    async fn search_profiles(
        &self,
        request: Request<proto::SearchProfilesRequest>,
    ) -> Result<Response<proto::SearchProfilesResponse>, Status> {
        let request = request.into_inner();
        let page = self
            .state
            .search_profiles
            .execute(
                &request.query,
                empty_to_none(&request.cursor),
                request.limit,
            )
            .await
            .map_err(to_invalid)?;
        Ok(Response::new(proto::SearchProfilesResponse {
            profiles: page.profiles.into_iter().map(to_proto_profile).collect(),
            next_cursor: page.next_cursor.unwrap_or_default(),
        }))
    }

    async fn upload_avatar(
        &self,
        request: Request<proto::UploadAvatarRequest>,
    ) -> Result<Response<proto::UploadAvatarResponse>, Status> {
        let request = request.into_inner();
        let response = self
            .state
            .upload_avatar
            .execute(
                &request.user_id,
                request.content,
                &request.content_type,
                &request.filename,
            )
            .await
            .map_err(to_invalid)?;
        Ok(Response::new(proto::UploadAvatarResponse {
            avatar_path: format!("/api/profile/avatar/{}", response.user_id),
        }))
    }

    async fn apply_user_deletion(
        &self,
        request: Request<proto::ApplyUserDeletionRequest>,
    ) -> Result<Response<proto::ApplyUserDeletionResponse>, Status> {
        // Irreversible account cleanup belongs to the deletion saga alone.
        authorize_call(&request, RpcAction::ApplyUserDeletion)?;
        let request = request.into_inner();
        let response = self
            .state
            .apply_user_deletion
            .execute(ApplyUserDeletionDto {
                saga_id: request.saga_id,
                user_id: request.user_id,
                actor_id: request.actor_id,
                retention_until: request.retention_until,
            })
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::ApplyUserDeletionResponse {
            applied: response.applied,
        }))
    }

    async fn get_avatar(
        &self,
        request: Request<proto::GetAvatarRequest>,
    ) -> Result<Response<proto::GetAvatarResponse>, Status> {
        let avatar = self
            .state
            .get_avatar
            .execute(&request.into_inner().user_id)
            .await
            .map_err(to_invalid)?;
        let avatar = avatar.ok_or_else(|| Status::not_found("Avatar not found"))?;
        Ok(Response::new(proto::GetAvatarResponse {
            content: avatar.content,
            content_type: avatar.content_type,
        }))
    }
}

fn to_proto_profile(profile: crate::application::dto::profile_dto::ProfileDto) -> proto::Profile {
    proto::Profile {
        user_id: profile.user_id,
        nickname: profile.nickname,
        avatar_url: profile.avatar_url,
        bio: profile.bio,
        follower_count: profile.follower_count,
        post_count: profile.post_count,
        username: profile.username,
        following_count: profile.following_count,
        account_visibility: profile.account_visibility,
        lifecycle_status: profile.lifecycle_status,
    }
}

fn to_invalid(error: ProfileError) -> Status {
    to_wire_status(error, ProfilePresentationMode::Input)
}

fn to_status(error: ProfileError) -> Status {
    to_wire_status(error, ProfilePresentationMode::Lookup)
}

fn to_mutation_status(error: ProfileError) -> Status {
    to_wire_status(error, ProfilePresentationMode::Mutation)
}

fn to_wire_status(error: ProfileError, mode: ProfilePresentationMode) -> Status {
    let failure = present_profile_error(error, mode);
    let mut status = match failure.kind {
        ProfileFailureKind::InvalidInput => Status::invalid_argument(failure.message),
        ProfileFailureKind::Missing => Status::not_found(failure.message),
        ProfileFailureKind::Denied => Status::permission_denied(failure.message),
        ProfileFailureKind::Busy | ProfileFailureKind::Uncertain => {
            Status::unavailable(failure.message)
        }
        ProfileFailureKind::Internal => Status::internal(failure.message),
        ProfileFailureKind::Conflict => Status::already_exists(failure.message),
    };
    if let Some(reason) = failure.mutation_reason {
        let code = match reason {
            ProfileMutationReason::Conflict => "conflict",
            ProfileMutationReason::Expired => "expired",
            ProfileMutationReason::Unknown => "unknown",
        };
        status.metadata_mut().insert(
            platform::mutation_context::ERROR_CODE,
            code.parse().expect("static operation error"),
        );
    }
    status
}

#[derive(Clone)]
struct LookupOnly;

#[allow(clippy::result_large_err)]
fn mutation_context<T>(request: &Request<T>) -> Result<Option<MutationContextDto>, Status> {
    Ok(
        platform::mutation_context::from_request(request)?.map(|context| MutationContextDto {
            operation_id: context.operation_id,
            actor_id: context.actor_id,
            idempotency_key: context.idempotency_key,
            lookup_only: request.extensions().get::<LookupOnly>().is_some(),
        }),
    )
}

fn invalid_user_id(_: uuid::Error) -> Status {
    Status::invalid_argument("User ID is required")
}

fn empty_to_none(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn empty_string_to_option(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::{to_invalid, to_status};
    use crate::application::errors::ProfileError;

    #[test]
    fn wrapped_repository_failures_are_server_errors_without_private_details() {
        for mapper in [to_invalid, to_status] {
            let status = mapper(ProfileError::Persistence);
            assert_eq!(status.code(), tonic::Code::Internal);
            assert_eq!(status.message(), "Profile persistence operation failed");
        }
        let validation = to_invalid(ProfileError::InvalidInput("Username is required"));
        assert_eq!(validation.code(), tonic::Code::InvalidArgument);
        assert_eq!(validation.message(), "Username is required");
    }

    #[test]
    fn avatar_decode_capacity_is_reported_as_service_unavailable() {
        let status = to_invalid(ProfileError::Busy);

        assert_eq!(status.code(), tonic::Code::Unavailable);
        assert_eq!(status.message(), "PROFILE_BUSY");
    }
    #[test]
    fn wire_error_matrix_preserves_codes_messages_and_mutation_metadata() {
        use super::to_mutation_status;
        use crate::application::dto::mutation_dto::MutationError;
        for (error, expected_code, expected_message) in [
            (
                ProfileError::ProfileNotFound,
                tonic::Code::NotFound,
                "Profile not found",
            ),
            (
                ProfileError::Forbidden,
                tonic::Code::PermissionDenied,
                "Forbidden",
            ),
            (
                ProfileError::AccountDeleted,
                tonic::Code::PermissionDenied,
                "Account is deleted",
            ),
            (ProfileError::Busy, tonic::Code::Unavailable, "PROFILE_BUSY"),
            (
                ProfileError::Persistence,
                tonic::Code::Internal,
                "Profile persistence operation failed",
            ),
        ] {
            let status = to_status(error);
            assert_eq!(status.code(), expected_code);
            assert_eq!(status.message(), expected_message);
            assert!(status
                .metadata()
                .get(platform::mutation_context::ERROR_CODE)
                .is_none());
        }
        for (error, code) in [
            (MutationError::Conflict, "conflict"),
            (MutationError::Expired, "expired"),
            (MutationError::NotCommitted, "unknown"),
        ] {
            let status = to_mutation_status(error.into());
            assert_eq!(status.code(), tonic::Code::AlreadyExists);
            assert_eq!(status.message(), error.to_string());
            assert_eq!(
                status
                    .metadata()
                    .get(platform::mutation_context::ERROR_CODE)
                    .unwrap()
                    .to_str()
                    .unwrap(),
                code
            );
        }
        for error in [
            ProfileError::Persistence,
            ProfileError::ReceiptUnavailable,
            MutationError::Unavailable.into(),
        ] {
            let status = to_mutation_status(error);
            assert_eq!(status.code(), tonic::Code::Unavailable);
            assert_eq!(status.message(), "Owner operation outcome unavailable");
        }
    }
}
