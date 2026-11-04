//! Concrete request-local production authority. The constructor needs both
//! durable owner registrations; no injected boolean/test authority is accepted.
use super::{
    read_admission::{ReadAdmission, ReadOperation},
    read_clients::{ReadClients, SubjectFence},
    read_tracking::RegisteredRead,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use post_application::concert_feed::{Access, Authority, Context, Fence, Filter};
use post_domain::concert_feed::{Error, Post, State};
use std::sync::Arc;

pub(crate) struct RequestReadAuthority {
    admission: Arc<ReadAdmission>,
    registered: Arc<RegisteredRead>,
    clients: ReadClients,
}
impl RequestReadAuthority {
    pub(crate) fn new(
        admission: Arc<ReadAdmission>,
        registered: Arc<RegisteredRead>,
        clients: ReadClients,
    ) -> Result<Self, Error> {
        admission.check()?;
        let expected = &admission
            .authority
            .auth
            .as_ref()
            .ok_or(Error::Authority)?
            .binding_digest;
        if registered.binding_digest() != expected {
            return Err(Error::Authority);
        }
        Ok(Self {
            admission,
            registered,
            clients,
        })
    }
    async fn post(&self, post: &Post) -> Result<Vec<SubjectFence>, Error> {
        post.visible_in(self.admission.context.event_id)?;
        self.admission.parent(post.post_id)?;
        post.content.validate()?;
        // Without actual related-object/media authority, fail the operation.
        // Mapping this to Forbidden would turn a dependency gap into empty data.
        if !post.content.media_ids.is_empty() || !post.content.links.is_empty() {
            return Err(Error::Unavailable);
        }
        Ok(vec![
            self.clients
                .subject(&self.admission, post.author_id)
                .await?,
        ])
    }
}
#[async_trait]
impl Authority for RequestReadAuthority {
    async fn acquire(
        &self,
        context: &Context,
        access: &Access,
        binding: [u8; 32],
    ) -> Result<Box<dyn Fence>, Error> {
        self.admission.check()?;
        if *context != self.admission.context || binding == [0; 32] {
            return Err(Error::Authority);
        }
        let subjects = match access {
            Access::ListPosts { window } => {
                if self.admission.operation != ReadOperation::Feed {
                    return Err(Error::Forbidden);
                }
                window.validate()?;
                if window.filter != Filter::All {
                    return Err(Error::Unavailable);
                }
                vec![
                    self.clients
                        .subject(&self.admission, context.actor_id)
                        .await?,
                ]
            }
            Access::ReadPost { post } => self.post(post).await?,
            Access::ReadComment { parent, comment } => {
                let mut subjects = self.post(parent).await?;
                if comment.event_id != context.event_id
                    || comment.post_id != parent.post_id
                    || comment.state != State::Published
                {
                    return Err(Error::NotFound);
                }
                subjects.push(
                    self.clients
                        .subject(&self.admission, comment.author_id)
                        .await?,
                );
                subjects
            }
            Access::Write { .. } | Access::Receipt { .. } => return Err(Error::Authority),
        };
        self.admission.check()?;
        let fence = ReadFence {
            admission: self.admission.clone(),
            _registered: self.registered.clone(),
            binding,
            subjects,
        };
        fence.check(context, &binding, Utc::now())?;
        Ok(Box::new(fence))
    }
}
struct ReadFence {
    admission: Arc<ReadAdmission>,
    _registered: Arc<RegisteredRead>,
    binding: [u8; 32],
    subjects: Vec<SubjectFence>,
}
impl Fence for ReadFence {
    fn check(
        &self,
        context: &Context,
        binding: &[u8; 32],
        now: DateTime<Utc>,
    ) -> Result<(), Error> {
        self.admission.check()?;
        context.validate(now)?;
        if *context != self.admission.context
            || *binding != self.binding
            || self.subjects.is_empty()
            || self
                .subjects
                .iter()
                .any(|subject| subject.expires_at <= now)
        {
            return Err(Error::Authority);
        }
        // The owner Track ledger retains Auth/Friend exclusion through the
        // actual transaction's terminal proof. The actual Concert callback
        // retained event/pair/participant child exclusion through root closure.
        // These facts came from those held guards, not a reusable observation.
        Ok(())
    }
}
