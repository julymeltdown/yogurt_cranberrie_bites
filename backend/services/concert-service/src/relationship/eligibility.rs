//! Transaction-local BE29/30 introduction and canonical-claim eligibility.
//! This does not register a route, provision a provider or create owner authority.
use super::{social::SocialEligibility, FencedRelationshipAuthority};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::age_policy::Context;
use concert_domain::{
    age_policy::{assess, AgeBounds, OwnerClaim, StoredStatus},
    relationship::{
        social::{FreshSharing, SharingContext},
        Field, PolicyQuery, RelationshipError,
    },
    AuthUserId,
};
use sqlx::{PgConnection, Row};

/// Trusted verifier configuration/availability, not a browser assertion. The
/// source implementation must bind the exact request and retain any revocation
/// fence through this transaction. No positive production fallback is provided.
#[async_trait]
pub trait SharingVerifierStatus: Send + Sync {
    async fn current_in(
        &self,
        connection: &mut PgConnection,
        request: Context,
    ) -> Result<VerifierStatus, RelationshipError>;
}
#[derive(PartialEq, Eq)]
pub struct VerifierStatus {
    pub verifier: String,
    pub source_revision: String,
    pub valid_until: DateTime<Utc>,
}
pub struct UnavailableSharingVerifier;
#[async_trait]
impl SharingVerifierStatus for UnavailableSharingVerifier {
    async fn current_in(
        &self,
        _: &mut PgConnection,
        _: Context,
    ) -> Result<VerifierStatus, RelationshipError> {
        Err(RelationshipError::Unavailable)
    }
}

pub struct PgSocialEligibility<A, V> {
    request: Context,
    authority: A,
    verifier: V,
}
impl<A: FencedRelationshipAuthority, V: SharingVerifierStatus> PgSocialEligibility<A, V> {
    /// `request` is binding data, never authority. `authority` must independently
    /// verify the authenticated actor/session and registered producer transaction.
    /// Use `from_guarded` for the canonical owner implementation.
    pub fn new(request: Context, authority: A, verifier: V) -> Self {
        Self {
            request,
            authority,
            verifier,
        }
    }
}
impl<V: SharingVerifierStatus>
    PgSocialEligibility<crate::adapters::out::relationship_authority::guarded::GuardedAuthority, V>
{
    pub fn from_guarded(
        scope: std::sync::Arc<
            crate::adapters::out::relationship_authority::guarded::OwnerGuardScope,
        >,
        verifier: V,
    ) -> Result<Self, RelationshipError> {
        use crate::adapters::out::relationship_authority::guarded::GuardedAuthority;
        let binding = scope.binding();
        binding
            .check()
            .map_err(|_| RelationshipError::Unavailable)?;
        let request = Context {
            actor: AuthUserId::from_uuid(binding.actor()),
            event: concert_domain::EventId::from_uuid(binding.event()),
            session: binding.session_family(),
            request: binding.request_id(),
            expires_at: DateTime::from_timestamp_micros(binding.deadline().expires_micros())
                .ok_or(RelationshipError::Unavailable)?,
        };
        Ok(Self::new(request, GuardedAuthority::new(scope), verifier))
    }
}

#[async_trait]
impl<A: FencedRelationshipAuthority, V: SharingVerifierStatus> SocialEligibility
    for PgSocialEligibility<A, V>
{
    async fn check_in(
        &self,
        c: &mut PgConnection,
        sharing: SharingContext,
        now: DateTime<Utc>,
    ) -> Result<FreshSharing, RelationshipError> {
        self.request
            .check(now)
            .map_err(|_| RelationshipError::Unavailable)?;
        if sharing.event != self.request.event
            || sharing.requester == sharing.recipient
            || sharing.intro_id.is_nil()
            || sharing.requester.uuid().is_nil()
            || sharing.recipient.uuid().is_nil()
            || ![sharing.requester, sharing.recipient].contains(&self.request.actor)
        {
            return Err(RelationshipError::Denied);
        }
        // Bound SQL occupancy by the original request and preserve any shorter
        // existing timeout. The final database-time check still rejects expiry.
        let remaining = (self.request.expires_at - Utc::now())
            .num_milliseconds()
            .clamp(1, 5000);
        sqlx::query("select set_config('lock_timeout',(extract(epoch from least(coalesce(nullif(current_setting('lock_timeout')::interval,interval '0'),interval '5 seconds'),$1::bigint*interval '1 millisecond'))*1000)::bigint::text||'ms',true),set_config('statement_timeout',(extract(epoch from least(coalesce(nullif(current_setting('statement_timeout')::interval,interval '0'),interval '5 seconds'),$1::bigint*interval '1 millisecond'))*1000)::bigint::text||'ms',true)")
            .bind(remaining).execute(&mut *c).await.map_err(db)?;
        let subject = if self.request.actor == sharing.requester {
            sharing.recipient
        } else {
            sharing.requester
        };
        let query = PolicyQuery {
            event: sharing.event,
            actor: self.request.actor,
            subject,
            field: Field::Interaction,
        };
        let authority = self.authority.check_in(c, query, now).await?;
        if authority.query != query
            || authority.checked_at != now
            || !authority.actor_allowed
            || !authority.subject_allowed
            || authority.inherited_block
            || authority.valid_until <= now
        {
            return Err(RelationshipError::Denied);
        }
        let status = self.verifier.current_in(c, self.request).await?;
        validate_status(&status, now)?;
        let mut until = self
            .request
            .expires_at
            .min(authority.valid_until)
            .min(status.valid_until);
        // All callers already hold the event lock. Match BE26's owner-before-
        // claim order, and sort both users before locking either canonical root.
        let mut users = [sharing.requester, sharing.recipient];
        users.sort_by_key(|u| u.uuid());
        for user in users {
            // Absence is unknown; do not create an invented verified owner row.
            let owner: Option<uuid::Uuid> = sqlx::query_scalar(
                "select user_id from public.concert_age_owners where user_id=$1 for share",
            )
            .bind(user.uuid())
            .fetch_optional(&mut *c)
            .await
            .map_err(db)?;
            if owner != Some(user.uuid()) {
                return Err(RelationshipError::Denied);
            }
        }
        let intro=sqlx::query("select owner_id,revision,state,minimum_age,maximum_age_exclusive,expires_at from public.concert_intros where event_id=$1 and intro_id=$2 for share")
            .bind(sharing.event.uuid()).bind(sharing.intro_id).fetch_optional(&mut *c).await.map_err(db)?.ok_or(RelationshipError::Denied)?;
        if intro.try_get::<uuid::Uuid, _>("owner_id").map_err(db)? != sharing.recipient.uuid()
            || intro.try_get::<i64, _>("revision").map_err(db)? != sharing.intro_revision.get()
            || intro.try_get::<String, _>("state").map_err(db)? != "PUBLISHED"
        {
            return Err(RelationshipError::Denied);
        }
        if let Some(expiry) = intro
            .try_get::<Option<DateTime<Utc>>, _>("expires_at")
            .map_err(db)?
        {
            until = until.min(expiry);
        }
        let bounds = AgeBounds::new(
            intro.try_get("minimum_age").map_err(db)?,
            intro.try_get("maximum_age_exclusive").map_err(db)?,
        )
        .map_err(|_| RelationshipError::Unavailable)?;
        let unbounded = AgeBounds::new(None, None).map_err(|_| RelationshipError::Unavailable)?;
        for user in users {
            // Canonical selection precedes status/expiry/event/provenance tests.
            // Filtering first would revive an older proof after replacement.
            let row=sqlx::query("select * from public.concert_age_claims where user_id=$1 order by verified_at desc,expires_at desc,claim_id desc limit 1 for share")
                .bind(user.uuid()).fetch_optional(&mut *c).await.map_err(db)?.ok_or(RelationshipError::Denied)?;
            let claim = owner_claim(row)?;
            if claim.verifier_id != status.verifier
                || claim.source_revision != status.source_revision
            {
                return Err(RelationshipError::Denied);
            }
            let assessment =
                assess(user, &[claim], now).map_err(|_| RelationshipError::Unavailable)?;
            // The introduction's audience bounds apply to its requester. Both
            // participants require verified age even for an unbounded offer.
            let audience = if user == sharing.requester {
                bounds
            } else {
                unbounded
            };
            if !audience.permits(&assessment, true) {
                return Err(RelationshipError::Denied);
            }
            until = until.min(assessment.recheck_at().ok_or(RelationshipError::Denied)?);
        }
        let final_status = self.verifier.current_in(c, self.request).await?;
        if final_status != status {
            return Err(RelationshipError::Unavailable);
        }
        let finished: DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
            .fetch_one(&mut *c)
            .await
            .map_err(db)?;
        self.request
            .check(finished)
            .map_err(|_| RelationshipError::Unavailable)?;
        if finished < now || until <= finished {
            return Err(RelationshipError::Denied);
        }
        // SocialService pins this earliest deadline into its deferred COMMIT
        // guard. All acquired source locks stay in the caller's transaction.
        Ok(FreshSharing {
            context: sharing,
            checked_at: now,
            expires_at: until,
            allowed: true,
        })
    }
}
fn validate_status(s: &VerifierStatus, now: DateTime<Utc>) -> Result<(), RelationshipError> {
    let identifier = |v: &str| {
        !v.is_empty()
            && v.len() <= 100
            && v.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    };
    if !identifier(&s.verifier) || !identifier(&s.source_revision) || s.valid_until <= now {
        Err(RelationshipError::Unavailable)
    } else {
        Ok(())
    }
}
// Mapping follows BE26 claim_reader.rs at the task base. No raw proof, exact age
// or provenance is returned to a public DTO or included in diagnostics.
fn owner_claim(row: sqlx::postgres::PgRow) -> Result<OwnerClaim, RelationshipError> {
    let status = match row.try_get::<String, _>("status").map_err(db)?.as_str() {
        "VERIFIED" | "EXPIRED" => StoredStatus::Verified,
        "REPLACED" => StoredStatus::Replaced,
        "REVOKED" => StoredStatus::Revoked,
        "UNSUPPORTED" => StoredStatus::Unsupported,
        _ => return Err(RelationshipError::Unavailable),
    };
    Ok(OwnerClaim {
        claim_id: row.try_get("claim_id").map_err(db)?,
        user_id: AuthUserId::from_uuid(row.try_get("user_id").map_err(db)?),
        verifier_id: row.try_get("verifier_id").map_err(db)?,
        source_revision: row.try_get("source_revision").map_err(db)?,
        status,
        verified_at: row.try_get("verified_at").map_err(db)?,
        expires_at: row.try_get("expires_at").map_err(db)?,
        age_at_verification: row.try_get("age_at_verification").map_err(db)?,
        next_birthday_at: row.try_get("next_birthday_at").map_err(db)?,
    })
}
fn db(_: sqlx::Error) -> RelationshipError {
    RelationshipError::Unavailable
}
