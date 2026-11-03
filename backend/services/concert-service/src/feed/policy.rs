//! Feed policy under one real canonical scope and the author's shared local
//! Concert guard. It never starts a second relationship-pair transaction.
use crate::adapters::out::{
    public_profile::PublicProfileReader, relationship_authority::guarded::OwnerGuardScope,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::feed::{Action, Context, FeedPolicy, Observation};
use concert_domain::feed::FeedError;
use std::sync::Arc;
use uuid::Uuid;

pub struct GuardedPolicy {
    scope: Arc<OwnerGuardScope>,
    profiles: Arc<PublicProfileReader>,
}
impl GuardedPolicy {
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
impl FeedPolicy for GuardedPolicy {
    async fn observe(
        &self,
        c: Context,
        subject: Uuid,
        action: Action,
    ) -> Result<Observation, FeedError> {
        self.check(c)?;
        if action != Action::Read || subject.is_nil() {
            return Err(FeedError::PermissionDenied);
        }
        let guarded = self
            .scope
            .public_profile_subject(subject)
            .await
            .map_err(|status| subject_failure(status, c.actor == subject))?;
        let query = guarded.query();
        if query.actor.uuid() != c.actor
            || query.event.uuid() != c.event
            || query.subject.uuid() != subject
        {
            return Err(FeedError::PermissionDenied);
        }
        guarded
            .check()
            .await
            .map_err(|status| subject_failure(status, c.actor == subject))?;
        // policy and author read share this exact retained local child. policy
        // does not require the viewing actor's own profile to be PUBLIC.
        let local = self.profiles.policy(&guarded).await.map_err(|error| {
            if c.actor != subject && error == FeedError::PermissionDenied {
                FeedError::ResourceUnavailable
            } else {
                error
            }
        })?;
        let owners = guarded.revision_vector();
        // Both independent owner epochs remain in the sealed canonical scope;
        // scalar feed observations are never exported as standalone authority.
        owners
            .auth
            .validate()
            .map_err(|_| FeedError::DependencyUnavailable)?;
        owners
            .friend
            .validate()
            .map_err(|_| FeedError::DependencyUnavailable)?;
        guarded
            .check()
            .await
            .map_err(|status| subject_failure(status, c.actor == subject))?;
        let deadline =
            DateTime::from_timestamp_micros(self.scope.binding().deadline().expires_micros())
                .ok_or(FeedError::DependencyUnavailable)?;
        let observation = Observation {
            context: c,
            subject,
            action,
            event_revision: local.event_revision,
            relationship_revision: local.pair_revision,
            auth_actor_revision: owners.auth.revision,
            auth_subject_revision: owners.auth.revision,
            friend_revision: owners.friend.revision,
            valid_until: local.valid_until.min(deadline).min(c.expires_at),
        };
        observation.check(c, subject, action, Utc::now())?;
        self.check(c)?;
        Ok(observation)
    }
    async fn fence(&self, observation: Observation) -> Result<(), FeedError> {
        observation.check(
            observation.context,
            observation.subject,
            observation.action,
            Utc::now(),
        )?;
        let current = self
            .observe(observation.context, observation.subject, observation.action)
            .await?;
        if current.event_revision != observation.event_revision
            || current.relationship_revision != observation.relationship_revision
            || current.auth_actor_revision != observation.auth_actor_revision
            || current.auth_subject_revision != observation.auth_subject_revision
            || current.friend_revision != observation.friend_revision
            || current.valid_until < observation.valid_until
        {
            return Err(FeedError::PermissionDenied);
        }
        Ok(())
    }
}

fn subject_failure(status: tonic::Status, actor: bool) -> FeedError {
    match status.code() {
        tonic::Code::NotFound | tonic::Code::PermissionDenied if actor => {
            FeedError::PermissionDenied
        }
        tonic::Code::NotFound | tonic::Code::PermissionDenied => FeedError::ResourceUnavailable,
        _ => FeedError::DependencyUnavailable,
    }
}
