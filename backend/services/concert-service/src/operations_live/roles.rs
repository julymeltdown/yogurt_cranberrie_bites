//! BE38 event-scoped operator-role and reviewed-notice owners. Both lock their
//! durable rows on the caller transaction; a role or review never defaults.
use super::{authority_error, ScopedOperationsRoles};
use crate::adapters::out::repository::event_operations::{
    notice_review_digest, FencedNoticeReview, NoticeReviewLease,
};
use crate::adapters::out::relationship_authority::guarded::OwnerGuardScope;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use concert_application::{
    admin::{notices::NoticeInput, Actor, AuthorityQuery, Role, RoleLease},
    commands::CommandError,
};
use sqlx::{PgConnection, Row};
use std::sync::Arc;
use uuid::Uuid;

const LEASE: Duration = Duration::seconds(30);

pub struct PgOperationsRoles;

impl PgOperationsRoles {
    pub async fn role_in(
        c: &mut PgConnection,
        actor: Actor,
        query: AuthorityQuery,
        now: DateTime<Utc>,
        expires_at: DateTime<Utc>,
    ) -> Result<RoleLease, CommandError> {
        if query.actor != actor
            || actor.actor.is_nil()
            || actor.event.is_nil()
            || query.target.is_nil()
            || expires_at <= now
        {
            return Err(CommandError::Denied);
        }
        let row = sqlx::query(
            "select revision from public.concert_event_operator_roles where event_id=$1 and user_id=$2 and role='OPERATOR' and revoked_at is null for share",
        )
        .bind(actor.event)
        .bind(actor.actor)
        .fetch_optional(c)
        .await
        .map_err(super::unavailable)?
        .ok_or(CommandError::Denied)?;
        let role = Role::Operator;
        let revision: i64 = row.try_get("revision").map_err(super::unavailable)?;
        if !(1..=concert_domain::MAX_REVISION).contains(&revision) || !query.action.permits(role) {
            return Err(CommandError::Denied);
        }
        Ok(RoleLease {
            query,
            role,
            revision,
            checked_at: now,
            expires_at: expires_at.min(now + LEASE),
        })
    }
}

#[async_trait]
impl ScopedOperationsRoles for PgOperationsRoles {
    async fn check_in(
        &self,
        scope: &Arc<OwnerGuardScope>,
        c: &mut PgConnection,
        query: AuthorityQuery,
        now: DateTime<Utc>,
    ) -> Result<RoleLease, CommandError> {
        let binding = scope.binding();
        let ceiling = scope.disclosure_ceiling().map_err(authority_error)?;
        let expires_at = DateTime::from_timestamp_micros(ceiling.expires_micros())
            .ok_or(CommandError::Unavailable)?;
        Self::role_in(
            c,
            Actor {
                actor: binding.actor(),
                event: binding.event(),
            },
            query,
            now,
            expires_at,
        )
        .await
    }
}

pub struct PgOperationsReview;

impl PgOperationsReview {
    pub async fn review_in(
        c: &mut PgConnection,
        actor: Actor,
        notice: Uuid,
        input: &NoticeInput,
        now: DateTime<Utc>,
    ) -> Result<NoticeReviewLease, CommandError> {
        if actor.actor.is_nil() || actor.event.is_nil() || notice.is_nil() {
            return Err(CommandError::Denied);
        }
        let digest = notice_review_digest(input);
        let row = sqlx::query(
            "select revision,reviewed_at from public.concert_operation_notice_reviews where event_id=$1 and input_digest=$2 and source_uri=$3 and reviewed_by=$4 for share",
        )
        .bind(actor.event)
        .bind(digest.as_slice())
        .bind(&input.source_uri)
        .bind(actor.actor)
        .fetch_optional(&mut *c)
        .await
        .map_err(super::unavailable)?
        .ok_or(CommandError::Unavailable)?;
        let revision: i64 = row.try_get("revision").map_err(super::unavailable)?;
        let reviewed_at: DateTime<Utc> = row.try_get("reviewed_at").map_err(super::unavailable)?;
        if !(1..=concert_domain::MAX_REVISION).contains(&revision) || reviewed_at > now {
            return Err(CommandError::Unavailable);
        }
        Ok(NoticeReviewLease {
            actor,
            notice_id: notice,
            input_digest: digest,
            revision,
            checked_at: now,
            expires_at: now + LEASE,
        })
    }
}

#[async_trait]
impl FencedNoticeReview for PgOperationsReview {
    async fn check_in(
        &self,
        c: &mut PgConnection,
        actor: Actor,
        notice: Uuid,
        input: &NoticeInput,
        now: DateTime<Utc>,
    ) -> Result<NoticeReviewLease, CommandError> {
        Self::review_in(c, actor, notice, input, now).await
    }
}

