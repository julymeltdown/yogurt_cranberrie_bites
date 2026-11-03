//! Database admission and nonblocking runtime views. External live Auth/Profile
//! and recipient authority are explicit dependencies, never inferred from a ticket.
use super::{
    ingress::{AdmissionIntent, SafeClose},
    interest::{ProjectionError, PublicIdentity, RecipientPolicy, RecipientView},
    membership::{CanonicalAuthorityPort, CanonicalMembership, ClockAnchor},
    simulation::{AuthorityError, Binding, ResourcePin},
    PgWorldRepository,
};
use crate::{adapters::r#in::world_ws::AdmissionPort, bootstrap::BootstrapService};
use async_trait::async_trait;
use chrono::{DateTime, TimeDelta, Utc};
use concert_application::{identity::AccessCredential, world_ticket::*};
use concert_domain::{EventId, ParticipantKey};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
    time::Instant,
};
use uuid::Uuid;

const MAX_MEMBERS: usize = 800;
#[derive(Clone)]
pub struct LiveOwnerView {
    pub subject: WorldSubject,
    pub checked_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub identity: PublicIdentity,
}
/// The implementation must use actual canonical owners. Every known Auth,
/// restriction, profile and block invalidation must take this exact fence before
/// it acknowledges the change. Unknown/outage is an error, not an empty block set.
#[async_trait]
pub trait LiveWorldOwners: Send + Sync + 'static {
    fn publication_fence(&self) -> Arc<Mutex<()>>;
    /// Advance on every known owner/privacy invalidation while holding that fence.
    /// Synchronous and nonblocking; current reads compare it before any publication.
    fn generation(&self) -> Uuid;
    async fn current(
        &self,
        subject: &WorldSubject,
        now: DateTime<Utc>,
    ) -> Result<LiveOwnerView, WorldError>;
    async fn visible(
        &self,
        recipient: &WorldSubject,
        subjects: &[WorldSubject],
        now: DateTime<Utc>,
    ) -> Result<BTreeSet<Uuid>, WorldError>;
}
#[derive(Clone)]
struct CurrentAuthority {
    owners: Arc<dyn LiveWorldOwners>,
    bootstrap: Arc<BootstrapService>,
}
#[async_trait]
impl WorldAuthority for CurrentAuthority {
    async fn authorize(
        &self,
        _: &AccessCredential,
        _: EventId,
        _: DateTime<Utc>,
    ) -> Result<AuthorizedWorld, WorldError> {
        Err(WorldError::Unavailable)
    }
    async fn reauthorize(
        &self,
        subject: &WorldSubject,
        now: DateTime<Utc>,
    ) -> Result<AuthorizationRenewal, WorldError> {
        let live = self.owners.current(subject, now).await?;
        validate_live(&live, subject, now)?;
        let session =
            Uuid::parse_str(subject.session_id.as_str()).map_err(|_| WorldError::Invalid)?;
        self.bootstrap
            .world_admission(
                subject.key.user_id,
                subject.event_id,
                session,
                Uuid::new_v4(),
            )
            .await
            .map_err(|_| WorldError::Denied)?;
        Ok(AuthorizationRenewal {
            session_active: true,
            event_allows_admission: true,
            participant_admitted: true,
            restrictions_allow: true,
            access_expires_at: live.expires_at,
        })
    }
}
struct Clock;
impl concert_application::Clock for Clock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}
struct Entropy;
impl TicketGenerator for Entropy {
    fn generate(&self) -> Result<AdmissionTicket, WorldError> {
        use ring::rand::SecureRandom;
        let mut bytes = [0; 32];
        ring::rand::SystemRandom::new()
            .fill(&mut bytes)
            .map_err(|_| WorldError::Unavailable)?;
        AdmissionTicket::from_entropy(&bytes)
    }
}
type Service = WorldService<CurrentAuthority, PgWorldRepository, Clock, Entropy>;
#[derive(Clone)]
struct Entry {
    binding: Binding,
    membership: Membership,
    live: LiveOwnerView,
    valid_until_ms: u64,
    visible: BTreeSet<Uuid>,
    generation: Uuid,
    owner_generation: Uuid,
}
#[derive(Default)]
struct Views {
    entries: BTreeMap<(Uuid, Uuid), Entry>,
    cleanup: BTreeMap<(Uuid, Uuid, Uuid), Binding>,
    absent: BTreeSet<(Uuid, Uuid, Uuid)>,
}
fn key(b: &Binding) -> (Uuid, Uuid) {
    (b.key.event_id.uuid(), b.key.user_id.uuid())
}
fn cleanup_key(b: &Binding) -> (Uuid, Uuid, Uuid) {
    (b.room_id, b.connection_id, b.stream_id)
}
fn subject(m: &Membership) -> WorldSubject {
    WorldSubject {
        key: m.key,
        event_id: m.key.event_id,
        session_id: m.session_id.clone(),
    }
}
fn subscription(b: &Binding, owner: &OwnerIdentity) -> Result<SubscriptionLease, WorldError> {
    Ok(SubscriptionLease {
        key: b.key,
        session_id: concert_application::identity::SessionId::new(b.session_id.to_string())?,
        room_id: RoomId::from_uuid(b.room_id),
        connection_id: ConnectionId::from_uuid(b.connection_id)?,
        owner: owner.clone(),
    })
}
fn validate_live(
    live: &LiveOwnerView,
    expected: &WorldSubject,
    now: DateTime<Utc>,
) -> Result<(), WorldError> {
    if &live.subject != expected
        || live.checked_at > now
        || live.checked_at + TimeDelta::milliseconds(5000) <= now
        || live.expires_at <= now
        || live.expires_at > live.checked_at + TimeDelta::milliseconds(5000)
    {
        return Err(WorldError::Unavailable);
    }
    Ok(())
}
fn safe(_: WorldError) -> SafeClose {
    SafeClose::UNAVAILABLE
}
/// Real production bridge once a qualified live-owner implementation is supplied.
/// All I/O occurs outside the shared publication fence; only bounded snapshots are read at 20Hz.
pub struct AdmissionBridge {
    pool: sqlx::PgPool,
    repository: PgWorldRepository,
    service: Service,
    authority: CurrentAuthority,
    owners: Arc<dyn LiveWorldOwners>,
    owner: OwnerIdentity,
    event: EventId,
    rooms: BTreeMap<Uuid, Uuid>,
    resources: ResourcePin,
    anchor: ClockAnchor,
    started: Instant,
    fence: Arc<Mutex<()>>,
    views: Mutex<Views>,
    admission_gate: tokio::sync::Mutex<()>,
    owner_until: Mutex<DateTime<Utc>>,
}
impl AdmissionBridge {
    pub fn new(
        pool: sqlx::PgPool,
        bootstrap: Arc<BootstrapService>,
        owners: Arc<dyn LiveWorldOwners>,
        owner: OwnerIdentity,
        event: EventId,
        rooms: BTreeMap<Uuid, Uuid>,
        resources: ResourcePin,
        anchor: ClockAnchor,
        started: Instant,
    ) -> Result<Arc<Self>, WorldError> {
        if rooms.is_empty()
            || rooms.len() > 16
            || event.uuid().is_nil()
            || owner.epoch.is_nil()
            || rooms.iter().any(|(r, e)| r.is_nil() || e.is_nil())
        {
            return Err(WorldError::Invalid);
        }
        let fence = owners.publication_fence();
        Ok(Arc::new(Self {
            authority: CurrentAuthority {
                owners: owners.clone(),
                bootstrap: bootstrap.clone(),
            },
            service: WorldService::new(
                CurrentAuthority {
                    owners: owners.clone(),
                    bootstrap,
                },
                PgWorldRepository::new(pool.clone()),
                Clock,
                Entropy,
            ),
            repository: PgWorldRepository::new(pool.clone()),
            pool,
            owners,
            owner,
            event,
            rooms,
            resources,
            anchor: anchor.clone(),
            started,
            fence,
            views: Mutex::new(Views::default()),
            admission_gate: tokio::sync::Mutex::new(()),
            owner_until: Mutex::new(anchor.utc),
        }))
    }
    fn now_ms(&self) -> Result<u64, WorldError> {
        self.anchor
            .monotonic_ms
            .checked_add(
                self.started
                    .elapsed()
                    .as_millis()
                    .try_into()
                    .map_err(|_| WorldError::Unavailable)?,
            )
            .ok_or(WorldError::Unavailable)
    }
    fn requested(
        &self,
        intent: &AdmissionIntent,
    ) -> Result<concert_application::world_ticket::ResourcePin, WorldError> {
        if intent.event_id != self.event.uuid()
            || !self.rooms.contains_key(&intent.room_id)
            || intent.map_id != self.resources.map_id
            || intent.map_revision != self.resources.map_revision
            || intent.avatar_manifest_id != self.resources.avatar_manifest_id
            || intent.avatar_manifest_version != self.resources.avatar_manifest_version
        {
            return Err(WorldError::MapVersionMismatch);
        }
        Ok(concert_application::world_ticket::ResourcePin {
            event_id: self.event,
            room_id: RoomId::from_uuid(intent.room_id),
            map: MapPin::new(intent.map_id.clone(), intent.map_revision),
            avatar: AvatarPin::new(
                intent.avatar_manifest_id.clone(),
                intent.avatar_manifest_version,
            ),
        })
    }
    fn invalidate(&self, b: &Binding) {
        if let Ok(_fence) = self.fence.lock() {
            if let Ok(mut views) = self.views.lock() {
                if views.entries.get(&key(b)).is_some_and(|e| e.binding == *b) {
                    views.entries.remove(&key(b));
                }
            }
        }
    }
    async fn publish(
        &self,
        membership: Membership,
        connection: Uuid,
        stream: Uuid,
    ) -> Result<Binding, WorldError> {
        let generation = self.owners.generation();
        if generation.is_nil() {
            return Err(WorldError::Unavailable);
        }
        let now = Utc::now();
        let expected = subject(&membership);
        let live = self.owners.current(&expected, now).await?;
        validate_live(&live, &expected, Utc::now())?;
        if live.identity.avatar.manifest_id != self.resources.avatar_manifest_id
            || live.identity.avatar.manifest_version != self.resources.avatar_manifest_version
        {
            return Err(WorldError::MapVersionMismatch);
        }
        let current = self
            .repository
            .current_owned_membership(membership.key, Utc::now())
            .await?
            .ok_or(WorldError::MembershipMissing)?;
        if current.membership != membership
            || current.owner != self.owner
            || membership.connection_id.uuid() != connection
        {
            return Err(WorldError::OwnerFenced);
        }
        let binding = Binding {
            key: membership.key,
            session_id: Uuid::parse_str(membership.session_id.as_str())
                .map_err(|_| WorldError::Invalid)?,
            connection_id: connection,
            owner_id: Uuid::parse_str(&self.owner.process_id).map_err(|_| WorldError::Invalid)?,
            room_id: membership.room_id.uuid(),
            room_epoch: *self
                .rooms
                .get(&membership.room_id.uuid())
                .ok_or(WorldError::Denied)?,
            stream_id: stream,
            resources: self.resources.clone(),
        };
        let subjects: Vec<_> = {
            let views = self.views.lock().map_err(|_| WorldError::Unavailable)?;
            views
                .entries
                .values()
                .filter(|e| e.binding.room_id == binding.room_id)
                .map(|e| subject(&e.membership))
                .chain(std::iter::once(expected.clone()))
                .collect()
        };
        let visible = self
            .owners
            .visible(&expected, &subjects, Utc::now())
            .await?;
        if !visible.contains(&membership.key.user_id.uuid()) {
            return Err(WorldError::Denied);
        }
        let expires = membership
            .lease_until
            .min(membership.access_expires_at)
            .min(live.expires_at)
            .min(
                *self
                    .owner_until
                    .lock()
                    .map_err(|_| WorldError::Unavailable)?,
            );
        let valid_until_ms = self
            .anchor
            .deadline(expires)
            .map_err(|_| WorldError::Unavailable)?;
        let _fence = self.fence.lock().map_err(|_| WorldError::Unavailable)?;
        if valid_until_ms <= self.now_ms()? || self.owners.generation() != generation {
            return Err(WorldError::Unavailable);
        }
        let mut views = self.views.lock().map_err(|_| WorldError::Unavailable)?;
        if views.entries.len() >= MAX_MEMBERS && !views.entries.contains_key(&key(&binding)) {
            return Err(WorldError::RoomFull);
        }
        views.entries.insert(
            key(&binding),
            Entry {
                binding: binding.clone(),
                membership,
                live,
                valid_until_ms,
                visible,
                generation: Uuid::new_v4(),
                owner_generation: generation,
            },
        );
        Ok(binding)
    }
    /// Claim all configured rooms before listening, and renew independently of clients.
    pub async fn claim_rooms(&self) -> Result<(), WorldError> {
        let mut until = Utc::now() + TimeDelta::milliseconds(5000);
        for room in self.rooms.keys() {
            let lease = self
                .service
                .claim_owner(
                    self.event,
                    RoomId::from_uuid(*room),
                    &self.owner,
                    TimeDelta::milliseconds(5000),
                )
                .await?;
            until = until.min(lease.expires_at);
        }
        *self
            .owner_until
            .lock()
            .map_err(|_| WorldError::Unavailable)? = until;
        Ok(())
    }
    /// Bounded maintenance, called by the supervised async authority task, never by a socket heartbeat.
    pub async fn maintain(&self) -> Result<(), WorldError> {
        let _gate = self.admission_gate.lock().await;
        let mut until = Utc::now() + TimeDelta::milliseconds(5000);
        for room in self.rooms.keys() {
            let lease = self
                .service
                .renew_owner(
                    self.event,
                    RoomId::from_uuid(*room),
                    &self.owner,
                    TimeDelta::milliseconds(5000),
                )
                .await?;
            until = until.min(lease.expires_at);
        }
        *self
            .owner_until
            .lock()
            .map_err(|_| WorldError::Unavailable)? = until;
        self.cleanup().await?;
        let entries: Vec<_> = self
            .views
            .lock()
            .map_err(|_| WorldError::Unavailable)?
            .entries
            .values()
            .cloned()
            .collect();
        for entry in entries {
            let b = &entry.binding;
            let checked = self.service.renew(&subscription(b, &self.owner)?).await;
            let result=async {
                let LeaseState::Active(m)=checked? else { return Err(WorldError::Denied); };
                // Repository renew is a current read, not an expiry extension. Extend
                // only after fresh Auth/event/restriction checks and exact owner fencing.
                let lease=Utc::now().checked_add_signed(TimeDelta::milliseconds(5000)).ok_or(WorldError::Invalid)?.min(m.access_expires_at);
                let changed=sqlx::query("update public.concert_world_memberships m set lease_until=$6 where event_id=$1 and user_id=$2 and room_id=$3 and session_id=$4 and connection_id=$5 and access_expires_at=$7 and lease_until>clock_timestamp() and not disconnected and exists(select 1 from public.concert_world_owner_leases l where l.event_id=m.event_id and l.room_id=m.room_id and l.owner_process_id=$8 and l.owner_epoch=$9 and l.lease_until>clock_timestamp())")
                    .bind(b.key.event_id.uuid()).bind(b.key.user_id.uuid()).bind(b.room_id).bind(b.session_id).bind(b.connection_id).bind(lease).bind(m.access_expires_at).bind(&self.owner.process_id).bind(self.owner.epoch).execute(&self.pool).await.map_err(|_|WorldError::Unavailable)?.rows_affected();
                if changed!=1 { return Err(WorldError::OwnerFenced); }
                let mut m=m; m.lease_until=lease; self.publish(m,b.connection_id,b.stream_id).await.map(|_|())
            }.await;
            if result.is_err() {
                self.invalidate(b);
                let _ = self.request_cleanup(b);
            }
        }
        Ok(())
    }
    fn request_cleanup(&self, b: &Binding) -> Result<(), AuthorityError> {
        let mut views = self.views.lock().map_err(|_| AuthorityError::Unavailable)?;
        if views.entries.get(&key(b)).is_some_and(|e| {
            e.binding.connection_id == b.connection_id
                && e.binding.session_id == b.session_id
                && e.binding != *b
        }) {
            return Ok(());
        }
        if views.absent.remove(&cleanup_key(b)) {
            return Ok(());
        }
        if views.cleanup.len() >= MAX_MEMBERS && !views.cleanup.contains_key(&cleanup_key(b)) {
            return Err(AuthorityError::Unavailable);
        }
        views.cleanup.insert(cleanup_key(b), b.clone());
        Err(AuthorityError::Unavailable)
    }
    pub async fn cleanup(&self) -> Result<usize, WorldError> {
        let pending: Vec<_> = self
            .views
            .lock()
            .map_err(|_| WorldError::Unavailable)?
            .cleanup
            .values()
            .cloned()
            .collect();
        for b in pending {
            if self
                .service
                .leave(&subscription(&b, &self.owner)?)
                .await
                .is_ok()
            {
                let _fence = self.fence.lock().map_err(|_| WorldError::Unavailable)?;
                let mut views = self.views.lock().map_err(|_| WorldError::Unavailable)?;
                views.cleanup.remove(&cleanup_key(&b));
                if views.absent.len() < MAX_MEMBERS {
                    views.absent.insert(cleanup_key(&b));
                }
                if views.entries.get(&key(&b)).is_some_and(|e| e.binding == b) {
                    views.entries.remove(&key(&b));
                }
            }
        }
        Ok(self
            .views
            .lock()
            .map_err(|_| WorldError::Unavailable)?
            .cleanup
            .len())
    }
}
impl CanonicalAuthorityPort for Arc<AdmissionBridge> {
    fn publication_fence(&self) -> Arc<Mutex<()>> {
        self.fence.clone()
    }
    fn current(&self, b: &Binding, now: u64) -> Result<CanonicalMembership, AuthorityError> {
        let views = self.views.lock().map_err(|_| AuthorityError::Unavailable)?;
        let e = views.entries.get(&key(b)).ok_or(AuthorityError::Revoked)?;
        if e.binding != *b
            || now >= e.valid_until_ms
            || e.owner_generation != self.owners.generation()
        {
            return Err(AuthorityError::Revoked);
        }
        Ok(CanonicalMembership {
            binding: b.clone(),
            checked_at_ms: now,
            lease_expires_at: e.membership.lease_until.min(e.live.expires_at),
            access_expires_at: e.membership.access_expires_at,
            identity: e.live.identity.clone(),
        })
    }
    fn release(&self, b: &Binding) -> Result<(), AuthorityError> {
        self.request_cleanup(b)
    }
}
impl RecipientPolicy for Arc<AdmissionBridge> {
    fn current(
        &self,
        b: &Binding,
        subjects: &[Binding],
        now: u64,
    ) -> Result<RecipientView, ProjectionError> {
        let views = self
            .views
            .lock()
            .map_err(|_| ProjectionError::PolicyUnavailable)?;
        let e = views
            .entries
            .get(&key(b))
            .ok_or(ProjectionError::PolicyUnavailable)?;
        if e.binding != *b
            || now >= e.valid_until_ms
            || e.owner_generation != self.owners.generation()
            || subjects.iter().any(|s| {
                s.key.event_id != b.key.event_id
                    || s.room_id != b.room_id
                    || !views
                        .entries
                        .get(&key(s))
                        .is_some_and(|v| v.binding == *s && now < v.valid_until_ms)
            })
        {
            return Err(ProjectionError::PolicyUnavailable);
        }
        Ok(RecipientView {
            binding: b.clone(),
            checked_at_ms: now,
            valid_until_ms: e.valid_until_ms,
            visible_users: e.visible.clone(),
            invalidation_id: e.generation,
        })
    }
}
#[async_trait]
impl AdmissionPort for AdmissionBridge {
    async fn admit(&self, intent: AdmissionIntent, connection: Uuid) -> Result<Binding, SafeClose> {
        let _gate = self.admission_gate.lock().await;
        let requested = self.requested(&intent).map_err(safe)?;
        let ticket =
            AdmissionTicket::from_secret(intent.ticket.expose_for_consumption().to_owned())
                .map_err(safe)?;
        // Consume first, even when later pin/authority/capacity checks deny admission.
        // The typed repository claim reveals the subject before a takeover mutates DB.
        let claim = self
            .repository
            .consume(ConsumeClaim {
                digest: ticket.digest(),
                connection_id: ConnectionId::from_uuid(connection).map_err(safe)?,
                requested: requested.clone(),
                now: Utc::now(),
            })
            .await
            .map_err(safe)?;
        let ConsumeOutcome::Consumed(claim) = claim else {
            return Err(SafeClose::AUTH_REQUIRED);
        };
        if claim.record.pin != requested {
            return Err(SafeClose::AUTH_REQUIRED);
        }
        self.authority
            .reauthorize(&claim.record.subject, Utc::now())
            .await
            .map_err(safe)?;
        // Known takeover invalidation precedes the durable commit. Upstream I/O
        // never holds the synchronous publication fence across an await.
        {
            let _f = self.fence.lock().map_err(|_| SafeClose::UNAVAILABLE)?;
            self.views
                .lock()
                .map_err(|_| SafeClose::UNAVAILABLE)?
                .entries
                .remove(&(
                    claim.record.subject.key.event_id.uuid(),
                    claim.record.subject.key.user_id.uuid(),
                ));
        }
        let admitted = match self
            .repository
            .admit(&self.owner, &claim, Utc::now())
            .await
            .map_err(safe)?
        {
            AdmissionOutcome::Admitted(a) | AdmissionOutcome::Replaced(a) => a,
            _ => return Err(SafeClose::UNAVAILABLE),
        };
        self.publish(admitted.membership, connection, Uuid::new_v4())
            .await
            .map_err(safe)
    }
    async fn switch(
        &self,
        current: &Binding,
        intent: AdmissionIntent,
        _request: Uuid,
    ) -> Result<Binding, SafeClose> {
        let _gate = self.admission_gate.lock().await;
        let requested = self.requested(&intent).map_err(safe)?;
        let ticket =
            AdmissionTicket::from_secret(intent.ticket.expose_for_consumption().to_owned())
                .map_err(safe)?;
        self.invalidate(current);
        let result = self
            .service
            .switch_room(
                ticket,
                &subscription(current, &self.owner).map_err(safe)?,
                requested,
            )
            .await
            .map_err(safe)?;
        self.publish(result.membership, current.connection_id, Uuid::new_v4())
            .await
            .map_err(safe)
    }
    async fn resync(&self, current: &Binding, _request: Uuid) -> Result<Binding, SafeClose> {
        let _gate = self.admission_gate.lock().await;
        let m = self
            .repository
            .current_owned_membership(current.key, Utc::now())
            .await
            .map_err(safe)?
            .ok_or(SafeClose::UNAVAILABLE)?;
        if m.owner != self.owner
            || m.membership.connection_id.uuid() != current.connection_id
            || m.membership.session_id.as_str() != current.session_id.to_string()
        {
            return Err(SafeClose::UNAVAILABLE);
        }
        self.invalidate(current);
        self.publish(m.membership, current.connection_id, Uuid::new_v4())
            .await
            .map_err(safe)
    }
    async fn disconnected(&self, current: &Binding) {
        self.invalidate(current);
        let _ = self.request_cleanup(current);
        let _ = self.cleanup().await;
    }
}
