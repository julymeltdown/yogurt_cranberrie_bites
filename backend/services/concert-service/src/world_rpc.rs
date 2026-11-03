//! Additive C05 rooms/ticket producer. Composition supplies a typed checked
//! canonical subject; no browser actor, role, map or session is trusted here.
use crate::bootstrap::BootstrapService;
use crate::world::rpc::{authorize_current_membership, authorize_world};
use crate::world::{CurrentOwnedMembership, PgWorldRepository};
use chrono::{DateTime, SecondsFormat, Utc};
use concert_application::world_ticket::*;
use concert_domain::{AssetId, AuthUserId, EventId, ParticipantKey, Revision};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::Arc;
use tonic::{Request, Response, Status};
use uuid::Uuid;

pub use crate::world::rpc::{proto, POLICY, WORLD_BUDGET};

#[tonic::async_trait]
pub trait WorldCanonicalAuthority: Send + Sync {
    async fn rooms_subject(
        &self,
        actor: AuthUserId,
        event_id: EventId,
        session_family_id: Uuid,
    ) -> Result<(), WorldError>;

    async fn subject(
        &self,
        actor: AuthUserId,
        event_id: EventId,
        session_family_id: Uuid,
        release: &ReleasePins,
        access_expires_at: DateTime<Utc>,
        _observed_at: DateTime<Utc>,
    ) -> Result<VerifiedTicketSubject, WorldError>;
}

/// Production base adapter. Gateway's sealed Access + fresh Auth/Profile chain
/// has already established user/session/expiry; Concert adds a current event,
/// participant and restriction read and binds it to the validated release pins.
pub struct PgWorldCanonicalAuthority {
    bootstrap: Arc<BootstrapService>,
    pool: PgPool,
}
impl PgWorldCanonicalAuthority {
    pub fn new(bootstrap: Arc<BootstrapService>, pool: PgPool) -> Self {
        Self { bootstrap, pool }
    }
}
#[tonic::async_trait]
impl WorldCanonicalAuthority for PgWorldCanonicalAuthority {
    async fn rooms_subject(
        &self,
        actor: AuthUserId,
        event_id: EventId,
        session_family_id: Uuid,
    ) -> Result<(), WorldError> {
        self.bootstrap
            .world_admission(actor, event_id, session_family_id, Uuid::new_v4())
            .await
            .map(|_| ())
            .map_err(admission_error)
    }

    async fn subject(
        &self,
        actor: AuthUserId,
        event_id: EventId,
        session_family_id: Uuid,
        release: &ReleasePins,
        access_expires_at: DateTime<Utc>,
        _observed_at: DateTime<Utc>,
    ) -> Result<VerifiedTicketSubject, WorldError> {
        if release.event_id != event_id {
            return Err(WorldError::MapVersionMismatch);
        }
        let admission = self
            .bootstrap
            .world_admission(actor, event_id, session_family_id, Uuid::new_v4())
            .await
            .map_err(admission_error)?;
        if admission.key.user_id != actor || admission.key.event_id != event_id {
            return Err(WorldError::Denied);
        }
        let current: Option<(String, i64, String, i64)> = sqlx::query_as(
            "select map_id,map_revision,avatar_manifest_id,avatar_manifest_version from public.concert_world_rooms where event_id=$1 and is_default",
        )
        .bind(event_id.uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| WorldError::Unavailable)?;
        let Some((map_id, map_revision, avatar_id, avatar_version)) = current else {
            return Err(WorldError::Denied);
        };
        if release.map.id.as_str() != map_id
            || release.map.revision.get() != map_revision
            || release.avatar.manifest_id.as_str() != avatar_id
            || release.avatar.manifest_version.get() != avatar_version
        {
            return Err(WorldError::MapVersionMismatch);
        }
        Ok(VerifiedTicketSubject {
            key: admission.key,
            event_id,
            session_id: admission.session_id,
            access_expires_at,
            release: release.clone(),
        })
    }
}

fn admission_error(error: concert_application::bootstrap::BootstrapError) -> WorldError {
    match error {
        concert_application::bootstrap::BootstrapError::Denied
        | concert_application::bootstrap::BootstrapError::Unauthenticated
        | concert_application::bootstrap::BootstrapError::EventNotFound
        | concert_application::bootstrap::BootstrapError::EventNotOpen => WorldError::Denied,
        _ => WorldError::Unavailable,
    }
}

pub trait WorldTicketEntropy: Send + Sync {
    fn generate(&self) -> Result<AdmissionTicket, WorldError>;
}

/// Production CSPRNG adapter. It has no deterministic fallback.
pub struct CsprngTicketEntropy;
impl WorldTicketEntropy for CsprngTicketEntropy {
    fn generate(&self) -> Result<AdmissionTicket, WorldError> {
        use ring::rand::SecureRandom;
        let mut bytes = [0_u8; TICKET_BYTES];
        ring::rand::SystemRandom::new()
            .fill(&mut bytes)
            .map_err(|_| WorldError::Unavailable)?;
        AdmissionTicket::from_entropy(&bytes)
    }
}

pub struct WorldRpcService<A> {
    pub(crate) pool: PgPool,
    pub(crate) bootstrap: Arc<BootstrapService>,
    pub(crate) authority: A,
    pub(crate) generator: Arc<dyn WorldTicketEntropy>,
}
impl<A: WorldCanonicalAuthority> WorldRpcService<A> {
    pub fn new(
        pool: PgPool,
        bootstrap: Arc<BootstrapService>,
        authority: A,
        generator: Arc<dyn WorldTicketEntropy>,
    ) -> Self {
        Self {
            pool,
            bootstrap,
            authority,
            generator,
        }
    }
    fn repo(&self) -> PgWorldRepository {
        PgWorldRepository::new(self.pool.clone())
    }
    fn pins(&self, event: EventId) -> Result<ReleasePins, Status> {
        self.bootstrap
            .canonical_pins(event)
            .map_err(|_| Status::unavailable("World release unavailable"))
    }
}

impl<A: Clone> Clone for WorldRpcService<A> {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
            bootstrap: self.bootstrap.clone(),
            authority: self.authority.clone(),
            generator: self.generator.clone(),
        }
    }
}

fn stamp(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn status(error: WorldError) -> Status {
    match error {
        WorldError::Invalid | WorldError::CredentialInvalid => {
            Status::invalid_argument("Invalid world request")
        }
        WorldError::CredentialExpired | WorldError::CredentialConsumed => {
            Status::permission_denied("World ticket is no longer current")
        }
        WorldError::Denied | WorldError::OwnerFenced | WorldError::MembershipMissing => {
            Status::permission_denied("World access is not permitted")
        }
        WorldError::RoomFull => Status::resource_exhausted("World room is full"),
        WorldError::Conflict => Status::aborted("World admission conflict"),
        WorldError::MapVersionMismatch => Status::aborted("MAP_VERSION_MISMATCH"),
        WorldError::Unavailable => Status::unavailable("World authority unavailable"),
    }
}

fn envelope(value: Value) -> Result<Response<proto::WorldResponse>, Status> {
    let bytes = serde_json::to_vec(&value).map_err(|_| Status::internal("World JSON invalid"))?;
    if bytes.is_empty() || bytes.len() > 65_536 {
        return Err(Status::internal("World JSON exceeds bounds"));
    }
    let mut response = Response::new(proto::WorldResponse { world_json: bytes });
    response.metadata_mut().insert(
        "cache-control",
        "no-store"
            .parse()
            .map_err(|_| Status::internal("Response metadata unavailable"))?,
    );
    Ok(response)
}

fn parse_request(bytes: &[u8]) -> Result<(RoomId, MapPin, AvatarPin), Status> {
    if bytes.is_empty() || bytes.len() > 4096 {
        return Err(Status::invalid_argument("Invalid world ticket request"));
    }
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| Status::invalid_argument("Invalid world ticket request"))?;
    let map = value
        .as_object()
        .filter(|map| map.len() == 5)
        .ok_or_else(|| Status::invalid_argument("Invalid world ticket request"))?;
    let text = |name: &str| {
        map.get(name)
            .and_then(Value::as_str)
            .ok_or_else(|| Status::invalid_argument("Invalid world ticket request"))
    };
    let revision = |name: &str| {
        Revision::new(
            map.get(name)
                .and_then(Value::as_i64)
                .ok_or_else(|| Status::invalid_argument("Invalid world ticket request"))?,
        )
        .map_err(|_| Status::invalid_argument("Invalid world ticket request"))
    };
    let room = text("room_id")?;
    let room = Uuid::parse_str(room)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == room)
        .map(RoomId::from_uuid)
        .ok_or_else(|| Status::invalid_argument("Invalid world ticket request"))?;
    let asset = |name: &str| {
        AssetId::new(text(name)?.to_owned())
            .map_err(|_| Status::invalid_argument("Invalid world ticket request"))
    };
    Ok((
        room,
        MapPin::new(asset("map_id")?, revision("map_revision")?),
        AvatarPin::new(
            asset("avatar_manifest_id")?,
            revision("avatar_manifest_version")?,
        ),
    ))
}

fn catalog(value: &RoomCatalog) -> Value {
    json!({
        "event_id": value.event_id.uuid().to_string(),
        "default_room_id": value.default_room_id.uuid().to_string(),
        "items": value.rooms.iter().map(|room| json!({
            "room_id": room.room_id.uuid().to_string(),
            "label": room.label,
            "capacity": room.capacity,
            "availability": room.availability.as_str(),
        })).collect::<Vec<_>>(),
        "server_time": stamp(value.server_time),
    })
}

fn issuance(value: &TicketIssuance) -> Value {
    json!({
        "protocol_version": value.protocol_version(),
        "ticket": value.ticket.as_str(),
        "event_id": value.event_id.uuid().to_string(),
        "room_id": value.room_id.uuid().to_string(),
        "map_id": value.map.id.as_str(),
        "map_revision": value.map.revision.get(),
        "avatar_manifest_id": value.avatar.manifest_id.as_str(),
        "avatar_manifest_version": value.avatar.manifest_version.get(),
        "server_time": stamp(value.server_time),
        "expires_at": stamp(value.expires_at),
        "ws_path": value.ws_path(),
        "auth_timeout_ms": value.auth_timeout_milliseconds(),
    })
}

#[tonic::async_trait]
impl<A: WorldCanonicalAuthority + 'static> proto::world_service_server::WorldService
    for WorldRpcService<A>
{
    async fn list_rooms(
        &self,
        request: Request<proto::WorldRequest>,
    ) -> Result<Response<proto::WorldResponse>, Status> {
        let verified = authorize_world(&request, "ListRooms")?;
        verified.check()?;
        let event = EventId::from_uuid(verified.event_id());
        self.authority
            .rooms_subject(
                AuthUserId::from_uuid(verified.actor()),
                event,
                verified.session_family_id(),
            )
            .await
            .map_err(status)?;
        verified.check()?;
        let result = self.repo().room_catalog(event).await.map_err(status)?;
        verified.check()?;
        envelope(catalog(&result))
    }

    async fn issue_ticket(
        &self,
        request: Request<proto::WorldRequest>,
    ) -> Result<Response<proto::WorldResponse>, Status> {
        let verified = authorize_world(&request, "IssueTicket")?;
        verified.check()?;
        let (room, map, avatar) = parse_request(verified.http_body())?;
        let event = EventId::from_uuid(verified.event_id());
        let pins = self.pins(event)?;
        if pins.map != map || pins.avatar != avatar {
            return Err(Status::aborted("MAP_VERSION_MISMATCH"));
        }
        let room_catalog = self.repo().room_catalog(event).await.map_err(status)?;
        if !room_catalog.rooms.iter().any(|item| item.room_id == room) {
            return Err(Status::not_found("World room unavailable"));
        }
        let observed = chrono::Utc::now();
        let access_expires = DateTime::from_timestamp_micros(verified.access_expires_unix_micros())
            .ok_or_else(|| Status::permission_denied("World authority expired"))?;
        let subject = self
            .authority
            .subject(
                AuthUserId::from_uuid(verified.actor()),
                event,
                verified.session_family_id(),
                &pins,
                access_expires,
                observed,
            )
            .await
            .map_err(status)?;
        let issued = WorldService::new(
            UnavailableAuthority,
            self.repo(),
            UtcClock,
            ArcTicketGenerator(self.generator.clone()),
        )
        .issue_verified(
            subject,
            TicketRequest {
                event_id: event,
                room_id: room,
                map: pins.map,
                avatar: pins.avatar,
            },
        )
        .await
        .map_err(status)?;
        verified.check()?;
        envelope(issuance(&issued))
    }

    async fn current_membership(
        &self,
        request: Request<proto::CurrentMembershipRequest>,
    ) -> Result<Response<proto::CurrentMembershipResponse>, Status> {
        let verified = authorize_current_membership(&request)?;
        verified.check()?;
        let current = self
            .repo()
            .current_owned_membership(
                ParticipantKey {
                    event_id: EventId::from_uuid(verified.event_id()),
                    user_id: AuthUserId::from_uuid(verified.actor()),
                },
                Utc::now(),
            )
            .await
            .map_err(status)?;
        verified.check()?;
        let current = current.map(membership).unwrap_or_default();
        if current.present
            && (current.user_id != verified.actor().to_string()
                || current.event_id != verified.event_id().to_string())
        {
            return Err(Status::unavailable("World membership authority mismatch"));
        }
        let mut response = Response::new(current);
        response.metadata_mut().insert(
            "cache-control",
            "no-store"
                .parse()
                .map_err(|_| Status::internal("Response metadata unavailable"))?,
        );
        Ok(response)
    }
}

fn membership(value: CurrentOwnedMembership) -> proto::CurrentMembershipResponse {
    let item = value.membership;
    proto::CurrentMembershipResponse {
        present: true,
        user_id: item.key.user_id.uuid().to_string(),
        event_id: item.key.event_id.uuid().to_string(),
        session_id: item.session_id.as_str().to_owned(),
        connection_id: item.connection_id.uuid().to_string(),
        room_id: item.room_id.uuid().to_string(),
        owner_process_id: value.owner.process_id,
        owner_epoch: value.owner.epoch.to_string(),
        map_id: item.pin.map.id.as_str().to_owned(),
        map_revision: item.pin.map.revision.get(),
        avatar_manifest_id: item.pin.avatar.manifest_id.as_str().to_owned(),
        avatar_manifest_version: item.pin.avatar.manifest_version.get(),
        checked_at_unix_micros: Utc::now().timestamp_micros(),
        lease_expires_at_unix_micros: item.lease_until.timestamp_micros(),
        access_expires_at_unix_micros: item.access_expires_at.timestamp_micros(),
    }
}

struct UtcClock;
impl concert_application::Clock for UtcClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}
struct UnavailableAuthority;
#[tonic::async_trait]
impl WorldAuthority for UnavailableAuthority {
    async fn authorize(
        &self,
        _: &concert_application::identity::AccessCredential,
        _: EventId,
        _: DateTime<Utc>,
    ) -> Result<AuthorizedWorld, WorldError> {
        Err(WorldError::Unavailable)
    }
    async fn reauthorize(
        &self,
        _: &WorldSubject,
        _: DateTime<Utc>,
    ) -> Result<AuthorizationRenewal, WorldError> {
        Err(WorldError::Unavailable)
    }
}
struct ArcTicketGenerator(Arc<dyn WorldTicketEntropy>);
impl TicketGenerator for ArcTicketGenerator {
    fn generate(&self) -> Result<AdmissionTicket, WorldError> {
        self.0.generate()
    }
}
