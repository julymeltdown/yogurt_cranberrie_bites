//! VENU-BE-L01 PostgreSQL adapter. The VENUE_FRIEND_V1 port fails closed until the
//! bilateral Friend read exists; this module never reconstructs it from a directional flag.
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::venue_location::{
    self as app, command_receipt, VenueLocation, VenueLocationPolicyProvider,
};
use concert_domain::venue_location::*;
use concert_domain::{AuthUserId, EventId, EventPhase, ParticipantKey, MAX_REVISION};
use sqlx::{PgConnection, PgPool, Row};
use std::str::FromStr;
use uuid::Uuid;

#[async_trait]
pub trait VenueLocationAuthority: Send + Sync {
    async fn actor_in(
        &self,
        connection: &mut PgConnection,
        actor: ParticipantKey,
        now: DateTime<Utc>,
    ) -> Result<(), VenueLocationError>;
    async fn friend_in(
        &self,
        connection: &mut PgConnection,
        event: EventId,
        viewer: AuthUserId,
        owner: AuthUserId,
        now: DateTime<Utc>,
    ) -> Result<bool, VenueLocationError>;
}

pub struct PgVenueLocationAuthority;
#[async_trait]
impl VenueLocationAuthority for PgVenueLocationAuthority {
    async fn actor_in(
        &self,
        c: &mut PgConnection,
        actor: ParticipantKey,
        _: DateTime<Utc>,
    ) -> Result<(), VenueLocationError> {
        require_actor(c, actor).await
    }
    async fn friend_in(
        &self,
        _: &mut PgConnection,
        _: EventId,
        _: AuthUserId,
        _: AuthUserId,
        _: DateTime<Utc>,
    ) -> Result<bool, VenueLocationError> {
        Err(VenueLocationError::PolicyUnavailable)
    }
}

pub struct PgVenueLocation<P, A> {
    pool: PgPool,
    policy: P,
    authority: A,
}
impl<P: VenueLocationPolicyProvider, A: VenueLocationAuthority> PgVenueLocation<P, A> {
    pub fn new(pool: PgPool, policy: P, authority: A) -> Self {
        Self {
            pool,
            policy,
            authority,
        }
    }
    async fn context(
        &self,
        c: &mut PgConnection,
        actor: ParticipantKey,
    ) -> Result<(DateTime<Utc>, VenueLocationPolicy), VenueLocationError> {
        let now = now_in(c).await?;
        let policy = self.policy.policy(actor.event_id).await?;
        policy.validate(now)?;
        self.authority.actor_in(c, actor, now).await?;
        Ok((now, policy))
    }
}

#[async_trait]
impl<P: VenueLocationPolicyProvider, A: VenueLocationAuthority> VenueLocation
    for PgVenueLocation<P, A>
{
    async fn set_sharing(
        &self,
        actor: ParticipantKey,
        command_key: Uuid,
        command: SharingCommand,
    ) -> Result<CommandReceipt, VenueLocationError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|_| VenueLocationError::DependencyUnavailable)?;
        let (now, policy) = self.context(&mut tx, actor).await?;
        command.validate(&policy)?;
        let current = load_sharing(&mut tx, actor.event_id, actor.user_id, true).await?;
        if current.revision != command.expected_revision() {
            return Err(VenueLocationError::Conflict);
        }
        let revision = current
            .revision
            .checked_add(1)
            .filter(|value| (1..=MAX_REVISION).contains(value))
            .ok_or(VenueLocationError::Conflict)?;
        let expires_at = command.lease_expiry(now, &policy, policy.valid_until)?;
        if command.state() == SharingState::Active {
            for viewer in command.viewer_ids() {
                let viewer = AuthUserId::from_uuid(*viewer);
                if viewer == actor.user_id
                    || !self
                        .authority
                        .friend_in(&mut tx, actor.event_id, viewer, actor.user_id, now)
                        .await?
                {
                    return Err(VenueLocationError::Denied);
                }
            }
            sqlx::query(
                "delete from public.concert_venue_location_viewers where event_id=$1 and owner_id=$2",
            )
            .bind(actor.event_id.uuid())
            .bind(actor.user_id.uuid())
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        } else {
            sqlx::query(
                "delete from public.concert_venue_location_viewers where event_id=$1 and owner_id=$2",
            )
            .bind(actor.event_id.uuid())
            .bind(actor.user_id.uuid())
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
            sqlx::query(
                "delete from public.concert_venue_location_fixes where event_id=$1 and owner_id=$2",
            )
            .bind(actor.event_id.uuid())
            .bind(actor.user_id.uuid())
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        }
        sqlx::query(
            "insert into public.concert_venue_location_sharing(event_id,owner_id,state,revision,expires_at,updated_at)
             values($1,$2,$3,$4,$5,clock_timestamp())
             on conflict(event_id,owner_id) do update set state=excluded.state,revision=excluded.revision,
               expires_at=excluded.expires_at,updated_at=clock_timestamp()",
        )
        .bind(actor.event_id.uuid())
        .bind(actor.user_id.uuid())
        .bind(command.state().as_str())
        .bind(revision)
        .bind(expires_at)
        .execute(&mut *tx)
        .await
        .map_err(storage)?;
        for viewer in command.viewer_ids() {
            sqlx::query(
                "insert into public.concert_venue_location_viewers(event_id,owner_id,viewer_id) values($1,$2,$3)",
            )
            .bind(actor.event_id.uuid())
            .bind(actor.user_id.uuid())
            .bind(viewer)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        }
        let receipt = command_receipt(
            &app::VenueLocationCommand::SetSharing(command),
            command_key,
            actor.user_id,
            revision,
            now,
            &policy,
        )?;
        tx.commit()
            .await
            .map_err(|_| VenueLocationError::DependencyUnavailable)?;
        Ok(receipt)
    }

    async fn publish_fix(
        &self,
        actor: ParticipantKey,
        command_key: Uuid,
        input: PhoneFixInput,
    ) -> Result<CommandReceipt, VenueLocationError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|_| VenueLocationError::DependencyUnavailable)?;
        let (now, policy) = self.context(&mut tx, actor).await?;
        input.validate(now, &policy)?;
        let current = load_sharing(&mut tx, actor.event_id, actor.user_id, true).await?;
        let lease = current
            .expires_at
            .filter(|expires| now < *expires)
            .ok_or(VenueLocationError::Denied)?;
        if current.effective(now) != SharingState::Active
            || current.revision != input.expected_sharing_revision
        {
            return Err(if current.effective(now) != SharingState::Active {
                VenueLocationError::Denied
            } else {
                VenueLocationError::Conflict
            });
        }
        if now >= lease {
            return Err(VenueLocationError::Denied);
        }
        sqlx::query(
            "insert into public.concert_venue_location_fixes(event_id,owner_id,latitude,longitude,accuracy_m,captured_at,received_at,sharing_revision)
             values($1,$2,$3,$4,$5,$6,clock_timestamp(),$7)
             on conflict(event_id,owner_id) do update set latitude=excluded.latitude,longitude=excluded.longitude,
               accuracy_m=excluded.accuracy_m,captured_at=excluded.captured_at,received_at=clock_timestamp(),
               sharing_revision=excluded.sharing_revision",
        )
        .bind(actor.event_id.uuid())
        .bind(actor.user_id.uuid())
        .bind(input.latitude)
        .bind(input.longitude)
        .bind(input.accuracy_m)
        .bind(input.captured_at)
        .bind(current.revision)
        .execute(&mut *tx)
        .await
        .map_err(storage)?;
        let receipt = command_receipt(
            &app::VenueLocationCommand::PublishFix(input),
            command_key,
            actor.user_id,
            current.revision,
            now,
            &policy,
        )?;
        tx.commit()
            .await
            .map_err(|_| VenueLocationError::DependencyUnavailable)?;
        Ok(receipt)
    }

    async fn snapshot(
        &self,
        actor: ParticipantKey,
    ) -> Result<LocationSnapshot, VenueLocationError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|_| VenueLocationError::DependencyUnavailable)?;
        let (now, policy) = self.context(&mut tx, actor).await?;
        let own = load_sharing(&mut tx, actor.event_id, actor.user_id, true).await?;
        let mut access_until = policy.valid_until;
        let self_location = if own.effective(now) == SharingState::Active {
            let expires = own
                .expires_at
                .ok_or(VenueLocationError::PolicyUnavailable)?;
            access_until = access_until.min(expires);
            match load_fix(&mut tx, actor.event_id, actor.user_id).await? {
                Some(fix) => project_phone(
                    now,
                    actor.user_id.uuid(),
                    &fix,
                    own.revision,
                    expires,
                    &policy,
                ),
                None => None,
            }
        } else {
            None
        };
        let owners: Vec<Uuid> = sqlx::query_scalar(
            "select s.owner_id
               from public.concert_venue_location_sharing s
               join public.concert_venue_location_viewers v
                 on v.event_id=s.event_id and v.owner_id=s.owner_id
              where s.event_id=$1 and v.viewer_id=$2 and s.state='ACTIVE' and s.expires_at>$3
              order by s.owner_id",
        )
        .bind(actor.event_id.uuid())
        .bind(actor.user_id.uuid())
        .bind(now)
        .fetch_all(&mut *tx)
        .await
        .map_err(storage)?;
        let mut friends = Vec::new();
        for owner in owners {
            let owner = AuthUserId::from_uuid(owner);
            let lease = load_sharing(&mut tx, actor.event_id, owner, true).await?;
            let expires = match lease.effective(now) {
                SharingState::Active if lease.viewers.contains(&actor.user_id.uuid()) => lease
                    .expires_at
                    .ok_or(VenueLocationError::PolicyUnavailable)?,
                _ => continue,
            };
            if owner == actor.user_id
                || !self
                    .authority
                    .friend_in(&mut tx, actor.event_id, actor.user_id, owner, now)
                    .await?
            {
                continue;
            }
            if let Some(fix) = load_fix(&mut tx, actor.event_id, owner).await? {
                if let Some(visible) =
                    project_phone(now, owner.uuid(), &fix, lease.revision, expires, &policy)
                {
                    access_until = access_until.min(visible.access_until);
                    friends.push(visible);
                }
            }
        }
        let state = own.effective(now);
        tx.commit()
            .await
            .map_err(|_| VenueLocationError::DependencyUnavailable)?;
        Ok(LocationSnapshot {
            event_id: actor.event_id.uuid(),
            actor_id: actor.user_id.uuid(),
            server_time: now,
            access_until,
            clock: VenueClock {
                event_id: actor.event_id.uuid(),
                venue_key: policy.venue_key.clone(),
                policy_revision: policy.revision,
                timezone: policy.timezone.clone(),
                server_time: now,
                end_of_day: seoul_end_of_day(now),
                valid_until: policy.valid_until,
            },
            sharing: own.projection(now),
            self_location,
            friends,
            capabilities: LocationCapabilities {
                can_share_phone: true,
                can_pause: state == SharingState::Active,
                can_revoke: matches!(state, SharingState::Active | SharingState::Paused),
                ..LocationCapabilities::default()
            },
            incoming_requests: Vec::new(),
            outgoing_requests: Vec::new(),
            policy,
        })
    }

    async fn visible_for(
        &self,
        viewer: ParticipantKey,
        owner: AuthUserId,
    ) -> Result<VisibleLocation, VenueLocationError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|_| VenueLocationError::DependencyUnavailable)?;
        let (now, policy) = self.context(&mut tx, viewer).await?;
        if owner == viewer.user_id {
            return Err(VenueLocationError::Denied);
        }
        let sharing = load_sharing(&mut tx, viewer.event_id, owner, true).await?;
        if sharing.effective(now) != SharingState::Active {
            return Err(VenueLocationError::NotFound);
        }
        if !sharing.viewers.contains(&viewer.user_id.uuid()) {
            return Err(VenueLocationError::Denied);
        }
        if !self
            .authority
            .friend_in(&mut tx, viewer.event_id, viewer.user_id, owner, now)
            .await?
        {
            return Err(VenueLocationError::Denied);
        }
        let expires = sharing
            .expires_at
            .ok_or(VenueLocationError::PolicyUnavailable)?;
        let fix = load_fix(&mut tx, viewer.event_id, owner)
            .await?
            .ok_or(VenueLocationError::NotFound)?;
        let visible = project_phone(now, owner.uuid(), &fix, sharing.revision, expires, &policy)
            .ok_or(VenueLocationError::NotFound)?;
        tx.commit()
            .await
            .map_err(|_| VenueLocationError::DependencyUnavailable)?;
        Ok(visible)
    }
}

async fn require_actor(
    c: &mut PgConnection,
    actor: ParticipantKey,
) -> Result<(), VenueLocationError> {
    let phase: Option<String> =
        sqlx::query_scalar("select phase from public.concert_events where event_id=$1 for share")
            .bind(actor.event_id.uuid())
            .fetch_optional(&mut *c)
            .await
            .map_err(storage)?;
    let phase = EventPhase::from_str(&phase.ok_or(VenueLocationError::NotFound)?)
        .map_err(|_| VenueLocationError::DependencyUnavailable)?;
    if !matches!(phase, EventPhase::Open | EventPhase::Live) {
        return Err(VenueLocationError::Denied);
    }
    let member: Option<Uuid> = sqlx::query_scalar(
        "select user_id from public.concert_participants where event_id=$1 and user_id=$2 for share",
    )
    .bind(actor.event_id.uuid())
    .bind(actor.user_id.uuid())
    .fetch_optional(&mut *c)
    .await
    .map_err(storage)?;
    member
        .filter(|id| *id == actor.user_id.uuid())
        .map(|_| ())
        .ok_or(VenueLocationError::Denied)
}

async fn now_in(c: &mut PgConnection) -> Result<DateTime<Utc>, VenueLocationError> {
    sqlx::query_scalar("select clock_timestamp()")
        .fetch_one(&mut *c)
        .await
        .map_err(storage)
}

async fn load_sharing(
    c: &mut PgConnection,
    event: EventId,
    owner: AuthUserId,
    lock: bool,
) -> Result<SharingRecord, VenueLocationError> {
    let query = if lock {
        "select state,revision,expires_at,updated_at from public.concert_venue_location_sharing where event_id=$1 and owner_id=$2 for update"
    } else {
        "select state,revision,expires_at,updated_at from public.concert_venue_location_sharing where event_id=$1 and owner_id=$2"
    };
    let row = sqlx::query(query)
        .bind(event.uuid())
        .bind(owner.uuid())
        .fetch_optional(&mut *c)
        .await
        .map_err(storage)?;
    let Some(row) = row else {
        return Ok(SharingRecord::off());
    };
    let state = match row.try_get::<String, _>("state").map_err(invalid)?.as_str() {
        "ACTIVE" => SharingState::Active,
        "PAUSED" => SharingState::Paused,
        "REVOKED" => SharingState::Revoked,
        _ => return Err(VenueLocationError::DependencyUnavailable),
    };
    let revision: i64 = row.try_get("revision").map_err(invalid)?;
    if !(1..=MAX_REVISION).contains(&revision) {
        return Err(VenueLocationError::DependencyUnavailable);
    }
    let viewers = sqlx::query_scalar(
        "select viewer_id from public.concert_venue_location_viewers where event_id=$1 and owner_id=$2 order by viewer_id",
    )
    .bind(event.uuid())
    .bind(owner.uuid())
    .fetch_all(&mut *c)
    .await
    .map_err(storage)?;
    Ok(SharingRecord {
        state,
        revision,
        viewers,
        expires_at: row.try_get("expires_at").map_err(invalid)?,
        updated_at: row.try_get("updated_at").map_err(invalid)?,
    })
}

async fn load_fix(
    c: &mut PgConnection,
    event: EventId,
    owner: AuthUserId,
) -> Result<Option<PhoneFix>, VenueLocationError> {
    let row = sqlx::query(
        "select latitude,longitude,accuracy_m,captured_at,received_at,sharing_revision
           from public.concert_venue_location_fixes where event_id=$1 and owner_id=$2",
    )
    .bind(event.uuid())
    .bind(owner.uuid())
    .fetch_optional(&mut *c)
    .await
    .map_err(storage)?;
    row.map(|row| {
        Ok(PhoneFix {
            latitude: row.try_get("latitude").map_err(invalid)?,
            longitude: row.try_get("longitude").map_err(invalid)?,
            accuracy_m: row.try_get("accuracy_m").map_err(invalid)?,
            captured_at: row.try_get("captured_at").map_err(invalid)?,
            received_at: row.try_get("received_at").map_err(invalid)?,
            sharing_revision: row.try_get("sharing_revision").map_err(invalid)?,
        })
    })
    .transpose()
}

fn storage(_: sqlx::Error) -> VenueLocationError {
    VenueLocationError::DependencyUnavailable
}
fn invalid(_: sqlx::Error) -> VenueLocationError {
    VenueLocationError::DependencyUnavailable
}

pub mod routes;
