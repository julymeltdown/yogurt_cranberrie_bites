//! Qualified producer composition. No legacy mutation/receipt service uses it.
use super::{
    read_admission::{ReadAdmission, ReadOperation},
    read_authority::RequestReadAuthority,
    read_clients::ReadClients,
    read_tracking::TrackingStore,
    read_transaction::CapturedReadTransaction,
    storage, Owner,
};
use crate::adapters::r#in::grpc::concert_feed::{
    proto,
    read_wire::{self, ReadQuery},
};
use platform::actor_delegation::concert_read::PostConcertRootReadSigner;
use post_domain::concert_feed::Error;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct QualifiedReadRuntime {
    pool: PgPool,
    clients: ReadClients,
    tracking: TrackingStore,
    admission_slots: Arc<tokio::sync::Semaphore>,
}
pub(crate) enum ReadResult {
    Post(proto::GetPostResponse),
    Posts(proto::ListPostsResponse),
    Comment(proto::GetCommentResponse),
    Comments(proto::ListCommentsResponse),
}
impl QualifiedReadRuntime {
    /// Actual fixed-workload mTLS clients and the dedicated Post→Concert
    /// signer. No constructor accepts an Authority or a policy callback.
    pub fn connect(
        pool: PgPool,
        auth_endpoint: &str,
        friend_endpoint: &str,
        concert_endpoint: &str,
        token: &str,
        tls: &platform::grpc_tls::TlsMaterial,
        signer: PostConcertRootReadSigner,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            pool.options().get_max_connections() >= 2,
            "qualified Post reads require two database connections"
        );
        let slots = (pool.options().get_max_connections() / 2).min(64) as usize;
        Ok(Self {
            admission_slots: Arc::new(tokio::sync::Semaphore::new(slots)),
            tracking: TrackingStore::new(pool.clone(), Uuid::new_v4())
                .map_err(|_| anyhow::anyhow!("read tracking unavailable"))?,
            pool,
            clients: ReadClients::connect(
                auth_endpoint,
                friend_endpoint,
                concert_endpoint,
                token,
                tls,
                signer,
            )?,
        })
    }
    pub(crate) fn tracking(&self) -> &TrackingStore {
        &self.tracking
    }
    pub(crate) async fn run(
        &self,
        a: ReadAdmission,
        query: ReadQuery,
    ) -> Result<ReadResult, Error> {
        let stop = a.deadline.stop();
        let work = async {
            let _slot = self
                .admission_slots
                .acquire()
                .await
                .map_err(|_| Error::Unavailable)?;
            a.check()?;
            match &query {
                ReadQuery::Posts(_) if a.operation != ReadOperation::Feed => {
                    return Err(Error::Forbidden)
                }
                ReadQuery::Post(post) | ReadQuery::Comments { post, .. } => a.parent(*post)?,
                _ => {}
            }
            // Actor eligibility/event/participant policy is checked before
            // opening any protected Post read. Private own profiles are valid.
            self.clients.subject(&a, a.context.actor_id).await?;
            let mut tx = self.pool.begin().await.map_err(storage)?;
            let effect = super::digest(&(&a.context, &query))?;
            let reference = super::digest(&(Uuid::new_v4(), Uuid::new_v4()))?;
            let identity = CapturedReadTransaction::capture(
                &mut tx,
                self.tracking.instance(),
                effect,
                reference,
            )
            .await?;
            let tracked = self.tracking.begin(&a, identity).await?;
            let registered = Arc::new(tracked.register(&self.clients, &a).await?);
            registered.witness(&mut tx).await?;
            let a = Arc::new(a);
            let authority = RequestReadAuthority::new(a.clone(), registered, self.clients.clone())?;
            let owner = Owner {
                pool: self.pool.clone(),
                authority: Arc::new(authority),
            };
            let response = match query {
                ReadQuery::Post(id) => ReadResult::Post(proto::GetPostResponse {
                    post: Some(read_wire::post(
                        owner
                            .post_in(&a.context, id, tx)
                            .await?
                            .ok_or(Error::NotFound)?,
                    )),
                }),
                ReadQuery::Posts(window) => ReadResult::Posts(read_wire::posts(
                    owner.page_in(&a.context, &window, tx).await?,
                )),
                ReadQuery::Comment(id) => ReadResult::Comment(proto::GetCommentResponse {
                    comment: Some(read_wire::comment(
                        owner
                            .comment_in(&a.context, id, tx)
                            .await?
                            .ok_or(Error::NotFound)?,
                    )),
                }),
                ReadQuery::Comments {
                    post,
                    upper,
                    after,
                    limit,
                } => ReadResult::Comments(read_wire::comments(
                    owner
                        .comments_in(&a.context, post, upper, after, limit, tx)
                        .await?,
                )),
            };
            // Root scope still owns every actual child guard through C04
            // assembly and HTTP completion. Post never completes that scope.
            a.check()?;
            Ok(response)
        };
        tokio::time::timeout_at(stop, work)
            .await
            .map_err(|_| Error::Unavailable)?
    }
}
