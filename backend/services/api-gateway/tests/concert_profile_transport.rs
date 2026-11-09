//! Signed loopback evidence for the bounded Profile self-content client.
//! The owner is a controlled double; this is not native Profile/DB acceptance.
use api_gateway::adapters::out::{
    jwt::{concert_access::ConcertAccessContext, JwtVerifier},
    profile_client::{concert_identity::*, proto, GrpcProfileClient},
};
use api_gateway::config::GatewayMetrics;
use chrono::{TimeZone, Utc};
use http_body_util::{BodyExt, Full};
use platform::{
    actor_delegation::{
        read_authority::VerifiedReadAuthority, ActorDelegationSigner, ActorDelegationVerifier,
        VerifiedActor,
    },
    call_deadline::CallDeadlineLayer,
    grpc_auth::{CallerRegistry, VerifiedCaller, Workload},
    grpc_method_policy::{guard, MethodPolicy},
};
use prost::Message;
use serde_json::json;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::{net::TcpListener, sync::oneshot, task::JoinHandle};
use tokio_stream::wrappers::TcpListenerStream;
use tonic::{
    transport::{Endpoint, Server},
    Code, Request, Response, Status,
};

use base64ct::{Base64UrlUnpadded, Encoding};
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use rand::rngs::OsRng;
use rsa::{pkcs8::EncodePrivateKey, traits::PublicKeyParts, RsaPrivateKey, RsaPublicKey};
use serde_json::Value;
use std::sync::OnceLock;
use uuid::Uuid;

static POLICY: MethodPolicy = MethodPolicy {
    service: "profile.v1.ProfileService",
    workload: Some(Workload::Profile),
    methods: &[("ReadConcertSelfProfile", &[Workload::Gateway])],
    actor_fields: &[("ReadConcertSelfProfile", &[&[1]])],
};
const METHOD: &str = "/profile.v1.ProfileService/ReadConcertSelfProfile";
const TOKEN: &str = "controlled-gateway-profile-token";
const USER: &str = "7e023b36-5ca9-4a40-850d-d01dc16874a8";
/// The canonical profile id for `USER`. Deliberately a different UUID.
const PROFILE: &str = "2c9f1a44-6b7e-4d2a-9f31-8e5d0c4b7a16";
const FAMILY: &str = "fa60b0de-7277-447c-aa36-eaf530dd3018";
const PRIVATE: &str = "PRIVATE_SQL_PROFILE_CANARY";

#[derive(Clone)]
enum Mode {
    Reply(proto::ReadConcertSelfProfileResponse),
    Delay(Duration),
    Block(Duration),
    Hold,
    Error(Code),
}
struct Owner {
    mode: Mutex<Mode>,
    calls: AtomicUsize,
    active: AtomicUsize,
    seen: Mutex<Vec<proto::ReadConcertSelfProfileRequest>>,
}
struct Active(Arc<Owner>);
impl Drop for Active {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::SeqCst);
    }
}

#[derive(Clone)]
struct FakeProfile(Arc<Owner>);
fn profile(name: &str, bio: Option<&str>) -> proto::ConcertSelfProfile {
    proto::ConcertSelfProfile {
        user_id: USER.into(),
        display_name: name.into(),
        bio: bio.map(str::to_owned),
        observed_at_unix_micros: Utc::now().timestamp_micros(),
        // A distinct profile id: the account id and the profile id are separate
        // UUIDs, and the Gateway refuses an observation that conflates them.
        profile_id: Some(PROFILE.into()),
    }
}
fn reply() -> proto::ReadConcertSelfProfileResponse {
    proto::ReadConcertSelfProfileResponse {
        profile: Some(profile("Mira", Some("Loves live sets"))),
    }
}

#[tonic::async_trait]
impl proto::profile_service_server::ProfileService for FakeProfile {
    async fn read_concert_self_profile(
        &self,
        request: Request<proto::ReadConcertSelfProfileRequest>,
    ) -> Result<Response<proto::ReadConcertSelfProfileResponse>, Status> {
        assert_eq!(
            request.extensions().get::<VerifiedCaller>().unwrap().0,
            Workload::Gateway
        );
        let actor = request.extensions().get::<VerifiedActor>().unwrap().0;
        assert_eq!(actor.to_string(), request.get_ref().user_id);
        let authority = *request.extensions().get::<VerifiedReadAuthority>().unwrap();
        assert_eq!(authority.actor(), actor);
        let deadline = authority.deadline(Duration::from_secs(2))?;
        deadline.remaining(Duration::from_secs(2), deadline.expires_micros())?;
        assert!(!request.metadata().contains_key("authorization"));
        self.0.calls.fetch_add(1, Ordering::SeqCst);
        self.0.active.fetch_add(1, Ordering::SeqCst);
        let _active = Active(self.0.clone());
        self.0.seen.lock().unwrap().push(request.into_inner());
        let mode = self.0.mode.lock().unwrap().clone();
        match mode {
            Mode::Reply(r) => Ok(Response::new(r)),
            Mode::Delay(d) => {
                tokio::time::sleep(d).await;
                Ok(Response::new(reply()))
            }
            Mode::Block(d) => {
                std::thread::sleep(d);
                Ok(Response::new(reply()))
            }
            Mode::Hold => std::future::pending().await,
            Mode::Error(code) => Err(Status::new(code, PRIVATE)),
        }
    }

    async fn replay_mutation(
        &self,
        _: Request<proto::ReplayMutationRequest>,
    ) -> Result<Response<proto::ReplayMutationResponse>, Status> {
        unused()
    }
    async fn get_mutation_receipt_status(
        &self,
        _: Request<proto::GetMutationReceiptStatusRequest>,
    ) -> Result<Response<proto::GetMutationReceiptStatusResponse>, Status> {
        unused()
    }
    async fn get_profile(
        &self,
        _: Request<proto::GetProfileRequest>,
    ) -> Result<Response<proto::GetProfileResponse>, Status> {
        unused()
    }
    async fn get_profile_by_username(
        &self,
        _: Request<proto::GetProfileByUsernameRequest>,
    ) -> Result<Response<proto::GetProfileResponse>, Status> {
        unused()
    }
    async fn get_profiles(
        &self,
        _: Request<proto::GetProfilesRequest>,
    ) -> Result<Response<proto::GetProfilesResponse>, Status> {
        unused()
    }
    async fn update_profile(
        &self,
        _: Request<proto::UpdateProfileRequest>,
    ) -> Result<Response<proto::UpdateProfileResponse>, Status> {
        unused()
    }
    async fn update_profile_privacy(
        &self,
        _: Request<proto::UpdateProfilePrivacyRequest>,
    ) -> Result<Response<proto::UpdateProfilePrivacyResponse>, Status> {
        unused()
    }
    async fn get_profile_privacy(
        &self,
        _: Request<proto::GetProfilePrivacyRequest>,
    ) -> Result<Response<proto::GetProfilePrivacyResponse>, Status> {
        unused()
    }
    async fn delete_profile(
        &self,
        _: Request<proto::DeleteProfileRequest>,
    ) -> Result<Response<proto::DeleteProfileResponse>, Status> {
        unused()
    }
    async fn search_profiles(
        &self,
        _: Request<proto::SearchProfilesRequest>,
    ) -> Result<Response<proto::SearchProfilesResponse>, Status> {
        unused()
    }
    async fn upload_avatar(
        &self,
        _: Request<proto::UploadAvatarRequest>,
    ) -> Result<Response<proto::UploadAvatarResponse>, Status> {
        unused()
    }
    async fn get_avatar(
        &self,
        _: Request<proto::GetAvatarRequest>,
    ) -> Result<Response<proto::GetAvatarResponse>, Status> {
        unused()
    }
    async fn apply_user_deletion(
        &self,
        _: Request<proto::ApplyUserDeletionRequest>,
    ) -> Result<Response<proto::ApplyUserDeletionResponse>, Status> {
        unused()
    }
}
fn unused<T>() -> Result<Response<T>, Status> {
    Err(Status::unimplemented("outside the read fixture"))
}

struct Harness {
    client: GrpcProfileClient,
    owner: Arc<Owner>,
    url: String,
    stop: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<Result<(), tonic::transport::Error>>>,
}
impl Harness {
    async fn start(mode: Mode, oversize: bool) -> Self {
        Self::with_frame_size(mode, oversize.then_some(1700)).await
    }
    async fn with_frame_size(mode: Mode, frame_size: Option<usize>) -> Self {
        let owner = Arc::new(Owner {
            mode: Mutex::new(mode),
            calls: AtomicUsize::new(0),
            active: AtomicUsize::new(0),
            seen: Mutex::new(Vec::new()),
        });
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let (stop, stopped) = oneshot::channel();
        let service = guard(
            &POLICY,
            Wire {
                inner: proto::profile_service_server::ProfileServiceServer::new(FakeProfile(
                    owner.clone(),
                )),
                frame_size,
            },
            CallerRegistry::new()
                .register(Workload::Gateway, TOKEN)
                .unwrap()
                .register(Workload::Post, "post-token")
                .unwrap(),
            ActorDelegationVerifier::local_parity(Workload::Profile),
        )
        .unwrap();
        let task = tokio::spawn(
            Server::builder()
                .layer(CallDeadlineLayer)
                .add_service(service)
                .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async {
                    let _ = stopped.await;
                }),
        );
        let client = Self::client(
            &url,
            Workload::Gateway,
            TOKEN,
            ActorDelegationSigner::local_parity(Workload::Gateway),
            GatewayMetrics::default(),
        )
        .await;
        Self {
            client,
            owner,
            url,
            stop: Some(stop),
            task: Some(task),
        }
    }
    async fn client(
        url: &str,
        caller: Workload,
        token: &str,
        signer: ActorDelegationSigner,
        metrics: GatewayMetrics,
    ) -> GrpcProfileClient {
        GrpcProfileClient::connect_with_metrics(url.into(), caller, token, None, signer, metrics)
            .await
            .unwrap()
    }
    async fn close(mut self) {
        let _ = self.stop.take().unwrap().send(());
        tokio::time::timeout(Duration::from_secs(3), self.task.take().unwrap())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }
}
impl Drop for Harness {
    fn drop(&mut self) {
        if let Some(s) = self.stop.take() {
            let _ = s.send(());
        }
        if let Some(t) = self.task.take() {
            t.abort();
        }
    }
}

#[derive(Clone)]
struct Wire<S> {
    inner: S,
    frame_size: Option<usize>,
}
impl<S: tonic::server::NamedService> tonic::server::NamedService for Wire<S> {
    const NAME: &'static str = S::NAME;
}
impl<S> tower::Service<http::Request<tonic::body::Body>> for Wire<S>
where
    S: tower::Service<
            http::Request<tonic::body::Body>,
            Response = http::Response<tonic::body::Body>,
        > + Send
        + 'static,
    S::Future: Send + 'static,
    S::Error: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;
    fn poll_ready(
        &mut self,
        c: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(c)
    }
    fn call(&mut self, r: http::Request<tonic::body::Body>) -> Self::Future {
        let f = self.inner.call(r);
        let frame_size = self.frame_size;
        Box::pin(async move {
            let response = f.await?;
            let Some(frame_size) = frame_size else {
                return Ok(response);
            };
            let (parts, body) = response.into_parts();
            let collected = body.collect().await.unwrap();
            let trailers = collected.trailers().cloned();
            let mut bytes = collected.to_bytes().to_vec();
            // One valid length-delimited unknown field (100); both test
            // payloads use a two-byte varint length, including the 1024 boundary.
            let extra = frame_size - (bytes.len() - 5) - 4;
            assert!((128..16384).contains(&extra));
            bytes.extend_from_slice(&[0xa2, 0x06, (extra as u8 & 0x7f) | 0x80, (extra >> 7) as u8]);
            bytes.resize(bytes.len() + extra, 0);
            let size = (bytes.len() - 5) as u32;
            bytes[1..5].copy_from_slice(&size.to_be_bytes());
            Ok(http::Response::from_parts(
                parts,
                tonic::body::Body::new(
                    Full::new(tonic::codegen::Bytes::from(bytes))
                        .with_trailers(async move { trailers.map(Ok) }),
                ),
            ))
        })
    }
}

struct Key {
    kid: &'static str,
    encoding: EncodingKey,
    public: RsaPublicKey,
}
impl Key {
    fn generate(kid: &'static str) -> Self {
        let private = RsaPrivateKey::new(&mut OsRng, 2048).unwrap();
        let pem = private.to_pkcs8_pem(Default::default()).unwrap();
        Self {
            kid,
            encoding: EncodingKey::from_rsa_pem(pem.as_bytes()).unwrap(),
            public: RsaPublicKey::from(&private),
        }
    }
    fn jwk(&self) -> Value {
        json!({"kid":self.kid,"kty":"RSA","alg":"RS256","use":"sig","n":Base64UrlUnpadded::encode_string(&self.public.n().to_bytes_be()),"e":Base64UrlUnpadded::encode_string(&self.public.e().to_bytes_be())})
    }
    fn token(&self, claims: &Value) -> String {
        jsonwebtoken::encode(
            &Header {
                kid: Some(self.kid.into()),
                ..Header::new(Algorithm::RS256)
            },
            claims,
            &self.encoding,
        )
        .unwrap()
    }
}
fn first() -> &'static Key {
    static KEY: OnceLock<Key> = OnceLock::new();
    KEY.get_or_init(|| Key::generate("profile-first"))
}
fn context(after: Duration) -> ConcertAccessContext {
    context_with_exp(after, Utc::now().timestamp() + 900)
}
fn context_with_exp(after: Duration, exp: i64) -> ConcertAccessContext {
    let verifier = JwtVerifier::from_jwks_json(
        "fixture-auth",
        "fixture-api",
        &json!({"keys":[first().jwk()]}).to_string(),
    )
    .unwrap();
    let token = first().token(&json!({"sub":USER,"sid":FAMILY,"iss":"fixture-auth","aud":"fixture-api","token_type":"access","exp":exp,"roles":[PRIVATE]}));
    verifier
        .verify_concert_access(
            &token,
            Some((tokio::time::Instant::now() + after).into_std()),
        )
        .unwrap()
}
fn live() -> ConcertAccessContext {
    context(Duration::from_secs(20))
}

#[tokio::test]
async fn signed_self_reads_validate_minimal_content_and_fresh_changes() {
    let h = Harness::start(Mode::Reply(reply()), false).await;
    let ctx = live();
    let a = h.client.read_concert_self_profile(&ctx).await.unwrap();
    assert_eq!(a.user_id().unwrap().to_string(), USER);
    assert_eq!(a.display_name().unwrap(), "Mira");
    assert_eq!(a.bio().unwrap(), Some("Loves live sets"));
    assert_eq!(format!("{a:?}"), "ConcertProfileObservation([REDACTED])");
    *h.owner.mode.lock().unwrap() = Mode::Reply(proto::ReadConcertSelfProfileResponse {
        profile: Some(profile("Updated", None)),
    });
    let b = h.client.read_concert_self_profile(&ctx).await.unwrap();
    assert_eq!(b.display_name().unwrap(), "Updated");
    assert_eq!(b.bio().unwrap(), None);
    assert_eq!(h.owner.calls.load(Ordering::SeqCst), 2);
    {
        let seen = h.owner.seen.lock().unwrap();
        assert_eq!(seen.len(), 2);
        for r in seen.iter() {
            assert_eq!(r.user_id, USER);
            assert_eq!(
                r.encode_to_vec(),
                proto::ReadConcertSelfProfileRequest {
                    user_id: USER.into()
                }
                .encode_to_vec()
            );
            assert!(!String::from_utf8_lossy(&r.encode_to_vec()).contains(PRIVATE));
        }
    }
    h.close().await;
}

/// The observation carries the profile id, and it is a different UUID from the
/// account id.
///
/// Every profile-owned table in the concert service is keyed by the profile id,
/// so a caller that had only the account id could not address them. The profile
/// service owns the mapping and reports it here.
#[tokio::test]
async fn the_observation_carries_the_profile_id_distinct_from_the_account_id() {
    let h = Harness::start(Mode::Reply(reply()), false).await;
    let ctx = live();
    let observation = h.client.read_concert_self_profile(&ctx).await.unwrap();
    assert_eq!(observation.user_id().unwrap().to_string(), USER);
    let profile_id = observation
        .profile_id()
        .unwrap()
        .expect("the fixture reports a profile id");
    assert_eq!(profile_id.to_string(), PROFILE);
    assert_ne!(
        profile_id.to_string(),
        USER,
        "the profile id must never be the account id"
    );
    h.close().await;
}

/// An account with no live profile identity reports no profile id.
///
/// The observation is still a valid read — the account has content — but there is
/// no profile to address. A caller that needs one must refuse; it must not
/// substitute the account id.
#[tokio::test]
async fn an_account_without_a_profile_reports_no_profile_id() {
    let mut without = profile("Mira", Some("Loves live sets"));
    without.profile_id = None;
    let h = Harness::start(
        Mode::Reply(proto::ReadConcertSelfProfileResponse {
            profile: Some(without),
        }),
        false,
    )
    .await;
    let ctx = live();
    let observation = h.client.read_concert_self_profile(&ctx).await.unwrap();
    assert_eq!(observation.user_id().unwrap().to_string(), USER);
    assert_eq!(observation.profile_id().unwrap(), None);
    h.close().await;
}

/// A producer that echoes the account id as the profile id is refused.
///
/// The two are separate UUIDs. Accepting the echo would send every profile-owned
/// write to rows keyed by an account id, which is exactly the defect this field
/// exists to remove.
#[tokio::test]
async fn a_profile_id_that_echoes_the_account_id_is_refused() {
    let mut conflated = profile("Mira", Some("Loves live sets"));
    conflated.profile_id = Some(USER.into());
    let h = Harness::start(
        Mode::Reply(proto::ReadConcertSelfProfileResponse {
            profile: Some(conflated),
        }),
        false,
    )
    .await;
    let ctx = live();
    assert_eq!(
        h.client.read_concert_self_profile(&ctx).await.err(),
        Some(ConcertProfileReadError::InvalidResponse)
    );
    h.close().await;
}

/// A non-canonical or nil profile id is refused rather than normalized.
#[tokio::test]
async fn a_malformed_profile_id_is_refused() {
    for bad in [
        "not-a-uuid",
        "00000000-0000-0000-0000-000000000000",
        "2C9F1A44-6B7E-4D2A-9F31-8E5D0C4B7A16",
    ] {
        let mut malformed = profile("Mira", Some("Loves live sets"));
        malformed.profile_id = Some(bad.into());
        let h = Harness::start(
            Mode::Reply(proto::ReadConcertSelfProfileResponse {
                profile: Some(malformed),
            }),
            false,
        )
        .await;
        let ctx = live();
        assert_eq!(
            h.client.read_concert_self_profile(&ctx).await.err(),
            Some(ConcertProfileReadError::InvalidResponse),
            "{bad}"
        );
        h.close().await;
    }
}

#[tokio::test]
async fn missing_misbound_malformed_and_oversized_replies_are_denied() {
    let h = Harness::start(Mode::Reply(reply()), false).await;
    let ctx = live();
    let cases = [
        (None, ConcertProfileReadError::InvalidResponse),
        (
            Some(proto::ConcertSelfProfile {
                user_id: "".into(),
                ..Default::default()
            }),
            ConcertProfileReadError::InvalidResponse,
        ),
    ];
    for (p, expected) in cases {
        *h.owner.mode.lock().unwrap() =
            Mode::Reply(proto::ReadConcertSelfProfileResponse { profile: p });
        assert_eq!(
            h.client.read_concert_self_profile(&ctx).await.unwrap_err(),
            expected
        );
    }
    for id in [
        FAMILY.to_owned(),
        USER.to_uppercase(),
        Uuid::nil().to_string(),
        "bad-uuid".into(),
        format!(" {USER}"),
        USER.replace('-', ""),
    ] {
        let mut changed = profile("Valid", None);
        changed.user_id = id;
        *h.owner.mode.lock().unwrap() = Mode::Reply(proto::ReadConcertSelfProfileResponse {
            profile: Some(changed),
        });
        assert_eq!(
            h.client.read_concert_self_profile(&ctx).await.unwrap_err(),
            ConcertProfileReadError::InvalidResponse
        );
    }
    h.close().await;
    let h = Harness::start(Mode::Reply(reply()), true).await;
    assert_eq!(
        h.client.read_concert_self_profile(&ctx).await.unwrap_err(),
        ConcertProfileReadError::InvalidResponse
    );
    h.close().await;
}

#[tokio::test]
async fn profile_content_limits_controls_empty_bio_and_calendar_are_exact() {
    let h = Harness::start(Mode::Reply(reply()), false).await;
    let ctx = live();
    let mut maximal = profile(&"😀".repeat(40), Some(&"a".repeat(160)));
    *h.owner.mode.lock().unwrap() = Mode::Reply(proto::ReadConcertSelfProfileResponse {
        profile: Some(maximal.clone()),
    });
    assert!(h.client.read_concert_self_profile(&ctx).await.is_ok());
    maximal.observed_at_unix_micros = 978307200000000;
    *h.owner.mode.lock().unwrap() = Mode::Reply(proto::ReadConcertSelfProfileResponse {
        profile: Some(maximal),
    });
    assert_eq!(
        h.client
            .read_concert_self_profile(&ctx)
            .await
            .unwrap()
            .observed_at()
            .unwrap()
            .timestamp_micros(),
        978307200000000
    );
    let empty = profile("Valid", Some(""));
    *h.owner.mode.lock().unwrap() = Mode::Reply(proto::ReadConcertSelfProfileResponse {
        profile: Some(empty),
    });
    assert_eq!(
        h.client
            .read_concert_self_profile(&ctx)
            .await
            .unwrap()
            .bio()
            .unwrap(),
        None
    );
    for p in [
        profile("   ", None),
        profile("a\n", None),
        profile(&"a".repeat(41), None),
        profile("Valid", Some(&"x".repeat(161))),
        profile("Valid", Some("a\n")),
        profile("a\0", None),
        profile(&"😀".repeat(41), None),
        profile("Valid", Some(&"界".repeat(161))),
    ] {
        *h.owner.mode.lock().unwrap() =
            Mode::Reply(proto::ReadConcertSelfProfileResponse { profile: Some(p) });
        assert_eq!(
            h.client.read_concert_self_profile(&ctx).await.unwrap_err(),
            ConcertProfileReadError::InvalidResponse
        );
    }
    for us in [
        i64::MIN,
        Utc.with_ymd_and_hms(0, 12, 31, 0, 0, 0)
            .unwrap()
            .timestamp_micros(),
        Utc.with_ymd_and_hms(10000, 1, 1, 0, 0, 0)
            .unwrap()
            .timestamp_micros(),
        i64::MAX,
    ] {
        let mut p = profile("Valid", None);
        p.observed_at_unix_micros = us;
        *h.owner.mode.lock().unwrap() =
            Mode::Reply(proto::ReadConcertSelfProfileResponse { profile: Some(p) });
        assert_eq!(
            h.client.read_concert_self_profile(&ctx).await.unwrap_err(),
            ConcertProfileReadError::InvalidResponse
        );
    }
    h.close().await;
}

#[tokio::test]
async fn dependency_statuses_are_fixed_safe_and_not_retried() {
    let h = Harness::start(Mode::Reply(reply()), false).await;
    let ctx = live();
    for (code, expected) in [
        (Code::NotFound, ConcertProfileReadError::NotFound),
        (Code::Unauthenticated, ConcertProfileReadError::Denied),
        (Code::PermissionDenied, ConcertProfileReadError::Denied),
        (Code::Unavailable, ConcertProfileReadError::Unavailable),
        (Code::Internal, ConcertProfileReadError::Unavailable),
        (Code::Cancelled, ConcertProfileReadError::Unavailable),
        (Code::Unknown, ConcertProfileReadError::Unavailable),
        (Code::Unimplemented, ConcertProfileReadError::Unavailable),
        (Code::InvalidArgument, ConcertProfileReadError::Unavailable),
        (Code::Aborted, ConcertProfileReadError::Unavailable),
        (
            Code::FailedPrecondition,
            ConcertProfileReadError::Unavailable,
        ),
        (Code::DataLoss, ConcertProfileReadError::InvalidResponse),
        (Code::OutOfRange, ConcertProfileReadError::InvalidResponse),
        (
            Code::DeadlineExceeded,
            ConcertProfileReadError::DeadlineExceeded,
        ),
        (
            Code::ResourceExhausted,
            ConcertProfileReadError::CapacityExhausted,
        ),
    ] {
        *h.owner.mode.lock().unwrap() = Mode::Error(code);
        let error = h.client.read_concert_self_profile(&ctx).await.unwrap_err();
        assert_eq!(error, expected);
        assert!(!format!("{error:?} {error}").contains(PRIVATE));
    }
    assert_eq!(h.owner.calls.load(Ordering::SeqCst), 15);
    h.close().await;
}

#[tokio::test]
async fn wrong_caller_token_and_signature_never_reach_owner() {
    let h = Harness::start(Mode::Reply(reply()), false).await;
    let ctx = live();
    for (caller, token, signer) in [
        (
            Workload::Gateway,
            "wrong",
            ActorDelegationSigner::local_parity(Workload::Gateway),
        ),
        (
            Workload::Post,
            "post-token",
            ActorDelegationSigner::local_parity(Workload::Post),
        ),
        (
            Workload::Gateway,
            TOKEN,
            ActorDelegationSigner::from_seed(Workload::Gateway, &[19; 32]).unwrap(),
        ),
    ] {
        let c = Harness::client(&h.url, caller, token, signer, GatewayMetrics::default()).await;
        assert_eq!(
            c.read_concert_self_profile(&ctx).await.unwrap_err(),
            ConcertProfileReadError::Denied
        );
    }
    assert_eq!(h.owner.calls.load(Ordering::SeqCst), 0);
    h.close().await;
}

#[tokio::test]
async fn awaited_delayed_and_noncooperative_results_cannot_outlive_context() {
    let h = Harness::start(Mode::Delay(Duration::from_millis(250)), false).await;
    let ctx = context(Duration::from_millis(90));
    tokio::time::sleep(Duration::from_millis(40)).await;
    assert!(matches!(
        h.client.read_concert_self_profile(&ctx).await,
        Err(ConcertProfileReadError::AccessExpired | ConcertProfileReadError::DeadlineExceeded)
    ));
    assert_eq!(h.owner.calls.load(Ordering::SeqCst), 1);
    h.close().await;
    let h = Harness::start(Mode::Block(Duration::from_millis(250)), false).await;
    let ctx = context(Duration::from_millis(90));
    assert!(matches!(
        h.client.read_concert_self_profile(&ctx).await,
        Err(ConcertProfileReadError::AccessExpired | ConcertProfileReadError::DeadlineExceeded)
    ));
    assert_eq!(h.owner.calls.load(Ordering::SeqCst), 1);
    h.close().await;
}

#[tokio::test]
async fn consumed_observation_stops_after_borrowed_context_expiry() {
    let h = Harness::start(Mode::Reply(reply()), false).await;
    let ctx = context(Duration::from_millis(160));
    let observation = h.client.read_concert_self_profile(&ctx).await.unwrap();
    tokio::time::sleep(Duration::from_millis(180)).await;
    assert_eq!(
        observation.user_id(),
        Err(ConcertProfileReadError::AccessExpired)
    );
    assert!(observation.display_name().is_err());
    assert!(observation.bio().is_err());
    assert!(observation.observed_at().is_err());
    h.close().await;
}

#[test]
fn read_policy_matches_installed_gateway_actor_contract() {
    let source = include_str!("../../profile-service/src/adapters/in/grpc/method_policy.rs");
    assert!(source.contains("const EDGE: &[Workload] = &[Workload::Gateway]"));
    assert!(source.contains("(\"ReadConcertSelfProfile\", EDGE)"));
    assert!(source.contains("(\"ReadConcertSelfProfile\", &[&[1]])"));
    assert!(POLICY.allows(METHOD, Workload::Gateway));
    assert!(!POLICY.allows(METHOD, Workload::Post));
}

const CAPACITY_TEST: &str = "global_gateway_profile_capacity_refuses_before_dispatch_and_releases";
#[test]
fn global_gateway_profile_capacity_refuses_before_dispatch_and_releases() {
    if std::env::var_os("MCR_GLOBAL_PROFILE_CAPACITY_ISOLATED").is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([CAPACITY_TEST, "--exact", "--test-threads=1", "--nocapture"])
            .env("MCR_GLOBAL_PROFILE_CAPACITY_ISOLATED", "1")
            .status()
            .unwrap();
        assert!(status.success(), "isolated capacity test failed: {status}");
        return;
    }
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(capacity_lifecycle());
}
fn counter(metrics: &GatewayMetrics, name: &str) -> u64 {
    let prefix = format!("{name}{{edge=\"gateway->profile\"}} ");
    metrics
        .render_prometheus()
        .lines()
        .find_map(|line| line.strip_prefix(&prefix).map(|n| n.parse().unwrap()))
        .unwrap_or(0)
}
async fn capacity_lifecycle() {
    use platform::edge_concurrency::EdgeLimiter;
    const REJECTED: &str = "grpc_edge_in_flight_rejections_total";
    const EXPIRED: &str = "grpc_edge_deadline_exhaustions_total";
    let edge = platform::edge::Edge::GatewayProfile;
    let h = Harness::start(Mode::Reply(reply()), false).await;
    let metrics = GatewayMetrics::default();
    let client = Harness::client(
        &h.url,
        Workload::Gateway,
        TOKEN,
        ActorDelegationSigner::local_parity(Workload::Gateway),
        metrics.clone(),
    )
    .await;
    let ctx = live();
    let held: Vec<_> = std::iter::from_fn(|| EdgeLimiter::global().try_acquire(edge)).collect();
    assert_eq!(held.len(), edge.max_in_flight());
    assert_eq!(counter(&metrics, REJECTED), 0);
    assert_eq!(
        client.read_concert_self_profile(&ctx).await.unwrap_err(),
        ConcertProfileReadError::CapacityExhausted
    );
    assert_eq!(h.owner.calls.load(Ordering::SeqCst), 0);
    assert_eq!(counter(&metrics, REJECTED), 1);
    assert_eq!(counter(&metrics, EXPIRED), 0);
    drop(held);
    assert_eq!(EdgeLimiter::global().available(edge), edge.max_in_flight());

    assert!(client.read_concert_self_profile(&ctx).await.is_ok());
    assert_eq!(h.owner.calls.load(Ordering::SeqCst), 1);
    assert_eq!(EdgeLimiter::global().available(edge), edge.max_in_flight());
    *h.owner.mode.lock().unwrap() = Mode::Error(Code::Unavailable);
    assert_eq!(
        client.read_concert_self_profile(&ctx).await.unwrap_err(),
        ConcertProfileReadError::Unavailable
    );
    assert_eq!(h.owner.calls.load(Ordering::SeqCst), 2);
    assert_eq!(EdgeLimiter::global().available(edge), edge.max_in_flight());
    assert_eq!(counter(&metrics, EXPIRED), 0);

    *h.owner.mode.lock().unwrap() = Mode::Error(Code::DeadlineExceeded);
    assert_eq!(
        client.read_concert_self_profile(&ctx).await.unwrap_err(),
        ConcertProfileReadError::DeadlineExceeded
    );
    assert_eq!(h.owner.calls.load(Ordering::SeqCst), 3);
    assert_eq!(counter(&metrics, EXPIRED), 1);
    assert_eq!(EdgeLimiter::global().available(edge), edge.max_in_flight());

    // An earlier task stop must shorten the live context and count once.
    *h.owner.mode.lock().unwrap() = Mode::Hold;
    assert_eq!(
        platform::call_deadline::scope(
            std::time::Instant::now() + Duration::from_millis(100),
            client.read_concert_self_profile(&ctx),
        )
        .await
        .unwrap_err(),
        ConcertProfileReadError::DeadlineExceeded
    );
    assert!(ctx.check().is_ok());
    assert_eq!(h.owner.calls.load(Ordering::SeqCst), 4);
    assert_eq!(counter(&metrics, EXPIRED), 2);
    assert_eq!(counter(&metrics, REJECTED), 1);
    assert_eq!(EdgeLimiter::global().available(edge), edge.max_in_flight());

    // A fresh harness plus observed owner entry AND permit ownership prevent
    // a stale notification or an unpolled spawned task proving cancellation.
    let cancellation = Harness::start(Mode::Hold, false).await;
    let cancellation_metrics = GatewayMetrics::default();
    let cancel_client = Harness::client(
        &cancellation.url,
        Workload::Gateway,
        TOKEN,
        ActorDelegationSigner::local_parity(Workload::Gateway),
        cancellation_metrics.clone(),
    )
    .await;
    let cancel_context = live();
    let task = tokio::spawn(async move {
        cancel_client
            .read_concert_self_profile(&cancel_context)
            .await
            .map(|_| ())
    });
    tokio::time::timeout(Duration::from_millis(600), async {
        loop {
            if cancellation.owner.calls.load(Ordering::SeqCst) == 1
                && cancellation.owner.active.load(Ordering::SeqCst) == 1
                && EdgeLimiter::global().available(edge) == edge.max_in_flight() - 1
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("cancelled read must hold a real permit and enter its owner");
    assert!(!task.is_finished());
    assert_eq!(cancellation.owner.calls.load(Ordering::SeqCst), 1);
    assert_eq!(cancellation.owner.active.load(Ordering::SeqCst), 1);
    assert_eq!(
        EdgeLimiter::global().available(edge),
        edge.max_in_flight() - 1
    );
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(EdgeLimiter::global().available(edge), edge.max_in_flight());
    assert_eq!(counter(&cancellation_metrics, EXPIRED), 0);
    assert_eq!(counter(&cancellation_metrics, REJECTED), 0);
    tokio::time::timeout(Duration::from_secs(2), async {
        while cancellation.owner.active.load(Ordering::SeqCst) != 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("owned cancelled handler must stop");
    *cancellation.owner.mode.lock().unwrap() = Mode::Reply(reply());
    assert!(cancellation
        .client
        .read_concert_self_profile(&live())
        .await
        .is_ok());
    assert_eq!(cancellation.owner.calls.load(Ordering::SeqCst), 2);
    assert_eq!(EdgeLimiter::global().available(edge), edge.max_in_flight());
    println!(
        "GatewayProfile cancellation: calls={} rejected={} deadlines={} available={}/{}",
        cancellation.owner.calls.load(Ordering::SeqCst),
        counter(&cancellation_metrics, REJECTED),
        counter(&cancellation_metrics, EXPIRED),
        EdgeLimiter::global().available(edge),
        edge.max_in_flight()
    );
    cancellation.close().await;

    *h.owner.mode.lock().unwrap() = Mode::Reply(reply());
    assert!(client.read_concert_self_profile(&ctx).await.is_ok());
    assert_eq!(h.owner.calls.load(Ordering::SeqCst), 5);
    assert_eq!(EdgeLimiter::global().available(edge), edge.max_in_flight());
    assert_eq!(counter(&metrics, EXPIRED), 2);
    assert_eq!(counter(&metrics, REJECTED), 1);
    println!(
        "GatewayProfile lifecycle: calls={} rejected={} deadlines={} available={}/{}",
        h.owner.calls.load(Ordering::SeqCst),
        counter(&metrics, REJECTED),
        counter(&metrics, EXPIRED),
        EdgeLimiter::global().available(edge),
        edge.max_in_flight()
    );
    h.close().await;
}

#[tokio::test]
async fn guard_rejects_wrong_actor_body_audience_signature_and_missing_original_deadline() {
    let h = Harness::start(Mode::Reply(reply()), false).await;
    let mut c = proto::profile_service_client::ProfileServiceClient::new(
        Endpoint::from_shared(h.url.clone())
            .unwrap()
            .connect()
            .await
            .unwrap(),
    );
    let msg = proto::ReadConcertSelfProfileRequest {
        user_id: USER.into(),
    };
    let frame = |m: &proto::ReadConcertSelfProfileRequest| {
        let payload = m.encode_to_vec();
        let mut b = vec![0];
        b.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        b.extend(payload);
        b
    };
    for attack in 0..6 {
        let signer = if attack == 3 {
            ActorDelegationSigner::from_seed(Workload::Gateway, &[23; 32]).unwrap()
        } else {
            ActorDelegationSigner::local_parity(Workload::Gateway)
        };
        let actor = if attack == 0 {
            Uuid::new_v4()
        } else {
            Uuid::parse_str(USER).unwrap()
        };
        let body = if attack == 1 {
            frame(&proto::ReadConcertSelfProfileRequest {
                user_id: Uuid::new_v4().to_string(),
                ..msg.clone()
            })
        } else {
            frame(&msg)
        };
        let proof = signer.issue(
            if attack == 2 {
                Workload::Auth
            } else {
                Workload::Profile
            },
            METHOD,
            actor,
            None,
            &body,
            Utc::now().timestamp(),
        );
        let mut request = Request::new(msg.clone());
        request
            .metadata_mut()
            .insert("x-service-caller", "gateway".parse().unwrap());
        request
            .metadata_mut()
            .insert("x-service-token", TOKEN.parse().unwrap());
        if attack != 5 {
            request
                .metadata_mut()
                .insert(platform::actor_delegation::HEADER, proof.parse().unwrap());
        }
        if attack != 4 {
            request.metadata_mut().insert(
                platform::call_deadline::REMAINING_HEADER,
                "2S".parse().unwrap(),
            );
        }
        assert!(c.read_concert_self_profile(request).await.is_err());
    }
    assert_eq!(h.owner.calls.load(Ordering::SeqCst), 0);
    h.close().await;
}

#[tokio::test]
async fn channel_readiness_is_bounded_by_original_context_before_any_server_dispatch() {
    let socket = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", socket.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let (_socket, _) = socket.accept().await.unwrap();
        std::future::pending::<()>().await;
    });
    let c = Harness::client(
        &url,
        Workload::Gateway,
        TOKEN,
        ActorDelegationSigner::local_parity(Workload::Gateway),
        GatewayMetrics::default(),
    )
    .await;
    let ctx = context(Duration::from_millis(80));
    let start = tokio::time::Instant::now();
    assert!(matches!(
        c.read_concert_self_profile(&ctx).await,
        Err(ConcertProfileReadError::AccessExpired | ConcertProfileReadError::DeadlineExceeded)
    ));
    assert!(start.elapsed() < Duration::from_millis(400));
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
}

#[tokio::test]
async fn captured_jwt_monotonic_expiry_during_reply_wait_refuses_live_wall_token() {
    let h = Harness::start(Mode::Hold, false).await;
    let client = h.client.clone();
    let ctx = context_with_exp(Duration::from_secs(3600), Utc::now().timestamp() + 900);
    let expiry = ctx.access_expires_micros();
    let task =
        tokio::spawn(async move { client.read_concert_self_profile(&ctx).await.map(|_| ()) });
    tokio::time::timeout(Duration::from_secs(1), entered(&h.owner))
        .await
        .unwrap();
    // Pause only after actual network dispatch, then move the test-owned clock.
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(901)).await;
    assert!(Utc::now().timestamp_micros() < expiry);
    assert_eq!(
        task.await.unwrap().unwrap_err(),
        ConcertProfileReadError::AccessExpired
    );
    tokio::time::resume();
    h.close().await;
}

#[tokio::test]
async fn actual_jwt_wall_expiry_during_reply_wait_refuses_success() {
    let h = Harness::start(Mode::Hold, false).await;
    // RSA fixture initialization must finish before choosing a short live exp.
    let _ = first();
    // Align before issuing ONE token so actual expiry is 500..600ms away,
    // with dispatch margin and below the existing 700ms local edge ceiling.
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let us = Utc::now().timestamp_subsec_micros();
            if (400_000..=500_000).contains(&us) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let ctx = context_with_exp(Duration::from_secs(5), Utc::now().timestamp() + 1);
    let expires = ctx.access_expires_micros();
    let client = h.client.clone();
    let sent = tokio::time::Instant::now();
    let call = client.read_concert_self_profile(&ctx);
    tokio::pin!(call);
    tokio::select! {biased;() = entered(&h.owner)=>{},result=&mut call=>panic!("premature return: {result:?}")}
    assert!(Utc::now().timestamp_micros() < expires);
    assert!(matches!(
        call.await,
        Err(ConcertProfileReadError::AccessExpired | ConcertProfileReadError::DeadlineExceeded)
    ));
    assert!(Utc::now().timestamp_micros() >= expires);
    assert!(
        sent.elapsed()
            < platform::grpc_deadline::local_deadline(platform::edge::Edge::GatewayProfile)
    );
    assert_eq!(h.owner.calls.load(Ordering::SeqCst), 1);
    h.close().await;
}

async fn entered(owner: &Owner) {
    while owner.calls.load(Ordering::SeqCst) == 0 {
        tokio::task::yield_now().await;
    }
    assert_eq!(owner.active.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn already_elapsed_context_never_dispatches_and_values_are_not_trimmed() {
    let h = Harness::start(Mode::Reply(reply()), false).await;
    let expired = context(Duration::from_millis(10));
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert_eq!(
        h.client
            .read_concert_self_profile(&expired)
            .await
            .unwrap_err(),
        ConcertProfileReadError::AccessExpired
    );
    assert_eq!(h.owner.calls.load(Ordering::SeqCst), 0);
    let ctx = live();
    for us in [
        0,
        Utc.with_ymd_and_hms(1, 1, 1, 0, 0, 0)
            .unwrap()
            .timestamp_micros(),
        Utc.with_ymd_and_hms(9999, 12, 31, 23, 59, 59)
            .unwrap()
            .timestamp_micros(),
    ] {
        let mut p = profile("  Mira  ", Some("   "));
        p.observed_at_unix_micros = us;
        *h.owner.mode.lock().unwrap() =
            Mode::Reply(proto::ReadConcertSelfProfileResponse { profile: Some(p) });
        let observation = h.client.read_concert_self_profile(&ctx).await.unwrap();
        assert_eq!(observation.display_name().unwrap(), "  Mira  ");
        assert_eq!(observation.bio().unwrap(), Some("   "));
        assert_eq!(observation.observed_at().unwrap().timestamp_micros(), us);
    }
    h.close().await;
}

#[tokio::test]
async fn decode_limit_counts_valid_unknown_fields_at_exact_boundary() {
    for (size, accepted) in [(1024, true), (1025, false)] {
        let h = Harness::with_frame_size(Mode::Reply(reply()), Some(size)).await;
        let ctx = live();
        let response = h.client.read_concert_self_profile(&ctx).await;
        if accepted {
            assert_eq!(response.unwrap().display_name().unwrap(), "Mira");
        } else {
            assert_eq!(
                response.unwrap_err(),
                ConcertProfileReadError::InvalidResponse
            );
        }
        assert_eq!(h.owner.calls.load(Ordering::SeqCst), 1);
        h.close().await;
    }
}
