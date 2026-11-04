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
pub use profile_service::adapters::r#in::grpc::concert_public_profile::{
    proto, PublicProfileServer, METHOD, PUBLIC_PROFILE_POLICY,
};
use profile_service::concert_public_profile::*;
use prost::Message;
pub use retained::{owner_pool, seed};
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
pub type Msg = proto::PublicProfileReadRequest;
pub type Reply = proto::PublicProfileObservation;
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
        (Workload::Concert, "public-profile-concert"),
        (Workload::Gateway, "public-profile-gateway"),
        (Workload::Auth, "public-profile-auth"),
        (Workload::Friend, "public-profile-friend"),
        (Workload::Post, "public-profile-post"),
    ] {
        result = result.register(workload, token).unwrap();
    }
    result
}
pub struct Running {
    pub client:
        proto::concert_public_profile_service_client::ConcertPublicProfileServiceClient<Channel>,
    stop: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<Result<(), tonic::transport::Error>>>,
}
impl Running {
    pub async fn start(handler: PublicProfileServer) -> anyhow::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = format!("http://{}", listener.local_addr()?);
        let (stop, stopped) = oneshot::channel();
        let guarded = platform::grpc_method_policy::guard(
            &PUBLIC_PROFILE_POLICY,
            proto::concert_public_profile_service_server::ConcertPublicProfileServiceServer::new(
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
            proto::concert_public_profile_service_client::ConcertPublicProfileServiceClient::new(
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
    let mut m = Msg {
        actor_id: actor.to_string(),
        subject_id: Uuid::new_v4().to_string(),
        event_id: Uuid::new_v4().to_string(),
        session_family_id: Uuid::new_v4().to_string(),
        request_id: Uuid::new_v4().to_string(),
        purpose: 1,
        expires_unix_micros: chrono::Utc::now().timestamp_micros() + 1_900_000,
        child_id: String::new(),
        completion_secret: String::new(),
    };
    allocate(&mut m);
    m
}
pub fn allocate(m: &mut Msg) {
    let binding = ReadBinding {
        actor: Uuid::parse_str(&m.actor_id).unwrap(),
        event: Uuid::parse_str(&m.event_id).unwrap(),
        subject: Uuid::parse_str(&m.subject_id).unwrap(),
        family: Uuid::parse_str(&m.session_family_id).unwrap(),
        request: Uuid::parse_str(&m.request_id).unwrap(),
    };
    m.child_id = binding.new_child_id().unwrap();
    m.completion_secret = Uuid::new_v4().simple().to_string();
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
        .insert("x-service-token", "public-profile-concert".parse().unwrap());
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
    pub held: Mutex<std::collections::BTreeMap<String, String>>,
}
struct Dropped<'a>(&'a AtomicUsize);
impl Drop for Dropped<'_> {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
#[async_trait::async_trait]
impl PublicProfileRepository for Controlled {
    async fn read_public(
        &self,
        binding: ReadBinding,
        child_id: &str,
        secret: &str,
    ) -> Result<HeldPublicRecord, ReadError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let _drop = Dropped(&self.dropped);
        let mode = *self.mode.lock().unwrap();
        match mode {
            Mode::Missing => return Err(ReadError::Hidden),
            Mode::Error => return Err(ReadError::Unavailable),
            Mode::Hold => std::future::pending::<()>().await,
            Mode::Block(d) => std::thread::sleep(d),
            _ => {}
        }
        self.completed.fetch_add(1, Ordering::SeqCst);
        let subject = binding.subject;
        let content = PublicRecord {
            subject: if matches!(mode, Mode::Misbound) {
                Uuid::new_v4()
            } else {
                subject
            },
            display_name: if matches!(mode, Mode::Malformed) {
                "bad\nname".into()
            } else {
                "Mira".into()
            },
            bio: Some("Public bio 音楽".into()),
            observed_at: chrono::Utc::now()
                + match mode {
                    Mode::Stale => chrono::Duration::seconds(-3),
                    Mode::Future => chrono::Duration::seconds(3),
                    _ => chrono::Duration::zero(),
                },
        };
        binding.validate_completion(child_id, secret)?;
        let child_id = child_id.to_owned();
        let completion_secret = secret.to_owned();
        self.held
            .lock()
            .unwrap()
            .insert(child_id.clone(), completion_secret.clone());
        Ok(HeldPublicRecord {
            content,
            child_id,
            completion_secret,
        })
    }
    async fn check(&self, binding: ReadBinding, child_id: &str) -> Result<(), ReadError> {
        binding.validate_child(child_id)?;
        if self.held.lock().unwrap().contains_key(child_id) {
            Ok(())
        } else {
            Err(ReadError::Hidden)
        }
    }
    async fn complete(
        &self,
        binding: ReadBinding,
        child_id: &str,
        secret: &str,
    ) -> Result<(), ReadError> {
        binding.validate_completion(child_id, secret)?;
        let mut map = self.held.lock().unwrap();
        if map.get(child_id).map(String::as_str) != Some(secret) {
            return Err(ReadError::Hidden);
        }
        map.remove(child_id);
        Ok(())
    }
}
pub async fn controlled(mode: Mode) -> anyhow::Result<(Running, Arc<Controlled>)> {
    let owner = Arc::new(Controlled {
        mode: Mutex::new(mode),
        calls: AtomicUsize::new(0),
        completed: AtomicUsize::new(0),
        dropped: AtomicUsize::new(0),
        held: Mutex::new(std::collections::BTreeMap::new()),
    });
    Ok((
        Running::start(PublicProfileServer::new(owner.clone())).await?,
        owner,
    ))
}
pub fn control(m: &Msg, reply: &Reply, secret: bool) -> proto::PublicProfileGuardRequest {
    proto::PublicProfileGuardRequest {
        actor_id: m.actor_id.clone(),
        event_id: m.event_id.clone(),
        subject_id: m.subject_id.clone(),
        session_family_id: m.session_family_id.clone(),
        request_id: m.request_id.clone(),
        purpose: 1,
        expires_unix_micros: m.expires_unix_micros,
        child_id: reply.child_id.clone(),
        completion_secret: if secret {
            reply.completion_secret.clone()
        } else {
            String::new()
        },
    }
}
pub fn control_request(
    m: proto::PublicProfileGuardRequest,
    method: &str,
) -> Request<proto::PublicProfileGuardRequest> {
    let bytes = m.encode_to_vec();
    let mut frame = vec![0];
    frame.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    frame.extend(bytes);
    let proof = signer().issue(
        platform::grpc_auth::Workload::Profile,
        method,
        Uuid::parse_str(&m.actor_id).unwrap(),
        None,
        &frame,
        chrono::Utc::now().timestamp(),
    );
    let mut r = Request::new(m);
    r.metadata_mut()
        .insert("x-service-caller", "concert".parse().unwrap());
    r.metadata_mut()
        .insert("x-service-token", "public-profile-concert".parse().unwrap());
    r.metadata_mut().insert(HEADER, proof.parse().unwrap());
    r.metadata_mut()
        .insert(REMAINING_HEADER, "2S".parse().unwrap());
    r
}
pub async fn finish(s: &mut Running, m: &Msg, reply: &Reply) -> anyhow::Result<()> {
    let r = s
        .client
        .complete(control_request(
            control(m, reply, true),
            "/profile.concert_public.v1.ConcertPublicProfileService/Complete",
        ))
        .await?;
    assert!(!r.into_inner().held);
    Ok(())
}
