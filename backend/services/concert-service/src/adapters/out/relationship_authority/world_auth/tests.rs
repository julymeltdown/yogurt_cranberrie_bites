use super::*;
use concert_application::{identity::SessionId, world_ticket::*};
use concert_domain::{AssetId, AuthUserId, EventId, ParticipantKey, Revision};
use grpc::proto as wire;
use platform::{
    actor_delegation::{
        relationship_read::RelationshipReadVerifier, ActorDelegationSigner,
        ActorDelegationVerifier, VerifiedActor,
    },
    call_deadline::CallDeadlineLayer,
    grpc_auth::{CallerRegistry, VerifiedCaller},
};
use std::sync::{atomic::AtomicUsize, Arc, Mutex};
use tokio::sync::{oneshot, Notify};
use tonic::{Request, Response, Status};

mod native;

fn deadline() -> AuthorityRpcDeadline {
    AuthorityRpcDeadline::new(Utc::now().timestamp_micros() + 1_500_000, BUDGET).unwrap()
}
fn claim() -> TicketClaim {
    let now = Utc::now();
    let event = EventId::from_uuid(Uuid::new_v4());
    TicketClaim {
        record: TicketRecord {
            digest: AdmissionTicket::from_entropy(&[37; 32]).unwrap().digest(),
            subject: WorldSubject {
                key: ParticipantKey {
                    event_id: event,
                    user_id: AuthUserId::from_uuid(Uuid::new_v4()),
                },
                event_id: event,
                session_id: SessionId::new(Uuid::new_v4().to_string()).unwrap(),
            },
            pin: ResourcePin {
                event_id: event,
                room_id: RoomId::from_uuid(Uuid::new_v4()),
                map: MapPin::new(
                    AssetId::new("map_world_auth".into()).unwrap(),
                    Revision::new(1).unwrap(),
                ),
                avatar: AvatarPin::new(
                    AssetId::new("avatar_manifest".into()).unwrap(),
                    Revision::new(1).unwrap(),
                ),
            },
            issued_at: now - chrono::Duration::seconds(1),
            expires_at: now + chrono::Duration::seconds(20),
            access_expires_at: now + chrono::Duration::minutes(10),
        },
        connection_id: ConnectionId::from_uuid(Uuid::new_v4()).unwrap(),
    }
}
fn owned(c: &TicketClaim) -> CurrentOwnedMembership {
    CurrentOwnedMembership {
        membership: Membership {
            key: c.record.subject.key,
            session_id: c.record.subject.session_id.clone(),
            room_id: c.record.pin.room_id,
            connection_id: c.connection_id,
            pin: c.record.pin.clone(),
            lease_until: Utc::now() + chrono::Duration::seconds(5),
            access_expires_at: c.record.access_expires_at,
        },
        owner: OwnerIdentity::new("fixture-owner".into(), Uuid::new_v4()).unwrap(),
    }
}
fn binding(c: &TicketClaim) -> WorldOwnerRequest {
    WorldOwnerRequest::from_consumed(c, Uuid::new_v4(), deadline()).unwrap()
}
fn gateway(audience: Workload) -> ActorDelegationVerifier {
    ActorDelegationVerifier::new(
        audience,
        &[
            ActorDelegationSigner::from_seed(Workload::Gateway, &[41; 32])
                .unwrap()
                .public_key(),
        ],
    )
    .unwrap()
}
fn signer() -> RelationshipReadSigner {
    RelationshipReadSigner::from_seed(&[37; 32], &gateway(Workload::Concert)).unwrap()
}
fn verifier() -> ActorDelegationVerifier {
    RelationshipReadVerifier::new(
        Workload::Auth,
        &[signer().public_key()],
        &gateway(Workload::Auth),
    )
    .unwrap()
    .into_inner()
}
fn client(endpoint: &str) -> GrpcWorldAuthReader {
    GrpcWorldAuthReader::connect(endpoint, "world-auth-fixture-concert", None, signer()).unwrap()
}

struct Running {
    endpoint: String,
    stop: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Running {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        self.task.abort();
    }
}
impl Running {
    async fn start<S>(service: S) -> Self
    where
        S: wire::auth_concert_relationship_service_server::AuthConcertRelationshipService,
    {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let mut callers = CallerRegistry::new();
        for w in Workload::ALL {
            callers = callers
                .register(w, &format!("world-auth-fixture-{}", w.as_str()))
                .unwrap();
        }
        let guarded=platform::grpc_method_policy::guard(
            &auth_service::adapters::r#in::grpc::concert_relationship::POLICY,
            wire::auth_concert_relationship_service_server::AuthConcertRelationshipServiceServer::new(service),callers,verifier()).unwrap();
        let (stop, rx) = oneshot::channel();
        // TcpListenerStream is not a direct Concert dependency; unfold uses retained futures.
        let incoming = futures::stream::unfold(listener, |listener| async move {
            Some((listener.accept().await.map(|(s, _)| s), listener))
        });
        let task = tokio::spawn(async move {
            tonic::transport::Server::builder()
                .layer(CallDeadlineLayer)
                .add_service(guarded)
                .serve_with_incoming_shutdown(incoming, async {
                    let _ = rx.await;
                })
                .await
                .unwrap();
        });
        Self {
            endpoint,
            stop: Some(stop),
            task,
        }
    }
}
struct Controlled {
    kind: Mutex<&'static str>,
    calls: AtomicUsize,
    entered: Notify,
    queries: Mutex<Vec<wire::RelationshipRead>>,
}
impl Controlled {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            kind: Mutex::new("ok"),
            calls: AtomicUsize::new(0),
            entered: Notify::new(),
            queries: Mutex::new(vec![]),
        })
    }
}
#[derive(Clone)]
struct Control(Arc<Controlled>);
#[tonic::async_trait]
impl wire::auth_concert_relationship_service_server::AuthConcertRelationshipService for Control {
    async fn read_current(
        &self,
        r: Request<wire::RelationshipRead>,
    ) -> Result<Response<wire::AuthRelationshipObservation>, Status> {
        assert!(matches!(
            r.extensions().get::<VerifiedCaller>(),
            Some(VerifiedCaller(Workload::Concert))
        ));
        assert_eq!(
            r.extensions().get::<VerifiedActor>().unwrap().0.to_string(),
            r.get_ref().actor_id
        );
        self.0.calls.fetch_add(1, Ordering::SeqCst);
        self.0.entered.notify_one();
        self.0.queries.lock().unwrap().push(r.get_ref().clone());
        let kind = *self.0.kind.lock().unwrap();
        if kind == "outage" {
            return Err(Status::unavailable("private upstream diagnostic"));
        }
        if kind == "delay" {
            tokio::time::sleep(Duration::from_millis(150)).await;
        }
        if kind == "block" {
            std::thread::sleep(Duration::from_millis(100));
        }
        if kind == "hold" {
            std::future::pending::<()>().await;
        }
        let mut q = r.into_inner();
        let now = Utc::now().timestamp_micros();
        let mut at = now;
        let mut until = q.expires_unix_micros;
        let mut epoch = Uuid::from_u128(37).to_string();
        let mut revision = 7;
        match kind {
            "actor" => q.actor_id = Uuid::new_v4().to_string(),
            "subject" => q.subject_id = Uuid::new_v4().to_string(),
            "event" => q.event_id = Uuid::new_v4().to_string(),
            "family" => q.session_family_id = Uuid::new_v4().to_string(),
            "request" => q.request_id = Uuid::new_v4().to_string(),
            "purpose" => q.purpose = "PRESENCE".into(),
            "echo_expiry" => q.expires_unix_micros += 1,
            "missing_query" => {}
            "zero_at" => at = 0,
            "future" => at = now + 1_000_000,
            "stale" => at = now - 1_000_000,
            "stale_short_window" => at = now - 100_000,
            "expired" => until = now - 1,
            "extended" => until += 1,
            "empty_window" => until = at,
            "nil_epoch" => epoch = Uuid::nil().to_string(),
            "bad_epoch" => epoch = "NOT-AN-EPOCH".into(),
            "uppercase_epoch" => epoch = "12345678-1234-1234-1234-1234567890AB".into(),
            "zero_revision" => revision = 0,
            "negative_revision" => revision = -1,
            "overflow_revision" => revision = super::super::MAX_REVISION + 1,
            _ => {}
        }
        let mut response = Response::new(wire::AuthRelationshipObservation {
            query: if kind == "missing_query" {
                None
            } else {
                Some(q)
            },
            observed_at_unix_micros: at,
            valid_until_unix_micros: until,
            owner_epoch: epoch,
            revision,
            actor_active: kind != "inactive",
            session_live: kind != "revoked",
            subject_active: kind != "deleted",
        });
        if kind != "missing_no_store" {
            response.metadata_mut().insert(
                "cache-control",
                if kind == "bad_no_store" {
                    "public"
                } else {
                    "no-store"
                }
                .parse()
                .unwrap(),
            );
        }
        Ok(response)
    }
}

#[test]
fn consumed_binding_is_canonical_bounded_and_preserves_credential_lifetime() {
    let c = claim();
    let b = binding(&c);
    assert_eq!(b.subject(), &c.record.subject);
    assert_eq!(b.credential_expires_at(), c.record.access_expires_at);
    assert!(b.deadline.expires_micros() < b.credential_expires_at.timestamp_micros());
    assert_eq!(format!("{b:?}"), "WorldOwnerRequest([REDACTED])");
    let mut bad = c.clone();
    bad.record.subject.event_id = EventId::from_uuid(Uuid::new_v4());
    assert!(WorldOwnerRequest::from_consumed(&bad, Uuid::new_v4(), deadline()).is_err());
    let mut bad = c.clone();
    bad.record.subject.key.user_id = AuthUserId::from_uuid(Uuid::nil());
    assert!(WorldOwnerRequest::from_consumed(&bad, Uuid::new_v4(), deadline()).is_err());
    for family in [
        Uuid::nil().to_string(),
        "12345678-1234-1234-1234-1234567890AB".into(),
        Uuid::new_v4().simple().to_string(),
    ] {
        match SessionId::new(family) {
            Ok(session) => {
                let mut bad = c.clone();
                bad.record.subject.session_id = session;
                assert!(
                    WorldOwnerRequest::from_consumed(&bad, Uuid::new_v4(), deadline()).is_err()
                );
            }
            // The retained SessionId boundary rejects noncanonical syntax before binding.
            Err(error) => assert_eq!(
                error,
                concert_application::identity::IdentityError::Unavailable
            ),
        }
    }
    for delta in [-2, 31] {
        let mut bad = c.clone();
        bad.record.expires_at = bad.record.issued_at + chrono::Duration::seconds(delta);
        assert!(WorldOwnerRequest::from_consumed(&bad, Uuid::new_v4(), deadline()).is_err());
    }
    let mut bad = c.clone();
    bad.record.access_expires_at = Utc::now() - chrono::Duration::seconds(1);
    assert!(WorldOwnerRequest::from_consumed(&bad, Uuid::new_v4(), deadline()).is_err());
    assert!(WorldOwnerRequest::from_consumed(&c, Uuid::nil(), deadline()).is_err());
}
#[test]
fn renewal_requires_exact_current_owner_subject_connection_pin_and_live_expiry() {
    let c = claim();
    let m = owned(&c);
    let make = |m: &CurrentOwnedMembership| {
        WorldOwnerRequest::from_current(
            m,
            &owned(&c).owner,
            &c.record.subject,
            c.connection_id,
            Uuid::new_v4(),
            deadline(),
        )
    };
    // Use one stable expected owner; unrelated owner epochs must never compare equal.
    assert!(make(&m).is_err());
    let expected = m.owner.clone();
    let make = |m: &CurrentOwnedMembership| {
        WorldOwnerRequest::from_current(
            m,
            &expected,
            &c.record.subject,
            c.connection_id,
            Uuid::new_v4(),
            deadline(),
        )
    };
    assert_eq!(
        make(&m).unwrap().credential_expires_at(),
        m.membership.access_expires_at
    );
    let mut bad = m.clone();
    bad.owner.epoch = Uuid::new_v4();
    assert_eq!(make(&bad).unwrap_err(), WorldError::OwnerFenced);
    let mut bad = m.clone();
    bad.membership.session_id = SessionId::new(Uuid::new_v4().to_string()).unwrap();
    assert!(make(&bad).is_err());
    let mut bad = m.clone();
    bad.membership.key.event_id = EventId::from_uuid(Uuid::new_v4());
    assert!(make(&bad).is_err());
    let mut bad = m.clone();
    bad.membership.key.user_id = AuthUserId::from_uuid(Uuid::new_v4());
    assert!(make(&bad).is_err());
    let mut bad = m.clone();
    bad.membership.connection_id = ConnectionId::from_uuid(Uuid::new_v4()).unwrap();
    assert!(make(&bad).is_err());
    let mut bad = m.clone();
    bad.membership.pin.room_id = RoomId::from_uuid(Uuid::new_v4());
    assert!(make(&bad).is_err());
    let mut bad = m.clone();
    bad.membership.lease_until = Utc::now() - chrono::Duration::seconds(1);
    assert!(make(&bad).is_err());
    let mut bad = m.clone();
    bad.membership.access_expires_at = Utc::now() - chrono::Duration::seconds(1);
    assert!(make(&bad).is_err());
}
#[tokio::test]
async fn signed_auth_only_read_exact_scope_no_friend_and_single_attempt() {
    let control = Controlled::new();
    let server = Running::start(Control(control.clone())).await;
    let reader = client(&server.endpoint);
    let c = claim();
    let b = binding(&c);
    let observation = reader.observe(&b).await.unwrap();
    observation.check(&b).unwrap();
    assert_eq!(observation.subject(), &c.record.subject);
    assert_eq!(
        observation.credential_expires_at(),
        c.record.access_expires_at
    );
    assert!(observation.valid_until() < observation.credential_expires_at());
    assert!(observation.observed_at() <= Utc::now());
    assert_eq!(observation.revision().revision, 7);
    assert_eq!(
        format!("{observation:?}"),
        "WorldAuthObservation([REDACTED])"
    );
    let q = control.queries.lock().unwrap()[0].clone();
    assert_eq!(q.actor_id, q.subject_id);
    assert_eq!(q.purpose, "SAFETY_SELF");
    assert_eq!(q.event_id, c.record.subject.event_id.uuid().to_string());
    assert_eq!(q.session_family_id, c.record.subject.session_id.as_str());
    assert_eq!(q.request_id, b.request_id().to_string());
    assert_eq!(
        reader.observe(&b).await.unwrap_err(),
        WorldError::CredentialConsumed
    );
    assert!(observation.check(&binding(&c)).is_err());
    assert_eq!(control.calls.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn malformed_or_mismatched_echo_window_revision_and_cache_policy_fail_closed() {
    let control = Controlled::new();
    let server = Running::start(Control(control.clone())).await;
    let reader = client(&server.endpoint);
    for kind in [
        "actor",
        "subject",
        "event",
        "family",
        "request",
        "purpose",
        "echo_expiry",
        "missing_query",
        "zero_at",
        "future",
        "stale",
        "stale_short_window",
        "expired",
        "extended",
        "empty_window",
        "nil_epoch",
        "bad_epoch",
        "uppercase_epoch",
        "zero_revision",
        "negative_revision",
        "overflow_revision",
        "missing_no_store",
        "bad_no_store",
    ] {
        *control.kind.lock().unwrap() = kind;
        assert_eq!(
            reader.observe(&binding(&claim())).await.unwrap_err(),
            WorldError::Unavailable,
            "{kind}"
        );
    }
    for kind in ["inactive", "revoked", "deleted"] {
        *control.kind.lock().unwrap() = kind;
        assert_eq!(
            reader.observe(&binding(&claim())).await.unwrap_err(),
            WorldError::Denied,
            "{kind}"
        );
    }
}
#[tokio::test]
async fn original_monotonic_stop_cancellation_outage_and_noncooperative_reply_fail_closed() {
    let control = Controlled::new();
    let server = Running::start(Control(control.clone())).await;
    let reader = client(&server.endpoint);
    let c = claim();
    let original = AuthorityRpcDeadline::within_original(
        Utc::now().timestamp_micros() + 60_000_000,
        std::time::Instant::now() + Duration::from_millis(30),
        BUDGET,
    )
    .unwrap();
    tokio::time::sleep(Duration::from_millis(40)).await;
    assert!(original.expires_micros() > Utc::now().timestamp_micros());
    assert!(WorldOwnerRequest::from_consumed(&c, Uuid::new_v4(), original).is_err());
    let b = binding(&c);
    b.cancel();
    assert!(reader.observe(&b).await.is_err());
    assert_eq!(control.calls.load(Ordering::SeqCst), 0);
    for kind in ["delay", "block", "outage"] {
        *control.kind.lock().unwrap() = kind;
        let original =
            AuthorityRpcDeadline::new(Utc::now().timestamp_micros() + 30_000, BUDGET).unwrap();
        let b = WorldOwnerRequest::from_consumed(&c, Uuid::new_v4(), original).unwrap();
        assert_eq!(
            reader.observe(&b).await.unwrap_err(),
            WorldError::Unavailable
        );
        assert!(reader.observe(&b).await.is_err());
    }
    assert_eq!(control.calls.load(Ordering::SeqCst), 3);
}
#[tokio::test]
async fn earlier_ambient_deadline_shortens_a_live_sealed_request() {
    let control = Controlled::new();
    *control.kind.lock().unwrap() = "delay";
    let server = Running::start(Control(control.clone())).await;
    let reader = client(&server.endpoint);
    let b = binding(&claim());
    let outer = std::time::Instant::now() + Duration::from_millis(30);
    let result = platform::call_deadline::scope(outer, reader.observe(&b)).await;
    assert_eq!(result.unwrap_err(), WorldError::Unavailable);
    assert!(
        b.check().is_ok(),
        "the binding's longer deadline is still live"
    );
    assert_eq!(control.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        reader.observe(&b).await.unwrap_err(),
        WorldError::CredentialConsumed
    );
}
#[tokio::test]
async fn cancellation_and_dropped_future_do_not_allow_another_attempt() {
    let control = Controlled::new();
    *control.kind.lock().unwrap() = "hold";
    let server = Running::start(Control(control.clone())).await;
    let reader = Arc::new(client(&server.endpoint));
    let b = Arc::new(binding(&claim()));
    let r = reader.clone();
    let request = b.clone();
    let task = tokio::spawn(async move { r.observe(&request).await });
    control.entered.notified().await;
    b.cancel();
    assert!(task.await.unwrap().is_err());
    let b = Arc::new(binding(&claim()));
    let r = reader.clone();
    let request = b.clone();
    let task = tokio::spawn(async move { r.observe(&request).await });
    control.entered.notified().await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(
        reader.observe(&b).await.unwrap_err(),
        WorldError::CredentialConsumed
    );
}
#[tokio::test]
async fn raw_generated_client_cannot_replace_dedicated_read_proof_or_workload() {
    let control = Controlled::new();
    let server = Running::start(Control(control.clone())).await;
    let mut raw=wire::auth_concert_relationship_service_client::AuthConcertRelationshipServiceClient::connect(server.endpoint.clone()).await.unwrap();
    for w in Workload::ALL {
        let mut r = Request::new(wire::RelationshipRead {
            actor_id: Uuid::new_v4().to_string(),
            event_id: Uuid::new_v4().to_string(),
            subject_id: Uuid::new_v4().to_string(),
            session_family_id: Uuid::new_v4().to_string(),
            request_id: Uuid::new_v4().to_string(),
            purpose: "SAFETY_SELF".into(),
            expires_unix_micros: Utc::now().timestamp_micros() + 1_000_000,
        });
        r.metadata_mut()
            .insert("x-service-caller", w.as_str().parse().unwrap());
        r.metadata_mut().insert(
            "x-service-token",
            format!("world-auth-fixture-{}", w.as_str())
                .parse()
                .unwrap(),
        );
        r.metadata_mut().insert(
            platform::actor_delegation::HEADER,
            "forged".parse().unwrap(),
        );
        assert!(raw.read_current(r).await.is_err());
    }
    assert_eq!(control.calls.load(Ordering::SeqCst), 0);
}
