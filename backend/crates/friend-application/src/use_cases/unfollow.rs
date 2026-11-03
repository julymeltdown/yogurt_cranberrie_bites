use crate::dto::friend_dto::FollowActionDto;
use crate::dto::mutation_dto::{
    MutationContextDto, MutationError, MutationKind, MutationOperationDto,
};
use crate::ports::FollowRepositoryPort;
use friend_domain::error::FriendError;
use uuid::Uuid;

#[derive(Clone)]
pub struct UnfollowUseCase<R> {
    repository: R,
}

impl<R> UnfollowUseCase<R>
where
    R: FollowRepositoryPort,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
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
        if follower_id == followee_id {
            return Err(FriendError::InvalidInput("Cannot unfollow yourself".into()));
        }
        let operation = context
            .map(|context| {
                MutationOperationDto::new(context, follower_id, followee_id, MutationKind::Unfollow)
            })
            .transpose()?;
        if let Some(operation) = operation.as_ref() {
            if let Some(outcome) = self
                .repository
                .lookup_mutation(operation)
                .await
                .map_err(super::follow::map_operation_error)?
            {
                return Ok(outcome);
            }
            if operation.context.lookup_only {
                return Err(anyhow::Error::new(MutationError::NotCommitted).into());
            }
            return self
                .repository
                .unfollow_with_receipt(follower_id, followee_id, operation)
                .await
                .map_err(super::follow::map_operation_error);
        }
        self.repository.unfollow(follower_id, followee_id).await?;
        Ok(FollowActionDto {
            edge: None,
            following: false,
            status: "INACTIVE".to_owned(),
            request_id: None,
        })
    }
}
