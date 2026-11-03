//! New endpoint fixtures. Retained database ownership helpers are reused read-only.
#![allow(dead_code)]
#[path = "../concert_identity_fixture.rs"]
pub mod retained;
use platform::{
    actor_delegation::{
        relationship_read::RelationshipReadVerifier, ActorDelegationSigner,
        ActorDelegationVerifier, HEADER,
    },
    call_deadline::{CallDeadlineLayer, REMAINING_HEADER},
    grpc_auth::{CallerRegistry, Workload},
};
use profile_application::concert_identity::*;
pub use profile_service::adapters::r#in::grpc::world_identity::{
    proto, WorldProfileServer, METHOD, WORLD_PROFILE_POLICY,
};
use prost::Message;
pub use retained::{owned_options, owner_pool, reader_pool, seed};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::{net::TcpListener, sync::oneshot, task::JoinHandle};
use tonic::codegen::tokio_stream::wrappers::TcpListenerStream;
use tonic::{
    transport::{Channel, Endpoint, Server},
    Request, Response, Status,
};
use uuid::Uuid;
pub type Msg = proto::WorldProfileReadRequest;
pub type Reply = proto::WorldProfileObservation;
pub fn signer() -> ActorDelegationSigner {
    ActorDelegationSigner::from_seed(Workload::Concert, &[37; 32]).unwrap()
}
pub fn gateway() -> ActorDelegationVerifier {
    ActorDelegationVerifier::new(
        Workload::Profile,
        &[
            ActorDelegationSigner::from_seed(Workload::Gateway, &[41; 32])
                .unwrap()
                .public_key(),
        ],
    )
    .unwrap()
}
pub fn registry() -> CallerRegistry {
    let mut result = CallerRegistry::new();
    for (workload, token) in [
        (Workload::Concert, "world-profile-concert"),
        (Workload::Gateway, "world-profile-gateway"),
        (Workload::Auth, "world-profile-auth"),
        (Workload::Friend, "world-profile-friend"),
        (Workload::Post, "world-profile-post"),
    ] {
        result = result.register(workload, token).unwrap();
    }
    result
}
pub struct Running {
    pub client:
        proto::concert_world_profile_service_client::ConcertWorldProfileServiceClient<Channel>,
    stop: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<Result<(), tonic::transport::Error>>>,
}
impl Running {
    pub async fn start(handler: WorldProfileServer) -> anyhow::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = format!("http://{}", listener.local_addr()?);
        let (stop, stopped) = oneshot::channel();
        let guarded = platform::grpc_method_policy::guard(
            &WORLD_PROFILE_POLICY,
            proto::concert_world_profile_service_server::ConcertWorldProfileServiceServer::new(
                handler,
            ),
            registry(),
            RelationshipReadVerifier::new(Workload::Profile, &[signer().public_key()], &gateway())?
                .into_inner(),
        )?;
        let task = tokio::spawn(
            Server::builder()
                .layer(CallDeadlineLayer)
                .add_service(guarded)
                .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async {
                    let _ = stopped.await;
                }),
        );
        let client =
            proto::concert_world_profile_service_client::ConcertWorldProfileServiceClient::new(
                Endpoint::from_shared(endpoint)?.connect().await?,
            );
        Ok(Self {
            client,
            stop: Some(stop),
            task: Some(task),
        })
    }
    pub async fn stop(mut self) -> anyhow::Result<()> {
        let _ = self.stop.take().unwrap().send(());
        tokio::time::timeout(Duration::from_secs(3), self.task.take().unwrap()).await???;
        Ok(())
    }
}
impl Drop for Running {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}
pub fn message(actor: Uuid) -> Msg {
    Msg {
        actor_id: actor.to_string(),
        subject_id: actor.to_string(),
        event_id: Uuid::new_v4().to_string(),
        session_family_id: Uuid::new_v4().to_string(),
        request_id: Uuid::new_v4().to_string(),
        purpose: 1,
        expires_unix_micros: chrono::Utc::now().timestamp_micros() + 1_900_000,
    }
}
pub fn frame(m: &Msg) -> Vec<u8> {
    let b = m.encode_to_vec();
    let mut f = vec![0];
    f.extend_from_slice(&(b.len() as u32).to_be_bytes());
    f.extend(b);
    f
}
pub fn token(m: &Msg) -> String {
    signer().issue(
        Workload::Profile,
        METHOD,
        Uuid::parse_str(&m.actor_id).unwrap(),
        None,
        &frame(m),
        chrono::Utc::now().timestamp(),
    )
}
pub fn request(m: Msg, proof: Option<&str>, deadline: Option<&str>) -> Request<Msg> {
    let mut r = Request::new(m);
    r.metadata_mut()
        .insert("x-service-caller", "concert".parse().unwrap());
    r.metadata_mut()
        .insert("x-service-token", "world-profile-concert".parse().unwrap());
    if let Some(p) = proof {
        r.metadata_mut().insert(HEADER, p.parse().unwrap());
    }
    if let Some(d) = deadline {
        r.metadata_mut()
            .insert(REMAINING_HEADER, d.parse().unwrap());
    }
    r
}
pub async fn call(s: &mut Running, m: Msg) -> Result<Response<Reply>, Status> {
    let p = token(&m);
    s.client
        .read_current(request(m, Some(&p), Some("2S")))
        .await
}
#[derive(Clone, Copy)]
pub enum Mode {
    Success,
    Missing,
    Error,
    Misbound,
    Malformed,
    Stale,
    Future,
    Hold,
    Block(Duration),
}
pub struct Controlled {
    pub mode: Mutex<Mode>,
    pub calls: AtomicUsize,
    pub completed: AtomicUsize,
    pub dropped: AtomicUsize,
}
struct Dropped<'a>(&'a AtomicUsize);
impl Drop for Dropped<'_> {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
#[async_trait::async_trait]
impl ConcertProfileRepository for Controlled {
    async fn find_self_content(
        &self,
        q: ConcertProfileQuery,
    ) -> Result<Option<ConcertProfileRecord>, ConcertProfileError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let _drop = Dropped(&self.dropped);
        let mode = *self.mode.lock().unwrap();
        match mode {
            Mode::Missing => return Ok(None),
            Mode::Error => return Err(ConcertProfileError::DependencyUnavailable),
            Mode::Hold => std::future::pending::<()>().await,
            Mode::Block(d) => std::thread::sleep(d),
            _ => {}
        }
        self.completed.fetch_add(1, Ordering::SeqCst);
        Ok(Some(ConcertProfileRecord {
            user_id: if matches!(mode, Mode::Misbound) {
                Uuid::new_v4()
            } else {
                q.user_id()
            },
            nickname: if matches!(mode, Mode::Malformed) {
                "bad\nname".into()
            } else {
                "Mira".into()
            },
            bio: "private-bio-canary".into(),
            observed_at: chrono::Utc::now()
                + match mode {
                    Mode::Stale => chrono::Duration::seconds(-3),
                    Mode::Future => chrono::Duration::seconds(3),
                    _ => chrono::Duration::zero(),
                },
        }))
    }
}
pub async fn controlled(mode: Mode) -> anyhow::Result<(Running, Arc<Controlled>)> {
    let owner = Arc::new(Controlled {
        mode: Mutex::new(mode),
        calls: AtomicUsize::new(0),
        completed: AtomicUsize::new(0),
        dropped: AtomicUsize::new(0),
    });
    Ok((
        Running::start(WorldProfileServer::new(owner.clone())).await?,
        owner,
    ))
}
