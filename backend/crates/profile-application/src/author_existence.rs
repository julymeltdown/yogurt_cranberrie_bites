//! Minimal owner projection for author existence, without profile contents.
use crate::errors::{ProfileError, ProfileResult};
use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Copy)]
pub struct AuthorProfileQueryDto(Uuid);
impl AuthorProfileQueryDto {
    pub fn new(user_id: Uuid) -> ProfileResult<Self> {
        if user_id.is_nil() {
            return Err(ProfileError::InvalidInput("Author ID must be non-nil"));
        }
        Ok(Self(user_id))
    }
    pub fn user_id(&self) -> Uuid {
        self.0
    }
}

#[derive(Clone, Copy)]
pub struct ExistingAuthorProfileDto(Uuid);
impl ExistingAuthorProfileDto {
    pub fn new(user_id: Uuid) -> ProfileResult<Self> {
        AuthorProfileQueryDto::new(user_id).map(|query| Self(query.user_id()))
    }
    pub fn user_id(&self) -> Uuid {
        self.0
    }
}

#[async_trait]
pub trait AuthorProfileLookupPort: Send + Sync {
    async fn find_existing_author(
        &self,
        query: AuthorProfileQueryDto,
    ) -> ProfileResult<Option<ExistingAuthorProfileDto>>;
}

#[derive(Clone)]
pub struct CheckAuthorProfileUseCase(Arc<dyn AuthorProfileLookupPort>);
impl CheckAuthorProfileUseCase {
    pub fn new(repository: Arc<dyn AuthorProfileLookupPort>) -> Self {
        Self(repository)
    }
    pub async fn check_author(
        &self,
        query: AuthorProfileQueryDto,
    ) -> ProfileResult<ExistingAuthorProfileDto> {
        let result = self
            .0
            .find_existing_author(query)
            .await?
            .ok_or(ProfileError::ProfileNotFound)?;
        if result.user_id() != query.user_id() {
            return Err(ProfileError::Persistence);
        }
        Ok(result)
    }
}
