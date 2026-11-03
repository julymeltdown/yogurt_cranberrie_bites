//! Signed Gateway-to-Concert C05 composition over loopback; no database or runtime claim.
use super::*;
use api_gateway::adapters::out::concert_world::{proto, GrpcConcertWorldClient, WorldClientError};
use uuid::Uuid;

#[path = "concert_world_http.rs"]
mod http_ingress;

#[derive(Clone, Copy, Debug)]
enum Reply {
    Valid,
    CrossEvent,
    Cacheable,
    Full,
    VersionMismatch,
}

#[derive(Clone)]
struct World {
    seen: Arc<Mutex<Vec<(proto::WorldRequest, &'static str)>>>,
    reply: Arc<Mutex<Reply>>,
}

static WORLD_POLICY: MethodPolicy = MethodPolicy {
    service: "concert.world.v1.WorldService",
    workload: Some(Workload::Concert),
    methods: &[
        ("ListRooms", &[Workload::Gateway]),
        ("IssueTicket", &[Workload::Gateway]),
        ("CurrentMembership", &[Workload::Gateway]),
    ],
    actor_fields: &[
        ("ListRooms", &[&[1]]),
        ("IssueTicket", &[&[1]]),
        ("CurrentMembership", &[&[1]]),
    ],
};

fn room() -> String {
    "00000000-0000-4000-8000-000000000011".into()
}

fn valid_world(message: &proto::WorldRequest, room: &str) -> Value {
    if message.http_method == "GET" {
        json!({
            "event_id": message.event_id,
            "default_room_id": room,
            "items": [{
                "room_id": room,
                "label": "Main",
                "capacity": 50,
                "availability": "AVAILABLE"
            }],
            "server_time": "2026-11-07T10:00:00Z"
        })
    } else {
        json!({
            "protocol_version": "1.0.0",
            "ticket": "A".repeat(43),
            "event_id": message.event_id,
            "room_id": room,
            "map_id": "map_fixture",
            "map_revision": 1,
            "avatar_manifest_id": "avatar_fixture",
            "avatar_manifest_version": 1,
            "server_time": "2026-11-07T10:00:00Z",
            "expires_at": "2026-11-07T10:00:30Z",
            "ws_path": "/world-ws",
            "auth_timeout_ms": 3000
        })
    }
}

#[tonic::async_trait]
impl proto::world_service_server::WorldService for World {
    async fn list_rooms(
        &self,
        request: Request<proto::WorldRequest>,
    ) -> Result<Response<proto::WorldResponse>, Status> {
        self.dispatch(request).await
    }

    async fn issue_ticket(
        &self,
        request: Request<proto::WorldRequest>,
    ) -> Result<Response<proto::WorldResponse>, Status> {
        self.dispatch(request).await
    }

    async fn current_membership(
        &self,
        _: Request<proto::CurrentMembershipRequest>,
    ) -> Result<Response<proto::CurrentMembershipResponse>, Status> {
        Err(Status::unimplemented("outside world HTTP fixture"))
    }
}

impl World {
    async fn dispatch(
        &self,
        request: Request<proto::WorldRequest>,
    ) -> Result<Response<proto::WorldResponse>, Status> {
        assert!(matches!(
            request.extensions().get::<VerifiedCaller>(),
            Some(VerifiedCaller(Workload::Gateway))
        ));
        let actor = request.extensions().get::<VerifiedActor>().unwrap().0;
        let proof = *request.extensions().get::<VerifiedReadAuthority>().unwrap();
        let deadline = proof.deadline(Duration::from_secs(2))?;
        let message = request.into_inner();
        assert_eq!(actor.to_string(), message.user_id);
        assert_eq!(message.session_family_id, FAMILY);
        assert_eq!(
            message.http_target,
            if message.http_method == "GET" {
                format!("/api/concert/events/{}/rooms", message.event_id)
            } else {
                format!("/api/concert/events/{}/world-tickets", message.event_id)
            }
        );
        assert!(message.access_expires_unix_micros > Utc::now().timestamp_micros());
        assert!(message.authority_expires_unix_micros > Utc::now().timestamp_micros());
        assert!(message.authority_expires_unix_micros <= message.access_expires_unix_micros);
        assert!(
            message.authority_expires_unix_micros - message.auth_observed_at_unix_micros
                <= 10_000_000
        );
        assert!(
            message.authority_expires_unix_micros - message.profile_observed_at_unix_micros
                <= 10_000_000
        );
        let method = if message.http_method == "GET" {
            "ListRooms"
        } else {
            "IssueTicket"
        };
        self.seen.lock().unwrap().push((message.clone(), method));
        let mode = *self.reply.lock().unwrap();
        deadline.remaining(
            Duration::from_secs(2),
            message.authority_expires_unix_micros,
        )?;
        match mode {
            Reply::Full => return Err(Status::resource_exhausted("room full")),
            Reply::VersionMismatch => return Err(Status::aborted("MAP_VERSION_MISMATCH")),
            _ => {}
        }
        let requested = serde_json::from_slice::<Value>(&message.http_body)
            .ok()
            .and_then(|value| value["room_id"].as_str().map(str::to_owned));
        let target = requested.unwrap_or_else(room);
        let mut value = valid_world(&message, &target);
        if matches!(mode, Reply::CrossEvent) {
            value["event_id"] = Uuid::new_v4().to_string().into();
        }
        let mut response = Response::new(proto::WorldResponse {
            world_json: serde_json::to_vec(&value).unwrap(),
        });
        if !matches!(mode, Reply::Cacheable) {
            response
                .metadata_mut()
                .insert("cache-control", "no-store".parse().unwrap());
        }
        Ok(response)
    }
}

struct RunningWorld {
    service: World,
    client: GrpcConcertWorldClient,
    stop: oneshot::Sender<()>,
    task: JoinHandle<()>,
}

impl RunningWorld {
    async fn start() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let service = World {
            seen: Arc::new(Mutex::new(Vec::new())),
            reply: Arc::new(Mutex::new(Reply::Valid)),
        };
        let guarded = guard(
            &WORLD_POLICY,
            proto::world_service_server::WorldServiceServer::new(service.clone()),
            public_registry(),
            ActorDelegationVerifier::local_parity(Workload::Concert),
        )
        .unwrap();
        let (stop, stopped) = oneshot::channel();
        let task = tokio::spawn(async move {
            Server::builder()
                .layer(CallDeadlineLayer)
                .add_service(guarded)
                .serve_with_incoming_shutdown(
                    tokio_stream::wrappers::TcpListenerStream::new(listener),
                    async {
                        let _ = stopped.await;
                    },
                )
                .await
                .unwrap();
        });
        let client = GrpcConcertWorldClient::connect(
            &endpoint,
            "composition-token",
            None,
            ActorDelegationSigner::local_parity(Workload::Gateway),
        )
        .unwrap();
        Self {
            service,
            client,
            stop,
            task,
        }
    }

    async fn close(self) {
        let _ = self.stop.send(());
        self.task.await.unwrap();
    }
}

#[tokio::test]
async fn rooms_and_tickets_use_fresh_signed_bound_owner_chain_with_exact_expiry() {
    let _ = key();
    let owners = Running::start().await;
    let world = RunningWorld::start().await;
    let ctx = context(Duration::from_secs(4));
    let event = Uuid::parse_str("00000000-0000-4000-8000-000000000010").unwrap();
    let rooms = world
        .client
        .rooms(&owners.reader.read(&ctx, None).await.unwrap(), &ctx, event)
        .await
        .unwrap();
    assert_eq!(rooms["event_id"], event.to_string());
    let body = serde_json::to_vec(&json!({
        "room_id": room(),
        "map_id": "map_fixture",
        "map_revision": 1,
        "avatar_manifest_id": "avatar_fixture",
        "avatar_manifest_version": 1
    }))
    .unwrap();
    let ticket = world
        .client
        .ticket(
            &owners.reader.read(&ctx, None).await.unwrap(),
            &ctx,
            event,
            body.clone(),
        )
        .await
        .unwrap();
    assert_eq!(ticket["room_id"], room());
    let seen = world.service.seen.lock().unwrap().clone();
    assert_eq!(seen.len(), 2);
    assert_eq!(seen[1].0.http_body, body);
    assert_eq!(seen[1].0.http_method, "POST");
    assert_eq!(seen[1].1, "IssueTicket");
    assert_eq!(
        seen[1].0.access_expires_unix_micros,
        ctx.access_expires_micros()
    );
    assert_eq!(owners.auth.calls.load(Ordering::SeqCst), 2);
    assert_eq!(owners.profile.calls.load(Ordering::SeqCst), 2);
    assert!(!format!("{seen:?}").contains(PRIVATE));
    world.close().await;
    owners.close().await;
}

#[tokio::test]
async fn gateway_world_client_refuses_invalid_cache_and_never_replays() {
    let _ = key();
    let owners = Running::start().await;
    let world = RunningWorld::start().await;
    let ctx = context(Duration::from_secs(4));
    let identity = owners.reader.read(&ctx, None).await.unwrap();
    *world.service.reply.lock().unwrap() = Reply::Cacheable;
    assert_eq!(
        world
            .client
            .rooms(&identity, &ctx, Uuid::new_v4())
            .await
            .unwrap_err(),
        WorldClientError::InvalidResponse
    );
    *world.service.reply.lock().unwrap() = Reply::CrossEvent;
    let body = serde_json::to_vec(&json!({
        "room_id": room(),
        "map_id": "map_fixture",
        "map_revision": 1,
        "avatar_manifest_id": "avatar_fixture",
        "avatar_manifest_version": 1
    }))
    .unwrap();
    assert_eq!(
        world
            .client
            .ticket(&identity, &ctx, Uuid::new_v4(), body)
            .await
            .unwrap_err(),
        WorldClientError::InvalidResponse
    );
    world.close().await;
    owners.close().await;
}

#[tokio::test]
async fn ticket_rejections_keep_full_and_version_mismatch_distinct() {
    let _ = key();
    let owners = Running::start().await;
    let world = RunningWorld::start().await;
    let ctx = context(Duration::from_secs(4));
    let identity = owners.reader.read(&ctx, None).await.unwrap();
    let body = serde_json::to_vec(&json!({
        "room_id": room(),
        "map_id": "map_fixture",
        "map_revision": 1,
        "avatar_manifest_id": "avatar_fixture",
        "avatar_manifest_version": 1
    }))
    .unwrap();
    for (mode, expected) in [
        (Reply::Full, WorldClientError::Full),
        (Reply::VersionMismatch, WorldClientError::VersionMismatch),
    ] {
        *world.service.reply.lock().unwrap() = mode;
        assert_eq!(
            world
                .client
                .ticket(&identity, &ctx, Uuid::new_v4(), body.clone())
                .await
                .unwrap_err(),
            expected
        );
    }
    world.close().await;
    owners.close().await;
}
