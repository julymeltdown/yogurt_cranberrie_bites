//! Guarded generated transport with a controlled owner, never native Auth/DB acceptance.
//! Non-read trait signatures adapted from verification_transport.rs at e6efa4c.
use api_gateway::adapters::out::{
    auth_client::{concert_admission::*, proto, GrpcAuthClient},
    jwt::{concert_access::ConcertAccessContext, JwtVerifier},
};
use api_gateway::config::GatewayMetrics;
use chrono::Utc;
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
use tokio::{
    net::TcpListener,
    sync::{oneshot, Notify},
    task::JoinHandle,
};
use tokio_stream::wrappers::TcpListenerStream;
use tonic::{
    transport::{Endpoint, Server},
    Code, Request, Response, Status,
};
use uuid::Uuid;

// Exact ReadConcertAdmission subset of Auth's installed policy, pinned below.
static POLICY: MethodPolicy = MethodPolicy {
    service: "auth.v1.AuthService",
    workload: Some(Workload::Auth),
    methods: &[("ReadConcertAdmission", &[Workload::Gateway])],
    actor_fields: &[("ReadConcertAdmission", &[&[1]])],
};
const METHOD: &str = "/auth.v1.AuthService/ReadConcertAdmission";
const TOKEN: &str = "controlled-gateway-token";
const USER: &str = "7e023b36-5ca9-4a40-850d-d01dc16874a8";
const FAMILY: &str = "fa60b0de-7277-447c-aa36-eaf530dd3018";
const PRIVATE: &str = "PRIVATE_SQL_EMAIL_ROLE_PROFILE_CANARY";

#[derive(Clone)]
enum Mode {
    Reply(proto::ReadConcertAdmissionResponse),
    Delay(Duration),
    Block(Duration),
    Hold,
    Error(Code),
}
struct Owner {
    mode: Mutex<Mode>,
    calls: AtomicUsize,
    active: AtomicUsize,
    entered: Notify,
    seen: Mutex<Vec<proto::ReadConcertAdmissionRequest>>,
}
struct Active(Arc<Owner>);
impl Drop for Active {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::SeqCst);
    }
}
#[derive(Clone)]
struct FakeAuth(Arc<Owner>);
fn reply() -> proto::ReadConcertAdmissionResponse {
    proto::ReadConcertAdmissionResponse {
        observed_at_unix_micros: Utc::now().timestamp_micros(),
        account_active: true,
        session_live: true,
        binding_state: 1,
    }
}
#[tonic::async_trait]
impl proto::auth_service_server::AuthService for FakeAuth {
    async fn read_concert_admission(
        &self,
        request: Request<proto::ReadConcertAdmissionRequest>,
    ) -> Result<Response<proto::ReadConcertAdmissionResponse>, Status> {
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
        self.0.entered.notify_one();
        let mode = self.0.mode.lock().unwrap().clone();
        match mode {
            Mode::Reply(r) => Ok(Response::new(r)),
            Mode::Delay(delay) => {
                tokio::time::sleep(delay).await;
                Ok(Response::new(reply()))
            }
            Mode::Block(delay) => {
                std::thread::sleep(delay);
                Ok(Response::new(reply()))
            }
            Mode::Hold => std::future::pending().await,
            Mode::Error(code) => Err(Status::new(code, PRIVATE)),
        }
    }
    async fn resend_verification(
        &self,
        _request: Request<proto::ResendVerificationRequest>,
    ) -> Result<Response<proto::ResendVerificationResponse>, Status> {
        Err(Status::unimplemented("outside controlled read fixture"))
    }
    async fn signup_email(
        &self,
        _request: Request<proto::SignupEmailRequest>,
    ) -> Result<Response<proto::SignupResponse>, Status> {
        Err(Status::unimplemented("outside controlled read fixture"))
    }
    async fn verify_email(
        &self,
        _request: Request<proto::VerifyEmailRequest>,
    ) -> Result<Response<proto::SignupResponse>, Status> {
        Err(Status::unimplemented("outside controlled read fixture"))
    }
    async fn check_email(
        &self,
        _request: Request<proto::CheckEmailRequest>,
    ) -> Result<Response<proto::EmailAvailabilityResponse>, Status> {
        Err(Status::unimplemented("outside controlled read fixture"))
    }
    async fn login_email(
        &self,
        _request: Request<proto::LoginEmailRequest>,
    ) -> Result<Response<proto::AuthResponse>, Status> {
        Err(Status::unimplemented("outside controlled read fixture"))
    }
    async fn refresh_tokens(
        &self,
        _request: Request<proto::RefreshTokensRequest>,
    ) -> Result<Response<proto::AuthResponse>, Status> {
        Err(Status::unimplemented("outside controlled read fixture"))
    }
    async fn logout(
        &self,
        _request: Request<proto::LogoutRequest>,
    ) -> Result<Response<proto::PasswordResetResponse>, Status> {
        Err(Status::unimplemented("outside controlled read fixture"))
    }
    async fn request_password_reset(
        &self,
        _request: Request<proto::RequestPasswordResetRequest>,
    ) -> Result<Response<proto::PasswordResetResponse>, Status> {
        Err(Status::unimplemented("outside controlled read fixture"))
    }
    async fn confirm_password_reset(
        &self,
        _request: Request<proto::ConfirmPasswordResetRequest>,
    ) -> Result<Response<proto::PasswordResetResponse>, Status> {
        Err(Status::unimplemented("outside controlled read fixture"))
    }
    async fn authorize_o_auth(
        &self,
        _request: Request<proto::AuthorizeOAuthRequest>,
    ) -> Result<Response<proto::OAuthAuthorizeResponse>, Status> {
        Err(Status::unimplemented("outside controlled read fixture"))
    }
    async fn o_auth_callback(
        &self,
        _request: Request<proto::OAuthCallbackRequest>,
    ) -> Result<Response<proto::AuthResponse>, Status> {
        Err(Status::unimplemented("outside controlled read fixture"))
    }
    async fn link_o_auth_provider(
        &self,
        _request: Request<proto::LinkOAuthProviderRequest>,
    ) -> Result<Response<proto::LinkedProvidersResponse>, Status> {
        Err(Status::unimplemented("outside controlled read fixture"))
    }
    async fn delete_user(
        &self,
        _request: Request<proto::DeleteUserRequest>,
    ) -> Result<Response<proto::DeleteUserResponse>, Status> {
        Err(Status::unimplemented("outside controlled read fixture"))
    }
    async fn get_user_deletion(
        &self,
        _request: Request<proto::GetUserDeletionRequest>,
    ) -> Result<Response<proto::GetUserDeletionResponse>, Status> {
        Err(Status::unimplemented("outside controlled read fixture"))
    }
    async fn get_user_deletion_by_saga_id(
        &self,
        _request: Request<proto::GetUserDeletionBySagaIdRequest>,
    ) -> Result<Response<proto::GetUserDeletionBySagaIdResponse>, Status> {
        Err(Status::unimplemented("outside controlled read fixture"))
    }
    async fn list_dead_lettered_user_deletions(
        &self,
        _request: Request<proto::ListDeadLetteredUserDeletionsRequest>,
    ) -> Result<Response<proto::ListDeadLetteredUserDeletionsResponse>, Status> {
        Err(Status::unimplemented("outside controlled read fixture"))
    }
    async fn retry_user_deletion(
        &self,
        _request: Request<proto::RetryUserDeletionRequest>,
    ) -> Result<Response<proto::RetryUserDeletionResponse>, Status> {
        Err(Status::unimplemented("outside controlled read fixture"))
    }
    async fn get_signing_keys(
        &self,
        _request: Request<proto::GetSigningKeysRequest>,
    ) -> Result<Response<proto::GetSigningKeysResponse>, Status> {
        Err(Status::unimplemented("outside controlled read fixture"))
    }
    async fn check_session(
        &self,
        _request: Request<proto::CheckSessionRequest>,
    ) -> Result<Response<proto::CheckSessionResponse>, Status> {
        Err(Status::unimplemented("outside controlled read fixture"))
    }
}

use base64ct::{Base64UrlUnpadded, Encoding};
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use rand::rngs::OsRng;
use rsa::{pkcs8::EncodePrivateKey, traits::PublicKeyParts, RsaPrivateKey, RsaPublicKey};
use serde_json::Value;
use std::sync::OnceLock;
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
        json!({"kid": self.kid, "kty": "RSA", "alg": "RS256", "use": "sig",
            "n": Base64UrlUnpadded::encode_string(&self.public.n().to_bytes_be()),
            "e": Base64UrlUnpadded::encode_string(&self.public.e().to_bytes_be())})
    }
    fn header(&self) -> Header {
        Header {
            kid: Some(self.kid.into()),
            ..Header::new(Algorithm::RS256)
        }
    }
    fn token(&self, claims: &Value) -> String {
        self.with_header(claims, self.header())
    }
    fn with_header(&self, claims: &Value, header: Header) -> String {
        jsonwebtoken::encode(&header, claims, &self.encoding).unwrap()
    }
}
fn first() -> &'static Key {
    static KEY: OnceLock<Key> = OnceLock::new();
    KEY.get_or_init(|| Key::generate("strict-first"))
}

fn context(after: Duration, exp: i64) -> ConcertAccessContext {
    let verifier = JwtVerifier::from_jwks_json(
        "fixture-auth",
        "fixture-api",
        &json!({"keys":[first().jwk()]}).to_string(),
    )
    .unwrap();
    let token=first().token(&json!({"sub":USER,"sid":FAMILY,"iss":"fixture-auth","aud":"fixture-api","token_type":"access","exp":exp,"roles":[PRIVATE]}));
    verifier
        .verify_concert_access(
            &token,
            Some((tokio::time::Instant::now() + after).into_std()),
        )
        .unwrap()
}
fn live() -> ConcertAccessContext {
    context(Duration::from_secs(20), Utc::now().timestamp() + 900)
}

#[derive(Clone)]
struct Wire<S> {
    inner: S,
    oversize: bool,
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
        let oversized = self.oversize;
        Box::pin(async move {
            let response = f.await?;
            if !oversized {
                return Ok(response);
            }
            let (parts, body) = response.into_parts();
            let mut bytes = body.collect().await.unwrap().to_bytes().to_vec();
            // Valid unknown protobuf field 100 (length-delimited), exceeding
            // the production client's bound before protobuf drops unknowns.
            bytes.extend_from_slice(&[0xa2, 0x06, 0x80, 0x04]);
            bytes.extend_from_slice(&[0; 512]);
            let size = (bytes.len() - 5) as u32;
            bytes[1..5].copy_from_slice(&size.to_be_bytes());
            Ok(http::Response::from_parts(
                parts,
                tonic::body::Body::new(Full::new(tonic::codegen::Bytes::from(bytes))),
            ))
        })
    }
}
struct Harness {
    client: GrpcAuthClient,
    owner: Arc<Owner>,
    url: String,
    stop: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<Result<(), tonic::transport::Error>>>,
}
impl Harness {
    async fn start(mode: Mode, oversize: bool) -> Self {
        let owner = Arc::new(Owner {
            mode: Mutex::new(mode),
            calls: AtomicUsize::new(0),
            active: AtomicUsize::new(0),
            entered: Notify::new(),
            seen: Mutex::new(Vec::new()),
        });
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let (stop, stopped) = oneshot::channel();
        let service = guard(
            &POLICY,
            Wire {
                inner: proto::auth_service_server::AuthServiceServer::new(FakeAuth(owner.clone())),
                oversize,
            },
            CallerRegistry::new()
                .register(Workload::Gateway, TOKEN)
                .unwrap()
                .register(Workload::Post, "post-token")
                .unwrap(),
            ActorDelegationVerifier::local_parity(Workload::Auth),
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
    ) -> GrpcAuthClient {
        GrpcAuthClient::connect_with_metrics(
            url.into(),
            caller,
            token,
            None,
            signer,
            GatewayMetrics::default(),
        )
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
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}
fn binding() -> BindingReference {
    BindingReference::new(
        "bdc6ac96-c145-4278-a3ef-72de355a6562",
        BindingProvider::Google,
        17,
    )
    .unwrap()
}

#[tokio::test]
async fn actual_configured_signed_read_is_fresh_email_only_and_keeps_negative_flags() {
    let h = Harness::start(Mode::Reply(reply()), false).await;
    let ctx = live();
    let a = h.client.read_concert_admission(&ctx, None).await.unwrap();
    assert!(a.account_active().unwrap());
    assert!(a.session_live().unwrap());
    assert_eq!(a.binding_state().unwrap(), BindingState::NotRequested);
    assert!(a.binding_reference().unwrap().is_none());
    assert_eq!(format!("{a:?}"), "AdmissionObservation([REDACTED])");
    let mut negative = reply();
    negative.account_active = false;
    negative.session_live = false;
    *h.owner.mode.lock().unwrap() = Mode::Reply(negative);
    let b = h.client.read_concert_admission(&ctx, None).await.unwrap();
    assert!(!b.account_active().unwrap());
    assert!(!b.session_live().unwrap());
    assert_eq!(h.owner.calls.load(Ordering::SeqCst), 2);
    for r in h.owner.seen.lock().unwrap().iter() {
        assert_eq!(r.user_id, USER);
        assert_eq!(r.session_family_id, FAMILY);
        assert!(r.binding.is_none());
        assert!(!r
            .encode_to_vec()
            .windows(PRIVATE.len())
            .any(|b| b == PRIVATE.as_bytes()));
    }
    h.close().await;
}
#[tokio::test]
async fn exact_requested_binding_shape_and_closed_states_are_preserved() {
    let h = Harness::start(Mode::Reply(reply()), false).await;
    let ctx = live();
    for (value, expected) in [(2, BindingState::Bound), (3, BindingState::NotBound)] {
        let mut r = reply();
        r.binding_state = value;
        *h.owner.mode.lock().unwrap() = Mode::Reply(r);
        let o = h
            .client
            .read_concert_admission(&ctx, Some(binding()))
            .await
            .unwrap();
        assert_eq!(o.binding_state().unwrap(), expected);
        assert!(o.binding_reference().unwrap().is_some());
    }
    let seen = h.owner.seen.lock().unwrap();
    for r in seen.iter() {
        let b = r.binding.as_ref().unwrap();
        assert_eq!(b.provider, "GOOGLE");
        assert_eq!(b.binding_epoch, 17);
        assert_eq!(b.identity_id, "bdc6ac96-c145-4278-a3ef-72de355a6562");
    }
    drop(seen);
    h.close().await;
}
#[test]
fn binding_inputs_are_shape_only_closed_canonical_and_bounded() {
    for provider in ["EMAIL", "GOOGLE", "APPLE", "X"] {
        assert!(BindingProvider::parse(provider).is_ok());
    }
    for provider in ["google", "", PRIVATE, "FACEBOOK"] {
        assert!(BindingProvider::parse(provider).is_err());
    }
    for id in [
        "",
        USER.to_uppercase().as_str(),
        "00000000-0000-0000-0000-000000000000",
        "7e023b365ca94a40850dd01dc16874a8",
    ] {
        assert!(BindingReference::new(id, BindingProvider::X, 1).is_err());
    }
    for epoch in [0, -1, MAX_BINDING_EPOCH + 1, i64::MAX] {
        assert!(BindingReference::new(USER, BindingProvider::Email, epoch).is_err());
    }
    assert!(BindingReference::new(USER, BindingProvider::Email, MAX_BINDING_EPOCH).is_ok());
    assert_eq!(format!("{:?}", binding()), "BindingReference([REDACTED])");
}
#[tokio::test]
async fn malformed_enum_consistency_timestamp_and_size_never_project_success() {
    let h = Harness::start(Mode::Reply(reply()), false).await;
    let ctx = live();
    for (state, requested) in [(0, false), (99, false), (2, false), (3, false), (1, true)] {
        let mut r = reply();
        r.binding_state = state;
        *h.owner.mode.lock().unwrap() = Mode::Reply(r);
        assert_eq!(
            h.client
                .read_concert_admission(&ctx, requested.then(binding))
                .await
                .unwrap_err(),
            AdmissionReadError::InvalidResponse
        );
    }
    let mut r = reply();
    r.observed_at_unix_micros = i64::MAX;
    *h.owner.mode.lock().unwrap() = Mode::Reply(r);
    assert_eq!(
        h.client
            .read_concert_admission(&ctx, None)
            .await
            .unwrap_err(),
        AdmissionReadError::InvalidResponse
    );
    h.close().await;
    let h = Harness::start(Mode::Reply(reply()), true).await;
    assert_eq!(
        h.client
            .read_concert_admission(&ctx, None)
            .await
            .unwrap_err(),
        AdmissionReadError::InvalidResponse
    );
    h.close().await;
}
#[tokio::test]
async fn upstream_private_errors_are_fixed_safe_and_never_retried() {
    let h = Harness::start(Mode::Reply(reply()), false).await;
    let ctx = live();
    for code in [
        Code::Internal,
        Code::Unavailable,
        Code::Cancelled,
        Code::Unimplemented,
        Code::Unauthenticated,
        Code::PermissionDenied,
    ] {
        *h.owner.mode.lock().unwrap() = Mode::Error(code);
        let error = h
            .client
            .read_concert_admission(&ctx, None)
            .await
            .unwrap_err();
        assert!(!format!("{error:?} {error}").contains(PRIVATE));
        assert_eq!(
            error,
            if matches!(code, Code::Unauthenticated | Code::PermissionDenied) {
                AdmissionReadError::Denied
            } else {
                AdmissionReadError::Unavailable
            }
        );
    }
    assert_eq!(h.owner.calls.load(Ordering::SeqCst), 6);
    h.close().await;
}
#[tokio::test]
async fn wrong_configured_service_caller_and_signing_key_are_denied_before_owner() {
    let h = Harness::start(Mode::Reply(reply()), false).await;
    let ctx = live();
    for (caller, token, signer) in [
        (
            Workload::Gateway,
            "bad-token",
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
        let c = Harness::client(&h.url, caller, token, signer).await;
        assert_eq!(
            c.read_concert_admission(&ctx, None).await.unwrap_err(),
            AdmissionReadError::Denied
        );
    }
    assert_eq!(h.owner.calls.load(Ordering::SeqCst), 0);
    h.close().await;
}

#[tokio::test]
async fn guard_rejects_wrong_actor_body_audience_signature_and_missing_original_deadline() {
    let h = Harness::start(Mode::Reply(reply()), false).await;
    let mut c = proto::auth_service_client::AuthServiceClient::new(
        Endpoint::from_shared(h.url.clone())
            .unwrap()
            .connect()
            .await
            .unwrap(),
    );
    let msg = proto::ReadConcertAdmissionRequest {
        user_id: USER.into(),
        session_family_id: FAMILY.into(),
        binding: None,
    };
    let frame = |m: &proto::ReadConcertAdmissionRequest| {
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
            frame(&proto::ReadConcertAdmissionRequest {
                session_family_id: Uuid::new_v4().to_string(),
                ..msg.clone()
            })
        } else {
            frame(&msg)
        };
        let proof = signer.issue(
            if attack == 2 {
                Workload::Profile
            } else {
                Workload::Auth
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
        assert!(c.read_concert_admission(request).await.is_err());
    }
    assert_eq!(h.owner.calls.load(Ordering::SeqCst), 0);
    h.close().await;
}

#[tokio::test]
async fn earlier_waits_and_already_elapsed_context_do_not_receive_fresh_edge_time() {
    let h = Harness::start(Mode::Delay(Duration::from_millis(300)), false).await;
    let ctx = context(Duration::from_millis(180), Utc::now().timestamp() + 900);
    tokio::time::sleep(Duration::from_millis(100)).await;
    let started = tokio::time::Instant::now();
    assert!(matches!(
        h.client.read_concert_admission(&ctx, None).await,
        Err(AdmissionReadError::AccessExpired | AdmissionReadError::DeadlineExceeded)
    ));
    assert!(started.elapsed() < Duration::from_millis(180));
    assert_eq!(h.owner.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        h.client
            .read_concert_admission(&ctx, None)
            .await
            .unwrap_err(),
        AdmissionReadError::AccessExpired
    );
    assert_eq!(h.owner.calls.load(Ordering::SeqCst), 1);
    h.close().await;
}

#[tokio::test]
async fn observation_consumption_rechecks_its_borrowed_context() {
    let h = Harness::start(Mode::Reply(reply()), false).await;
    let ctx = context(Duration::from_millis(250), Utc::now().timestamp() + 900);
    let observation = h.client.read_concert_admission(&ctx, None).await.unwrap();
    assert!(observation.observed_at().is_ok());
    tokio::time::sleep(Duration::from_millis(260)).await;
    assert_eq!(
        observation.account_active(),
        Err(AdmissionReadError::AccessExpired)
    );
    assert_eq!(
        observation.session_live(),
        Err(AdmissionReadError::AccessExpired)
    );
    assert_eq!(
        observation.binding_state(),
        Err(AdmissionReadError::AccessExpired)
    );
    assert_eq!(
        observation.observed_at(),
        Err(AdmissionReadError::AccessExpired)
    );
    assert!(observation.binding_reference().is_err());
    h.close().await;
}

#[tokio::test]
async fn slow_and_noncooperative_late_replies_cannot_outlive_original_stop() {
    for mode in [
        Mode::Delay(Duration::from_millis(200)),
        Mode::Block(Duration::from_millis(200)),
    ] {
        let h = Harness::start(mode, false).await;
        let ctx = context(Duration::from_millis(80), Utc::now().timestamp() + 900);
        assert!(matches!(
            h.client.read_concert_admission(&ctx, None).await,
            Err(AdmissionReadError::AccessExpired | AdmissionReadError::DeadlineExceeded)
        ));
        assert_eq!(h.owner.calls.load(Ordering::SeqCst), 1);
        h.close().await;
    }
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
    )
    .await;
    let ctx = context(Duration::from_millis(80), Utc::now().timestamp() + 900);
    let start = tokio::time::Instant::now();
    assert!(matches!(
        c.read_concert_admission(&ctx, None).await,
        Err(AdmissionReadError::AccessExpired | AdmissionReadError::DeadlineExceeded)
    ));
    assert!(start.elapsed() < Duration::from_millis(400));
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
}

#[tokio::test]
async fn dropped_future_releases_controlled_handler_and_next_read_recovers() {
    let h = Harness::start(Mode::Hold, false).await;
    let client = h.client.clone();
    let ctx = live();
    let task =
        tokio::spawn(async move { client.read_concert_admission(&ctx, None).await.map(|_| ()) });
    tokio::time::timeout(Duration::from_secs(1), h.owner.entered.notified())
        .await
        .unwrap();
    assert_eq!(h.owner.active.load(Ordering::SeqCst), 1);
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    tokio::time::timeout(Duration::from_secs(2), async {
        while h.owner.active.load(Ordering::SeqCst) != 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    *h.owner.mode.lock().unwrap() = Mode::Reply(reply());
    assert!(h.client.read_concert_admission(&live(), None).await.is_ok());
    assert_eq!(h.owner.calls.load(Ordering::SeqCst), 2);
    h.close().await;
}

#[tokio::test]
async fn captured_jwt_monotonic_expiry_during_reply_wait_refuses_live_wall_token() {
    let h = Harness::start(Mode::Hold, false).await;
    let client = h.client.clone();
    let ctx = context(Duration::from_secs(3600), Utc::now().timestamp() + 900);
    let expiry = ctx.access_expires_micros();
    let task =
        tokio::spawn(async move { client.read_concert_admission(&ctx, None).await.map(|_| ()) });
    tokio::time::timeout(Duration::from_secs(1), h.owner.entered.notified())
        .await
        .unwrap();
    // Pause only after actual network dispatch, then move the test-owned clock.
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(901)).await;
    assert!(Utc::now().timestamp_micros() < expiry);
    assert_eq!(
        task.await.unwrap().unwrap_err(),
        AdmissionReadError::AccessExpired
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
    // with dispatch margin and below the existing 800ms local edge ceiling.
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
    let ctx = context(Duration::from_secs(5), Utc::now().timestamp() + 1);
    let expires = ctx.access_expires_micros();
    let client = h.client.clone();
    let sent = tokio::time::Instant::now();
    let call = client.read_concert_admission(&ctx, None);
    tokio::pin!(call);
    tokio::select! {biased;() = h.owner.entered.notified()=>{},result=&mut call=>panic!("premature return: {result:?}")}
    assert!(Utc::now().timestamp_micros() < expires);
    assert!(matches!(
        call.await,
        Err(AdmissionReadError::AccessExpired | AdmissionReadError::DeadlineExceeded)
    ));
    assert!(Utc::now().timestamp_micros() >= expires);
    assert!(
        sent.elapsed() < platform::grpc_deadline::local_deadline(platform::edge::Edge::GatewayAuth)
    );
    assert_eq!(h.owner.calls.load(Ordering::SeqCst), 1);
    h.close().await;
}

#[tokio::test]
async fn observation_time_is_representation_checked_without_an_invented_ttl_and_outage_is_safe() {
    let mut old = reply();
    old.observed_at_unix_micros = 0;
    let h = Harness::start(Mode::Reply(old), false).await;
    let ctx = live();
    assert_eq!(
        h.client
            .read_concert_admission(&ctx, None)
            .await
            .unwrap()
            .observed_at()
            .unwrap()
            .timestamp_micros(),
        0
    );
    let client = h.client.clone();
    h.close().await;
    let error = client.read_concert_admission(&ctx, None).await.unwrap_err();
    assert!(matches!(
        error,
        AdmissionReadError::Unavailable | AdmissionReadError::DeadlineExceeded
    ));
    assert!(!format!("{error:?} {error}").contains(PRIVATE));
}

#[test]
fn read_policy_matches_the_installed_auth_caller_and_actor_contract() {
    let source = include_str!("../../auth-service/src/adapters/in/grpc/method_policy.rs");
    assert!(source.contains("const EDGE: &[Workload] = &[Workload::Gateway]"));
    assert!(source.contains("(\"ReadConcertAdmission\", EDGE)"));
    assert!(source.contains("(\"ReadConcertAdmission\", &[&[1]])"));
    assert!(POLICY.allows(METHOD, Workload::Gateway));
    assert!(!POLICY.allows(METHOD, Workload::Post));
}
