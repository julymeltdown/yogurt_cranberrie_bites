//! Sealed Auth-checked parent -> real signed Profile client -> guarded owner RPC.
use super::*;
use crate::adapters::out::relationship_authority::{
    grpc::proto as auth_wire,
    world_auth::{GrpcWorldAuthReader, WorldAuthRead, WorldOwnerRequest},
    world_projection::WorldProjectionParent,
};
use concert_application::{identity::SessionId, world_ticket::*};
use concert_domain::{AssetId, AuthUserId, EventId, ParticipantKey, Revision};
use platform::{
    actor_delegation::{
        authority_deadline::AuthorityRpcDeadline, relationship_read::RelationshipReadVerifier,
        ActorDelegationSigner, ActorDelegationVerifier,
    },
    call_deadline::CallDeadlineLayer,
    grpc_auth::CallerRegistry,
};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::sync::oneshot;
use tonic::codegen::http;
use tonic::{Request, Response, Status};

fn gateway(a: Workload) -> ActorDelegationVerifier {
    ActorDelegationVerifier::new(
        a,
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
fn verifier(a: Workload) -> ActorDelegationVerifier {
    RelationshipReadVerifier::new(a, &[signer().public_key()], &gateway(a))
        .unwrap()
        .into_inner()
}
fn callers() -> CallerRegistry {
    CallerRegistry::new()
        .register(Workload::Concert, "world-profile-client-fixture")
        .unwrap()
}
struct Running {
    endpoint: String,
    stop: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Running {
    fn drop(&mut self) {
        if let Some(s) = self.stop.take() {
            let _ = s.send(());
        }
        self.task.abort();
    }
}
async fn serve<S>(service: S) -> Running
where
    S: tower::Service<
            http::Request<tonic::body::Body>,
            Response = http::Response<tonic::service::interceptor::ResponseBody<tonic::body::Body>>,
            Error = std::convert::Infallible,
        > + tonic::server::NamedService
        + Clone
        + Send
        + Sync
        + 'static,
    S::Future: Send + 'static,
{
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", l.local_addr().unwrap());
    let (stop, rx) = oneshot::channel();
    let incoming =
        futures::stream::unfold(
            l,
            |l| async move { Some((l.accept().await.map(|(s, _)| s), l)) },
        );
    let task = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .layer(CallDeadlineLayer)
            .add_service(service)
            .serve_with_incoming_shutdown(incoming, async {
                let _ = rx.await;
            })
            .await
            .unwrap();
    });
    Running {
        endpoint,
        stop: Some(stop),
        task,
    }
}
#[derive(Clone)]
struct Auth;
#[tonic::async_trait]
impl auth_wire::auth_concert_relationship_service_server::AuthConcertRelationshipService for Auth {
    async fn read_current(
        &self,
        r: Request<auth_wire::RelationshipRead>,
    ) -> Result<Response<auth_wire::AuthRelationshipObservation>, Status> {
        let q = r.into_inner();
        let until = q.expires_unix_micros;
        let mut r = Response::new(auth_wire::AuthRelationshipObservation {
            query: Some(q),
            observed_at_unix_micros: Utc::now().timestamp_micros(),
            valid_until_unix_micros: until,
            owner_epoch: Uuid::from_u128(37).to_string(),
            revision: 1,
            actor_active: true,
            subject_active: true,
            session_live: true,
        });
        r.metadata_mut()
            .insert("cache-control", "no-store".parse().unwrap());
        Ok(r)
    }
}
async fn auth_server() -> Running {
    let service = auth_wire::auth_concert_relationship_service_server::AuthConcertRelationshipServiceServer::new(Auth);
    let guarded = platform::grpc_method_policy::guard(
        &auth_service::adapters::r#in::grpc::concert_relationship::POLICY,
        service,
        callers(),
        verifier(Workload::Auth),
    )
    .unwrap();
    serve(guarded).await
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
                    AssetId::new("map_profile".into()).unwrap(),
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
async fn parent(server: &Running, c: &TicketClaim, budget: Duration) -> WorldProjectionParent {
    let d = AuthorityRpcDeadline::new(
        Utc::now().timestamp_micros() + budget.as_micros() as i64,
        BUDGET,
    )
    .unwrap();
    let request = WorldOwnerRequest::from_consumed(c, Uuid::new_v4(), d).unwrap();
    let reader = GrpcWorldAuthReader::connect(
        &server.endpoint,
        "world-profile-client-fixture",
        None,
        signer(),
    )
    .unwrap();
    let auth = reader.observe(&request).await.unwrap();
    request.projection_parent(&auth).unwrap()
}
struct Control {
    mode: Mutex<&'static str>,
    calls: AtomicUsize,
    queries: Mutex<Vec<proto::WorldProfileReadRequest>>,
}
#[derive(Clone)]
struct Handler(Arc<Control>);
#[tonic::async_trait]
impl proto::concert_world_profile_service_server::ConcertWorldProfileService for Handler {
    async fn read_current(
        &self,
        r: Request<proto::WorldProfileReadRequest>,
    ) -> Result<Response<proto::WorldProfileObservation>, Status> {
        self.0.calls.fetch_add(1, Ordering::SeqCst);
        self.0.queries.lock().unwrap().push(r.get_ref().clone());
        let m = *self.0.mode.lock().unwrap();
        if m == "hold" {
            std::future::pending::<()>().await;
        }
        if m == "block" {
            std::thread::sleep(Duration::from_millis(100));
        }
        if m == "error" {
            return Err(Status::unavailable("private-profile-diagnostic"));
        }
        let mut b = proto::WorldProfileObservation {
            subject_id: r.into_inner().subject_id,
            display_name: "Mira".into(),
            observed_at_unix_micros: Utc::now().timestamp_micros(),
        };
        match m {
            "subject" => b.subject_id = Uuid::new_v4().to_string(),
            "name" => b.display_name = "bad\nname".into(),
            "empty" => b.display_name = " ".into(),
            "long" => b.display_name = "🎸".repeat(41),
            "zero" => b.observed_at_unix_micros = 0,
            "future" => b.observed_at_unix_micros += 1_000_000,
            "stale" => b.observed_at_unix_micros -= 3_000_000,
            _ => {}
        }
        let mut r = Response::new(b);
        if m != "no_store" {
            r.metadata_mut()
                .insert("cache-control", "no-store".parse().unwrap());
        }
        Ok(r)
    }
}
async fn controlled() -> (Running, Arc<Control>) {
    let c = Arc::new(Control {
        mode: Mutex::new("ok"),
        calls: AtomicUsize::new(0),
        queries: Mutex::new(vec![]),
    });
    let s = serve(
        platform::grpc_method_policy::guard(
            &profile_service::adapters::r#in::grpc::world_identity::WORLD_PROFILE_POLICY,
            proto::concert_world_profile_service_server::ConcertWorldProfileServiceServer::new(
                Handler(c.clone()),
            ),
            callers(),
            verifier(Workload::Profile),
        )
        .unwrap(),
    )
    .await;
    (s, c)
}
fn client(s: &Running) -> GrpcWorldProfileReader {
    GrpcWorldProfileReader::connect(&s.endpoint, "world-profile-client-fixture", None, signer())
        .unwrap()
}
#[tokio::test]
async fn self_context_signs_exact_scope_and_shared_gate_prevents_repeat_reads() {
    let auth = auth_server().await;
    let (s, c) = controlled().await;
    let claim = claim();
    let parent = parent(&auth, &claim, Duration::from_millis(1500)).await;
    let context = parent.profile_projection().unwrap();
    let result = client(&s).observe(&context).await.unwrap();
    result.check(&context).unwrap();
    assert_eq!(result.subject(), claim.record.subject.key.user_id.uuid());
    assert_eq!(result.display_name(), "Mira");
    assert!(result.observed_at() <= Utc::now());
    assert_eq!(format!("{result:?}"), "WorldProfileObservation([REDACTED])");
    let queries = c.queries.lock().unwrap();
    let q = &queries[0];
    assert_eq!(q.actor_id, context.actor().to_string());
    assert_eq!(q.subject_id, q.actor_id);
    assert_eq!(q.event_id, context.event_id().to_string());
    assert_eq!(q.session_family_id, context.session_family_id().to_string());
    assert_eq!(q.request_id, context.request_id().to_string());
    assert_eq!(q.purpose, 1);
    assert!(q.expires_unix_micros <= context.deadline().expires_micros());
    drop(queries);
    assert!(client(&s)
        .observe(&parent.clone().profile_projection().unwrap())
        .await
        .is_err());
    assert_eq!(c.calls.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn malformed_or_unbound_content_fails_closed_without_retries() {
    let auth = auth_server().await;
    let (s, c) = controlled().await;
    for mode in [
        "subject", "name", "empty", "long", "zero", "future", "stale", "no_store", "error",
    ] {
        *c.mode.lock().unwrap() = mode;
        let p = parent(&auth, &claim(), Duration::from_millis(1500)).await;
        let context = p.profile_projection().unwrap();
        assert!(client(&s).observe(&context).await.is_err());
        assert!(client(&s).observe(&context).await.is_err());
    }
    assert_eq!(c.calls.load(Ordering::SeqCst), 9);
}
#[tokio::test]
async fn returned_observation_cannot_outlive_a_shorter_ambient_stop() {
    let auth = auth_server().await;
    let (s, _) = controlled().await;
    let p = parent(&auth, &claim(), Duration::from_millis(1500)).await;
    let context = p.profile_projection().unwrap();
    let result = platform::call_deadline::scope(
        std::time::Instant::now() + Duration::from_millis(120),
        client(&s).observe(&context),
    )
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(150)).await;
    context.check().unwrap();
    assert!(result.check(&context).is_err());
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn original_stop_and_shared_cancel_bound_connect_and_owner_io() {
    let auth = auth_server().await;
    let (s, c) = controlled().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let stalled = GrpcWorldProfileReader::connect(
        &format!("http://{}", listener.local_addr().unwrap()),
        "world-profile-client-fixture",
        None,
        signer(),
    )
    .unwrap();
    let p = parent(&auth, &claim(), Duration::from_millis(80)).await;
    let context = p.profile_projection().unwrap();
    let start = std::time::Instant::now();
    assert!(stalled.observe(&context).await.is_err());
    assert!(start.elapsed() < Duration::from_millis(400));
    assert!(stalled.observe(&context).await.is_err());
    drop(listener);
    let p = parent(&auth, &claim(), Duration::from_millis(400)).await;
    let context = p.profile_projection().unwrap();
    p.cancel();
    assert!(client(&s).observe(&context).await.is_err());
    assert_eq!(c.calls.load(Ordering::SeqCst), 0);
    let p = parent(&auth, &claim(), Duration::from_millis(120)).await;
    let context = p.profile_projection().unwrap();
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert!(client(&s).observe(&context).await.is_err());
    assert_eq!(c.calls.load(Ordering::SeqCst), 0);
    for mode in ["hold", "block"] {
        *c.mode.lock().unwrap() = mode;
        let p = parent(&auth, &claim(), Duration::from_millis(80)).await;
        let context = p.profile_projection().unwrap();
        let start = std::time::Instant::now();
        assert!(client(&s).observe(&context).await.is_err());
        assert!(start.elapsed() < Duration::from_millis(500));
    }
    *c.mode.lock().unwrap() = "hold";
    let p = parent(&auth, &claim(), Duration::from_millis(1500)).await;
    let context = p.profile_projection().unwrap();
    let cancel = async {
        tokio::time::sleep(Duration::from_millis(30)).await;
        p.cancel();
    };
    let read = client(&s);
    let (result, _) = tokio::join!(read.observe(&context), cancel);
    assert!(result.is_err());
    let p = parent(&auth, &claim(), Duration::from_millis(1500)).await;
    let context = p.profile_projection().unwrap();
    let start = std::time::Instant::now();
    assert!(platform::call_deadline::scope(
        std::time::Instant::now() + Duration::from_millis(30),
        client(&s).observe(&context)
    )
    .await
    .is_err());
    assert!(start.elapsed() < Duration::from_millis(400));
}

#[tokio::test]
#[ignore = "requires explicitly owned Profile PG16 controller"]
async fn native_typed_profile_client_reads_current_owner_rows() -> anyhow::Result<()> {
    // Same retained marker/inode helper as new owner tests, with a fresh controller-owned root.
    use std::{os::unix::fs::MetadataExt, str::FromStr};
    let root = std::path::PathBuf::from(std::env::var("MCR_PROFILE_RPC_OWNER_ROOT")?);
    let meta = root.symlink_metadata()?;
    anyhow::ensure!(
        root.is_absolute()
            && root.canonicalize()? == root
            && meta.is_dir()
            && meta.mode() & 0o077 == 0,
        "private owned root required"
    );
    anyhow::ensure!(
        format!("{}:{}", meta.dev(), meta.ino())
            == std::env::var("MCR_PROFILE_RPC_OWNER_IDENTITY")?,
        "fixture changed"
    );
    let marker = root.join("mcr-owner.json");
    let mm = marker.symlink_metadata()?;
    anyhow::ensure!(
        mm.is_file()
            && !mm.file_type().is_symlink()
            && mm.mode() & 0o077 == 0
            && mm.uid() == meta.uid(),
        "private marker required"
    );
    let marker: serde_json::Value = serde_json::from_slice(&std::fs::read(marker)?)?;
    anyhow::ensure!(
        marker["token"].as_str() == Some(std::env::var("MCR_PROFILE_RPC_OWNER_TOKEN")?.as_str())
            && marker["uid"].as_u64() == Some(meta.uid() as u64),
        "fixture owner mismatch"
    );
    let options = sqlx::postgres::PgConnectOptions::from_str(&std::env::var(
        "MCR_PROFILE_RPC_DATABASE_URL",
    )?)?;
    anyhow::ensure!(
        options.get_database() == Some("mcr_profile_self_rpc")
            && options.get_username() == "mcr_owner"
            && options.get_socket() == Some(&root.join("socket"))
            && options.get_port() == 5432
            && options.get_options().is_none(),
        "foreign database refused"
    );
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect_with(options)
        .await?;
    let data: String = sqlx::query_scalar("show data_directory")
        .fetch_one(&pool)
        .await?;
    anyhow::ensure!(
        data == root.join("data").to_string_lossy(),
        "foreign server refused"
    );
    let auth = auth_server().await;
    let c = claim();
    let user = c.record.subject.key.user_id.uuid();
    sqlx::query("insert into profiles(user_id,username,nickname,bio,avatar_url,account_visibility,follower_count) values($1,$2,'Native Mira','private-bio-canary','https://private.invalid','PRIVATE',981)").bind(user).bind(format!("private-handle-{user}")).execute(&pool).await?;
    let server=serve(platform::grpc_method_policy::guard(&profile_service::adapters::r#in::grpc::world_identity::WORLD_PROFILE_POLICY,
        profile_service::adapters::r#in::grpc::world_identity::proto::concert_world_profile_service_server::ConcertWorldProfileServiceServer::new(profile_service::adapters::r#in::grpc::world_identity::WorldProfileServer::from_pool(pool.clone())),callers(),verifier(Workload::Profile))?).await;
    let p = parent(&auth, &c, Duration::from_millis(1500)).await;
    let projection = p.profile_projection()?;
    assert_eq!(
        client(&server).observe(&projection).await?.display_name(),
        "Native Mira"
    );
    sqlx::query("update profiles set nickname='Current Rename' where user_id=$1")
        .bind(user)
        .execute(&pool)
        .await?;
    let p = parent(&auth, &c, Duration::from_millis(1500)).await;
    assert_eq!(
        client(&server)
            .observe(&p.profile_projection()?)
            .await?
            .display_name(),
        "Current Rename"
    );
    sqlx::query("update profiles set deleted_at=now() where user_id=$1")
        .bind(user)
        .execute(&pool)
        .await?;
    let p = parent(&auth, &c, Duration::from_millis(1500)).await;
    assert!(client(&server)
        .observe(&p.profile_projection()?)
        .await
        .is_err());
    drop(server);
    pool.close().await;
    Ok(())
}
