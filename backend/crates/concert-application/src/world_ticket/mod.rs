//! One-use world admission credentials and authoritative room membership.
//!
//! This module never transports raw credentials over HTTP/WebSocket. A composition adapter creates
//! the 32-byte ticket from a CSPRNG, passes it here, and returns the opaque secret once in the C05
//! issuance response. Persistence receives only its SHA-256 digest.
use crate::identity::{AccessCredential, IdentityError, SessionId};
use crate::Clock;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use concert_domain::{AssetId, EventId, ParticipantKey, Revision};
use sha2::{Digest, Sha256};
use std::fmt;

pub const PROTOCOL_VERSION: &str = "1.0.0";
pub const WORLD_PATH: &str = "/world-ws";
pub const TICKET_TTL_SECONDS: i64 = 30;
pub const AUTH_TIMEOUT_MILLISECONDS: u32 = 3_000;
pub const FRESH_AUTHORITY_MILLISECONDS: i64 = 5_000;
pub const MIN_ROOM_CAPACITY: u16 = 1;
pub const MAX_ROOM_CAPACITY: u16 = 50;
pub const MAX_ROOMS: usize = 16;
pub const MAX_ROOM_LABEL: usize = 48;
pub const TICKET_BYTES: usize = 32;
pub const TICKET_ENCODED_LENGTH: usize = 43;
pub const DIGEST_BYTES: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum WorldError {
    #[error("world request is invalid")]
    Invalid,
    #[error("world admission credential is invalid")]
    CredentialInvalid,
    #[error("world admission credential expired")]
    CredentialExpired,
    #[error("world admission credential was already consumed")]
    CredentialConsumed,
    #[error("world authority denied admission")]
    Denied,
    #[error("world authority is unavailable")]
    Unavailable,
    #[error("room is full")]
    RoomFull,
    #[error("room owner is no longer current")]
    OwnerFenced,
    #[error("world membership is missing")]
    MembershipMissing,
    #[error("world membership conflicts with current authority")]
    Conflict,
}

impl From<IdentityError> for WorldError {
    fn from(error: IdentityError) -> Self {
        match error {
            IdentityError::Unavailable => Self::Unavailable,
            IdentityError::Unauthenticated | IdentityError::Denied => Self::Denied,
        }
    }
}

/// The only representation that may be returned to the browser. Debug never exposes its bytes.
#[derive(Clone, PartialEq, Eq)]
pub struct AdmissionTicket(String);
impl AdmissionTicket {
    /// Accept only the canonical unpadded base64url shape generated from exactly 32 bytes. The
    /// decode is strict so alternate encodings cannot create multiple stored representations.
    pub fn from_secret(value: String) -> Result<Self, WorldError> {
        if value.len() != TICKET_ENCODED_LENGTH {
            return Err(WorldError::CredentialInvalid);
        }
        let mut decoded = Vec::with_capacity(TICKET_BYTES);
        let bytes = value.as_bytes();
        let mut index = 0;
        while bytes.len() - index >= 2 {
            let remaining = bytes.len() - index;
            let width = remaining.min(4);
            if width == 1 {
                return Err(WorldError::CredentialInvalid);
            }
            let mut block = [0_u8; 4];
            for offset in 0..width {
                block[offset] = decode_base64url(bytes[index + offset])?;
            }
            decoded.push((block[0] << 2) | (block[1] >> 4));
            if width >= 3 {
                decoded.push((block[1] << 4) | (block[2] >> 2));
            } else if block[1] & 0x0f != 0 {
                return Err(WorldError::CredentialInvalid);
            }
            if width == 4 {
                decoded.push((block[2] << 6) | block[3]);
            } else if width == 3 && block[2] & 0x03 != 0 {
                return Err(WorldError::CredentialInvalid);
            }
            index += width;
        }
        if index != bytes.len() {
            return Err(WorldError::CredentialInvalid);
        }
        let digest = TicketDigest::of_bytes(&decoded);
        if encode_base64url(&decoded) != value || digest.0 == [0; DIGEST_BYTES] {
            return Err(WorldError::CredentialInvalid);
        }
        Ok(Self(value))
    }
    pub fn from_entropy(bytes: &[u8]) -> Result<Self, WorldError> {
        if bytes.len() != TICKET_BYTES {
            return Err(WorldError::CredentialInvalid);
        }
        Ok(Self(encode_base64url(bytes)))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn digest(&self) -> TicketDigest {
        TicketDigest(Sha256::digest(self.0.as_bytes()).into())
    }
}
impl fmt::Debug for AdmissionTicket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AdmissionTicket([REDACTED])")
    }
}

/// Hash-only storage identity. Raw ticket bytes never cross a persistence port.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TicketDigest([u8; DIGEST_BYTES]);
impl TicketDigest {
    pub fn from_bytes(value: [u8; DIGEST_BYTES]) -> Result<Self, WorldError> {
        if value == [0; DIGEST_BYTES] {
            return Err(WorldError::Invalid);
        }
        Ok(Self(value))
    }
    pub fn as_bytes(&self) -> &[u8; DIGEST_BYTES] {
        &self.0
    }
    fn of_bytes(value: &[u8]) -> Self {
        Self(Sha256::digest(value).into())
    }
}
impl fmt::Debug for TicketDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TicketDigest([REDACTED])")
    }
}

fn decode_base64url(value: u8) -> Result<u8, WorldError> {
    match value {
        b'A'..=b'Z' => Ok(value - b'A'),
        b'a'..=b'z' => Ok(value - b'a' + 26),
        b'0'..=b'9' => Ok(value - b'0' + 52),
        b'-' => Ok(62),
        b'_' => Ok(63),
        _ => Err(WorldError::CredentialInvalid),
    }
}
fn encode_base64url(value: &[u8]) -> String {
    // Canonical encoder for the one admitted length avoids adding a production dependency.
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut result = String::with_capacity(TICKET_ENCODED_LENGTH);
    for chunk in value.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        result.push(ALPHABET[(first >> 2) as usize] as char);
        result.push(ALPHABET[(((first & 0x03) << 4) | (second >> 4)) as usize] as char);
        if chunk.len() > 1 {
            result.push(ALPHABET[(((second & 0x0f) << 2) | (third >> 6)) as usize] as char);
        }
        if chunk.len() > 2 {
            result.push(ALPHABET[(third & 0x3f) as usize] as char);
        }
    }
    result
}

/// Stable room identifier. It deliberately has no hidden natural-key fallback.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RoomId(uuid::Uuid);
impl RoomId {
    pub fn from_uuid(value: uuid::Uuid) -> Self {
        Self(value)
    }
    pub fn uuid(self) -> uuid::Uuid {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ConnectionId(uuid::Uuid);
impl ConnectionId {
    pub fn from_uuid(value: uuid::Uuid) -> Result<Self, WorldError> {
        if value.is_nil() {
            return Err(WorldError::Invalid);
        }
        Ok(Self(value))
    }
    pub fn uuid(self) -> uuid::Uuid {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapPin {
    pub id: AssetId,
    pub revision: Revision,
}
impl MapPin {
    pub fn new(id: AssetId, revision: Revision) -> Self {
        Self { id, revision }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AvatarPin {
    pub manifest_id: AssetId,
    pub manifest_version: Revision,
}
impl AvatarPin {
    pub fn new(manifest_id: AssetId, manifest_version: Revision) -> Self {
        Self {
            manifest_id,
            manifest_version,
        }
    }
}

/// Exact immutable pins echoed into C05 authentication and snapshots.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourcePin {
    pub event_id: EventId,
    pub room_id: RoomId,
    pub map: MapPin,
    pub avatar: AvatarPin,
}

/// The narrow authoritative read bound to one requested session. Implementations must not treat a
/// caller claim as a grant and must return Unavailable when any required owner is unreachable.
#[async_trait]
pub trait WorldAuthority: Send + Sync {
    async fn authorize(
        &self,
        credential: &AccessCredential,
        event_id: EventId,
        at: DateTime<Utc>,
    ) -> Result<AuthorizedWorld, WorldError>;

    async fn reauthorize(
        &self,
        expected: &WorldSubject,
        at: DateTime<Utc>,
    ) -> Result<AuthorizationRenewal, WorldError>;
}

/// Canonical owner results. Event admission, current live session, suspension and checksum
/// metadata are represented by the explicit flags; the application checks every one.
#[derive(Clone, Debug)]
pub struct AuthorizedWorld {
    pub key: ParticipantKey,
    pub session_id: SessionId,
    pub access_expires_at: DateTime<Utc>,
    pub session_active: bool,
    pub event_allows_admission: bool,
    pub participant_admitted: bool,
    pub restrictions_allow: bool,
    pub map: MapPin,
    pub avatar: AvatarPin,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorldSubject {
    pub key: ParticipantKey,
    pub event_id: EventId,
    pub session_id: SessionId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuthorizationRenewal {
    pub session_active: bool,
    pub event_allows_admission: bool,
    pub participant_admitted: bool,
    pub restrictions_allow: bool,
    pub access_expires_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TicketRequest {
    pub event_id: EventId,
    pub room_id: RoomId,
    pub map: MapPin,
    pub avatar: AvatarPin,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TicketIssuance {
    pub ticket: AdmissionTicket,
    pub event_id: EventId,
    pub room_id: RoomId,
    pub map: MapPin,
    pub avatar: AvatarPin,
    pub server_time: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}
impl TicketIssuance {
    pub fn protocol_version(&self) -> &'static str {
        PROTOCOL_VERSION
    }
    pub fn ws_path(&self) -> &'static str {
        WORLD_PATH
    }
    pub fn auth_timeout_milliseconds(&self) -> u32 {
        AUTH_TIMEOUT_MILLISECONDS
    }
}

/// Subject data persisted with a digest. Access expiry is the JWT absolute deadline, while expires
/// at is the earlier of the 30-second ticket deadline and that access deadline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TicketRecord {
    pub digest: TicketDigest,
    pub subject: WorldSubject,
    pub pin: ResourcePin,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub access_expires_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IssueOutcome {
    Issued,
    Full,
    Hidden,
}

/// A candidate record is hashed and fully validated by the application before persistence. The
/// store must atomically insert it, enforce retention bounds and refuse a reused digest.
#[async_trait]
pub trait TicketRepository: Send + Sync {
    async fn issue(&self, record: TicketRecord) -> Result<IssueOutcome, WorldError>;
    async fn room_catalog(&self, event_id: EventId) -> Result<RoomCatalog, WorldError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomAvailability {
    Available,
    Full,
    Unavailable,
}
impl RoomAvailability {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Available => "AVAILABLE",
            Self::Full => "FULL",
            Self::Unavailable => "UNAVAILABLE",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomSummary {
    pub room_id: RoomId,
    pub label: String,
    pub capacity: u16,
    pub availability: RoomAvailability,
}
impl RoomSummary {
    pub fn new(
        room_id: RoomId,
        label: String,
        capacity: u16,
        availability: RoomAvailability,
    ) -> Result<Self, WorldError> {
        if label.trim().is_empty()
            || label.chars().count() > MAX_ROOM_LABEL
            || label.chars().any(char::is_control)
            || !(MIN_ROOM_CAPACITY..=MAX_ROOM_CAPACITY).contains(&capacity)
        {
            return Err(WorldError::Invalid);
        }
        Ok(Self {
            room_id,
            label,
            capacity,
            availability,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomCatalog {
    pub event_id: EventId,
    pub default_room_id: RoomId,
    pub rooms: Vec<RoomSummary>,
    pub server_time: DateTime<Utc>,
}
impl RoomCatalog {
    pub fn new(
        event_id: EventId,
        default_room_id: RoomId,
        rooms: Vec<RoomSummary>,
        server_time: DateTime<Utc>,
    ) -> Result<Self, WorldError> {
        if rooms.is_empty()
            || rooms.len() > MAX_ROOMS
            || rooms.iter().any(|room| room.room_id == default_room_id) == false
            || rooms
                .iter()
                .enumerate()
                .any(|(i, left)| rooms[..i].iter().any(|right| right.room_id == left.room_id))
        {
            return Err(WorldError::Invalid);
        }
        Ok(Self {
            event_id,
            default_room_id,
            rooms,
            server_time,
        })
    }
}

/// Immutable owner identity. The epoch distinguishes a restarted process that happens to reuse a
/// stable display name; only the highest leased epoch may write membership changes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnerIdentity {
    pub process_id: String,
    pub epoch: uuid::Uuid,
}
impl OwnerIdentity {
    pub fn new(process_id: String, epoch: uuid::Uuid) -> Result<Self, WorldError> {
        if process_id.is_empty()
            || process_id.len() > 128
            || process_id.chars().any(char::is_control)
            || epoch.is_nil()
        {
            return Err(WorldError::Invalid);
        }
        Ok(Self { process_id, epoch })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnerLease {
    pub event_id: EventId,
    pub room_id: RoomId,
    pub owner: OwnerIdentity,
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Membership {
    pub key: ParticipantKey,
    pub session_id: SessionId,
    pub room_id: RoomId,
    pub connection_id: ConnectionId,
    pub pin: ResourcePin,
    pub lease_until: DateTime<Utc>,
    pub access_expires_at: DateTime<Utc>,
}

/// Fields verified after digest consumption. The store must never inspect the caller's room or pins
/// before claiming the digest, and must not restore it when any later check fails.
#[derive(Clone, Debug)]
pub struct ConsumeClaim {
    pub digest: TicketDigest,
    pub connection_id: ConnectionId,
    pub requested: ResourcePin,
    pub now: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConsumeOutcome {
    Consumed(TicketClaim),
    Expired,
    AlreadyConsumed,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TicketClaim {
    pub record: TicketRecord,
    pub connection_id: ConnectionId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AdmissionOutcome {
    Admitted(Admission),
    Replaced(Admission),
    Full,
    Denied,
    Conflict,
    OwnerFenced,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Admission {
    pub membership: Membership,
    pub replaced: Option<ConnectionId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SwitchOutcome {
    Switched(Admission),
    Full,
    Denied,
    Conflict,
    OwnerFenced,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LeaseState {
    Active(Membership),
    Disconnected(Membership),
    Gone,
}

#[async_trait]
pub trait WorldRepository: Send + Sync {
    /// Claim the single owner process for one event room. An unexpired different epoch is fenced.
    async fn claim_owner(
        &self,
        event_id: EventId,
        room_id: RoomId,
        owner: &OwnerIdentity,
        now: DateTime<Utc>,
        duration: Duration,
    ) -> Result<OwnerLease, WorldError>;

    /// Claim the digest before any binding, authority or capacity check. The returned transaction
    /// token fences all later work; implementations must persist consumption immediately.
    async fn consume(&self, claim: ConsumeClaim) -> Result<ConsumeOutcome, WorldError>;

    async fn admit(
        &self,
        owner: &OwnerIdentity,
        claimed: &TicketClaim,
        now: DateTime<Utc>,
    ) -> Result<AdmissionOutcome, WorldError>;

    async fn switch(
        &self,
        owner: &OwnerIdentity,
        current: &Membership,
        claimed: &TicketClaim,
        now: DateTime<Utc>,
    ) -> Result<SwitchOutcome, WorldError>;

    async fn renew_owner(
        &self,
        event_id: EventId,
        room_id: RoomId,
        owner: &OwnerIdentity,
        now: DateTime<Utc>,
        duration: Duration,
    ) -> Result<OwnerLease, WorldError>;
    async fn disconnect(
        &self,
        key: ParticipantKey,
        event_id: EventId,
        connection_id: ConnectionId,
        session_id: &SessionId,
        now: DateTime<Utc>,
        grace: Duration,
    ) -> Result<LeaseState, WorldError>;
    async fn leave(
        &self,
        key: ParticipantKey,
        subscription: &SubscriptionLease,
        auth: &SessionId,
    ) -> Result<(), WorldError>;
    async fn reauthorize(
        &self,
        subscription: &SubscriptionLease,
        now: DateTime<Utc>,
    ) -> Result<LeaseState, WorldError>;
    async fn prune(&self, now: DateTime<Utc>) -> Result<Pruned, WorldError>;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pruned {
    pub tickets: u64,
    pub memberships: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubscriptionLease {
    pub key: ParticipantKey,
    pub session_id: SessionId,
    pub room_id: RoomId,
    pub connection_id: ConnectionId,
    pub owner: OwnerIdentity,
}

pub struct WorldService<A, R, C, G> {
    authority: A,
    repository: R,
    clock: C,
    generator: G,
}
impl<A: WorldAuthority, R: TicketRepository + WorldRepository, C: Clock, G: TicketGenerator>
    WorldService<A, R, C, G>
{
    pub fn new(authority: A, repository: R, clock: C, generator: G) -> Self {
        Self {
            authority,
            repository,
            clock,
            generator,
        }
    }

    pub async fn issue_ticket(
        &self,
        credential: &AccessCredential,
        request: TicketRequest,
    ) -> Result<TicketIssuance, WorldError> {
        let now = self.clock.now();
        let authority = self
            .authority
            .authorize(credential, request.event_id, now)
            .await?;
        validate_authority(&authority, request.event_id, &request.map, &request.avatar)?;
        let expires_at = now
            .checked_add_signed(Duration::seconds(TICKET_TTL_SECONDS))
            .ok_or(WorldError::Invalid)?
            .min(authority.access_expires_at);
        if expires_at <= now {
            return Err(WorldError::CredentialExpired);
        }
        let ticket = self.generator.generate()?;
        let pin = ResourcePin {
            event_id: request.event_id,
            room_id: request.room_id,
            map: request.map.clone(),
            avatar: request.avatar.clone(),
        };
        let record = TicketRecord {
            digest: ticket.digest(),
            subject: WorldSubject {
                key: authority.key,
                event_id: request.event_id,
                session_id: authority.session_id.clone(),
            },
            pin: pin.clone(),
            issued_at: now,
            expires_at,
            access_expires_at: authority.access_expires_at,
        };
        match self.repository.issue(record).await? {
            IssueOutcome::Issued => Ok(TicketIssuance {
                ticket,
                event_id: request.event_id,
                room_id: request.room_id,
                map: request.map,
                avatar: request.avatar,
                server_time: now,
                expires_at,
            }),
            IssueOutcome::Full => Err(WorldError::RoomFull),
            IssueOutcome::Hidden => Err(WorldError::Unavailable),
        }
    }

    pub async fn consume_and_admit(
        &self,
        ticket: AdmissionTicket,
        requested: ResourcePin,
        connection_id: ConnectionId,
        owner: &OwnerIdentity,
    ) -> Result<Admission, WorldError> {
        let now = self.clock.now();
        let digest = ticket.digest();
        drop(ticket);
        let claim = self
            .repository
            .consume(ConsumeClaim {
                digest,
                connection_id,
                requested: requested.clone(),
                now,
            })
            .await?;
        let claim = match claim {
            ConsumeOutcome::Consumed(value) => value,
            ConsumeOutcome::Expired => return Err(WorldError::CredentialExpired),
            ConsumeOutcome::AlreadyConsumed => return Err(WorldError::CredentialConsumed),
            ConsumeOutcome::Unknown => return Err(WorldError::CredentialInvalid),
        };
        if claim.record.pin != requested {
            return Err(WorldError::CredentialInvalid);
        }
        let durable_claim = self.reauthorize(&claim.record.subject).await?;
        if durable_claim.key != claim.record.subject.key
            || durable_claim.event_id != claim.record.subject.event_id
            || durable_claim.session_id != claim.record.subject.session_id
        {
            return Err(WorldError::Denied);
        }
        match self.repository.admit(owner, &claim, now).await? {
            AdmissionOutcome::Admitted(value) => Ok(value),
            AdmissionOutcome::Replaced(value) => Ok(value),
            AdmissionOutcome::Full => Err(WorldError::RoomFull),
            AdmissionOutcome::Denied => Err(WorldError::Denied),
            AdmissionOutcome::Conflict => Err(WorldError::Conflict),
            AdmissionOutcome::OwnerFenced => Err(WorldError::OwnerFenced),
        }
    }

    pub async fn switch_room(
        &self,
        ticket: AdmissionTicket,
        current: &SubscriptionLease,
        requested: ResourcePin,
    ) -> Result<Admission, WorldError> {
        let now = self.clock.now();
        let claim = self
            .repository
            .consume(ConsumeClaim {
                digest: ticket.digest(),
                connection_id: current.connection_id,
                requested: requested.clone(),
                now,
            })
            .await?;
        let claim = match claim {
            ConsumeOutcome::Consumed(value) => value,
            ConsumeOutcome::Expired => return Err(WorldError::CredentialExpired),
            ConsumeOutcome::AlreadyConsumed => return Err(WorldError::CredentialConsumed),
            ConsumeOutcome::Unknown => return Err(WorldError::CredentialInvalid),
        };
        if claim.record.pin != requested
            || claim.record.subject.key != current.key
            || claim.record.subject.event_id != requested.event_id
        {
            return Err(WorldError::CredentialInvalid);
        }
        let durable_claim = self.reauthorize(&claim.record.subject).await?;
        if durable_claim != claim.record.subject {
            return Err(WorldError::Denied);
        }
        let membership = self.repository.reauthorize(current, now).await?;
        let membership = match membership {
            LeaseState::Active(value) | LeaseState::Disconnected(value) => value,
            LeaseState::Gone => return Err(WorldError::MembershipMissing),
        };
        match self
            .repository
            .switch(&current.owner, &membership, &claim, now)
            .await?
        {
            SwitchOutcome::Switched(value) => Ok(value),
            SwitchOutcome::Full => Err(WorldError::RoomFull),
            SwitchOutcome::Denied => Err(WorldError::Denied),
            SwitchOutcome::Conflict => Err(WorldError::Conflict),
            SwitchOutcome::OwnerFenced => Err(WorldError::OwnerFenced),
        }
    }

    async fn reauthorize(&self, subject: &WorldSubject) -> Result<WorldSubject, WorldError> {
        let current = self
            .authority
            .reauthorize(subject, self.clock.now())
            .await?;
        let now = self.clock.now();
        if !current.session_active
            || !current.event_allows_admission
            || !current.participant_admitted
            || !current.restrictions_allow
            || current.access_expires_at <= now
        {
            return Err(WorldError::Denied);
        }
        Ok(subject.clone())
    }

    pub async fn catalog(&self, event_id: EventId) -> Result<RoomCatalog, WorldError> {
        self.repository.room_catalog(event_id).await
    }
    pub async fn disconnect(
        &self,
        subscription: &SubscriptionLease,
        grace: Duration,
    ) -> Result<LeaseState, WorldError> {
        self.repository
            .disconnect(
                subscription.key,
                subscription.key.event_id,
                subscription.connection_id,
                &subscription.session_id,
                self.clock.now(),
                grace,
            )
            .await
    }
    pub async fn leave(&self, subscription: &SubscriptionLease) -> Result<(), WorldError> {
        self.repository
            .leave(subscription.key, subscription, &subscription.session_id)
            .await
    }
    pub async fn renew(&self, subscription: &SubscriptionLease) -> Result<LeaseState, WorldError> {
        let now = self.clock.now();
        self.reauthorize(&WorldSubject {
            key: subscription.key,
            event_id: subscription.key.event_id,
            session_id: subscription.session_id.clone(),
        })
        .await?;
        self.repository.reauthorize(subscription, now).await
    }
    pub async fn claim_owner(
        &self,
        event_id: EventId,
        room_id: RoomId,
        owner: &OwnerIdentity,
        duration: Duration,
    ) -> Result<OwnerLease, WorldError> {
        self.repository
            .claim_owner(event_id, room_id, owner, self.clock.now(), duration)
            .await
    }
    pub async fn renew_owner(
        &self,
        event_id: EventId,
        room_id: RoomId,
        owner: &OwnerIdentity,
        duration: Duration,
    ) -> Result<OwnerLease, WorldError> {
        self.repository
            .renew_owner(event_id, room_id, owner, self.clock.now(), duration)
            .await
    }
    pub async fn prune(&self) -> Result<Pruned, WorldError> {
        self.repository.prune(self.clock.now()).await
    }
}

fn validate_authority(
    value: &AuthorizedWorld,
    event_id: EventId,
    map: &MapPin,
    avatar: &AvatarPin,
) -> Result<(), WorldError> {
    if value.key.event_id != event_id
        || !value.session_active
        || !value.event_allows_admission
        || !value.participant_admitted
        || !value.restrictions_allow
        || value.map != *map
        || value.avatar != *avatar
    {
        return Err(WorldError::Denied);
    }
    Ok(())
}

/// Entropy is injected at the composition boundary; tests use deterministic 32-byte values.
pub trait TicketGenerator: Send + Sync {
    fn generate(&self) -> Result<AdmissionTicket, WorldError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticket_is_exact_unpadded_base64url_and_redacted() {
        let ticket = AdmissionTicket::from_entropy(&[0xfb; TICKET_BYTES]).unwrap();
        assert_eq!(ticket.as_str().len(), TICKET_ENCODED_LENGTH);
        assert!(ticket
            .as_str()
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'));
        assert_eq!(format!("{ticket:?}"), "AdmissionTicket([REDACTED])");
        assert!(!format!("{:?}", ticket.digest()).contains(ticket.as_str()));
        assert_eq!(
            AdmissionTicket::from_secret(ticket.as_str().to_owned()).unwrap(),
            ticket
        );
    }

    #[test]
    fn malformed_or_short_ticket_is_rejected_before_hashing() {
        let invalid_tail = ".......................................";
        for value in [
            "",
            "a",
            "!_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-",
            &"a".repeat(43),
            invalid_tail,
        ] {
            assert!(AdmissionTicket::from_secret(value.into()).is_err());
        }
        assert!(AdmissionTicket::from_entropy(&[0; 31]).is_err());
    }

    #[test]
    fn room_catalog_bounds_and_default_membership_are_validated() {
        let event = EventId::from_uuid(uuid::Uuid::new_v4());
        let room = RoomId::from_uuid(uuid::Uuid::new_v4());
        let item =
            RoomSummary::new(room, "Main room".into(), 50, RoomAvailability::Available).unwrap();
        assert!(RoomCatalog::new(event, room, vec![item.clone()], Utc::now()).is_ok());
        assert!(RoomCatalog::new(event, room, vec![item.clone(); 17], Utc::now()).is_err());
        assert!(
            RoomSummary::new(room, "Main room".into(), 51, RoomAvailability::Available).is_err()
        );
        assert!(RoomSummary::new(room, String::new(), 1, RoomAvailability::Available).is_err());
    }
}
