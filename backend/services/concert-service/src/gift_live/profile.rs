//! Actual published PublicProfile adapter. No UUID-only or fallback projection.
use super::runtime::GiftProfiles;
use crate::adapters::out::{
    public_profile::PublicProfileReader, relationship_authority::guarded::GuardedSubject,
};
use async_trait::async_trait;
use concert_application::feed::PublicAuthor;
use concert_domain::feed::FeedError;

#[async_trait]
impl GiftProfiles for PublicProfileReader {
    async fn read(&self, subject: &GuardedSubject) -> Result<PublicAuthor, FeedError> {
        PublicProfileReader::read(self, subject).await
    }
}
