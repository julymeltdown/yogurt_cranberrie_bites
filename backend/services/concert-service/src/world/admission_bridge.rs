//! Database admission and nonblocking runtime views. External live Auth/Profile
//! and recipient authority are explicit dependencies, never inferred from a ticket.
use super::{
    ingress::{AdmissionIntent, SafeClose, SwitchFailure, SwitchRejection},
    interest::{ProjectionError, PublicIdentity, RecipientPolicy, RecipientView},
    membership::{CanonicalAuthorityPort, CanonicalMembership, ClockAnchor},
    production_owners::ProductionOwners,
    simulation::{AuthorityError, Binding, ResourcePin},
    CurrentOwnedMembership, PgWorldRepository,
};
use crate::{
    adapters::{
        out::relationship_authority::{
            world_auth::{WorldAuthRead, WorldOwnerRequest},
            world_projection::WorldProjectionParent,
        },
        r#in::world_ws::AdmissionPort,
    },
    bootstrap::BootstrapService,
};
use async_trait::async_trait;
use chrono::{DateTime, TimeDelta, Utc};
use concert_application::{identity::AccessCredential, world_ticket::*};
use concert_domain::EventId;
use platform::actor_delegation::authority_deadline::AuthorityRpcDeadline;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use uuid::Uuid;

const MAX_MEMBERS: usize = 800;
/// Only production composition can supply the sealed Auth reader. The explicit
/// fixture branch is used by isolated DB tests and is never selected by startup.
enum AuthMode {
    Production(Arc<dyn WorldAuthRead>),
    Fixture,
}
#[derive(Clone)]
struct AuthWindow {
    subject: WorldSubject,
    credential_expires_at: DateTime<Utc>,
    valid_until: DateTime<Utc>,
    parent: Option<WorldProjectionParent>,
}
impl AuthWindow {
    fn check(&self, expected: &WorldSubject, expiry: DateTime<Utc>) -> Result<(), WorldError> {
        if let Some(parent) = &self.parent {
            parent.check()?;
        }
        if &self.subject != expected || self.credential_expires_at != expiry {
            return Err(WorldError::Conflict);
        }
        if Utc::now() >= self.valid_until || Utc::now() >= expiry {
            return Err(WorldError::CredentialExpired);
        }
        Ok(())
    }
}
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
enum OwnerPorts {
    Fixture(Arc<dyn LiveWorldOwners>),
    Production(Arc<ProductionOwners>),
}
impl OwnerPorts {
    fn fence(&self) -> Arc<Mutex<()>> {
        match self {
            Self::Fixture(p) => p.publication_fence(),
            Self::Production(p) => p.fence(),
        }
    }
    fn generation(&self) -> Uuid {
        match self {
            Self::Fixture(p) => p.generation(),
            Self::Production(p) => p.generation(),
        }
    }
    async fn pause(&self) -> Result<(), WorldError> {
        match self {
            Self::Fixture(_) => Ok(()),
            Self::Production(p) => p.pause().await,
        }
    }
    async fn activate(&self) -> Result<(), WorldError> {
        match self {
            Self::Fixture(_) => Ok(()),
            Self::Production(p) => p.activate().await,
        }
    }
    fn ready(&self, parent: Option<&WorldProjectionParent>) -> bool {
        match self {
            Self::Fixture(_) => true,
            Self::Production(p) => parent.is_some_and(|parent| p.ready(parent)),
        }
    }
    async fn current(&self, auth: &AuthWindow) -> Result<LiveOwnerView, WorldError> {
        match self {
            Self::Fixture(p) => p.current(&auth.subject, Utc::now()).await,
            Self::Production(p) => {
                p.current(auth.parent.as_ref().ok_or(WorldError::Unavailable)?)
                    .await
            }
        }
    }
}
#[derive(Clone)]
struct CurrentAuthority {
    owners: OwnerPorts,
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
        // Production Profile/presence requires the retained sealed parent and
        // is checked in publish, never reconstructed from this legacy raw port.
        let expires_at = match &self.owners {
            OwnerPorts::Fixture(owners) => {
                let live = owners.current(subject, now).await?;
                validate_live(&live, subject, now)?;
                live.expires_at
            }
            OwnerPorts::Production(_) => now + TimeDelta::milliseconds(5000),
        };
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
            access_expires_at: expires_at,
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
    auth_until: DateTime<Utc>,
    valid_until_ms: u64,
    visible: BTreeSet<Uuid>,
    generation: Uuid,
    owner_generation: Uuid,
    parent: Option<WorldProjectionParent>,
}
#[derive(Default)]
struct Views {
    entries: BTreeMap<(Uuid, Uuid), Entry>,
    pending: BTreeMap<(Uuid, Uuid), Entry>,
    cleanup: BTreeMap<(Uuid, Uuid, Uuid), Binding>,
    absent: BTreeSet<(Uuid, Uuid, Uuid)>,
    invalidations: BTreeMap<Uuid, super::invalidation::Removal>,
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
fn safe(error: WorldError) -> SafeClose {
    match error {
        WorldError::CredentialInvalid
        | WorldError::CredentialExpired
        | WorldError::CredentialConsumed => SafeClose::AUTH_REQUIRED,
        WorldError::Denied => SafeClose {
            code: 4403,
            label: "FORBIDDEN",
        },
        WorldError::MapVersionMismatch => SafeClose {
            code: 4409,
            label: "MAP_VERSION_MISMATCH",
        },
        WorldError::Invalid => SafeClose::INVALID,
        _ => SafeClose::UNAVAILABLE,
    }
}
/// Real production bridge once a qualified live-owner implementation is supplied.
/// All I/O occurs outside the shared publication fence; only bounded snapshots are read at 20Hz.
pub struct AdmissionBridge {
    pool: sqlx::PgPool,
    repository: PgWorldRepository,
    service: Service,
    authority: CurrentAuthority,
    owners: OwnerPorts,
    auth: AuthMode,
    owner: OwnerIdentity,
    event: EventId,
    rooms: BTreeMap<Uuid, Uuid>,
    resources: ResourcePin,
    anchor: ClockAnchor,
    started: Instant,
    fence: Arc<Mutex<()>>,
    views: Mutex<Views>,
    admission_gate: tokio::sync::Mutex<()>,
    invalidation_delivery_gate: tokio::sync::Mutex<()>,
    owner_until: Mutex<DateTime<Utc>>,
}
/// Reserve cleanup before the first durable admission mutation. Cancellation
/// leaves an exact bounded obligation for supervised async cleanup.
struct AdmissionAttempt<'a> {
    bridge: &'a AdmissionBridge,
    binding: Option<Binding>,
    invalidate_on_cancel: Option<Binding>,
}
impl Drop for AdmissionAttempt<'_> {
    fn drop(&mut self) {
        if let Some(b) = self.binding.as_ref() {
            if let Some(old) = self.invalidate_on_cancel.as_ref() {
                self.bridge.invalidate(old);
            }
            let _ = self.bridge.request_cleanup(b);
        }
    }
}
impl AdmissionBridge {
    pub(crate) fn invalidation_context(&self) -> (sqlx::PgPool, Uuid, Uuid, Uuid) {
        (
            self.pool.clone(),
            Uuid::parse_str(&self.owner.process_id).expect("validated owner"),
            self.owner.epoch,
            self.event.uuid(),
        )
    }
    pub(crate) fn invalidation_delivery_gate(&self) -> &tokio::sync::Mutex<()> {
        &self.invalidation_delivery_gate
    }
    pub(crate) fn invalidation_admission_gate(&self) -> &tokio::sync::Mutex<()> {
        &self.admission_gate
    }
    pub(crate) async fn pause_invalidation(&self) -> Result<(), WorldError> {
        self.owners.pause().await
    }
    pub(crate) fn begin_invalidation(
        &self,
        id: Uuid,
        removal: super::invalidation::Removal,
    ) -> Result<(), WorldError> {
        self.begin_invalidation_if(id, removal, || true)
    }
    /// Recheck an owning authorization budget after waiting for the actual
    /// native-handoff fence and before removing any active or pending view.
    pub(crate) fn begin_invalidation_if(
        &self,
        id: Uuid,
        removal: super::invalidation::Removal,
        authorized: impl FnOnce() -> bool,
    ) -> Result<(), WorldError> {
        let _f = self.fence.lock().map_err(|_| WorldError::Unavailable)?;
        let mut views = self.views.lock().map_err(|_| WorldError::Unavailable)?;
        if views.invalidations.len() >= 1000 && !views.invalidations.contains_key(&id) {
            return Err(WorldError::Unavailable);
        }
        if !authorized() {
            return Err(WorldError::Unavailable);
        }
        views.invalidations.insert(id, removal.clone());
        let mut removed = BTreeSet::new();
        // Split maps explicitly so removal never holds two mutable field borrows.
        views.entries.retain(|_, e| {
            if removal.matches(&e.binding, self.owner.epoch) {
                if let Some(p) = &e.parent {
                    p.cancel();
                }
                removed.insert(e.binding.key.user_id.uuid());
                false
            } else {
                true
            }
        });
        views.pending.retain(|_, e| {
            if removal.matches(&e.binding, self.owner.epoch) {
                if let Some(p) = &e.parent {
                    p.cancel();
                }
                removed.insert(e.binding.key.user_id.uuid());
                false
            } else {
                true
            }
        });
        if let super::invalidation::Removal::Scope(super::invalidation::Scope::Pair {
            a, b, ..
        }) = &removal
        {
            removed.insert(*a);
            removed.insert(*b);
        }
        for e in views.entries.values_mut() {
            let n = e.visible.len();
            e.visible.retain(|u| !removed.contains(u));
            if e.visible.len() != n {
                e.generation = Uuid::new_v4();
            }
        }
        for e in views.pending.values_mut() {
            let n = e.visible.len();
            e.visible.retain(|u| !removed.contains(u));
            if e.visible.len() != n {
                e.generation = Uuid::new_v4();
            }
        }
        drop(views);
        if let OwnerPorts::Production(p) = &self.owners {
            p.forget_invalidated(&removed)?;
        }
        Ok(())
    }
    pub(crate) fn finish_invalidation(&self, id: Uuid) -> Result<(), WorldError> {
        let _f = self.fence.lock().map_err(|_| WorldError::Unavailable)?;
        self.views
            .lock()
            .map_err(|_| WorldError::Unavailable)?
            .invalidations
            .remove(&id);
        Ok(())
    }
    /// Synthetic external authority for isolated DB regression fixtures only.
    /// Production settings have no route to this constructor.
    pub fn new_fixture(
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
        Self::build(
            pool,
            bootstrap,
            OwnerPorts::Fixture(owners),
            AuthMode::Fixture,
            owner,
            event,
            rooms,
            resources,
            anchor,
            started,
        )
    }
    pub(crate) fn new_auth_fixture(
        pool: sqlx::PgPool,
        bootstrap: Arc<BootstrapService>,
        owners: Arc<dyn LiveWorldOwners>,
        auth: Arc<dyn WorldAuthRead>,
        owner: OwnerIdentity,
        event: EventId,
        rooms: BTreeMap<Uuid, Uuid>,
        resources: ResourcePin,
        anchor: ClockAnchor,
        started: Instant,
    ) -> Result<Arc<Self>, WorldError> {
        Self::build(
            pool,
            bootstrap,
            OwnerPorts::Fixture(owners),
            AuthMode::Production(auth),
            owner,
            event,
            rooms,
            resources,
            anchor,
            started,
        )
    }
    pub(crate) fn new_production(
        pool: sqlx::PgPool,
        bootstrap: Arc<BootstrapService>,
        owners: Arc<ProductionOwners>,
        auth: Arc<dyn WorldAuthRead>,
        owner: OwnerIdentity,
        event: EventId,
        rooms: BTreeMap<Uuid, Uuid>,
        resources: ResourcePin,
        anchor: ClockAnchor,
        started: Instant,
    ) -> Result<Arc<Self>, WorldError> {
        Self::build(
            pool,
            bootstrap,
            OwnerPorts::Production(owners),
            AuthMode::Production(auth),
            owner,
            event,
            rooms,
            resources,
            anchor,
            started,
        )
    }
    fn build(
        pool: sqlx::PgPool,
        bootstrap: Arc<BootstrapService>,
        owners: OwnerPorts,
        auth: AuthMode,
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
        let fence = owners.fence();
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
            auth,
            owner,
            event,
            rooms,
            resources,
            anchor: anchor.clone(),
            started,
            fence,
            views: Mutex::new(Views::default()),
            admission_gate: tokio::sync::Mutex::new(()),
            invalidation_delivery_gate: tokio::sync::Mutex::new(()),
            owner_until: Mutex::new(anchor.utc),
        }))
    }
    fn auth_deadline(&self, expires: DateTime<Utc>) -> Result<AuthorityRpcDeadline, WorldError> {
        AuthorityRpcDeadline::new(expires.timestamp_micros(), Duration::from_secs(2))
            .map_err(|_| WorldError::Unavailable)
    }
    async fn observe_auth(&self, request: WorldOwnerRequest) -> Result<AuthWindow, WorldError> {
        let result = match &self.auth {
            AuthMode::Production(auth) => {
                let observation = auth.observe(&request).await?;
                observation.check(&request)?;
                AuthWindow {
                    subject: observation.subject().clone(),
                    credential_expires_at: observation.credential_expires_at(),
                    valid_until: observation.valid_until(),
                    parent: Some(request.projection_parent(&observation)?),
                }
            }
            AuthMode::Fixture => {
                request.check()?;
                AuthWindow {
                    subject: request.subject().clone(),
                    credential_expires_at: request.credential_expires_at(),
                    valid_until: (Utc::now() + TimeDelta::milliseconds(5000))
                        .min(request.credential_expires_at()),
                    parent: None,
                }
            }
        };
        result.check(request.subject(), request.credential_expires_at())?;
        Ok(result)
    }
    async fn auth_consumed(
        &self,
        claim: &TicketClaim,
        request_id: Uuid,
    ) -> Result<AuthWindow, WorldError> {
        self.observe_auth(WorldOwnerRequest::from_consumed(
            claim,
            request_id,
            self.auth_deadline(claim.record.expires_at)?,
        )?)
        .await
    }
    async fn auth_current(
        &self,
        binding: &Binding,
        request_id: Uuid,
        captured_expiry: DateTime<Utc>,
        deadline: Option<AuthorityRpcDeadline>,
    ) -> Result<AuthWindow, WorldError> {
        let current = self
            .repository
            .current_owned_membership(binding.key, Utc::now())
            .await?
            .ok_or(WorldError::MembershipMissing)?;
        let expected = subscription(binding, &self.owner)?;
        if current.membership.room_id != expected.room_id
            || current.membership.access_expires_at != captured_expiry
            || current.membership.pin.map.id != binding.resources.map_id
            || current.membership.pin.map.revision != binding.resources.map_revision
            || current.membership.pin.avatar.manifest_id != binding.resources.avatar_manifest_id
            || current.membership.pin.avatar.manifest_version
                != binding.resources.avatar_manifest_version
        {
            return Err(WorldError::Conflict);
        }
        let subject = WorldSubject {
            key: expected.key,
            event_id: expected.key.event_id,
            session_id: expected.session_id,
        };
        self.observe_auth(WorldOwnerRequest::from_current(
            &current,
            &self.owner,
            &subject,
            expected.connection_id,
            request_id,
            match deadline {
                Some(deadline) => deadline,
                None => self.auth_deadline(current.membership.lease_until)?,
            },
        )?)
        .await
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
    fn validate_requested(&self, intent: &AdmissionIntent) -> Result<(), WorldError> {
        if intent.event_id != self.event.uuid()
            || !self.rooms.contains_key(&intent.room_id)
            || intent.map_id != self.resources.map_id
            || intent.map_revision != self.resources.map_revision
            || intent.avatar_manifest_id != self.resources.avatar_manifest_id
            || intent.avatar_manifest_version != self.resources.avatar_manifest_version
        {
            return Err(WorldError::MapVersionMismatch);
        }
        Ok(())
    }
    fn invalidate(&self, b: &Binding) {
        if let Ok(_fence) = self.fence.lock() {
            if let Ok(mut views) = self.views.lock() {
                if views.entries.get(&key(b)).is_some_and(|e| e.binding == *b) {
                    views.entries.remove(&key(b));
                }
                if views.pending.get(&key(b)).is_some_and(|e| e.binding == *b) {
                    views.pending.remove(&key(b));
                }
            }
        }
    }
    async fn publish(
        &self,
        membership: Membership,
        connection: Uuid,
        stream: Uuid,
        mut auth: AuthWindow,
    ) -> Result<Binding, WorldError> {
        let generation = self.owners.generation();
        if generation.is_nil() {
            return Err(WorldError::Unavailable);
        }
        let now = Utc::now();
        let expected = subject(&membership);
        auth.check(&expected, membership.access_expires_at)?;
        if matches!(&self.owners, OwnerPorts::Production(_))
            && auth
                .parent
                .as_ref()
                .and_then(|p| p.current_membership())
                .is_none()
        {
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
            let parent = auth.parent.as_ref().ok_or(WorldError::Unavailable)?;
            let request = WorldOwnerRequest::from_current(
                &current,
                &self.owner,
                &expected,
                membership.connection_id,
                parent.request_id(),
                parent.deadline(),
            )?;
            auth = self.observe_auth(request).await?;
        }
        let live = self.owners.current(&auth).await?;
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
        let (mut visible, visibility_until) = match &self.owners {
            OwnerPorts::Fixture(owners) => (
                owners.visible(&expected, &subjects, Utc::now()).await?,
                auth.valid_until,
            ),
            OwnerPorts::Production(owners) => {
                let mut candidates = Vec::new();
                for subject in subjects {
                    if candidates
                        .iter()
                        .any(|m: &CurrentOwnedMembership| m.membership.key == subject.key)
                    {
                        continue;
                    }
                    if let Some(current) = self
                        .repository
                        .current_owned_membership(subject.key, Utc::now())
                        .await?
                    {
                        if current.owner == self.owner
                            && current.membership.room_id == membership.room_id
                        {
                            candidates.push(current);
                        }
                    }
                }
                owners
                    .visible(
                        auth.parent.as_ref().ok_or(WorldError::Unavailable)?,
                        &candidates,
                        &live.identity,
                    )
                    .await?
            }
        };
        if !visible.contains(&membership.key.user_id.uuid()) {
            return Err(WorldError::Denied);
        }
        let expires = membership
            .lease_until
            .min(membership.access_expires_at)
            .min(live.expires_at)
            .min(auth.valid_until)
            .min(visibility_until)
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
        auth.check(&expected, membership.access_expires_at)?;
        let mut views = self.views.lock().map_err(|_| WorldError::Unavailable)?;
        if views
            .invalidations
            .values()
            .any(|r| r.matches(&binding, self.owner.epoch))
        {
            return Err(WorldError::Denied);
        }
        // A signal may arrive during an unrelated recipient read. Mask pending
        // removals before reinstalling any delayed projection.
        for (key, entry) in views.entries.iter().chain(views.pending.iter()) {
            if views
                .invalidations
                .values()
                .any(|r| r.matches(&entry.binding, self.owner.epoch))
            {
                visible.remove(&key.1);
            }
        }
        for removal in views.invalidations.values() {
            if let super::invalidation::Removal::Scope(scope) = removal {
                visible.retain(|u| !scope.matches(binding.key.event_id.uuid(), *u));
            }
        }
        if views.entries.len() >= MAX_MEMBERS && !views.entries.contains_key(&key(&binding)) {
            return Err(WorldError::RoomFull);
        }
        let active = views
            .entries
            .get(&key(&binding))
            .is_some_and(|e| e.binding == binding);
        if !active
            && views.pending.len() >= MAX_MEMBERS
            && !views.pending.contains_key(&key(&binding))
        {
            return Err(WorldError::RoomFull);
        }
        let destination = if active {
            &mut views.entries
        } else {
            &mut views.pending
        };
        destination.insert(
            key(&binding),
            Entry {
                binding: binding.clone(),
                membership,
                live,
                auth_until: auth.valid_until,
                valid_until_ms,
                visible,
                generation: Uuid::new_v4(),
                owner_generation: generation,
                parent: auth.parent.clone(),
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
        self.owners.pause().await?;
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
        self.cleanup_inner().await?;
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
            let result=async {
                // Bind fresh Auth to the exact current DB owner, canonical family
                // and connection before any persistent lease extension.
                let auth = self.auth_current(b, Uuid::new_v4(),entry.membership.access_expires_at,None).await?;
                let LeaseState::Active(m)=self.service.renew(&subscription(b, &self.owner)?).await? else { return Err(WorldError::Denied); };
                auth.check(&subject(&m), m.access_expires_at)?;
                // Repository renew is a current read, not an expiry extension. Extend
                // only after fresh Auth/event/restriction checks and exact owner fencing.
                let lease=Utc::now().checked_add_signed(TimeDelta::milliseconds(5000)).ok_or(WorldError::Invalid)?.min(m.access_expires_at);
                let mut tx=self.pool.begin().await.map_err(|_|WorldError::Unavailable)?;
                super::closure::lock_event_open_in(&mut tx,b.key.event_id.uuid()).await?;
                let changed=sqlx::query("update public.concert_world_memberships m set lease_until=$6 where event_id=$1 and user_id=$2 and room_id=$3 and session_id=$4 and connection_id=$5 and access_expires_at=$7 and lease_until>clock_timestamp() and not disconnected and exists(select 1 from public.concert_world_owner_leases l where l.event_id=m.event_id and l.room_id=m.room_id and l.owner_process_id=$8 and l.owner_epoch=$9 and l.lease_until>clock_timestamp())")
                    .bind(b.key.event_id.uuid()).bind(b.key.user_id.uuid()).bind(b.room_id).bind(b.session_id).bind(b.connection_id).bind(lease).bind(m.access_expires_at).bind(&self.owner.process_id).bind(self.owner.epoch).execute(&mut *tx).await.map_err(|_|WorldError::Unavailable)?.rows_affected();
                if changed!=1 { return Err(WorldError::OwnerFenced); }
                tx.commit().await.map_err(|_|WorldError::Unavailable)?;
                let mut m=m; m.lease_until=lease; self.publish(m,b.connection_id,b.stream_id,auth).await.map(|_|())
            }.await;
            if result.is_err() {
                self.invalidate(b);
                let _ = self.request_cleanup(b);
            }
        }
        self.owners.activate().await?;
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
        if views.absent.contains(&cleanup_key(b)) {
            return Ok(());
        }
        if views.cleanup.len() >= MAX_MEMBERS && !views.cleanup.contains_key(&cleanup_key(b)) {
            return Err(AuthorityError::Unavailable);
        }
        views.cleanup.insert(cleanup_key(b), b.clone());
        Err(AuthorityError::Unavailable)
    }
    pub async fn cleanup(&self) -> Result<usize, WorldError> {
        let _gate = self.admission_gate.lock().await;
        self.owners.pause().await?;
        self.cleanup_inner().await
    }
    async fn cleanup_inner(&self) -> Result<usize, WorldError> {
        let pending: Vec<_> = self
            .views
            .lock()
            .map_err(|_| WorldError::Unavailable)?
            .cleanup
            .values()
            .cloned()
            .collect();
        for b in pending {
            let replaced = {
                let views = self.views.lock().map_err(|_| WorldError::Unavailable)?;
                views.entries.get(&key(&b)).is_some_and(|e| {
                    e.binding.connection_id == b.connection_id
                        && e.binding.session_id == b.session_id
                        && e.binding != b
                })
            };
            if replaced {
                let mut views = self.views.lock().map_err(|_| WorldError::Unavailable)?;
                views.cleanup.remove(&cleanup_key(&b));
                // Eviction loses only a confirmed-absence receipt, never an
                // obligation. A late repeat rechecks idempotent DB absence.
                if views.absent.len() >= MAX_MEMBERS {
                    views.absent.pop_first();
                }
                views.absent.insert(cleanup_key(&b));
                continue;
            }
            if self
                .service
                .leave(&subscription(&b, &self.owner)?)
                .await
                .is_ok()
            {
                let _fence = self.fence.lock().map_err(|_| WorldError::Unavailable)?;
                let mut views = self.views.lock().map_err(|_| WorldError::Unavailable)?;
                views.cleanup.remove(&cleanup_key(&b));
                // Eviction loses only a confirmed-absence receipt, never an
                // obligation. A late repeat rechecks idempotent DB absence.
                if views.absent.len() >= MAX_MEMBERS {
                    views.absent.pop_first();
                }
                views.absent.insert(cleanup_key(&b));
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
    fn publication_ready(&self, b: &Binding) -> bool {
        self.views.lock().is_ok_and(|views| {
            views.entries.get(&key(b)).is_some_and(|entry| {
                entry.binding == *b && self.owners.ready(entry.parent.as_ref())
            })
        })
    }
    fn publication_fence(&self) -> Arc<Mutex<()>> {
        self.fence.clone()
    }
    fn install_pending(&self, b: &Binding, now: u64) -> Result<(), AuthorityError> {
        let mut views = self.views.lock().map_err(|_| AuthorityError::Unavailable)?;
        if views.entries.get(&key(b)).is_some_and(|e| e.binding == *b) {
            return Ok(());
        }
        let pending = views
            .pending
            .get(&key(b))
            .filter(|e| {
                e.binding == *b
                    && e.valid_until_ms > now
                    && e.owner_generation == self.owners.generation()
            })
            .cloned()
            .ok_or(AuthorityError::Revoked)?;
        views.pending.remove(&key(b));
        views.entries.insert(key(b), pending);
        Ok(())
    }
    fn current(&self, b: &Binding, now: u64) -> Result<CanonicalMembership, AuthorityError> {
        let views = self.views.lock().map_err(|_| AuthorityError::Unavailable)?;
        let e = views.entries.get(&key(b)).ok_or(AuthorityError::Revoked)?;
        if e.binding != *b
            || e.parent
                .as_ref()
                .is_some_and(|parent| parent.check().is_err())
            || now >= e.valid_until_ms
            || e.owner_generation != self.owners.generation()
        {
            return Err(AuthorityError::Revoked);
        }
        Ok(CanonicalMembership {
            binding: b.clone(),
            checked_at_ms: now,
            lease_expires_at: e
                .membership
                .lease_until
                .min(e.live.expires_at)
                .min(e.auth_until)
                .min(
                    *self
                        .owner_until
                        .lock()
                        .map_err(|_| AuthorityError::Unavailable)?,
                ),
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
        self.validate_requested(&intent).map_err(safe)?;
        let auth = self
            .auth_consumed(&claim, Uuid::new_v4())
            .await
            .map_err(safe)?;
        self.authority
            .reauthorize(&claim.record.subject, Utc::now())
            .await
            .map_err(safe)?;
        auth.check(&claim.record.subject, claim.record.access_expires_at)
            .map_err(safe)?;
        self.owners.pause().await.map_err(safe)?;
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
        let stream = Uuid::new_v4();
        let pending = Binding {
            key: claim.record.subject.key,
            session_id: Uuid::parse_str(claim.record.subject.session_id.as_str())
                .map_err(|_| SafeClose::AUTH_REQUIRED)?,
            connection_id: connection,
            owner_id: Uuid::parse_str(&self.owner.process_id)
                .map_err(|_| SafeClose::UNAVAILABLE)?,
            room_id: claim.record.pin.room_id.uuid(),
            room_epoch: *self
                .rooms
                .get(&claim.record.pin.room_id.uuid())
                .ok_or(SafeClose::UNAVAILABLE)?,
            stream_id: stream,
            resources: self.resources.clone(),
        };
        {
            let mut views = self.views.lock().map_err(|_| SafeClose::UNAVAILABLE)?;
            if views.cleanup.len() >= MAX_MEMBERS {
                return Err(SafeClose::UNAVAILABLE);
            }
            views.cleanup.insert(cleanup_key(&pending), pending.clone());
        }
        let mut attempt = AdmissionAttempt {
            bridge: self,
            binding: Some(pending.clone()),
            invalidate_on_cancel: None,
        };
        let admitted = match self
            .repository
            .admit(&self.owner, &claim, Utc::now())
            .await
            .map_err(safe)?
        {
            AdmissionOutcome::Admitted(a) | AdmissionOutcome::Replaced(a) => a,
            _ => return Err(SafeClose::UNAVAILABLE),
        };
        let result = self
            .publish(admitted.membership, connection, stream, auth)
            .await
            .map_err(safe)?;
        self.views
            .lock()
            .map_err(|_| SafeClose::UNAVAILABLE)?
            .cleanup
            .remove(&cleanup_key(&pending));
        self.owners.activate().await.map_err(safe)?;
        attempt.binding = None;
        Ok(result)
    }
    async fn switch(
        &self,
        current: &Binding,
        intent: AdmissionIntent,
        request: Uuid,
    ) -> Result<Binding, SwitchFailure> {
        let _gate = self.admission_gate.lock().await;
        let requested = self
            .requested(&intent)
            .map_err(|e| SwitchFailure::Close(safe(e)))?;
        let ticket =
            AdmissionTicket::from_secret(intent.ticket.expose_for_consumption().to_owned())
                .map_err(|_| SwitchFailure::Rejected(SwitchRejection::TicketInvalid))?;
        let consumed = self
            .repository
            .consume(ConsumeClaim {
                digest: ticket.digest(),
                connection_id: ConnectionId::from_uuid(current.connection_id)
                    .map_err(|e| SwitchFailure::Close(safe(e)))?,
                requested: requested.clone(),
                now: Utc::now(),
            })
            .await
            .map_err(|e| SwitchFailure::Close(safe(e)))?;
        let ConsumeOutcome::Consumed(claim) = consumed else {
            return Err(SwitchFailure::Rejected(SwitchRejection::TicketInvalid));
        };
        if claim.record.pin != requested
            || claim.record.subject.key != current.key
            || claim.record.subject.session_id.as_str() != current.session_id.to_string()
        {
            return Err(SwitchFailure::Rejected(SwitchRejection::TicketInvalid));
        }
        self.validate_requested(&intent)
            .map_err(|_| SwitchFailure::Rejected(SwitchRejection::MapVersionMismatch))?;
        // Denied capacity leaves the current canonical stream untouched. The
        // eventual repository switch repeats this capacity check under its locks.
        let space:Option<bool>=sqlx::query_scalar("select capacity > (select count(*) from public.concert_world_memberships m where m.event_id=r.event_id and m.room_id=r.room_id and m.lease_until>clock_timestamp() and m.user_id<>$3) from public.concert_world_rooms r where event_id=$1 and room_id=$2 and availability='AVAILABLE'")
            .bind(current.key.event_id.uuid()).bind(intent.room_id).bind(current.key.user_id.uuid()).fetch_optional(&self.pool).await.map_err(|_|SwitchFailure::Close(SafeClose::UNAVAILABLE))?;
        match space {
            Some(true) => {}
            Some(false) => return Err(SwitchFailure::Rejected(SwitchRejection::RoomFull)),
            None => {
                return Err(SwitchFailure::Rejected(
                    SwitchRejection::ResourceUnavailable,
                ))
            }
        }
        let auth = self
            .auth_consumed(&claim, request)
            .await
            .map_err(|e| SwitchFailure::Close(safe(e)))?;
        self.authority
            .reauthorize(&claim.record.subject, Utc::now())
            .await
            .map_err(|e| SwitchFailure::Close(safe(e)))?;
        let held = self
            .repository
            .reauthorize(
                &subscription(current, &self.owner).map_err(|e| SwitchFailure::Close(safe(e)))?,
                Utc::now(),
            )
            .await
            .map_err(|e| SwitchFailure::Close(safe(e)))?;
        let LeaseState::Active(held) = held else {
            return Err(SwitchFailure::Close(SafeClose::AUTH_REQUIRED));
        };
        if held.access_expires_at != claim.record.access_expires_at {
            return Err(SwitchFailure::Rejected(SwitchRejection::TicketInvalid));
        }
        auth.check(&subject(&held), held.access_expires_at)
            .map_err(|e| SwitchFailure::Close(safe(e)))?;
        let stream = Uuid::new_v4();
        let pending = Binding {
            key: current.key,
            session_id: current.session_id,
            connection_id: current.connection_id,
            owner_id: current.owner_id,
            room_id: claim.record.pin.room_id.uuid(),
            room_epoch: *self
                .rooms
                .get(&claim.record.pin.room_id.uuid())
                .ok_or(SwitchFailure::Close(SafeClose::UNAVAILABLE))?,
            stream_id: stream,
            resources: self.resources.clone(),
        };
        {
            let mut views = self
                .views
                .lock()
                .map_err(|_| SwitchFailure::Close(SafeClose::UNAVAILABLE))?;
            if views.cleanup.len() >= MAX_MEMBERS {
                return Err(SwitchFailure::Close(SafeClose::UNAVAILABLE));
            }
            views.cleanup.insert(cleanup_key(&pending), pending.clone());
        }
        let mut attempt = AdmissionAttempt {
            bridge: self,
            binding: Some(pending.clone()),
            invalidate_on_cancel: Some(current.clone()),
        };
        self.owners
            .pause()
            .await
            .map_err(|e| SwitchFailure::Close(safe(e)))?;
        // Runtime BeginSwitch suppresses old publication. Until the DB response,
        // the prior qualified view is retained; a rejected transaction therefore
        // preserves its simulated position and sequence. Cancellation invalidates it.
        let result = self
            .repository
            .switch(&self.owner, &held, &claim, Utc::now())
            .await
            .map_err(|e| SwitchFailure::Close(safe(e)))?;
        let result = match result {
            SwitchOutcome::Switched(result) => result,
            SwitchOutcome::Full => {
                self.views
                    .lock()
                    .map_err(|_| SwitchFailure::Close(SafeClose::UNAVAILABLE))?
                    .cleanup
                    .remove(&cleanup_key(&pending));
                attempt.binding = None;
                return Err(SwitchFailure::Rejected(SwitchRejection::RoomFull));
            }
            _ => return Err(SwitchFailure::Close(SafeClose::UNAVAILABLE)),
        };
        // Known committed scope change is invalidated immediately; BE12 can
        // remove old presence while the bounded owner transition retains the
        // unclosed recovery slot until installation of the checked new stream.
        self.invalidate(current);
        let result = self
            .publish(result.membership, current.connection_id, stream, auth)
            .await
            .map_err(|e| SwitchFailure::Close(safe(e)))?;
        self.views
            .lock()
            .map_err(|_| SwitchFailure::Close(SafeClose::UNAVAILABLE))?
            .cleanup
            .remove(&cleanup_key(&pending));
        self.owners
            .activate()
            .await
            .map_err(|e| SwitchFailure::Close(safe(e)))?;
        attempt.binding = None;
        Ok(result)
    }
    async fn resync(&self, current: &Binding, request: Uuid) -> Result<Binding, SafeClose> {
        let _gate = self.admission_gate.lock().await;
        self.owners.pause().await.map_err(safe)?;
        let expiry = self
            .views
            .lock()
            .map_err(|_| SafeClose::UNAVAILABLE)?
            .entries
            .get(&key(current))
            .filter(|e| e.binding == *current)
            .map(|e| e.membership.access_expires_at)
            .ok_or(SafeClose::UNAVAILABLE)?;
        let auth = self
            .auth_current(current, request, expiry, None)
            .await
            .map_err(safe)?;
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
        let result = self
            .publish(m.membership, current.connection_id, Uuid::new_v4(), auth)
            .await
            .map_err(safe)?;
        self.owners.activate().await.map_err(safe)?;
        Ok(result)
    }
    async fn disconnected(&self, current: &Binding) {
        self.invalidate(current);
        let _ = self.request_cleanup(current);
        let _ = self.cleanup().await;
    }
}

#[cfg(test)]
#[path = "../../tests/world_runtime_support/auth_bridge.rs"]
mod auth_bridge_tests;

#[cfg(test)]
#[path = "../../tests/world_owner_composition/mod.rs"]
mod owner_composition_tests;
