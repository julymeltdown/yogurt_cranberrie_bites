use crate::dto::friend_dto::FollowActionDto;
use crate::dto::mutation_dto::{
    MutationContextDto, MutationError, MutationKind, MutationOperationDto,
};
use crate::ports::{
    FollowRepositoryPort, FollowRequestRepositoryPort, ProfilePrivacyPort,
    RelationshipPolicyRepositoryPort,
};
use chrono::Utc;
use friend_domain::error::FriendError;
use friend_domain::follow_edge::FollowEdge;
use friend_domain::follow_request::AccountVisibility;
use uuid::Uuid;

#[derive(Clone)]
pub struct FollowUseCase<R, P> {
    repository: R,
    privacy: P,
}

impl<R, P> FollowUseCase<R, P>
where
    R: FollowRepositoryPort + FollowRequestRepositoryPort + RelationshipPolicyRepositoryPort,
    P: ProfilePrivacyPort,
{
    pub fn new(repository: R, privacy: P) -> Self {
        Self {
            repository,
            privacy,
        }
    }

    pub async fn execute(
        &self,
        follower_id: Uuid,
        followee_id: Uuid,
    ) -> Result<FollowActionDto, FriendError> {
        self.execute_with_operation(follower_id, followee_id, None)
            .await
    }

    pub async fn execute_with_operation(
        &self,
        follower_id: Uuid,
        followee_id: Uuid,
        context: Option<MutationContextDto>,
    ) -> Result<FollowActionDto, FriendError> {
        let edge = FollowEdge::new(follower_id, followee_id, Utc::now())
            .map_err(|error| FriendError::InvalidInput(error.to_string()))?;
        let operation = context
            .map(|context| {
                MutationOperationDto::new(context, follower_id, followee_id, MutationKind::Follow)
            })
            .transpose()?;
        let policy = self.repository.policy(follower_id, followee_id).await?;
        if policy.is_blocked() {
            return Err(FriendError::PermissionDenied("Blocked relationship".into()));
        }
        if let Some(operation) = operation.as_ref() {
            if let Some(outcome) = self
                .repository
                .lookup_mutation(operation)
                .await
                .map_err(map_operation_error)?
            {
                return Ok(outcome);
            }
            if operation.context.lookup_only {
                return Err(anyhow::Error::new(MutationError::NotCommitted).into());
            }
        }
        if policy.following {
            if let Some(operation) = operation.as_ref() {
                return self
                    .repository
                    .follow_with_receipt(&edge, false, operation)
                    .await
                    .map_err(map_operation_error);
            }
            let edge = self.repository.follow(&edge).await?;
            return Ok(FollowActionDto {
                edge: Some(edge.into()),
                following: true,
                status: "ACTIVE".to_owned(),
                request_id: None,
            });
        }

        match self.privacy.account_visibility(followee_id).await? {
            AccountVisibility::Public => {
                if let Some(operation) = operation.as_ref() {
                    return self
                        .repository
                        .follow_with_receipt(&edge, false, operation)
                        .await
                        .map_err(map_operation_error);
                }
                let edge = self.repository.follow(&edge).await?;
                Ok(FollowActionDto {
                    edge: Some(edge.into()),
                    following: true,
                    status: "ACTIVE".to_owned(),
                    request_id: None,
                })
            }
            AccountVisibility::Private => {
                if let Some(operation) = operation.as_ref() {
                    return self
                        .repository
                        .follow_with_receipt(&edge, true, operation)
                        .await
                        .map_err(map_operation_error);
                }
                let request = self
                    .repository
                    .create_request(follower_id, followee_id, Utc::now())
                    .await?;
                Ok(FollowActionDto {
                    edge: None,
                    following: false,
                    status: request.status.as_str().to_owned(),
                    request_id: Some(request.id.to_string()),
                })
            }
        }
    }
}

pub(crate) fn map_operation_error(error: anyhow::Error) -> FriendError {
    if error
        .chain()
        .any(|source| source.is::<MutationError>() || source.is::<FriendError>())
    {
        FriendError::Unexpected(error)
    } else {
        FriendError::Unexpected(MutationError::Unavailable.into())
    }
}
