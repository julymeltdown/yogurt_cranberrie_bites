//! Actual PublicProfile owner children retained by the original canonical scope.
use crate::adapters::out::{
    public_profile::{LocalPolicySnapshot, PublicProfileReader},
    relationship_authority::guarded::{GuardedSubject, OwnerGuardScope},
    repository::intro::{self as engine, IntroProfileProjection},
};
use chrono::Utc;
use concert_application::intro;
use sqlx::PgConnection;
use std::sync::Arc;
use uuid::Uuid;

/// One instance per actual root scope. Neither event/owner identifiers nor a
/// Profile response can create or replace this owner-enrolled subject.
#[derive(Clone)]
pub struct ScopedIntroProfile {
    scope: Arc<OwnerGuardScope>,
    reader: Arc<PublicProfileReader>,
    subjects: Arc<tokio::sync::Mutex<std::collections::BTreeMap<Uuid, Arc<GuardedSubject>>>>,
}
impl ScopedIntroProfile {
    pub fn new(scope: Arc<OwnerGuardScope>, reader: Arc<PublicProfileReader>) -> Self {
        Self {
            scope,
            reader,
            subjects: Arc::new(tokio::sync::Mutex::new(Default::default())),
        }
    }
    /// Acquire this BEFORE any conflicting pair/admission/avatar write lock.
    /// Root retains the real prepared LocalChild through HTTP terminal delivery.
    pub(super) async fn prepare(
        &self,
        event: Uuid,
        owner: Uuid,
    ) -> Result<(Arc<GuardedSubject>, LocalPolicySnapshot), intro::IntroError> {
        if event != self.scope.binding().event() || owner.is_nil() {
            return Err(intro::IntroError::Unavailable);
        }
        self.scope.binding().check().map_err(guard_error)?;
        let subjects = self.subjects.lock().await;
        let subject = match subjects.get(&owner) {
            Some(value) => value.clone(),
            None => {
                let value = Arc::new(
                    self.scope
                        .public_profile_subject(owner)
                        .await
                        .map_err(guard_error)?,
                );
                value
            }
        };
        drop(subjects);
        let policy = self
            .reader
            .local_subject(&subject)
            .await
            .map_err(profile_error)?;
        let query = subject.query();
        if query.event.uuid() != event
            || query.actor.uuid() != self.scope.binding().actor()
            || query.subject.uuid() != owner
            || policy.event_revision < 1
            || policy.pair_revision < 1
            || !matches!(
                policy.phase,
                concert_domain::EventPhase::Open
                    | concert_domain::EventPhase::Live
                    | concert_domain::EventPhase::ReadOnly
            )
            || policy.valid_until <= Utc::now()
        {
            return Err(intro::IntroError::Unavailable);
        }
        subject.check().await.map_err(guard_error)?;
        // A typed local denial before child preparation is skippable. Keep the
        // SDK's attempted enrollment/intents, but acknowledge only successful
        // local subjects here; a rejected candidate must not poison the page.
        self.subjects.lock().await.insert(owner, subject.clone());
        Ok((subject, policy))
    }
    pub(super) async fn acknowledge(&self) -> Result<(), intro::IntroError> {
        let subjects = self
            .subjects
            .lock()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for subject in subjects {
            self.prepare(self.scope.binding().event(), subject.query().subject.uuid())
                .await?;
        }
        self.scope.binding().check().map_err(guard_error)
    }
}
impl IntroProfileProjection for ScopedIntroProfile {
    async fn author(
        &self,
        _: &mut PgConnection,
        event: Uuid,
        owner: Uuid,
    ) -> Result<intro::PublicAuthor, intro::IntroError> {
        let (subject, _) = self.prepare(event, owner).await?;
        let profile = self.reader.read(&subject).await.map_err(profile_error)?;
        if profile.user_id != owner {
            return Err(intro::IntroError::Unavailable);
        }
        let author = intro::PublicAuthor {
            user_id: concert_domain::AuthUserId::from_uuid(owner),
            json: profile.json,
        };
        engine::validate_public_profile(&author.json, author.user_id)?;
        subject.check().await.map_err(guard_error)?;
        Ok(author)
    }
}
pub(super) fn profile_error(error: concert_domain::feed::FeedError) -> intro::IntroError {
    match error {
        concert_domain::feed::FeedError::ResourceUnavailable => intro::IntroError::NotFound,
        _ => intro::IntroError::Unavailable,
    }
}
pub(super) fn guard_error(error: tonic::Status) -> intro::IntroError {
    // Only a typed owner absence is skippable; PermissionDenied may mean a
    // rebound/sealed scope and must abort the whole projection.
    if error.code() == tonic::Code::NotFound {
        intro::IntroError::NotFound
    } else {
        intro::IntroError::Unavailable
    }
}
