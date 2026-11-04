//! C04 descending creation-order scans. Retained mutations remain the business writer.
use super::{
    cursor::{Collection, CursorBinding, CursorCodec, CursorError, Position, Window},
    protocol::GiftIdentity,
};
use crate::community::{
    giveaway::{GiftAuthority, GiftFilter, GiftServiceError, GiftSummary, PgGifts},
    CommunityAccess,
};
use chrono::{DateTime, Utc};
use concert_application::commands::CommandError;
use concert_domain::community::giveaway::{Giveaway, Reservation, ReservationState};
use sqlx::{postgres::PgRow, PgConnection, PgPool, Row};
use std::collections::HashSet;
use uuid::Uuid;

pub(crate) struct Page<T> {
    pub items: Vec<T>,
    pub next: Option<Window>,
}

#[cfg(test)]
#[path = "native_queries.rs"]
pub(crate) mod native_tests;
pub(crate) struct ReservationItem {
    pub giveaway: Uuid,
    pub value: Reservation,
}
pub(crate) struct Management {
    pub giveaway: GiftSummary,
    pub reserved: i32,
    pub claimed: i32,
    pub reservations: Vec<Reservation>,
    pub next: Option<Window>,
}
pub(crate) struct GiftReads<'a, A> {
    pub pool: &'a PgPool,
    pub authority: &'a A,
    pub gifts: &'a PgGifts<A>,
    pub identity: GiftIdentity,
    pub cursor: &'a CursorCodec,
}
fn db(_: sqlx::Error) -> GiftServiceError {
    CommandError::Unavailable.into()
}
fn cursor(e: CursorError) -> GiftServiceError {
    match e {
        CursorError::Invalid => CommandError::InvalidInput,
        CursorError::Unavailable => CommandError::Unavailable,
    }
    .into()
}
pub(crate) fn parse(row: &PgRow, event: Uuid) -> Result<Giveaway, GiftServiceError> {
    let g: Giveaway = serde_json::from_value(row.try_get("aggregate").map_err(db)?)
        .map_err(|_| CommandError::Unavailable)?;
    g.validate().map_err(|_| CommandError::Unavailable)?;
    let state: String = row.try_get("state").map_err(db)?;
    if g.reservations.len() > 10000
        || g.event != event
        || g.id != row.try_get::<Uuid, _>("giveaway_id").map_err(db)?
        || g.owner != row.try_get::<Uuid, _>("owner_id").map_err(db)?
        || g.revision != row.try_get::<i64, _>("revision").map_err(db)?
        || serde_json::to_value(g.state).map_err(|_| CommandError::Unavailable)? != state
        || g.fields.quantity_total != row.try_get::<i32, _>("quantity_total").map_err(db)?
        || g.reserved_unclaimed != row.try_get::<i32, _>("reserved_unclaimed").map_err(db)?
        || g.quantity_claimed != row.try_get::<i32, _>("quantity_claimed").map_err(db)?
        || [g.id, g.event, g.owner].iter().any(Uuid::is_nil)
    {
        return Err(CommandError::Unavailable.into());
    }
    Ok(g)
}
impl<'a, A: GiftAuthority> GiftReads<'a, A> {
    async fn now(&self, c: &mut PgConnection) -> Result<DateTime<Utc>, GiftServiceError> {
        sqlx::query_scalar("select clock_timestamp()")
            .fetch_one(c)
            .await
            .map_err(db)
    }
    async fn bounds(
        &self,
        c: &mut PgConnection,
        b: &CursorBinding,
        token: Option<&str>,
    ) -> Result<(DateTime<Utc>, Option<Position>), GiftServiceError> {
        if (b.actor, b.event) != (self.identity.actor, self.identity.event)
            || !(1..=100).contains(&b.limit)
        {
            return Err(CommandError::InvalidInput.into());
        }
        let now = self.now(c).await?;
        match token {
            Some(t) => {
                let w = self.cursor.open(b, t, now).map_err(cursor)?;
                Ok((w.upper, Some(w.position)))
            }
            None => Ok((now, None)),
        }
    }
    pub async fn root_for_reservation(
        &self,
        id: Uuid,
        safety: bool,
        owner_only: bool,
        recipient_only: bool,
    ) -> Result<Uuid, GiftServiceError> {
        let i = self.identity;
        let access = if safety {
            CommunityAccess::SafetyGifts
        } else {
            CommunityAccess::ReadGifts
        };
        let mut tx = self.pool.begin().await.map_err(db)?;
        self.authority
            .actor_in(&mut tx, i.actor, i.event, access)
            .await?;
        // An unlocked internal ID hint is not authority. Lock the actual Gift
        // root first, then recheck/lock the reservation, matching retained writes.
        let root:Uuid=sqlx::query_scalar("select giveaway_id from concert_community_reservations where event_id=$1 and reservation_id=$2").bind(i.event).bind(id).fetch_optional(&mut *tx).await.map_err(db)?.ok_or(CommandError::Denied)?;
        let owner:Uuid=sqlx::query_scalar("select owner_id from concert_community_giveaways where event_id=$1 and giveaway_id=$2 for share").bind(i.event).bind(root).fetch_optional(&mut *tx).await.map_err(db)?.ok_or(CommandError::Denied)?;
        let recipient:Uuid=sqlx::query_scalar("select recipient_id from concert_community_reservations where event_id=$1 and giveaway_id=$2 and reservation_id=$3 for share").bind(i.event).bind(root).bind(id).fetch_optional(&mut *tx).await.map_err(db)?.ok_or(CommandError::Denied)?;
        if (i.actor != owner && i.actor != recipient)
            || (owner_only && i.actor != owner)
            || (recipient_only && i.actor != recipient)
        {
            return Err(CommandError::Denied.into());
        }
        let mut subjects = vec![owner, recipient];
        subjects.sort();
        subjects.dedup();
        for subject in subjects {
            self.authority
                .authorize_in(&mut tx, i.actor, i.event, subject, access)
                .await?;
        }
        tx.commit().await.map_err(db)?;
        Ok(root)
    }
    pub async fn list(
        &self,
        b: &CursorBinding,
        token: Option<&str>,
        filter: GiftFilter,
    ) -> Result<Page<GiftSummary>, GiftServiceError> {
        if !matches!(b.collection, Collection::Public | Collection::Giving) {
            return Err(CommandError::InvalidInput.into());
        }
        let i = self.identity;
        let mut tx = self.pool.begin().await.map_err(db)?;
        self.authority
            .actor_in(&mut tx, i.actor, i.event, CommunityAccess::ReadGifts)
            .await?;
        let (upper, after) = self.bounds(&mut tx, b, token).await?;
        let (after_time, after_id) = match after {
            Some(Position::Gift { created_at, id }) => (Some(created_at), Some(id)),
            None => (None, None),
            _ => return Err(CommandError::InvalidInput.into()),
        };
        let giving = b.collection == Collection::Giving;
        let state = filter
            .state
            .map(|s| {
                serde_json::to_value(s)
                    .ok()
                    .and_then(|s| s.as_str().map(str::to_owned))
                    .ok_or(CommandError::Unavailable)
            })
            .transpose()?;
        let mut rows=sqlx::query("select g.* from concert_community_giveaways g where event_id=$1 and (($2 and owner_id=$3) or (not $2 and state<>'DRAFT')) and ($4::text is null or state=$4) and ($5::boolean is null or (quantity_total-reserved_unclaimed-quantity_claimed>0)=$5) and ($6::timestamptz is null or (aggregate->'fields'->>'pickup_starts_at')::timestamptz>=$6) and ($7::timestamptz is null or (aggregate->'fields'->>'pickup_starts_at')::timestamptz<$7) and ($8::uuid is null or (aggregate->'fields'->>'public_place_id')::uuid=$8) and (aggregate->>'created_at')::timestamptz<=$9 and ($10::timestamptz is null or ((aggregate->>'created_at')::timestamptz,giveaway_id)<($10,$11)) order by (aggregate->>'created_at')::timestamptz desc,giveaway_id desc limit $12 for share of g")
            .bind(i.event).bind(giving).bind(i.actor).bind(state).bind(filter.in_stock).bind(filter.pickup_after).bind(filter.pickup_before).bind(filter.public_place_id).bind(upper).bind(after_time).bind(after_id).bind(i64::from(b.limit)+1).fetch_all(&mut *tx).await.map_err(db)?;
        let more = rows.len() > usize::from(b.limit);
        rows.truncate(usize::from(b.limit));
        let gifts = rows
            .iter()
            .map(|r| parse(r, i.event))
            .collect::<Result<Vec<_>, _>>()?;
        let next = if more {
            gifts.last().map(|g| Window {
                upper,
                position: Position::Gift {
                    created_at: g.created_at,
                    id: g.id,
                },
            })
        } else {
            None
        };
        let mut subjects = gifts.iter().map(|g| g.owner).collect::<Vec<_>>();
        subjects.sort();
        subjects.dedup();
        let mut allowed = HashSet::new();
        for subject in subjects {
            match self
                .authority
                .authorize_in(
                    &mut tx,
                    i.actor,
                    i.event,
                    subject,
                    CommunityAccess::ReadGifts,
                )
                .await
            {
                Ok(()) => {
                    allowed.insert(subject);
                }
                Err(CommandError::Denied) => {}
                Err(e) => return Err(e.into()),
            }
        }
        let items = gifts
            .iter()
            .filter(|g| allowed.contains(&g.owner))
            .map(GiftSummary::from)
            .collect();
        tx.commit().await.map_err(db)?;
        Ok(Page { items, next })
    }
    pub async fn receiving(
        &self,
        b: &CursorBinding,
        token: Option<&str>,
    ) -> Result<Page<ReservationItem>, GiftServiceError> {
        if b.collection != Collection::Receiving {
            return Err(CommandError::InvalidInput.into());
        }
        let i = self.identity;
        let mut tx = self.pool.begin().await.map_err(db)?;
        self.authority
            .actor_in(&mut tx, i.actor, i.event, CommunityAccess::ReadGifts)
            .await?;
        let (upper, after) = self.bounds(&mut tx, b, token).await?;
        let (after_time, after_id) = match after {
            Some(Position::Reservation { created_at, id }) => (Some(created_at), Some(id)),
            None => (None, None),
            _ => return Err(CommandError::InvalidInput.into()),
        };
        let mut rows=sqlx::query("select r.giveaway_id,r.reservation_id,(j.value->>'created_at')::timestamptz created_at from concert_community_reservations r join concert_community_giveaways g using(event_id,giveaway_id) cross join lateral jsonb_array_elements(g.aggregate->'reservations') j where r.event_id=$1 and r.recipient_id=$2 and (j.value->>'id')::uuid=r.reservation_id and (j.value->>'created_at')::timestamptz<=$3 and ($4::timestamptz is null or ((j.value->>'created_at')::timestamptz,r.reservation_id)<($4,$5)) order by (j.value->>'created_at')::timestamptz desc,r.reservation_id desc limit $6")
            .bind(i.event).bind(i.actor).bind(upper).bind(after_time).bind(after_id).bind(i64::from(b.limit)+1).fetch_all(&mut *tx).await.map_err(db)?;
        let more = rows.len() > usize::from(b.limit);
        rows.truncate(usize::from(b.limit));
        let next = if more {
            rows.last()
                .map(|r| -> Result<Window, GiftServiceError> {
                    Ok(Window {
                        upper,
                        position: Position::Reservation {
                            created_at: r.try_get("created_at").map_err(db)?,
                            id: r.try_get("reservation_id").map_err(db)?,
                        },
                    })
                })
                .transpose()?
        } else {
            None
        };
        tx.commit().await.map_err(db)?;
        let mut items = vec![];
        for row in rows {
            let giveaway = row.try_get("giveaway_id").map_err(db)?;
            let id = row.try_get("reservation_id").map_err(db)?;
            match self.gifts.reservation(i.actor, i.event, giveaway, id).await {
                Ok(value) => items.push(ReservationItem { giveaway, value }),
                Err(GiftServiceError::Command(CommandError::Denied)) => {}
                Err(e) => return Err(e),
            }
        }
        Ok(Page { items, next })
    }
    pub async fn owned(&self, id: Uuid) -> Result<Giveaway, GiftServiceError> {
        let i = self.identity;
        let mut tx = self.pool.begin().await.map_err(db)?;
        self.authority
            .actor_in(&mut tx, i.actor, i.event, CommunityAccess::ReadGifts)
            .await?;
        let row=sqlx::query("select * from concert_community_giveaways where event_id=$1 and giveaway_id=$2 and owner_id=$3 for share").bind(i.event).bind(id).bind(i.actor).fetch_optional(&mut *tx).await.map_err(db)?.ok_or(CommandError::Denied)?;
        let g = parse(&row, i.event)?;
        self.authority
            .authorize_in(
                &mut tx,
                i.actor,
                i.event,
                g.owner,
                CommunityAccess::ReadGifts,
            )
            .await?;
        tx.commit().await.map_err(db)?;
        Ok(g)
    }
    pub async fn management(
        &self,
        id: Uuid,
        b: &CursorBinding,
        token: Option<&str>,
        state: Option<ReservationState>,
    ) -> Result<Management, GiftServiceError> {
        if b.collection != Collection::Management || b.giveaway != Some(id) {
            return Err(CommandError::InvalidInput.into());
        }
        let i = self.identity;
        // Reuse the retained durable DB-clock transition before the current read.
        // Bounded at the existing aggregate cap; original scope.run stops all work.
        for _ in 0..=100 {
            let g = self.owned(id).await?;
            let now: DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
                .fetch_one(self.pool)
                .await
                .map_err(db)?;
            let due = g.reservations.iter().find(|r| {
                r.state.active()
                    && (r.expires_at <= now
                        || (r.state == ReservationState::Held && g.fields.pickup_starts_at <= now))
            });
            if let Some(r) = due {
                self.gifts.reservation(i.actor, i.event, id, r.id).await?;
            } else {
                break;
            }
        }
        let mut tx = self.pool.begin().await.map_err(db)?;
        self.authority
            .actor_in(&mut tx, i.actor, i.event, CommunityAccess::ReadGifts)
            .await?;
        let (upper, after) = self.bounds(&mut tx, b, token).await?;
        let after = match after {
            Some(Position::Reservation { created_at, id }) => Some((created_at, id)),
            None => None,
            _ => return Err(CommandError::InvalidInput.into()),
        };
        let row=sqlx::query("select * from concert_community_giveaways where event_id=$1 and giveaway_id=$2 and owner_id=$3 for share").bind(i.event).bind(id).bind(i.actor).fetch_optional(&mut *tx).await.map_err(db)?.ok_or(CommandError::Denied)?;
        let g = parse(&row, i.event)?;
        let now = self.now(&mut tx).await?;
        if g.reservations.iter().any(|r| {
            r.state.active()
                && (r.expires_at <= now
                    || (r.state == ReservationState::Held && g.fields.pickup_starts_at <= now))
        }) {
            // The bounded maintenance loop must not silently certify stale
            // counts/state when another reservation becomes due at its edge.
            return Err(CommandError::Unavailable.into());
        }
        let mut selected = g
            .reservations
            .iter()
            .filter(|r| {
                r.created_at <= upper
                    && after.is_none_or(|p| (r.created_at, r.id) < p)
                    && state.is_none_or(|s| r.state == s)
            })
            .cloned()
            .collect::<Vec<_>>();
        selected.sort_by_key(|r| std::cmp::Reverse((r.created_at, r.id)));
        let more = selected.len() > usize::from(b.limit);
        selected.truncate(usize::from(b.limit));
        let next = if more {
            selected.last().map(|r| Window {
                upper,
                position: Position::Reservation {
                    created_at: r.created_at,
                    id: r.id,
                },
            })
        } else {
            None
        };
        let mut subjects = selected.iter().map(|r| r.recipient).collect::<Vec<_>>();
        subjects.push(i.actor);
        subjects.sort();
        subjects.dedup();
        let mut allowed = HashSet::new();
        for subject in subjects {
            match self
                .authority
                .authorize_in(
                    &mut tx,
                    i.actor,
                    i.event,
                    subject,
                    CommunityAccess::ReadGifts,
                )
                .await
            {
                Ok(()) => {
                    allowed.insert(subject);
                }
                Err(CommandError::Denied) if subject != i.actor => {}
                Err(e) => return Err(e.into()),
            }
        }
        let result = Management {
            giveaway: GiftSummary::from(&g),
            reserved: g.reserved_unclaimed,
            claimed: g.quantity_claimed,
            reservations: selected
                .into_iter()
                .filter(|r| allowed.contains(&r.recipient))
                .collect(),
            next,
        };
        tx.commit().await.map_err(db)?;
        Ok(result)
    }
}
