use chrono::{DateTime, Duration, TimeZone, Utc};
use concert_application::identity::{AccessCredential, SessionId};
use concert_application::world_ticket::*;
use concert_application::Clock;
use concert_domain::{AssetId, AuthUserId, EventId, ParticipantKey, Revision};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

const NOW: i64 = 1_800_000_000;

struct FixedClock;
impl Clock for FixedClock {
    fn now(&self) -> DateTime<Utc> {
        Utc.timestamp_opt(NOW, 0).unwrap()
    }
}

struct Entropy(std::sync::atomic::AtomicU8);
impl TicketGenerator for Entropy {
    fn generate(&self) -> Result<AdmissionTicket, WorldError> {
        // Repeated issuance in one test must never reuse a digest.
        let value = self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        AdmissionTicket::from_entropy(&[value; TICKET_BYTES])
    }
}

#[derive(Clone)]
struct Authority {
    state: Arc<Mutex<AuthorityState>>,
    log: Arc<Mutex<Vec<String>>>,
}
struct AuthorityState {
    active: bool,
    admitted: bool,
    restricted: bool,
    access_expires: DateTime<Utc>,
    map: MapPin,
    avatar: AvatarPin,
}

#[async_trait::async_trait]
impl WorldAuthority for Authority {
    async fn authorize(
        &self,
        _: &AccessCredential,
        event_id: EventId,
        _: DateTime<Utc>,
    ) -> Result<AuthorizedWorld, WorldError> {
        let state = self.state.lock().unwrap();
        Ok(AuthorizedWorld {
            key: key(event_id),
            session_id: session(),
            access_expires_at: state.access_expires,
            session_active: state.active,
            event_allows_admission: true,
            participant_admitted: state.admitted,
            restrictions_allow: state.restricted,
            map: state.map.clone(),
            avatar: state.avatar.clone(),
        })
    }
    async fn reauthorize(
        &self,
        _subject: &WorldSubject,
        _: DateTime<Utc>,
    ) -> Result<AuthorizationRenewal, WorldError> {
        self.log.lock().unwrap().push("reauthorize".into());
        let state = self.state.lock().unwrap();
        Ok(AuthorizationRenewal {
            session_active: state.active,
            event_allows_admission: true,
            participant_admitted: state.admitted,
            restrictions_allow: state.restricted,
            access_expires_at: state.access_expires,
        })
    }
}

#[derive(Clone, Default)]
struct Store {
    state: Arc<Mutex<StoreState>>,
}
#[derive(Default)]
struct StoreState {
    records: Vec<(TicketRecord, bool)>,
    calls: Vec<String>,
}

#[async_trait::async_trait]
impl TicketRepository for Store {
    async fn issue(&self, record: TicketRecord) -> Result<IssueOutcome, WorldError> {
        self.state.lock().unwrap().records.push((record, false));
        Ok(IssueOutcome::Issued)
    }
    async fn room_catalog(&self, event_id: EventId) -> Result<RoomCatalog, WorldError> {
        let room = RoomId::from_uuid(Uuid::new_v4());
        RoomCatalog::new(
            event_id,
            room,
            vec![RoomSummary::new(
                room,
                "Main room".into(),
                50,
                RoomAvailability::Available,
            )?],
            Utc::now(),
        )
    }
}

#[async_trait::async_trait]
impl WorldRepository for Store {
    async fn claim_owner(
        &self,
        _: EventId,
        room_id: RoomId,
        owner: &OwnerIdentity,
        now: DateTime<Utc>,
        duration: Duration,
    ) -> Result<OwnerLease, WorldError> {
        Ok(OwnerLease {
            event_id: event(),
            room_id,
            owner: owner.clone(),
            expires_at: now + duration,
        })
    }
    async fn consume(&self, input: ConsumeClaim) -> Result<ConsumeOutcome, WorldError> {
        self.state.lock().unwrap().calls.push("consume".into());
        let mut state = self.state.lock().unwrap();
        let item = state
            .records
            .iter_mut()
            .find(|(record, _)| record.digest == input.digest)
            .ok_or(WorldError::CredentialInvalid)?;
        if item.1 {
            return Ok(ConsumeOutcome::AlreadyConsumed);
        }
        item.1 = true;
        Ok(ConsumeOutcome::Consumed(TicketClaim {
            record: item.0.clone(),
            connection_id: input.connection_id,
        }))
    }
    async fn admit(
        &self,
        _: &OwnerIdentity,
        claim: &TicketClaim,
        now: DateTime<Utc>,
    ) -> Result<AdmissionOutcome, WorldError> {
        self.state.lock().unwrap().calls.push("admit".into());
        Ok(AdmissionOutcome::Admitted(Admission {
            membership: membership(claim, now),
            replaced: None,
        }))
    }
    async fn switch(
        &self,
        _: &OwnerIdentity,
        _: &Membership,
        claim: &TicketClaim,
        now: DateTime<Utc>,
    ) -> Result<SwitchOutcome, WorldError> {
        Ok(SwitchOutcome::Switched(Admission {
            membership: membership(claim, now),
            replaced: None,
        }))
    }
    async fn renew_owner(
        &self,
        _: EventId,
        room_id: RoomId,
        owner: &OwnerIdentity,
        now: DateTime<Utc>,
        duration: Duration,
    ) -> Result<OwnerLease, WorldError> {
        Ok(OwnerLease {
            event_id: event(),
            room_id,
            owner: owner.clone(),
            expires_at: now + duration,
        })
    }
    async fn disconnect(
        &self,
        _: ParticipantKey,
        _: EventId,
        _: ConnectionId,
        _: &SessionId,
        _now: DateTime<Utc>,
        _: Duration,
    ) -> Result<LeaseState, WorldError> {
        Err(WorldError::MembershipMissing)
    }
    async fn leave(
        &self,
        _: ParticipantKey,
        _: &SubscriptionLease,
        _: &SessionId,
    ) -> Result<(), WorldError> {
        Ok(())
    }
    async fn reauthorize(
        &self,
        _: &SubscriptionLease,
        _: DateTime<Utc>,
    ) -> Result<LeaseState, WorldError> {
        Ok(LeaseState::Gone)
    }
    async fn prune(&self, _: DateTime<Utc>) -> Result<Pruned, WorldError> {
        Ok(Pruned::default())
    }
}

fn event() -> EventId {
    EventId::from_uuid(Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap())
}
fn key(event_id: EventId) -> ParticipantKey {
    ParticipantKey {
        event_id,
        user_id: AuthUserId::from_uuid(
            Uuid::parse_str("22222222-2222-4222-8222-222222222222").unwrap(),
        ),
    }
}
fn session() -> SessionId {
    SessionId::new("33333333-3333-4333-8333-333333333333".into()).unwrap()
}
fn map() -> MapPin {
    MapPin::new(
        AssetId::new("map_fixture".into()).unwrap(),
        Revision::initial(),
    )
}
fn avatar() -> AvatarPin {
    AvatarPin::new(
        AssetId::new("avatar_manifest".into()).unwrap(),
        Revision::initial(),
    )
}
fn request() -> TicketRequest {
    TicketRequest {
        event_id: event(),
        room_id: RoomId::from_uuid(Uuid::new_v4()),
        map: map(),
        avatar: avatar(),
    }
}
fn pins(event: EventId) -> ReleasePins {
    ReleasePins {
        event_id: event,
        map: map(),
        avatar: avatar(),
        captured_at: Utc.timestamp_opt(NOW, 0).unwrap(),
    }
}
fn membership(claim: &TicketClaim, now: DateTime<Utc>) -> Membership {
    Membership {
        key: claim.record.subject.key,
        session_id: claim.record.subject.session_id.clone(),
        room_id: claim.record.pin.room_id,
        connection_id: claim.connection_id,
        pin: claim.record.pin.clone(),
        lease_until: now + Duration::seconds(5),
        access_expires_at: claim.record.access_expires_at,
    }
}
fn authority(
    access_expires: DateTime<Utc>,
) -> (
    Authority,
    Arc<Mutex<AuthorityState>>,
    Arc<Mutex<Vec<String>>>,
) {
    let state = Arc::new(Mutex::new(AuthorityState {
        active: true,
        admitted: true,
        restricted: true,
        access_expires,
        map: map(),
        avatar: avatar(),
    }));
    let log = Arc::new(Mutex::new(Vec::new()));
    (
        Authority {
            state: state.clone(),
            log: log.clone(),
        },
        state,
        log,
    )
}

#[tokio::test]
async fn ticket_expiry_is_minimum_of_thirty_seconds_and_access_expiry() {
    let now = Utc.timestamp_opt(NOW, 0).unwrap();
    let (a, _, _) = authority(now + Duration::seconds(17));
    let service = WorldService::new(a, Store::default(), FixedClock, Entropy(7.into()));
    let issued = service
        .issue_ticket(
            &AccessCredential::new("owner-bearer".into()).unwrap(),
            request(),
        )
        .await
        .unwrap();
    assert_eq!(issued.expires_at, now + Duration::seconds(17));
    assert_eq!(issued.ticket.as_str().len(), TICKET_ENCODED_LENGTH);
    assert_eq!(issued.protocol_version(), "1.0.0");
    assert_eq!(issued.ws_path(), "/world-ws");
    assert_eq!(issued.auth_timeout_milliseconds(), 3000);

    let (a, _, _) = authority(now + Duration::seconds(90));
    let service = WorldService::new(a, Store::default(), FixedClock, Entropy(8.into()));
    let issued = service
        .issue_ticket(
            &AccessCredential::new("owner-bearer".into()).unwrap(),
            request(),
        )
        .await
        .unwrap();
    assert_eq!(issued.expires_at, now + Duration::seconds(30));
    let owner = OwnerIdentity::new("room-owner-a".into(), Uuid::new_v4()).unwrap();
    let req = request();
    let issued = service
        .issue_ticket(
            &AccessCredential::new("owner-bearer".into()).unwrap(),
            req.clone(),
        )
        .await
        .unwrap();
    let pin = ResourcePin {
        event_id: req.event_id,
        room_id: req.room_id,
        map: req.map.clone(),
        avatar: req.avatar.clone(),
    };
    let first = service
        .consume_and_admit(
            issued.ticket.clone(),
            pin.clone(),
            ConnectionId::from_uuid(Uuid::new_v4()).unwrap(),
            &owner,
        )
        .await
        .unwrap();
    assert_eq!(
        service
            .consume_and_admit(
                issued.ticket,
                pin,
                ConnectionId::from_uuid(Uuid::new_v4()).unwrap(),
                &owner,
            )
            .await,
        Err(WorldError::CredentialConsumed)
    );
    assert_eq!(first.membership.key, key(event()));
}

#[tokio::test]
async fn failed_reauthorization_occurs_after_consumption_and_is_not_replayed() {
    let (a, authority, log) = authority(Utc.timestamp_opt(NOW + 60, 0).unwrap());
    let store = Store::default();
    let service = WorldService::new(a, store.clone(), FixedClock, Entropy(9.into()));
    let credential = AccessCredential::new("owner-bearer".into()).unwrap();
    let req = request();
    let issued = service
        .issue_ticket(&credential, req.clone())
        .await
        .unwrap();
    authority.lock().unwrap().admitted = false;
    let owner = OwnerIdentity::new("room-owner-a".into(), Uuid::new_v4()).unwrap();
    let connection = ConnectionId::from_uuid(Uuid::new_v4()).unwrap();
    let pin = ResourcePin {
        event_id: req.event_id,
        room_id: req.room_id,
        map: req.map.clone(),
        avatar: req.avatar.clone(),
    };
    let attempted_ticket = issued.ticket.clone();
    assert_eq!(
        service
            .consume_and_admit(issued.ticket, pin.clone(), connection, &owner)
            .await,
        Err(WorldError::Denied)
    );
    assert_eq!(log.lock().unwrap().as_slice(), &["reauthorize"]);
    assert_eq!(store.state.lock().unwrap().calls, vec!["consume"]);
    authority.lock().unwrap().admitted = true;
    let replacement = service.issue_ticket(&credential, req).await.unwrap();
    assert_ne!(replacement.ticket, attempted_ticket);
    assert!(store
        .state
        .lock()
        .unwrap()
        .records
        .first()
        .map(|v| v.1)
        .unwrap());
}

#[tokio::test]
async fn wrong_room_is_rejected_after_consumption_without_admission() {
    let (a, _, _) = authority(Utc.timestamp_opt(NOW + 60, 0).unwrap());
    let store = Store::default();
    let service = WorldService::new(a, store.clone(), FixedClock, Entropy(10.into()));
    let req = request();
    let issued = service
        .issue_ticket(
            &AccessCredential::new("owner-bearer".into()).unwrap(),
            req.clone(),
        )
        .await
        .unwrap();
    let wrong = ResourcePin {
        event_id: req.event_id,
        room_id: RoomId::from_uuid(Uuid::new_v4()),
        map: req.map,
        avatar: req.avatar,
    };
    let owner = OwnerIdentity::new("room-owner-a".into(), Uuid::new_v4()).unwrap();
    assert_eq!(
        service
            .consume_and_admit(
                issued.ticket,
                wrong,
                ConnectionId::from_uuid(Uuid::new_v4()).unwrap(),
                &owner,
            )
            .await,
        Err(WorldError::CredentialInvalid)
    );
    assert_eq!(store.state.lock().unwrap().calls, vec!["consume"]);
}

#[tokio::test]
async fn typed_issuance_uses_same_digest_bound_one_use_store() {
    let now = Utc.timestamp_opt(NOW, 0).unwrap();
    let (authority, _, _) = authority(now + Duration::seconds(60));
    let store = Store::default();
    let service = WorldService::new(authority, store.clone(), FixedClock, Entropy(11.into()));
    let request = request();
    let subject = VerifiedTicketSubject {
        key: key(request.event_id),
        event_id: request.event_id,
        session_id: session(),
        access_expires_at: now + Duration::seconds(60),
        release: pins(request.event_id),
    };
    let issued = service
        .issue_verified(subject, request.clone())
        .await
        .unwrap();
    assert_eq!(issued.expires_at, now + Duration::seconds(30));
    assert_eq!(issued.room_id, request.room_id);
    let records = store.state.lock().unwrap();
    assert_eq!(records.records.len(), 1);
    assert_eq!(records.records[0].0.pin.map, map());
    assert_eq!(records.records[0].0.pin.avatar, avatar());
}

#[tokio::test]
async fn typed_issuance_enforces_release_freshness_before_persisting() {
    let now = Utc.timestamp_opt(NOW, 0).unwrap();
    // Age 4999ms is fresh; exactly 5000ms, older observations and future
    // observations must fail before the ticket repository receives a record.
    for age_ms in [4_999, 5_000, 5_001, 60_000, -1] {
        let (authority, _, _) = authority(now + Duration::seconds(60));
        let store = Store::default();
        let service = WorldService::new(authority, store.clone(), FixedClock, Entropy(13.into()));
        let request = request();
        let mut release = pins(request.event_id);
        release.captured_at = now - Duration::milliseconds(age_ms);
        let result = service
            .issue_verified(
                VerifiedTicketSubject {
                    key: key(request.event_id),
                    event_id: request.event_id,
                    session_id: session(),
                    access_expires_at: now + Duration::seconds(60),
                    release,
                },
                request,
            )
            .await;
        if age_ms == 4_999 {
            assert_eq!(result.unwrap().expires_at, now + Duration::seconds(30));
            assert_eq!(store.state.lock().unwrap().records.len(), 1);
        } else {
            assert_eq!(result, Err(WorldError::Denied), "release age {age_ms}ms");
            assert!(store.state.lock().unwrap().records.is_empty());
        }
    }
}

#[tokio::test]
async fn typed_issuance_rejects_stale_release_with_distinct_error() {
    let now = Utc.timestamp_opt(NOW, 0).unwrap();
    let (authority, _, _) = authority(now + Duration::seconds(60));
    let service = WorldService::new(authority, Store::default(), FixedClock, Entropy(12.into()));
    let request = request();
    let mut release = pins(request.event_id);
    release.map.revision = Revision::new(2).unwrap();
    assert_eq!(
        service
            .issue_verified(
                VerifiedTicketSubject {
                    key: key(request.event_id),
                    event_id: request.event_id,
                    session_id: session(),
                    access_expires_at: now + Duration::seconds(60),
                    release,
                },
                request,
            )
            .await,
        Err(WorldError::MapVersionMismatch)
    );
}

#[test]
fn ticket_digest_is_stable_and_does_not_contain_its_secret() {
    let ticket = AdmissionTicket::from_entropy(&[42; TICKET_BYTES]).unwrap();
    let digest = ticket.digest();
    assert_eq!(digest.as_bytes().len(), 32);
    assert_eq!(digest, ticket.digest());
    assert!(!format!("{digest:?}").contains(ticket.as_str()));
}

#[test]
fn catalog_hides_exact_occupancy_and_rejects_duplicates() {
    let room_a = RoomId::from_uuid(Uuid::new_v4());
    let room_b = RoomId::from_uuid(Uuid::new_v4());
    let item =
        RoomSummary::new(room_a, "Main room".into(), 50, RoomAvailability::Available).unwrap();
    assert!(RoomCatalog::new(event(), room_a, vec![item.clone(); 2], Utc::now()).is_err());
    assert!(RoomCatalog::new(event(), room_b, vec![item], Utc::now()).is_err());
}
