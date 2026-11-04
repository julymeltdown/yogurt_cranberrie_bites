//! Same-scope Intro policy and borrowed same-transaction Age projection.
use super::{
    profile::{guard_error, ScopedIntroProfile},
    witness::register_intro_transaction,
};
use crate::adapters::out::{
    public_profile::PublicProfileReader,
    relationship_authority::guarded::OwnerGuardScope,
    repository::intro::{self as engine, IntroPolicy, IntroProjectionExecutor},
};
use chrono::{DateTime, Utc};
use concert_application::intro;
use sqlx::{PgConnection, PgPool, Row};
use std::sync::Arc;

#[derive(Clone)]
pub struct ScopedIntroExecutor {
    pool: PgPool,
    scope: Arc<OwnerGuardScope>,
    profiles: ScopedIntroProfile,
    claims: crate::adapters::out::age_provider::claim_reader::PgClaimProjection,
    pending: Arc<
        tokio::sync::Mutex<
            Vec<crate::adapters::out::age_provider::claim_reader::PendingAgeProjection<()>>,
        >,
    >,
    until: Arc<std::sync::Mutex<DateTime<Utc>>>,
}
impl ScopedIntroExecutor {
    pub fn new(
        pool: PgPool,
        scope: Arc<OwnerGuardScope>,
        profiles: Arc<PublicProfileReader>,
        claims: crate::adapters::out::age_provider::claim_reader::PgClaimProjection,
    ) -> Result<Self, intro::IntroError> {
        scope.binding().check().map_err(guard_error)?;
        let until = DateTime::from_timestamp_micros(scope.binding().deadline().expires_micros())
            .ok_or(intro::IntroError::Unavailable)?;
        Ok(Self {
            pool,
            profiles: ScopedIntroProfile::new(scope.clone(), profiles),
            scope,
            claims,
            pending: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            until: Arc::new(std::sync::Mutex::new(until)),
        })
    }
    pub fn profiles(&self) -> ScopedIntroProfile {
        self.profiles.clone()
    }
    fn context(&self, context: intro::IntroReadContext) -> Result<(), intro::IntroError> {
        let binding = self.scope.binding();
        binding.check().map_err(guard_error)?;
        if context.actor.uuid() != binding.actor()
            || context.event.uuid() != binding.event()
            || context.session != binding.session_family()
            || context.request != binding.request_id()
            || context.expires_at.timestamp_micros() != binding.deadline().expires_micros()
        {
            return Err(intro::IntroError::Unavailable);
        }
        context.check(Utc::now())
    }
    async fn actor(
        &self,
        c: &mut PgConnection,
        context: intro::IntroReadContext,
        write: bool,
    ) -> Result<(), intro::IntroError> {
        self.context(context)?;
        register_intro_transaction(c, &self.scope).await?;
        if write {
            // Acquire the final event mode before actor authorization takes a
            // shared event lock and before BE15 takes the global receipt budget.
            engine::lock_intro_event_for_write(c, context.event.uuid()).await?;
        }
        let policy = crate::relationship::PgRelationshipPolicy::new(
            self.pool.clone(),
            crate::adapters::out::relationship_authority::guarded::GuardedAuthority::new(
                self.scope.clone(),
            ),
        );
        let decision = policy
            .authorize_actor_in(
                c,
                concert_domain::ParticipantKey {
                    event_id: context.event,
                    user_id: context.actor,
                },
            )
            .await
            .map_err(relationship_error)?;
        let admission: Option<String> = sqlx::query_scalar("select state from public.concert_event_admissions where event_id=$1 and user_id=$2 for share")
            .bind(context.event.uuid()).bind(context.actor.uuid()).fetch_optional(&mut *c).await
            .map_err(|_| intro::IntroError::Unavailable)?;
        if admission.as_deref() != Some("ACTIVE") {
            return Err(intro::IntroError::Denied);
        }
        self.pin(c, decision.valid_until()).await
    }
    async fn pin(
        &self,
        c: &mut PgConnection,
        until: DateTime<Utc>,
    ) -> Result<(), intro::IntroError> {
        let until = {
            let mut held = self
                .until
                .lock()
                .map_err(|_| intro::IntroError::Unavailable)?;
            *held = (*held).min(until);
            *held
        };
        if until <= Utc::now() {
            return Err(intro::IntroError::Unavailable);
        }
        // A public Intro's own expiry also bounds serialization/HTTP delivery,
        // not only the SQL commit and producer's after-commit acknowledgement.
        self.scope
            .limit_disclosure_until(until)
            .map_err(guard_error)?;
        sqlx::query("select set_config('mcr.intro_authority_until',least(coalesce(nullif(current_setting('mcr.intro_authority_until',true),'')::timestamptz,'infinity'::timestamptz),$1::timestamptz)::text,true)")
            .bind(until).execute(c).await.map_err(|_| intro::IntroError::Unavailable)?;
        Ok(())
    }
    async fn row_in(
        &self,
        c: &mut PgConnection,
        context: intro::IntroReadContext,
        expected: &intro::PublicForumIntro,
    ) -> Result<concert_domain::age_policy::AgeBounds, intro::IntroError> {
        self.context(context)?;
        // This is a negative-only private filter on our registered transaction.
        // A departed author or established local block is absence, not a Profile
        // outage. A positive observation grants nothing: the actual current
        // owner enrollment and held PublicProfile child below remain mandatory.
        // Do not turn a generic Profile/guard failure into a skippable candidate.
        let visible_locally: bool = sqlx::query_scalar("select exists(select 1 from public.concert_event_admissions a join public.concert_participants p using(event_id,user_id) where a.event_id=$1 and a.user_id=$2 and a.state='ACTIVE') and not exists(select 1 from public.concert_relationship_pairs where event_id=$1 and low_user=least($2,$3) and high_user=greatest($2,$3) and (low_blocks_high or high_blocks_low))")
            .bind(context.event.uuid()).bind(expected.author.uuid()).bind(context.actor.uuid())
            .fetch_one(&mut *c).await.map_err(|_| intro::IntroError::Unavailable)?;
        if !visible_locally {
            return Err(intro::IntroError::NotFound);
        }
        // Prepare the actual shared local policy child before any conflicting
        // pair/admission/avatar write. Do NOT stack lock_state's pair FOR UPDATE.
        let (subject, policy) = self
            .profiles
            .prepare(context.event.uuid(), expected.author.uuid())
            .await?;
        let row = sqlx::query("select event_id,intro_id,owner_id,text,created_at,revision,minimum_age,maximum_age_exclusive,expires_at from public.concert_intros where event_id=$1 and intro_id=$2 and state='PUBLISHED' for share")
            .bind(context.event.uuid()).bind(expected.intro_id).fetch_optional(&mut *c).await
            .map_err(|_| intro::IntroError::Unavailable)?.ok_or(intro::IntroError::NotFound)?;
        let current = engine::row_public(&row)?;
        let now: DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
            .fetch_one(&mut *c)
            .await
            .map_err(|_| intro::IntroError::Unavailable)?;
        let expiry: Option<DateTime<Utc>> = row
            .try_get("expires_at")
            .map_err(|_| intro::IntroError::Unavailable)?;
        if current.author != expected.author
            || current.revision != expected.revision
            || current.text != expected.text
            || current.created_at != expected.created_at
            || current.created_at > now
            || expiry.is_some_and(|v| v <= now)
            || current.text.trim().is_empty()
            || concert_domain::intro_policy::contact::contact_bypass(&current.text)
        {
            return Err(intro::IntroError::NotFound);
        }
        use crate::relationship::FencedRelationshipAuthority;
        let authority =
            crate::adapters::out::relationship_authority::guarded::GuardedAuthority::new(
                self.scope.clone(),
            )
            .check_in(c, subject.query(), now)
            .await
            .map_err(relationship_error)?;
        if authority.query != subject.query()
            || !authority.actor_allowed
            || !authority.subject_allowed
            || authority.inherited_block
            || authority.checked_at != now
        {
            return Err(intro::IntroError::Denied);
        }
        self.pin(
            c,
            policy
                .valid_until
                .min(authority.valid_until)
                .min(expiry.unwrap_or(context.expires_at)),
        )
        .await?;
        subject.check().await.map_err(guard_error)?;
        concert_domain::age_policy::AgeBounds::new(
            row.try_get("minimum_age")
                .map_err(|_| intro::IntroError::Unavailable)?,
            row.try_get("maximum_age_exclusive")
                .map_err(|_| intro::IntroError::Unavailable)?,
        )
        .map_err(|_| intro::IntroError::Unavailable)
    }
}
fn relationship_error(error: concert_domain::relationship::RelationshipError) -> intro::IntroError {
    match error {
        concert_domain::relationship::RelationshipError::Denied => intro::IntroError::Denied,
        _ => intro::IntroError::Unavailable,
    }
}
fn age_error(error: concert_application::age_policy::PolicyError) -> intro::IntroError {
    match error {
        concert_application::age_policy::PolicyError::Denied => intro::IntroError::Denied,
        _ => intro::IntroError::Unavailable,
    }
}

pub struct ScopedIntroPolicy {
    executor: ScopedIntroExecutor,
}
impl ScopedIntroPolicy {
    pub fn new(executor: ScopedIntroExecutor) -> Self {
        Self { executor }
    }
}
#[async_trait::async_trait]
impl IntroPolicy for ScopedIntroPolicy {
    async fn check(
        &self,
        c: &mut PgConnection,
        command: &intro::IntroCommand,
    ) -> Result<(), intro::IntroError> {
        let binding = self.executor.scope.binding();
        if binding.actor() != command.actor().uuid()
            || binding.event() != command.event().uuid()
            || binding.operation() != command.operation()
        {
            return Err(intro::IntroError::Unavailable);
        }
        self.executor
            .actor(c, scope_context(&self.executor.scope)?, true)
            .await
    }
    async fn edit_consent(
        &self,
        c: &mut PgConnection,
        command: &intro::IntroCommand,
        next: &intro::OwnerIntro,
    ) -> Result<(), intro::IntroError> {
        self.executor.scope.binding().check().map_err(guard_error)?;
        if next.owner_id != command.actor().uuid()
            || next.event_id != command.event().uuid()
            || next.intro_id != command.intro()
        {
            return Err(intro::IntroError::Unavailable);
        }
        crate::relationship::social::revoke_intro_consent_in(
            c,
            command.event(),
            command.intro(),
            command.actor(),
            next.revision,
        )
        .await
        .map_err(relationship_error)
    }
}
pub(super) fn scope_context(
    scope: &Arc<OwnerGuardScope>,
) -> Result<intro::IntroReadContext, intro::IntroError> {
    let binding = scope.binding();
    binding.check().map_err(guard_error)?;
    Ok(intro::IntroReadContext {
        actor: concert_domain::AuthUserId::from_uuid(binding.actor()),
        event: concert_domain::EventId::from_uuid(binding.event()),
        session: binding.session_family(),
        request: binding.request_id(),
        expires_at: DateTime::from_timestamp_micros(binding.deadline().expires_micros())
            .ok_or(intro::IntroError::Unavailable)?,
    })
}
impl IntroProjectionExecutor for ScopedIntroExecutor {
    async fn check_collection(
        &self,
        c: &mut PgConnection,
        context: intro::IntroReadContext,
        _: DateTime<Utc>,
    ) -> Result<(), intro::IntroError> {
        self.context(context)?;
        // An empty collection still requires actual local admission and both owners.
        let (_, local) = self
            .profiles
            .prepare(context.event.uuid(), context.actor.uuid())
            .await?;
        self.actor(c, context, false).await?;
        self.pin(c, local.valid_until).await
    }
    async fn check_owner(
        &self,
        c: &mut PgConnection,
        context: intro::IntroReadContext,
        _: DateTime<Utc>,
    ) -> Result<(), intro::IntroError> {
        self.actor(c, context, false).await
    }
    async fn project_public(
        &self,
        c: &mut PgConnection,
        context: intro::IntroReadContext,
        row: &intro::PublicForumIntro,
        now: DateTime<Utc>,
    ) -> Result<(), intro::IntroError> {
        self.context(context)?;
        // This first private metadata read only selects the dependency path. The
        // protected row and bounds are locked and checked again inside projection.
        let bounds: Option<(Option<i16>,Option<i16>)> = sqlx::query_as("select minimum_age,maximum_age_exclusive from public.concert_intros where event_id=$1 and intro_id=$2")
            .bind(context.event.uuid()).bind(row.intro_id).fetch_optional(&mut *c).await
            .map_err(|_| intro::IntroError::Unavailable)?;
        let bounds = bounds.ok_or(intro::IntroError::NotFound)?;
        let restricted = concert_domain::age_policy::AgeBounds::new(bounds.0, bounds.1)
            .map_err(|_| intro::IntroError::Unavailable)?
            .restricted();
        if !restricted {
            // If a concurrent edit adds bounds, never bypass the claim path.
            if self.row_in(c, context, row).await?.restricted() {
                return Err(intro::IntroError::NotFound);
            }
            return Ok(());
        }
        let query = concert_application::age_policy::ClaimQuery {
            context: concert_application::age_policy::Context {
                actor: context.actor,
                event: context.event,
                session: context.session,
                request: context.request,
                expires_at: context.expires_at,
            },
            subject: context.actor,
            requested_at: now,
        };
        let executor = self.clone();
        let expected = intro::PublicForumIntro {
            intro_id: row.intro_id,
            event_id: row.event_id,
            author: row.author,
            text: row.text.clone(),
            created_at: row.created_at,
            revision: row.revision,
        };
        let pending = self
            .claims
            .project_in(c, query, move |c, age| {
                Box::pin(async move {
                    let bounds =
                        executor
                            .row_in(c, context, &expected)
                            .await
                            .map_err(|e| match e {
                                intro::IntroError::Denied | intro::IntroError::NotFound => {
                                    concert_application::age_policy::PolicyError::Denied
                                }
                                _ => concert_application::age_policy::PolicyError::Unavailable,
                            })?;
                    if !age.permits(bounds, false) {
                        return Err(concert_application::age_policy::PolicyError::Denied);
                    }
                    Ok(())
                })
            })
            .await
            .map_err(age_error)?;
        self.pending.lock().await.push(pending);
        Ok(())
    }
    async fn acknowledge(
        &self,
        _: &mut PgConnection,
        context: intro::IntroReadContext,
        now: DateTime<Utc>,
    ) -> Result<(), intro::IntroError> {
        self.context(context)?;
        if now
            >= *self
                .until
                .lock()
                .map_err(|_| intro::IntroError::Unavailable)?
        {
            return Err(intro::IntroError::Unavailable);
        }
        self.profiles.acknowledge().await
    }
    async fn after_commit(&self) -> Result<(), intro::IntroError> {
        for pending in self.pending.lock().await.drain(..) {
            pending.after_commit().map_err(age_error)?;
        }
        if Utc::now()
            >= *self
                .until
                .lock()
                .map_err(|_| intro::IntroError::Unavailable)?
        {
            return Err(intro::IntroError::Unavailable);
        }
        self.profiles.acknowledge().await
    }
}
