//! Concrete read composition. Root retains this exact scope until actual HTTP
//! completion; assembling a JSON value does not settle it or its owner children.
use super::{
    author_filter::AuthorFilteredReader,
    cursor::CursorCodec,
    live_read::{CommentsQuery, FeedQuery, ReadFeed},
    owner::PostOwner,
    policy::GuardedPolicy,
    qualified_transport::QualifiedPostTransport,
    resources::GuardedResources,
};
use crate::adapters::out::{
    public_profile::PublicProfileReader, relationship_authority::guarded::OwnerGuardScope,
};
use concert_application::{
    feed::{Context, Feed},
    Clock,
};
use concert_domain::feed::FeedError;
use platform::{actor_delegation::concert_read::ConcertPostReadSigner, grpc_tls::TlsMaterial};
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

type Reader = AuthorFilteredReader<PostOwner<QualifiedPostTransport>, GuardedResources>;

pub struct QualifiedReadFeed<C> {
    inner: ReadFeed<Reader, GuardedPolicy, GuardedResources, C>,
}
impl<C: Clock> QualifiedReadFeed<C> {
    pub fn connect(
        endpoint: &str,
        token: &str,
        tls: &TlsMaterial,
        signer: ConcertPostReadSigner,
        scope: Arc<OwnerGuardScope>,
        profiles: Arc<PublicProfileReader>,
        cursors: CursorCodec,
        clock: C,
    ) -> anyhow::Result<Self> {
        let resources = GuardedResources::new(scope.clone(), profiles.clone())?;
        let policy = GuardedPolicy::new(scope.clone(), profiles)?;
        let transport = QualifiedPostTransport::connect(endpoint, token, tls, signer, scope)?;
        let reader = AuthorFilteredReader::new(PostOwner { transport }, resources.clone());
        Ok(Self {
            inner: ReadFeed::new(
                Feed {
                    reader,
                    policy,
                    resources,
                    clock,
                },
                cursors,
            ),
        })
    }
    pub async fn page(&self, context: Context, query: FeedQuery) -> Result<Value, FeedError> {
        self.inner.page(context, query).await
    }
    pub async fn detail(&self, context: Context, post: Uuid) -> Result<Value, FeedError> {
        self.inner.detail(context, post).await
    }
    pub async fn comments(
        &self,
        context: Context,
        post: Uuid,
        query: CommentsQuery,
    ) -> Result<Value, FeedError> {
        self.inner.comments(context, post, query).await
    }
}
