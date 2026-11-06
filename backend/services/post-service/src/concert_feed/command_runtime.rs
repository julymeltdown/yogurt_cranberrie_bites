//! Qualified writes and metadata lookup on one actually registered transaction.
//! Dropping or timing out never fabricates a commit or completes owner guards.
use super::{
    command_admission::CommandAdmission,
    read_clients::ReadClients,
    read_runtime::QualifiedReadRuntime,
    read_tracking::{RegisteredRead, TrackingStore},
    read_transaction::CapturedReadTransaction,
    storage, Owner,
};
use crate::adapters::r#in::grpc::concert_feed::proto;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use post_application::concert_feed::{Access, Authority, Context, Fence, PostView};
use post_domain::concert_feed::{Command, Error, Mutation};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct QualifiedCommandRuntime {
    pool: PgPool,
    clients: ReadClients,
    tracking: TrackingStore,
    slots: Arc<tokio::sync::Semaphore>,
}
impl QualifiedCommandRuntime {
    pub(crate) fn from_read(read: &QualifiedReadRuntime) -> Self {
        Self {
            pool: read.pool.clone(),
            clients: read.clients.clone(),
            tracking: read.tracking.clone(),
            slots: read.admission_slots.clone(),
        }
    }
    pub(crate) fn tracking(&self) -> &TrackingStore {
        &self.tracking
    }
    pub(crate) async fn run(&self, a: CommandAdmission) -> Result<proto::Receipt, Error> {
        let stop = a.owners.deadline.stop();
        let work = async {
            let _slot = self.slots.acquire().await.map_err(|_| Error::Unavailable)?;
            a.owners.check()?;
            self.clients
                .subject(&a.owners, a.owners.context.actor_id)
                .await?;
            let mut tx = self.pool.begin().await.map_err(storage)?;
            let identity = CapturedReadTransaction::capture(
                &mut tx,
                self.tracking.instance(),
                super::digest(&(&a.owners.context, &a.command))?,
                super::digest(&(Uuid::new_v4(), Uuid::new_v4()))?,
            )
            .await?;
            let tracked = self.tracking.begin(&a.owners, identity).await?;
            let registered = Arc::new(tracked.register(&self.clients, &a.owners).await?);
            registered.witness(&mut tx).await?;
            let a = Arc::new(a);
            let owner = Owner {
                pool: self.pool.clone(),
                authority: Arc::new(CommandAuthority {
                    a: a.clone(),
                    registered,
                    clients: self.clients.clone(),
                }),
            };
            let outcome = owner.execute_in(&a.owners.context, &a.command, tx).await?;
            a.owners.check()?;
            Ok(super::command_wire::receipt(
                outcome,
                a.command.idempotency_key.clone(),
            ))
        };
        tokio::time::timeout_at(stop, work)
            .await
            .map_err(|_| Error::Unavailable)?
    }
    pub(crate) async fn receipt(
        &self,
        a: CommandAdmission,
    ) -> Result<Option<proto::Receipt>, Error> {
        let stop = a.owners.deadline.stop();
        let work = async {
            let _slot = self.slots.acquire().await.map_err(|_| Error::Unavailable)?;
            a.owners.check()?;
            self.clients
                .subject(&a.owners, a.owners.context.actor_id)
                .await?;
            let mut tx = self.pool.begin().await.map_err(storage)?;
            let identity = CapturedReadTransaction::capture(
                &mut tx,
                self.tracking.instance(),
                super::digest(&(&a.owners.context, &a.command, "receipt"))?,
                super::digest(&(Uuid::new_v4(), Uuid::new_v4()))?,
            )
            .await?;
            let tracked = self.tracking.begin(&a.owners, identity).await?;
            let registered = Arc::new(tracked.register(&self.clients, &a.owners).await?);
            registered.witness(&mut tx).await?;
            let a = Arc::new(a);
            let owner = Owner {
                pool: self.pool.clone(),
                authority: Arc::new(CommandAuthority {
                    a: a.clone(),
                    registered,
                    clients: self.clients.clone(),
                }),
            };
            let outcome = owner.receipt_in(&a.owners.context, &a.command, tx).await?;
            a.owners.check()?;
            Ok(outcome.map(|v| super::command_wire::receipt(v, a.command.idempotency_key.clone())))
        };
        tokio::time::timeout_at(stop, work)
            .await
            .map_err(|_| Error::Unavailable)?
    }
    pub(crate) async fn preview(
        &self,
        a: CommandAdmission,
    ) -> Result<(Option<PostView>, Option<post_domain::concert_feed::Comment>), Error> {
        let stop = a.owners.deadline.stop();
        let work = async {
            let _slot = self.slots.acquire().await.map_err(|_| Error::Unavailable)?;
            a.owners.check()?;
            self.clients
                .subject(&a.owners, a.owners.context.actor_id)
                .await?;
            let context = a.owners.context.clone();
            let mut command = a.command.clone();
            if let Some(id) = command.mutation.comment_id() {
                let parent: Option<Uuid> = sqlx::query_scalar(
                    "select post_id from post_concert_comments where event_id=$1 and comment_id=$2",
                )
                .bind(command.event_id)
                .bind(id)
                .fetch_optional(&self.pool)
                .await
                .map_err(storage)?;
                let Some(parent) = parent else {
                    return Ok((None, None));
                };
                command.post_id = parent;
            }
            let admission = Arc::new(a);
            let owner = Owner {
                pool: self.pool.clone(),
                authority: Arc::new(PreviewAuthority {
                    admission: admission.clone(),
                    command: command.clone(),
                    clients: self.clients.clone(),
                }),
            };
            let post = owner.post(&context, command.post_id).await?;
            let comment = match command.mutation.comment_id() {
                Some(id) => owner.comment(&context, id).await?,
                None => None,
            };
            Ok((post, comment))
        };
        tokio::time::timeout_at(stop, work)
            .await
            .map_err(|_| Error::Unavailable)?
    }
}
struct PreviewAuthority {
    admission: Arc<CommandAdmission>,
    command: Command,
    clients: ReadClients,
}
#[async_trait]
impl Authority for PreviewAuthority {
    async fn acquire(
        &self,
        context: &Context,
        access: &Access,
        binding: [u8; 32],
    ) -> Result<Box<dyn Fence>, Error> {
        self.admission.owners.check()?;
        if *context != self.admission.owners.context || binding == [0; 32] {
            return Err(Error::Authority);
        }
        let mut ids = vec![context.actor_id];
        match access {
            Access::ReadPost { post } => {
                if post.post_id != self.command.post_id || post.event_id != context.event_id {
                    return Err(Error::Authority);
                }
                ids.push(post.author_id);
            }
            Access::ReadComment { parent, comment } => {
                if Some(comment.comment_id) != self.command.mutation.comment_id()
                    || parent.post_id != self.command.post_id
                    || comment.post_id != parent.post_id
                    || comment.event_id != context.event_id
                {
                    return Err(Error::Authority);
                }
                ids.push(parent.author_id);
                ids.push(comment.author_id);
            }
            _ => return Err(Error::Authority),
        }
        let mut subjects = Vec::new();
        for id in ids {
            subjects.push(self.clients.subject(&self.admission.owners, id).await?);
        }
        let expires = subjects
            .iter()
            .map(|s| s.expires_at)
            .min()
            .unwrap_or(context.expires_at)
            .min(context.expires_at);
        let fence = CommandFence {
            a: self.admission.clone(),
            _registered: None,
            binding,
            expires,
            subjects,
        };
        fence.check(context, &binding, Utc::now())?;
        Ok(Box::new(fence))
    }
}
struct CommandAuthority {
    a: Arc<CommandAdmission>,
    registered: Arc<RegisteredRead>,
    clients: ReadClients,
}
#[async_trait]
impl Authority for CommandAuthority {
    async fn acquire(
        &self,
        context: &Context,
        access: &Access,
        binding: [u8; 32],
    ) -> Result<Box<dyn Fence>, Error> {
        self.a.owners.check()?;
        if *context != self.a.owners.context || binding == [0; 32] {
            return Err(Error::Authority);
        }
        let (command, parent) = match access {
            Access::Write {
                command, parent, ..
            } => (command, parent),
            Access::Receipt {
                command, parent, ..
            } => (command, parent),
            _ => return Err(Error::Authority),
        };
        if command != &self.a.command {
            return Err(Error::Authority);
        }
        if let Mutation::CreatePost { content } | Mutation::EditPost { content, .. } =
            &command.mutation
        {
            if !content.media_ids.is_empty() || !content.links.is_empty() {
                return Err(Error::Unavailable);
            }
        }
        self.a.owners.parent(command.post_id)?;
        let mut subjects = vec![
            self.clients
                .subject(&self.a.owners, context.actor_id)
                .await?,
        ];
        if let Some(parent) = parent {
            subjects.push(
                self.clients
                    .subject(&self.a.owners, parent.author_id)
                    .await?,
            );
        }
        let expires = subjects
            .iter()
            .map(|s| s.expires_at)
            .min()
            .unwrap_or(context.expires_at)
            .min(context.expires_at);
        let fence = CommandFence {
            a: self.a.clone(),
            _registered: Some(self.registered.clone()),
            binding,
            expires,
            subjects,
        };
        fence.check(context, &binding, Utc::now())?;
        Ok(Box::new(fence))
    }
}
struct CommandFence {
    a: Arc<CommandAdmission>,
    _registered: Option<Arc<RegisteredRead>>,
    binding: [u8; 32],
    expires: DateTime<Utc>,
    subjects: Vec<super::read_clients::SubjectFence>,
}
impl Fence for CommandFence {
    fn check(
        &self,
        context: &Context,
        binding: &[u8; 32],
        now: DateTime<Utc>,
    ) -> Result<(), Error> {
        self.a.owners.check()?;
        context.validate(now)?;
        if *context != self.a.owners.context
            || binding != &self.binding
            || self.expires <= now
            || self.subjects.iter().any(|s| s.expires_at <= now)
        {
            return Err(Error::Authority);
        }
        Ok(())
    }
}
