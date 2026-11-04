//! Durable BE10/BE11 PostgreSQL adapter. No HTTP, WebSocket or public DTO is exposed here.
pub mod rpc;
pub mod account_deletion;
pub(crate) mod recipient_owners;
pub mod simulation;
pub mod interest;
pub mod replication;
pub mod session;
pub mod recovery;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use concert_application::world_ticket::*;
use concert_domain::{AssetId, AuthUserId, EventId, ParticipantKey, Revision};
use sqlx::{postgres::PgRow, PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

#[derive(Clone)]
pub struct PgWorldRepository {
    pool: PgPool,
}
impl PgWorldRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Read one current server-owned membership. Stream and epoch are runtime
    /// fields and are deliberately absent from this canonical lease.
    pub async fn current_owned_membership(
        &self,
        key: ParticipantKey,
        now: DateTime<Utc>,
    ) -> Result<Option<CurrentOwnedMembership>, WorldError> {
        let row = sqlx::query("select m.event_id,m.user_id,m.room_id,m.session_id,m.connection_id,m.map_id,m.map_revision,m.avatar_manifest_id,m.avatar_manifest_version,m.lease_until,m.access_expires_at,m.disconnected,l.owner_process_id,l.owner_epoch from public.concert_world_memberships m join public.concert_world_owner_leases l on l.event_id=m.event_id and l.room_id=m.room_id where m.event_id=$1 and m.user_id=$2 and m.lease_until>$3 and not m.disconnected and l.lease_until>$3")
            .bind(key.event_id.uuid())
            .bind(key.user_id.uuid())
            .bind(now)
            .fetch_optional(&self.pool)
            .await
            .map_err(unavailable)?;
        row.map(|row| {
            Ok(CurrentOwnedMembership {
                membership: membership(&row)?,
                owner: OwnerIdentity::new(
                    row.try_get("owner_process_id").map_err(invalid)?,
                    row.try_get("owner_epoch").map_err(invalid)?,
                )?,
            })
        })
        .transpose()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CurrentOwnedMembership {
    pub membership: Membership,
    pub owner: OwnerIdentity,
}

fn unavailable(_: sqlx::Error) -> WorldError {
    WorldError::Unavailable
}
fn invalid(_: sqlx::Error) -> WorldError {
    WorldError::Invalid
}
fn revision(value: i64) -> Result<Revision, WorldError> {
    Revision::new(value).map_err(|_| WorldError::Invalid)
}
fn asset(row: &PgRow, column: &str) -> Result<AssetId, WorldError> {
    AssetId::new(row.try_get(column).map_err(invalid)?).map_err(|_| WorldError::Invalid)
}
fn session(value: Uuid) -> Result<concert_application::identity::SessionId, WorldError> {
    concert_application::identity::SessionId::new(value.to_string())
        .map_err(|_| WorldError::Invalid)
}
fn session_uuid(value: &concert_application::identity::SessionId) -> Result<Uuid, WorldError> {
    Uuid::parse_str(value.as_str()).map_err(|_| WorldError::Invalid)
}
fn pin(row: &PgRow) -> Result<ResourcePin, WorldError> {
    Ok(ResourcePin {
        event_id: EventId::from_uuid(row.try_get("event_id").map_err(invalid)?),
        room_id: RoomId::from_uuid(row.try_get("room_id").map_err(invalid)?),
        map: MapPin::new(
            asset(row, "map_id")?,
            revision(row.try_get("map_revision").map_err(invalid)?)?,
        ),
        avatar: AvatarPin::new(
            asset(row, "avatar_manifest_id")?,
            revision(row.try_get("avatar_manifest_version").map_err(invalid)?)?,
        ),
    })
}
fn membership(row: &PgRow) -> Result<Membership, WorldError> {
    Ok(Membership {
        key: ParticipantKey {
            event_id: EventId::from_uuid(row.try_get("event_id").map_err(invalid)?),
            user_id: AuthUserId::from_uuid(row.try_get("user_id").map_err(invalid)?),
        },
        session_id: session(row.try_get("session_id").map_err(invalid)?)?,
        room_id: RoomId::from_uuid(row.try_get("room_id").map_err(invalid)?),
        connection_id: ConnectionId::from_uuid(row.try_get("connection_id").map_err(invalid)?)?,
        pin: pin(row)?,
        lease_until: row.try_get("lease_until").map_err(invalid)?,
        access_expires_at: row.try_get("access_expires_at").map_err(invalid)?,
    })
}
const MEMBERSHIP_COLUMNS: &str = "event_id,user_id,room_id,session_id,connection_id,map_id,map_revision,avatar_manifest_id,avatar_manifest_version,lease_until,access_expires_at";

async fn lock_event_user(
    transaction: &mut Transaction<'_, Postgres>,
    event_id: EventId,
    user_id: AuthUserId,
) -> Result<(), WorldError> {
    sqlx::query("select pg_advisory_xact_lock(hashtextextended('mcr-world-member:' || $1::uuid || ':' || $2::uuid, 0))")
        .bind(event_id.uuid())
        .bind(user_id.uuid())
        .execute(&mut **transaction)
        .await
        .map_err(unavailable)?;
    Ok(())
}

async fn lock_room(
    transaction: &mut Transaction<'_, Postgres>,
    pin: &ResourcePin,
) -> Result<String, WorldError> {
    let status: Option<String> = sqlx::query_scalar(
        "select availability from public.concert_world_rooms where event_id=$1 and room_id=$2 for no key update",
    )
    .bind(pin.event_id.uuid())
    .bind(pin.room_id.uuid())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(unavailable)?;
    status.ok_or(WorldError::Denied)
}

async fn check_owner(
    transaction: &mut Transaction<'_, Postgres>,
    event_id: EventId,
    room_id: RoomId,
    expected: &OwnerIdentity,
    now: DateTime<Utc>,
) -> Result<(), WorldError> {
    let row = sqlx::query("select owner_process_id,owner_epoch,lease_until from public.concert_world_owner_leases where event_id=$1 and room_id=$2 for update")
        .bind(event_id.uuid())
        .bind(room_id.uuid())
        .fetch_optional(&mut **transaction)
        .await
        .map_err(unavailable)?;
    let Some(row) = row else {
        return Err(WorldError::OwnerFenced);
    };
    let actual = OwnerIdentity::new(
        row.try_get("owner_process_id").map_err(invalid)?,
        row.try_get("owner_epoch").map_err(invalid)?,
    )?;
    let until: DateTime<Utc> = row.try_get("lease_until").map_err(invalid)?;
    if &actual != expected || until <= now {
        return Err(WorldError::OwnerFenced);
    }
    Ok(())
}

async fn room_has_space(
    transaction: &mut Transaction<'_, Postgres>,
    pin: &ResourcePin,
    now: DateTime<Utc>,
    user_id: AuthUserId,
) -> Result<bool, WorldError> {
    let result = sqlx::query("select capacity > (select count(*) from public.concert_world_memberships m where m.event_id=$1 and m.room_id=$2 and m.lease_until>$3 and m.user_id<>$4) as has_space from public.concert_world_rooms where event_id=$1 and room_id=$2")
        .bind(pin.event_id.uuid())
        .bind(pin.room_id.uuid())
        .bind(now)
        .bind(user_id.uuid())
        .fetch_optional(&mut **transaction)
        .await
        .map_err(unavailable)?;
    result
        .map(|row| row.try_get("has_space").map_err(invalid))
        .transpose()?
        .ok_or(WorldError::Denied)
}

async fn write_membership(
    transaction: &mut Transaction<'_, Postgres>,
    claim: &TicketClaim,
    owner: &OwnerIdentity,
    connection_id: ConnectionId,
    now: DateTime<Utc>,
) -> Result<Membership, WorldError> {
    let record = &claim.record;
    let lease_until = record
        .access_expires_at
        .min(now + Duration::milliseconds(FRESH_AUTHORITY_MILLISECONDS));
    if lease_until <= now {
        return Err(WorldError::CredentialExpired);
    }
    let row = sqlx::query(&format!(
        "insert into public.concert_world_memberships({MEMBERSHIP_COLUMNS},owner_process_id,owner_epoch,admitted_at,disconnected)
         values($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,false)
         on conflict(event_id,user_id) do update set room_id=excluded.room_id,session_id=excluded.session_id,
           connection_id=excluded.connection_id,owner_process_id=excluded.owner_process_id,owner_epoch=excluded.owner_epoch,
           map_id=excluded.map_id,map_revision=excluded.map_revision,avatar_manifest_id=excluded.avatar_manifest_id,
           avatar_manifest_version=excluded.avatar_manifest_version,admitted_at=excluded.admitted_at,
           lease_until=excluded.lease_until,access_expires_at=excluded.access_expires_at,disconnected=false
         returning {MEMBERSHIP_COLUMNS}"
    ))
    .bind(record.subject.key.event_id.uuid())
    .bind(record.subject.key.user_id.uuid())
    .bind(record.pin.room_id.uuid())
    .bind(session_uuid(&record.subject.session_id)?)
    .bind(connection_id.uuid())
    .bind(record.pin.map.id.as_str())
    .bind(
        record.pin.map.revision.get(),
    )
    .bind(record.pin.avatar.manifest_id.as_str())
    .bind(record.pin.avatar.manifest_version.get())
    .bind(lease_until)
    .bind(record.access_expires_at)
    .bind(&owner.process_id)
    .bind(owner.epoch)
    .bind(now)
    .fetch_one(&mut **transaction)
    .await
    .map_err(unavailable)?;
    membership(&row)
}

#[async_trait]
impl TicketRepository for PgWorldRepository {
    async fn issue(&self, record: TicketRecord) -> Result<IssueOutcome, WorldError> {
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        account_deletion::lock_user(&mut tx, record.subject.key.user_id.uuid()).await.map_err(unavailable)?;
        if account_deletion::is_fenced_in(&mut tx, record.subject.key.user_id.uuid()).await.map_err(unavailable)? {
            return Err(WorldError::Denied);
        }
        closure::lock_event_open_in(&mut tx,record.pin.event_id.uuid()).await?;
        // Issuance observes availability/capacity but reserves no membership.
        // Share the room with live publication permits; admission below keeps
        // its serialized room lock and enforces capacity when a ticket is used.
        let room = sqlx::query("select availability,capacity from public.concert_world_rooms where event_id=$1 and room_id=$2 for share")
            .bind(record.pin.event_id.uuid())
            .bind(record.pin.room_id.uuid())
            .fetch_optional(&mut *tx)
            .await
            .map_err(unavailable)?;
        let Some(room) = room else {
            return Ok(IssueOutcome::Hidden);
        };
        if room.try_get::<String, _>("availability").map_err(invalid)? != "AVAILABLE" {
            return Ok(IssueOutcome::Hidden);
        }
        let capacity: i32 = room.try_get("capacity").map_err(invalid)?;
        let active: i64 = sqlx::query_scalar(
            "select count(*) from public.concert_world_memberships where event_id=$1 and room_id=$2 and lease_until>$3 and user_id<>$4",
        )
        .bind(record.pin.event_id.uuid())
        .bind(record.pin.room_id.uuid())
        .bind(record.issued_at)
        .bind(record.subject.key.user_id.uuid())
        .fetch_one(&mut *tx)
        .await
        .map_err(unavailable)?;
        if active >= i64::from(capacity) {
            return Ok(IssueOutcome::Full);
        }
        let inserted = sqlx::query("insert into public.concert_world_tickets(ticket_digest,user_id,session_id,event_id,room_id,map_id,map_revision,avatar_manifest_id,avatar_manifest_version,issued_at,expires_at,access_expires_at) values($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12) on conflict(ticket_digest) do nothing")
            .bind(record.digest.as_bytes().as_slice())
            .bind(record.subject.key.user_id.uuid())
            .bind(session_uuid(&record.subject.session_id)?)
            .bind(record.pin.event_id.uuid())
            .bind(record.pin.room_id.uuid())
            .bind(record.pin.map.id.as_str())
            .bind(record.pin.map.revision.get())
            .bind(record.pin.avatar.manifest_id.as_str())
            .bind(record.pin.avatar.manifest_version.get())
            .bind(record.issued_at)
            .bind(record.expires_at)
            .bind(record.access_expires_at)
            .execute(&mut *tx)
            .await;
        let inserted = inserted.map_err(unavailable)?;
        if inserted.rows_affected() != 1 {
            return Err(WorldError::Conflict);
        }
        tx.commit().await.map_err(unavailable)?;
        Ok(IssueOutcome::Issued)
    }

    async fn room_catalog(&self, event_id: EventId) -> Result<RoomCatalog, WorldError> {
        let rows = sqlx::query("select r.room_id,r.label,r.capacity,r.availability,r.is_default,(r.capacity > (select count(*) from public.concert_world_memberships m where m.event_id=r.event_id and m.room_id=r.room_id and m.lease_until>statement_timestamp())) as has_space from public.concert_world_rooms r where r.event_id=$1 order by r.room_id limit 17")
            .bind(event_id.uuid())
            .fetch_all(&self.pool)
            .await
            .map_err(unavailable)?;
        if rows.is_empty() || rows.len() > MAX_ROOMS {
            return Err(WorldError::Invalid);
        }
        let mut default = None;
        let mut rooms = Vec::with_capacity(rows.len());
        for row in rows {
            let room_id = RoomId::from_uuid(row.try_get("room_id").map_err(invalid)?);
            let capacity: i32 = row.try_get("capacity").map_err(invalid)?;
            let available: String = row.try_get("availability").map_err(invalid)?;
            let availability = match (
                available.as_str(),
                row.try_get::<bool, _>("has_space").map_err(invalid)?,
            ) {
                ("UNAVAILABLE", _) => RoomAvailability::Unavailable,
                ("AVAILABLE", true) => RoomAvailability::Available,
                ("AVAILABLE", false) => RoomAvailability::Full,
                _ => return Err(WorldError::Invalid),
            };
            if row.try_get::<bool, _>("is_default").map_err(invalid)? {
                if default.replace(room_id).is_some() {
                    return Err(WorldError::Invalid);
                }
            }
            rooms.push(RoomSummary::new(
                room_id,
                row.try_get("label").map_err(invalid)?,
                u16::try_from(capacity).map_err(|_| WorldError::Invalid)?,
                availability,
            )?);
        }
        RoomCatalog::new(
            event_id,
            default.ok_or(WorldError::Invalid)?,
            rooms,
            Utc::now(),
        )
    }
}

#[async_trait]
impl WorldRepository for PgWorldRepository {
    async fn claim_owner(
        &self,
        event_id: EventId,
        room_id: RoomId,
        owner: &OwnerIdentity,
        now: DateTime<Utc>,
        duration: Duration,
    ) -> Result<OwnerLease, WorldError> {
        if duration <= Duration::zero() {
            return Err(WorldError::Invalid);
        }
        let expires = now
            .checked_add_signed(duration)
            .ok_or(WorldError::Invalid)?;
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        closure::lock_event_open_in(&mut tx,event_id.uuid()).await?;
        let room: Option<Uuid> = sqlx::query_scalar(
            "select room_id from public.concert_world_rooms where event_id=$1 and room_id=$2 for update",
        )
        .bind(event_id.uuid())
        .bind(room_id.uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(unavailable)?;
        if room.is_none() {
            return Err(WorldError::Denied);
        }
        let existing = sqlx::query("select owner_process_id,owner_epoch,lease_until from public.concert_world_owner_leases where event_id=$1 and room_id=$2 for update")
            .bind(event_id.uuid())
            .bind(room_id.uuid()).fetch_optional(&mut *tx).await.map_err(unavailable)?;
        if let Some(row) = &existing {
            let old = OwnerIdentity::new(
                row.try_get("owner_process_id").map_err(invalid)?,
                row.try_get("owner_epoch").map_err(invalid)?,
            )?;
            let until: DateTime<Utc> = row.try_get("lease_until").map_err(invalid)?;
            if old != *owner && until > now {
                return Err(WorldError::OwnerFenced);
            }
        }
        sqlx::query("insert into public.concert_world_owner_leases(event_id,room_id,owner_process_id,owner_epoch,leased_at,lease_until) values($1,$2,$3,$4,$5,$6) on conflict(event_id,room_id) do update set owner_process_id=excluded.owner_process_id,owner_epoch=excluded.owner_epoch,leased_at=excluded.leased_at,lease_until=excluded.lease_until")
            .bind(event_id.uuid()).bind(room_id.uuid()).bind(&owner.process_id).bind(owner.epoch).bind(now).bind(expires)
            .execute(&mut *tx).await.map_err(unavailable)?;
        tx.commit().await.map_err(unavailable)?;
        Ok(OwnerLease {
            event_id,
            room_id,
            owner: owner.clone(),
            expires_at: expires,
        })
    }

    async fn consume(&self, claim: ConsumeClaim) -> Result<ConsumeOutcome, WorldError> {
        // Serialize competing claims on the same digest before reading the one-use state. This
        // keeps the outcome stable across concurrent transactions and preserves the row predicate
        // as a second atomic guard against accidental reintroduction of a reusable update path.
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        let actual = sqlx::query("select event_id,user_id from public.concert_world_tickets where ticket_digest=$1")
            .bind(claim.digest.as_bytes().as_slice()).fetch_optional(&mut *tx).await.map_err(unavailable)?;
        let Some(actual) = actual else { return Ok(ConsumeOutcome::Unknown) };
        let actual_event: Uuid = actual.try_get("event_id").map_err(invalid)?;
        let actual_user: Uuid = actual.try_get("user_id").map_err(invalid)?;
        account_deletion::lock_user(&mut tx, actual_user).await.map_err(unavailable)?;
        if account_deletion::is_fenced_in(&mut tx, actual_user).await.map_err(unavailable)? {
            return Err(WorldError::Denied);
        }
        closure::lock_event_open_in(&mut tx,actual_event).await?;

        sqlx::query("select pg_advisory_xact_lock(hashtextextended('mcr-world-ticket:' || encode($1::bytea,'hex'), 0))")
            .bind(claim.digest.as_bytes().as_slice())
            .execute(&mut *tx)
            .await
            .map_err(unavailable)?;
        let existing = sqlx::query("select event_id,user_id,consumed_at,expires_at from public.concert_world_tickets where ticket_digest=$1 for update")
            .bind(claim.digest.as_bytes().as_slice())
            .fetch_optional(&mut *tx)
            .await
            .map_err(unavailable)?;
        let Some(existing) = existing else {
            tx.commit().await.map_err(unavailable)?;
            return Ok(ConsumeOutcome::Unknown);
        };
        if existing.try_get::<Uuid,_>("event_id").map_err(invalid)? != actual_event
            || existing.try_get::<Uuid,_>("user_id").map_err(invalid)? != actual_user {
            return Err(WorldError::CredentialInvalid);
        }
        if existing
            .try_get::<Option<DateTime<Utc>>, _>("consumed_at")
            .map_err(invalid)?
            .is_some()
        {
            tx.commit().await.map_err(unavailable)?;
            return Ok(ConsumeOutcome::AlreadyConsumed);
        }
        if existing
            .try_get::<DateTime<Utc>, _>("expires_at")
            .map_err(invalid)?
            <= claim.now
        {
            tx.commit().await.map_err(unavailable)?;
            return Ok(ConsumeOutcome::Expired);
        }
        let row = sqlx::query("update public.concert_world_tickets set consumed_at=$2,consumed_connection_id=$3 where ticket_digest=$1 and consumed_at is null and expires_at>$2 returning user_id,session_id,event_id,room_id,map_id,map_revision,avatar_manifest_id,avatar_manifest_version,issued_at,expires_at,access_expires_at")
            .bind(claim.digest.as_bytes().as_slice()).bind(claim.now).bind(claim.connection_id.uuid())
            .fetch_optional(&mut *tx).await.map_err(unavailable)?;
        if let Some(row) = row {
            let record = TicketRecord {
                digest: claim.digest,
                subject: WorldSubject {
                    key: ParticipantKey {
                        event_id: EventId::from_uuid(row.try_get("event_id").map_err(invalid)?),
                        user_id: AuthUserId::from_uuid(row.try_get("user_id").map_err(invalid)?),
                    },
                    event_id: EventId::from_uuid(row.try_get("event_id").map_err(invalid)?),
                    session_id: session(row.try_get("session_id").map_err(invalid)?)?,
                },
                pin: pin(&row)?,
                issued_at: row.try_get("issued_at").map_err(invalid)?,
                expires_at: row.try_get("expires_at").map_err(invalid)?,
                access_expires_at: row.try_get("access_expires_at").map_err(invalid)?,
            };
            if record.pin != claim.requested {
                tx.commit().await.map_err(unavailable)?;
                return Err(WorldError::CredentialInvalid);
            }
            tx.commit().await.map_err(unavailable)?;
            return Ok(ConsumeOutcome::Consumed(TicketClaim {
                record,
                connection_id: claim.connection_id,
            }));
        }
        tx.commit().await.map_err(unavailable)?;
        Ok(ConsumeOutcome::AlreadyConsumed)
    }

    async fn admit(
        &self,
        owner: &OwnerIdentity,
        claim: &TicketClaim,
        now: DateTime<Utc>,
    ) -> Result<AdmissionOutcome, WorldError> {
        let record = &claim.record;
        if record.expires_at <= now
            || record.access_expires_at <= now
            || record.pin.event_id != record.subject.event_id
            || record.pin.event_id != record.subject.key.event_id
        {
            return Ok(AdmissionOutcome::Denied);
        }
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        account_deletion::lock_user(&mut tx, record.subject.key.user_id.uuid()).await.map_err(unavailable)?;
        if account_deletion::is_fenced_in(&mut tx, record.subject.key.user_id.uuid()).await.map_err(unavailable)? {
            return Ok(AdmissionOutcome::Denied);
        }
        closure::lock_event_open_in(&mut tx,record.subject.key.event_id.uuid()).await?;
        lock_event_user(
            &mut tx,
            record.subject.key.event_id,
            record.subject.key.user_id,
        )
        .await?;
        let status = lock_room(&mut tx, &record.pin).await?;
        check_owner(&mut tx, record.pin.event_id, record.pin.room_id, owner, now).await?;
        let target_room = sqlx::query("select map_id,map_revision,avatar_manifest_id,avatar_manifest_version from public.concert_world_rooms where event_id=$1 and room_id=$2")
            .bind(record.pin.event_id.uuid()).bind(record.pin.room_id.uuid()).fetch_one(&mut *tx).await.map_err(unavailable)?;
        if target_room
            .try_get::<String, _>("map_id")
            .map_err(invalid)?
            != record.pin.map.id.as_str()
            || target_room
                .try_get::<i64, _>("map_revision")
                .map_err(invalid)?
                != record.pin.map.revision.get()
            || target_room
                .try_get::<String, _>("avatar_manifest_id")
                .map_err(invalid)?
                != record.pin.avatar.manifest_id.as_str()
            || target_room
                .try_get::<i64, _>("avatar_manifest_version")
                .map_err(invalid)?
                != record.pin.avatar.manifest_version.get()
        {
            return Ok(AdmissionOutcome::Denied);
        }
        if status != "AVAILABLE" {
            return Ok(AdmissionOutcome::Denied);
        }
        let existing = sqlx::query(&format!("select {MEMBERSHIP_COLUMNS},owner_process_id,owner_epoch,admitted_at,disconnected from public.concert_world_memberships where event_id=$1 and user_id=$2 for update"))
            .bind(record.subject.key.event_id.uuid()).bind(record.subject.key.user_id.uuid())
            .fetch_optional(&mut *tx).await.map_err(unavailable)?;
        let replaced = existing.as_ref().and_then(|row| {
            let old_connection: Uuid = row.try_get("connection_id").ok()?;
            let until: DateTime<Utc> = row.try_get("lease_until").ok()?;
            (until > now)
                .then(|| ConnectionId::from_uuid(old_connection).ok())
                .flatten()
        });
        if !room_has_space(&mut tx, &record.pin, now, record.subject.key.user_id).await? {
            return Ok(AdmissionOutcome::Full);
        }
        let value = write_membership(&mut tx, claim, owner, claim.connection_id, now).await?;
        tx.commit().await.map_err(unavailable)?;
        Ok(if replaced.is_some() {
            AdmissionOutcome::Replaced(Admission {
                membership: value,
                replaced,
            })
        } else {
            AdmissionOutcome::Admitted(Admission {
                membership: value,
                replaced: None,
            })
        })
    }

    async fn switch(
        &self,
        owner: &OwnerIdentity,
        current: &Membership,
        claim: &TicketClaim,
        now: DateTime<Utc>,
    ) -> Result<SwitchOutcome, WorldError> {
        let record = &claim.record;
        if current.key != record.subject.key
            || record.expires_at <= now
            || record.access_expires_at <= now
        {
            return Ok(SwitchOutcome::Denied);
        }
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        account_deletion::lock_user(&mut tx, current.key.user_id.uuid()).await.map_err(unavailable)?;
        if account_deletion::is_fenced_in(&mut tx, current.key.user_id.uuid()).await.map_err(unavailable)? {
            return Ok(SwitchOutcome::Denied);
        }
        closure::lock_event_open_in(&mut tx,current.key.event_id.uuid()).await?;
        lock_event_user(&mut tx, current.key.event_id, current.key.user_id).await?;
        let status = lock_room(&mut tx, &record.pin).await?;
        check_owner(&mut tx, record.pin.event_id, record.pin.room_id, owner, now).await?;
        if current.room_id == record.pin.room_id {
            return Ok(SwitchOutcome::Denied);
        }
        if status != "AVAILABLE" {
            return Ok(SwitchOutcome::Denied);
        }
        let target = sqlx::query("select map_id,map_revision,avatar_manifest_id,avatar_manifest_version from public.concert_world_rooms where event_id=$1 and room_id=$2")
            .bind(record.pin.event_id.uuid()).bind(record.pin.room_id.uuid()).fetch_one(&mut *tx).await.map_err(unavailable)?;
        if target.try_get::<String, _>("map_id").map_err(invalid)? != record.pin.map.id.as_str()
            || target.try_get::<i64, _>("map_revision").map_err(invalid)?
                != record.pin.map.revision.get()
            || target
                .try_get::<String, _>("avatar_manifest_id")
                .map_err(invalid)?
                != record.pin.avatar.manifest_id.as_str()
            || target
                .try_get::<i64, _>("avatar_manifest_version")
                .map_err(invalid)?
                != record.pin.avatar.manifest_version.get()
        {
            return Ok(SwitchOutcome::Denied);
        }
        let held = sqlx::query(&format!("select {MEMBERSHIP_COLUMNS},owner_process_id,owner_epoch,admitted_at,disconnected from public.concert_world_memberships where event_id=$1 and user_id=$2 for update"))
            .bind(current.key.event_id.uuid()).bind(current.key.user_id.uuid()).fetch_optional(&mut *tx).await.map_err(unavailable)?;
        let Some(held) = held else {
            return Ok(SwitchOutcome::Conflict);
        };
        if held.try_get::<Uuid, _>("connection_id").map_err(invalid)?
            != current.connection_id.uuid()
            || held.try_get::<Uuid, _>("room_id").map_err(invalid)? != current.room_id.uuid()
            || held.try_get::<Uuid, _>("session_id").map_err(invalid)?
                != session_uuid(&current.session_id)?
            || held
                .try_get::<DateTime<Utc>, _>("lease_until")
                .map_err(invalid)?
                <= now
        {
            return Ok(SwitchOutcome::Conflict);
        }
        if !room_has_space(&mut tx, &record.pin, now, record.subject.key.user_id).await? {
            return Ok(SwitchOutcome::Full);
        }
        let value = write_membership(&mut tx, claim, owner, current.connection_id, now).await?;
        tx.commit().await.map_err(unavailable)?;
        Ok(SwitchOutcome::Switched(Admission {
            membership: value,
            replaced: None,
        }))
    }

    async fn renew_owner(
        &self,
        event_id: EventId,
        room_id: RoomId,
        owner: &OwnerIdentity,
        now: DateTime<Utc>,
        duration: Duration,
    ) -> Result<OwnerLease, WorldError> {
        if duration <= Duration::zero() {
            return Err(WorldError::Invalid);
        }
        let expires = now
            .checked_add_signed(duration)
            .ok_or(WorldError::Invalid)?;
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        closure::lock_event_open_in(&mut tx,event_id.uuid()).await?;
        let row = sqlx::query("select owner_process_id,owner_epoch,lease_until from public.concert_world_owner_leases where event_id=$1 and room_id=$2 for update")
            .bind(event_id.uuid())
            .bind(room_id.uuid()).fetch_optional(&mut *tx).await.map_err(unavailable)?;
        let Some(row) = row else {
            return Err(WorldError::OwnerFenced);
        };
        let actual = OwnerIdentity::new(
            row.try_get("owner_process_id").map_err(invalid)?,
            row.try_get("owner_epoch").map_err(invalid)?,
        )?;
        let old: DateTime<Utc> = row.try_get("lease_until").map_err(invalid)?;
        if actual != *owner || old <= now {
            return Err(WorldError::OwnerFenced);
        }
        sqlx::query("update public.concert_world_owner_leases set leased_at=$3,lease_until=$4 where event_id=$1 and room_id=$2")
            .bind(event_id.uuid()).bind(room_id.uuid()).bind(now).bind(expires).execute(&mut *tx).await.map_err(unavailable)?;
        tx.commit().await.map_err(unavailable)?;
        Ok(OwnerLease {
            event_id,
            room_id,
            owner: owner.clone(),
            expires_at: expires,
        })
    }

    async fn disconnect(
        &self,
        key: ParticipantKey,
        event_id: EventId,
        connection_id: ConnectionId,
        session_id: &concert_application::identity::SessionId,
        now: DateTime<Utc>,
        grace: Duration,
    ) -> Result<LeaseState, WorldError> {
        if key.event_id != event_id || grace < Duration::zero() {
            return Err(WorldError::Invalid);
        }
        let until = now.checked_add_signed(grace).ok_or(WorldError::Invalid)?;
        let row = sqlx::query(&format!("update public.concert_world_memberships set disconnected=true,lease_until=least(access_expires_at,$4) where event_id=$1 and user_id=$2 and connection_id=$3 and session_id=$5 and lease_until>$6 returning {MEMBERSHIP_COLUMNS}"))
            .bind(key.event_id.uuid()).bind(key.user_id.uuid()).bind(connection_id.uuid()).bind(until)
            .bind(session_uuid(session_id)?).bind(now)
            .fetch_optional(&self.pool).await.map_err(unavailable)?;
        row.map(|row| membership(&row).map(LeaseState::Disconnected))
            .transpose()
            .map(|value| value.unwrap_or(LeaseState::Gone))
    }

    async fn leave(
        &self,
        key: ParticipantKey,
        subscription: &SubscriptionLease,
        auth: &concert_application::identity::SessionId,
    ) -> Result<(), WorldError> {
        if key != subscription.key || auth != &subscription.session_id {
            return Err(WorldError::Denied);
        }
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        check_owner(
            &mut tx,
            subscription.key.event_id,
            subscription.room_id,
            &subscription.owner,
            Utc::now(),
        )
        .await?;
        sqlx::query("delete from public.concert_world_memberships where event_id=$1 and user_id=$2 and room_id=$3 and session_id=$4 and connection_id=$5")
            .bind(key.event_id.uuid()).bind(key.user_id.uuid()).bind(subscription.room_id.uuid())
            .bind(session_uuid(auth)?).bind(subscription.connection_id.uuid()).execute(&mut *tx).await.map_err(unavailable)?;
        tx.commit().await.map_err(unavailable)?;
        Ok(())
    }

    async fn reauthorize(
        &self,
        subscription: &SubscriptionLease,
        now: DateTime<Utc>,
    ) -> Result<LeaseState, WorldError> {
        let row = sqlx::query(&format!("select m.event_id,m.user_id,m.room_id,m.session_id,m.connection_id,m.map_id,m.map_revision,m.avatar_manifest_id,m.avatar_manifest_version,m.lease_until,m.access_expires_at,m.disconnected from public.concert_world_memberships m where m.event_id=$1 and m.user_id=$2 and m.room_id=$3 and m.session_id=$4 and m.connection_id=$5 and m.lease_until>$6 and exists(select 1 from public.concert_world_owner_leases l where l.event_id=m.event_id and l.room_id=m.room_id and l.owner_process_id=$7 and l.owner_epoch=$8 and l.lease_until>$6)"))
            .bind(subscription.key.event_id.uuid()).bind(subscription.key.user_id.uuid()).bind(subscription.room_id.uuid())
            .bind(session_uuid(&subscription.session_id)?).bind(subscription.connection_id.uuid()).bind(now)
            .bind(&subscription.owner.process_id).bind(subscription.owner.epoch)
            .fetch_optional(&self.pool).await.map_err(unavailable)?;
        match row {
            Some(row) if row.try_get::<bool, _>("disconnected").map_err(invalid)? => {
                Ok(LeaseState::Disconnected(membership(&row)?))
            }
            Some(row) => Ok(LeaseState::Active(membership(&row)?)),
            None => Ok(LeaseState::Gone),
        }
    }

    async fn prune(&self, now: DateTime<Utc>) -> Result<Pruned, WorldError> {
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        let memberships = sqlx::query("delete from public.concert_world_memberships where ctid in (select ctid from public.concert_world_memberships where lease_until<=$1 order by lease_until limit 1000)")
            .bind(now).execute(&mut *tx).await.map_err(unavailable)?.rows_affected();
        let tickets = sqlx::query("delete from public.concert_world_tickets where ticket_digest in (select ticket_digest from public.concert_world_tickets where expires_at<=$1 order by expires_at limit 1000)")
            .bind(now).execute(&mut *tx).await.map_err(unavailable)?.rows_affected();
        tx.commit().await.map_err(unavailable)?;
        Ok(Pruned {
            tickets,
            memberships,
        })
    }
}

pub mod ingress;
pub mod membership;
pub mod runtime;
pub mod admission_bridge;

pub mod mural_subscription;

pub mod production_owners;

pub mod invalidation;

pub(crate) mod closure;
