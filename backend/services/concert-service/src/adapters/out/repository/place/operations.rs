//! BE38 place-owner port. Released catalog metadata remains immutable.
use super::{place, PLACE_SELECT};
use crate::adapters::out::repository::event_operations::{
    self as ops, AdminPlace, FencedOperationsAuthority, FencedPlaceOperations,
};
use async_trait::async_trait;
use concert_application::{
    admin::{places::PlaceChange, Action, Actor, AuthorityQuery},
    commands::{CommandError, SafeFailure},
};
use concert_domain::{EventPhase, Revision};
use sqlx::{PgConnection, Row};
use uuid::Uuid;

/// Uses the same qualified authority owner as the calling BE38 producer. This
/// port repeats current operator authorization, never accepts a boolean permit.
/// Caller MUST retain its transaction through audit, BE15 receipt/outbox and commit.
pub struct PgPlaceOperations<A> {
    authority: A,
}
impl<A: FencedOperationsAuthority> PgPlaceOperations<A> {
    pub fn new(authority: A) -> Self {
        Self { authority }
    }
}
fn select() -> String {
    PLACE_SELECT.replacen(" from ", ",s.revision as place_revision from ", 1)
}
#[async_trait]
impl<A: FencedOperationsAuthority> FencedPlaceOperations for PgPlaceOperations<A> {
    async fn list_in(
        &self,
        c: &mut PgConnection,
        actor: Actor,
        after: Option<Uuid>,
        closed: Option<bool>,
        limit: u16,
    ) -> Result<Vec<AdminPlace>, CommandError> {
        if !(1..=100).contains(&limit) {
            return Err(CommandError::InvalidInput);
        }
        ops::authorize(
            c,
            &self.authority,
            AuthorityQuery {
                actor,
                action: Action::PlacesRead,
                target: actor.event,
            },
            false,
        )
        .await?;
        let head=sqlx::query("select map_id,map_revision from concert_events_map_releases where event_id=$1 for share").bind(actor.event).fetch_optional(&mut *c).await.map_err(ops::unavailable)?.ok_or(CommandError::Unavailable)?;
        let map: String = head.try_get("map_id").map_err(ops::unavailable)?;
        let revision: i64 = head.try_get("map_revision").map_err(ops::unavailable)?;
        let rows=sqlx::query(&format!("{} where q.map_id=$1 and q.map_revision=$2 and ($3::uuid is null or q.place_id>$3) and ($4::boolean is null or (s.closure_state='CLOSED')=$4) order by q.place_id limit $5 for share of s",select())).bind(map).bind(revision).bind(after).bind(closed).bind(i64::from(limit)).fetch_all(c).await.map_err(ops::unavailable)?;
        rows.iter()
            .map(|r| {
                Ok(AdminPlace {
                    place: place(r).map_err(|_| CommandError::InvalidState)?,
                    revision: Revision::new(r.try_get("place_revision").map_err(ops::unavailable)?)
                        .map_err(|_| CommandError::InvalidState)?,
                })
            })
            .collect()
    }
    async fn update_in(
        &self,
        c: &mut PgConnection,
        actor: Actor,
        change: &PlaceChange,
    ) -> Result<Result<AdminPlace, SafeFailure>, CommandError> {
        change.validate()?;
        let (phase, _) = ops::authorize(
            c,
            &self.authority,
            AuthorityQuery {
                actor,
                action: Action::PlaceWrite,
                target: change.place.id.uuid(),
            },
            true,
        )
        .await?;
        if matches!(phase, EventPhase::ReadOnly | EventPhase::Archived) {
            return Ok(Err(SafeFailure::EventReadOnly));
        }
        // Same global release publication lock as PgPlaceRepository::publish_release.
        // Inspect without a lock, acquire it before binding SHARE, then recheck to
        // avoid inversion with a publisher holding release lock then binding UPDATE.
        let candidate = sqlx::query(
            "select map_id,map_revision from concert_events_map_releases where event_id=$1",
        )
        .bind(actor.event)
        .fetch_optional(&mut *c)
        .await
        .map_err(ops::unavailable)?;
        let Some(candidate) = candidate else {
            return Ok(Err(SafeFailure::ResourceUnavailable));
        };
        let candidate_map: String = candidate.try_get("map_id").map_err(ops::unavailable)?;
        let candidate_revision: i64 = candidate
            .try_get("map_revision")
            .map_err(ops::unavailable)?;
        sqlx::query("select pg_advisory_xact_lock(hashtextextended($1::text,0))")
            .bind(format!("{}:{}", candidate_map, candidate_revision))
            .execute(&mut *c)
            .await
            .map_err(ops::unavailable)?;
        let head=sqlx::query("select map_id,map_revision from concert_events_map_releases where event_id=$1 for share").bind(actor.event).fetch_optional(&mut *c).await.map_err(ops::unavailable)?;
        let Some(head) = head else {
            return Ok(Err(SafeFailure::ResourceUnavailable));
        };
        let map: String = head.try_get("map_id").map_err(ops::unavailable)?;
        let map_revision: i64 = head.try_get("map_revision").map_err(ops::unavailable)?;
        if map != candidate_map || map_revision != candidate_revision {
            return Ok(Err(SafeFailure::MapVersionMismatch));
        }
        // State currently belongs to a map release. Mutating a release shared by
        // multiple events would silently affect another event. Fail closed until an
        // owning event-specific release is bound; never change another event's state.
        let bindings: i64 = sqlx::query_scalar(
            "select count(*) from concert_events_map_releases where map_id=$1 and map_revision=$2",
        )
        .bind(&map)
        .bind(map_revision)
        .fetch_one(&mut *c)
        .await
        .map_err(ops::unavailable)?;
        if bindings != 1 {
            return Ok(Err(SafeFailure::MapVersionMismatch));
        }
        let row = sqlx::query(&format!(
            "{} where q.map_id=$1 and q.map_revision=$2 and q.place_id=$3 for update of s",
            select()
        ))
        .bind(&map)
        .bind(map_revision)
        .bind(change.place.id.uuid())
        .fetch_optional(&mut *c)
        .await
        .map_err(ops::unavailable)?;
        let Some(row) = row else {
            return Ok(Err(SafeFailure::ResourceUnavailable));
        };
        let old = place(&row).map_err(|_| CommandError::InvalidState)?;
        let revision = Revision::new(row.try_get("place_revision").map_err(ops::unavailable)?)
            .map_err(|_| CommandError::InvalidState)?;
        if revision != change.expected || revision.next().is_err() {
            return Ok(Err(SafeFailure::VersionConflict));
        }
        let mut proposed = change.place.clone();
        proposed.closure = old.closure;
        // Accessibility/provenance/label changes require a new reviewed immutable
        // release, not an in-place rewrite of the source catalog.
        if proposed != old {
            return Ok(Err(SafeFailure::MapVersionMismatch));
        }
        let time = ops::now(c).await?;
        if change
            .place
            .provenance
            .reviewed_at
            .is_some_and(|at| at > time)
        {
            return Err(CommandError::Unavailable);
        }
        let result=sqlx::query("update concert_place_current_state set closure_state=$4,revision=revision+1 where map_id=$1 and map_revision=$2 and place_id=$3 and revision=$5").bind(map).bind(map_revision).bind(change.place.id.uuid()).bind(change.place.closure.as_str()).bind(revision.get()).execute(&mut *c).await.map_err(ops::unavailable)?;
        if result.rows_affected() != 1 {
            return Ok(Err(SafeFailure::VersionConflict));
        }
        Ok(Ok(AdminPlace {
            place: change.place.clone(),
            revision: revision.next().map_err(|_| CommandError::InvalidState)?,
        }))
    }
}
