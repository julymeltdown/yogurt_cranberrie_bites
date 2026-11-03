//! One fresh read policy for direct and list projections. No mutation entrypoint.
use crate::{
    age_policy::{self, ClaimReader, Context, PolicyError},
    Clock,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_domain::intro_policy::{IntroRecord, OwnerIntro, PublicIntroContent};
use concert_domain::{AuthUserId, Revision};
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ReadKind {
    Public,
    Own,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ReadTarget {
    Collection,
    Intro {
        intro_id: Uuid,
        subject: AuthUserId,
        revision: Revision,
        kind: ReadKind,
    },
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ReadQuery {
    pub context: Context,
    pub target: ReadTarget,
    pub requested_at: DateTime<Utc>,
}
/// Implementors hold real read-authority stability as required until projection
/// finishes, including session/event, both memberships/restrictions, bidirectional
/// blocks, and resource state/revision. Collection reads attest the current actor
/// and event even for empty pages. This is NOT a remote mutation fence.
pub trait ReadLease: Send + Sync {
    fn check(&self, query: ReadQuery, now: DateTime<Utc>) -> Result<(), PolicyError>;
}
#[async_trait]
pub trait IntroAuthority: Send + Sync {
    async fn read(&self, query: ReadQuery) -> Result<Box<dyn ReadLease>, PolicyError>;
}
pub struct UnavailableIntroAuthority;
#[async_trait]
impl IntroAuthority for UnavailableIntroAuthority {
    async fn read(&self, _: ReadQuery) -> Result<Box<dyn ReadLease>, PolicyError> {
        Err(PolicyError::Unavailable)
    }
}
struct Prepared {
    view: PublicIntroContent,
    query: ReadQuery,
    lease: Box<dyn ReadLease>,
    valid_until: DateTime<Utc>,
    age: Option<age_policy::CurrentAge>,
}
impl Prepared {
    fn check(&self, now: DateTime<Utc>) -> Result<(), PolicyError> {
        self.query.context.check(now)?;
        self.lease.check(self.query, now)?;
        if let Some(age) = &self.age {
            age.check(now)?;
        }
        if self.valid_until <= now {
            Err(PolicyError::Denied)
        } else {
            Ok(())
        }
    }
}
pub struct IntroPolicy<R, A, C> {
    claims: R,
    authority: A,
    clock: C,
}
impl<R: ClaimReader, A: IntroAuthority, C: Clock> IntroPolicy<R, A, C> {
    pub fn new(claims: R, authority: A, clock: C) -> Self {
        Self {
            claims,
            authority,
            clock,
        }
    }
    async fn admit(
        &self,
        context: Context,
        intro: &IntroRecord,
        kind: ReadKind,
    ) -> Result<(ReadQuery, Box<dyn ReadLease>), PolicyError> {
        let now = self.clock.now();
        context.check(now)?;
        if intro.event_id != context.event
            || intro.intro_id.is_nil()
            || intro.owner_id.uuid().is_nil()
        {
            return Err(PolicyError::Denied);
        }
        if kind == ReadKind::Own && context.actor != intro.owner_id {
            return Err(PolicyError::Denied);
        }
        let query = ReadQuery {
            context,
            target: ReadTarget::Intro {
                intro_id: intro.intro_id,
                subject: intro.owner_id,
                revision: intro.revision,
                kind,
            },
            requested_at: now,
        };
        let lease = self.authority.read(query).await?;
        lease.check(query, self.clock.now())?;
        Ok((query, lease))
    }
    async fn prepare(
        &self,
        context: Context,
        intro: &IntroRecord,
    ) -> Result<Prepared, PolicyError> {
        let (query, lease) = self.admit(context, intro, ReadKind::Public).await?;
        let age = if intro.age_bounds.restricted() {
            Some(age_policy::current(&self.claims, &self.clock, context, context.actor).await?)
        } else {
            None
        };
        let now = self.clock.now();
        context.check(now)?;
        lease.check(query, now)?;
        if !intro.visible(context.event, now, age.as_ref().map(|a| &a.assessment)) {
            return Err(PolicyError::Denied);
        }
        let mut valid_until = intro
            .expires_at
            .unwrap_or(context.expires_at)
            .min(context.expires_at);
        if let Some(age) = &age {
            valid_until = valid_until.min(age.valid_until);
        }
        let prepared = Prepared {
            view: intro.public_content(),
            query,
            lease,
            valid_until,
            age,
        };
        prepared.check(now)?;
        Ok(prepared)
    }
    pub async fn direct(
        &self,
        context: Context,
        intro: &IntroRecord,
    ) -> Result<PublicIntroContent, PolicyError> {
        let prepared = self.prepare(context, intro).await?;
        prepared.check(self.clock.now())?;
        Ok(prepared.view)
    }
    pub async fn list(
        &self,
        context: Context,
        intros: &[IntroRecord],
    ) -> Result<Vec<PublicIntroContent>, PolicyError> {
        let requested_at = self.clock.now();
        context.check(requested_at)?;
        if intros.len() > 100 {
            return Err(PolicyError::Unavailable);
        }
        let query = ReadQuery {
            context,
            target: ReadTarget::Collection,
            requested_at,
        };
        let collection = self.authority.read(query).await?;
        collection.check(query, self.clock.now())?;
        let mut prepared = vec![];
        for intro in intros {
            match self.prepare(context, intro).await {
                Ok(view) => prepared.push(view),
                Err(PolicyError::Denied) => (),
                Err(e) => return Err(e),
            }
        }
        let now = self.clock.now();
        context.check(now)?;
        collection.check(query, now)?;
        let mut included = vec![];
        for item in &prepared {
            match item.check(now) {
                Ok(()) => included.push(true),
                Err(PolicyError::Denied) => included.push(false),
                Err(e) => return Err(e),
            }
        }
        // Keep every lease alive through final checks; private bounds never leave.
        Ok(prepared
            .into_iter()
            .zip(included)
            .filter_map(|(p, keep)| keep.then_some(p.view))
            .collect())
    }
    pub async fn own(
        &self,
        context: Context,
        intro: &IntroRecord,
    ) -> Result<OwnerIntro, PolicyError> {
        let (query, lease) = self.admit(context, intro, ReadKind::Own).await?;
        let now = self.clock.now();
        context.check(now)?;
        lease.check(query, now)?;
        Ok(intro.owner_projection())
    }
}
