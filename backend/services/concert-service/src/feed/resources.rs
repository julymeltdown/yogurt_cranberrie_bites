//! Concrete production author adapter. GuardedSubject cannot be constructed
//! from a feed Context, and no fallback author/link/media authority is installed.
use crate::adapters::out::{
    public_profile::PublicProfileReader, relationship_authority::guarded::OwnerGuardScope,
};
use async_trait::async_trait;
use concert_application::feed::{Context, FeedResources, PublicAuthor};
use concert_domain::feed::{FeedError, ResourceLink};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct GuardedResources {
    scope: Arc<OwnerGuardScope>,
    profiles: Arc<PublicProfileReader>,
}
impl GuardedResources {
    pub fn new(
        scope: Arc<OwnerGuardScope>,
        profiles: Arc<PublicProfileReader>,
    ) -> Result<Self, FeedError> {
        scope
            .binding()
            .check()
            .map_err(|_| FeedError::PermissionDenied)?;
        Ok(Self { scope, profiles })
    }
    fn check(&self, c: Context) -> Result<(), FeedError> {
        self.scope
            .binding()
            .assert_context(
                c.actor,
                c.event,
                c.session_family,
                c.request,
                c.authority_digest,
                c.expires_at,
            )
            .map_err(|_| FeedError::PermissionDenied)
    }
}
#[async_trait]
impl FeedResources for GuardedResources {
    async fn author(&self, c: Context, user: Uuid) -> Result<PublicAuthor, FeedError> {
        self.check(c)?;
        let subject = self
            .scope
            .public_profile_subject(user)
            .await
            .map_err(subject_failure)?;
        let query = subject.query();
        if query.actor.uuid() != c.actor
            || query.event.uuid() != c.event
            || query.subject.uuid() != user
        {
            return Err(FeedError::PermissionDenied);
        }
        subject
            .binding()
            .assert_context(
                c.actor,
                c.event,
                c.session_family,
                c.request,
                c.authority_digest,
                c.expires_at,
            )
            .map_err(|_| FeedError::PermissionDenied)?;
        subject.check().await.map_err(subject_failure)?;
        // The Profile reader retains its own exclusion with this exact subject's
        // root scope; no child completes before the final response is assembled.
        let author = self.profiles.read(&subject).await?;
        super::wire::author(author.clone(), user)?;
        subject.check().await.map_err(subject_failure)?;
        self.check(c)?;
        Ok(author)
    }
    async fn public_link(&self, c: Context, _: ResourceLink) -> Result<bool, FeedError> {
        self.check(c)?;
        Err(FeedError::DependencyUnavailable)
    }
    async fn public_media(&self, c: Context, _: Uuid, _: Uuid) -> Result<bool, FeedError> {
        self.check(c)?;
        Err(FeedError::DependencyUnavailable)
    }
}

fn subject_failure(status: tonic::Status) -> FeedError {
    match status.code() {
        tonic::Code::NotFound | tonic::Code::PermissionDenied => FeedError::ResourceUnavailable,
        _ => FeedError::DependencyUnavailable,
    }
}
