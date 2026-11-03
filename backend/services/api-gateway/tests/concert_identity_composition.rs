//! Signed generated-client composition with controlled Auth and Profile owners.
//! This proves Gateway sequencing and borrowed validity, not HTTP or native owners.
use api_gateway::adapters::out::{
    auth_client::{
        concert_admission::{AdmissionReadError, BindingProvider, BindingReference, BindingState},
        proto as auth_proto, GrpcAuthClient,
    },
    concert_identity::{ConcertIdentityReadError, ConcertIdentityReader},
    jwt::{concert_access::ConcertAccessContext, JwtVerifier},
    profile_client::{
        concert_identity::ConcertProfileReadError, proto as profile_proto, GrpcProfileClient,
    },
};
use api_gateway::config::GatewayMetrics;
use base64ct::{Base64UrlUnpadded, Encoding};
use chrono::Utc;
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use platform::{
    actor_delegation::{
        read_authority::VerifiedReadAuthority, ActorDelegationSigner, ActorDelegationVerifier,
        VerifiedActor,
    },
    call_deadline::CallDeadlineLayer,
    grpc_auth::{CallerRegistry, VerifiedCaller, Workload},
    grpc_method_policy::{guard, MethodPolicy},
};
use rand::rngs::OsRng;
use rsa::{pkcs8::EncodePrivateKey, traits::PublicKeyParts, RsaPrivateKey, RsaPublicKey};
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::Duration,
};
use tokio::{
    sync::oneshot,
    task::JoinHandle,
};
use tonic::{transport::Server, Code, Request, Response, Status};

const USER: &str = "7e023b36-5ca9-4a40-850d-d01dc16874a8";
const FAMILY: &str = "fa60b0de-7277-447c-aa36-eaf530dd3018";
const OTHER: &str = "bdc6ac96-c145-4278-a3ef-72de355a6562";
const PRIVATE: &str = "PRIVATE_COMPOSITION_CANARY";
static AUTH_POLICY: MethodPolicy = MethodPolicy {
    service: "auth.v1.AuthService",
    workload: Some(Workload::Auth),
    methods: &[("ReadConcertAdmission", &[Workload::Gateway])],
    actor_fields: &[("ReadConcertAdmission", &[&[1]])],
};
static PROFILE_POLICY: MethodPolicy = MethodPolicy {
    service: "profile.v1.ProfileService",
    workload: Some(Workload::Profile),
    methods: &[("ReadConcertSelfProfile", &[Workload::Gateway])],
    actor_fields: &[("ReadConcertSelfProfile", &[&[1]])],
};

#[derive(Clone)]
enum Mode<T> {
    Reply(T),
    Hold,
    Error(Code),
}
struct AuthOwner {
    mode: Mutex<Mode<auth_proto::ReadConcertAdmissionResponse>>,
    calls: AtomicUsize,
    active: AtomicUsize,
    seen: Mutex<Vec<auth_proto::ReadConcertAdmissionRequest>>,
    remaining: Mutex<Vec<String>>,
}
struct ProfileOwner {
    mode: Mutex<Mode<profile_proto::ReadConcertSelfProfileResponse>>,
    calls: AtomicUsize,
    seen: Mutex<Vec<profile_proto::ReadConcertSelfProfileRequest>>,
    remaining: Mutex<Vec<String>>,
}
#[derive(Clone)]
struct FakeAuth(Arc<AuthOwner>);
#[derive(Clone)]
struct FakeProfile(Arc<ProfileOwner>);
fn admission() -> auth_proto::ReadConcertAdmissionResponse {
    auth_proto::ReadConcertAdmissionResponse {
        observed_at_unix_micros: Utc::now().timestamp_micros(),
        account_active: true,
        session_live: true,
        binding_state: 1,
    }
}
fn profile(user: &str, name: &str) -> profile_proto::ReadConcertSelfProfileResponse {
    profile_proto::ReadConcertSelfProfileResponse {
        profile: Some(profile_proto::ConcertSelfProfile {
            user_id: user.into(),
            display_name: name.into(),
            bio: None,
            observed_at_unix_micros: Utc::now().timestamp_micros(),
        }),
    }
}
#[tonic::async_trait]
impl auth_proto::auth_service_server::AuthService for FakeAuth {
    async fn read_concert_admission(
        &self,
        request: Request<auth_proto::ReadConcertAdmissionRequest>,
    ) -> Result<Response<auth_proto::ReadConcertAdmissionResponse>, Status> {
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
        self.0.calls.fetch_add(1, Ordering::SeqCst);
        self.0.active.fetch_add(1, Ordering::SeqCst);
        let remaining = request
            .metadata()
            .get("x-call-deadline-remaining")
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        self.0.remaining.lock().unwrap().push(remaining);
        self.0.seen.lock().unwrap().push(request.into_inner());
        let mode = self.0.mode.lock().unwrap().clone();
        match mode {
            Mode::Reply(value) => Ok(Response::new(value)),
            Mode::Hold => std::future::pending().await,
            Mode::Error(code) => Err(Status::new(code, PRIVATE)),
        }
    }
    async fn resend_verification(
        &self,
        _: Request<auth_proto::ResendVerificationRequest>,
    ) -> Result<Response<auth_proto::ResendVerificationResponse>, Status> {
        unsupported()
    }
    async fn signup_email(
        &self,
        _: Request<auth_proto::SignupEmailRequest>,
    ) -> Result<Response<auth_proto::SignupResponse>, Status> {
        unsupported()
    }
    async fn verify_email(
        &self,
        _: Request<auth_proto::VerifyEmailRequest>,
    ) -> Result<Response<auth_proto::SignupResponse>, Status> {
        unsupported()
    }
    async fn check_email(
        &self,
        _: Request<auth_proto::CheckEmailRequest>,
    ) -> Result<Response<auth_proto::EmailAvailabilityResponse>, Status> {
        unsupported()
    }
    async fn login_email(
        &self,
        _: Request<auth_proto::LoginEmailRequest>,
    ) -> Result<Response<auth_proto::AuthResponse>, Status> {
        unsupported()
    }
    async fn refresh_tokens(
        &self,
        _: Request<auth_proto::RefreshTokensRequest>,
    ) -> Result<Response<auth_proto::AuthResponse>, Status> {
        unsupported()
    }
    async fn logout(
        &self,
        _: Request<auth_proto::LogoutRequest>,
    ) -> Result<Response<auth_proto::PasswordResetResponse>, Status> {
        unsupported()
    }
    async fn request_password_reset(
        &self,
        _: Request<auth_proto::RequestPasswordResetRequest>,
    ) -> Result<Response<auth_proto::PasswordResetResponse>, Status> {
        unsupported()
    }
    async fn confirm_password_reset(
        &self,
        _: Request<auth_proto::ConfirmPasswordResetRequest>,
    ) -> Result<Response<auth_proto::PasswordResetResponse>, Status> {
        unsupported()
    }
    async fn authorize_o_auth(
        &self,
        _: Request<auth_proto::AuthorizeOAuthRequest>,
    ) -> Result<Response<auth_proto::OAuthAuthorizeResponse>, Status> {
        unsupported()
    }
    async fn o_auth_callback(
        &self,
        _: Request<auth_proto::OAuthCallbackRequest>,
    ) -> Result<Response<auth_proto::AuthResponse>, Status> {
        unsupported()
    }
    async fn link_o_auth_provider(
        &self,
        _: Request<auth_proto::LinkOAuthProviderRequest>,
    ) -> Result<Response<auth_proto::LinkedProvidersResponse>, Status> {
        unsupported()
    }
    async fn delete_user(
        &self,
        _: Request<auth_proto::DeleteUserRequest>,
    ) -> Result<Response<auth_proto::DeleteUserResponse>, Status> {
        unsupported()
    }
    async fn get_user_deletion(
        &self,
        _: Request<auth_proto::GetUserDeletionRequest>,
    ) -> Result<Response<auth_proto::GetUserDeletionResponse>, Status> {
        unsupported()
    }
    async fn get_user_deletion_by_saga_id(
        &self,
        _: Request<auth_proto::GetUserDeletionBySagaIdRequest>,
    ) -> Result<Response<auth_proto::GetUserDeletionBySagaIdResponse>, Status> {
        unsupported()
    }
    async fn list_dead_lettered_user_deletions(
        &self,
        _: Request<auth_proto::ListDeadLetteredUserDeletionsRequest>,
    ) -> Result<Response<auth_proto::ListDeadLetteredUserDeletionsResponse>, Status> {
        unsupported()
    }
    async fn retry_user_deletion(
        &self,
        _: Request<auth_proto::RetryUserDeletionRequest>,
    ) -> Result<Response<auth_proto::RetryUserDeletionResponse>, Status> {
        unsupported()
    }
    async fn get_signing_keys(
        &self,
        _: Request<auth_proto::GetSigningKeysRequest>,
    ) -> Result<Response<auth_proto::GetSigningKeysResponse>, Status> {
        unsupported()
    }
    async fn check_session(
        &self,
        _: Request<auth_proto::CheckSessionRequest>,
    ) -> Result<Response<auth_proto::CheckSessionResponse>, Status> {
        unsupported()
    }
}

#[tonic::async_trait]
impl profile_proto::profile_service_server::ProfileService for FakeProfile {
    async fn read_concert_self_profile(
        &self,
        request: Request<profile_proto::ReadConcertSelfProfileRequest>,
    ) -> Result<Response<profile_proto::ReadConcertSelfProfileResponse>, Status> {
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
        self.0.calls.fetch_add(1, Ordering::SeqCst);
        let remaining = request
            .metadata()
            .get("x-call-deadline-remaining")
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        self.0.remaining.lock().unwrap().push(remaining);
        self.0.seen.lock().unwrap().push(request.into_inner());
        let mode = self.0.mode.lock().unwrap().clone();
        match mode {
            Mode::Reply(value) => Ok(Response::new(value)),
            Mode::Hold => std::future::pending().await,
            Mode::Error(code) => Err(Status::new(code, PRIVATE)),
        }
    }
    async fn replay_mutation(
        &self,
        _: Request<profile_proto::ReplayMutationRequest>,
    ) -> Result<Response<profile_proto::ReplayMutationResponse>, Status> {
        unsupported()
    }
    async fn get_mutation_receipt_status(
        &self,
        _: Request<profile_proto::GetMutationReceiptStatusRequest>,
    ) -> Result<Response<profile_proto::GetMutationReceiptStatusResponse>, Status> {
        unsupported()
    }
    async fn get_profile(
        &self,
        _: Request<profile_proto::GetProfileRequest>,
    ) -> Result<Response<profile_proto::GetProfileResponse>, Status> {
        unsupported()
    }
    async fn get_profile_by_username(
        &self,
        _: Request<profile_proto::GetProfileByUsernameRequest>,
    ) -> Result<Response<profile_proto::GetProfileResponse>, Status> {
        unsupported()
    }
    async fn get_profiles(
        &self,
        _: Request<profile_proto::GetProfilesRequest>,
    ) -> Result<Response<profile_proto::GetProfilesResponse>, Status> {
        unsupported()
    }
    async fn update_profile(
        &self,
        _: Request<profile_proto::UpdateProfileRequest>,
    ) -> Result<Response<profile_proto::UpdateProfileResponse>, Status> {
        unsupported()
    }
    async fn update_profile_privacy(
        &self,
        _: Request<profile_proto::UpdateProfilePrivacyRequest>,
    ) -> Result<Response<profile_proto::UpdateProfilePrivacyResponse>, Status> {
        unsupported()
    }
    async fn get_profile_privacy(
        &self,
        _: Request<profile_proto::GetProfilePrivacyRequest>,
    ) -> Result<Response<profile_proto::GetProfilePrivacyResponse>, Status> {
        unsupported()
    }
    async fn delete_profile(
        &self,
        _: Request<profile_proto::DeleteProfileRequest>,
    ) -> Result<Response<profile_proto::DeleteProfileResponse>, Status> {
        unsupported()
    }
    async fn search_profiles(
        &self,
        _: Request<profile_proto::SearchProfilesRequest>,
    ) -> Result<Response<profile_proto::SearchProfilesResponse>, Status> {
        unsupported()
    }
    async fn upload_avatar(
        &self,
        _: Request<profile_proto::UploadAvatarRequest>,
    ) -> Result<Response<profile_proto::UploadAvatarResponse>, Status> {
        unsupported()
    }
    async fn get_avatar(
        &self,
        _: Request<profile_proto::GetAvatarRequest>,
    ) -> Result<Response<profile_proto::GetAvatarResponse>, Status> {
        unsupported()
    }
    async fn apply_user_deletion(
        &self,
        _: Request<profile_proto::ApplyUserDeletionRequest>,
    ) -> Result<Response<profile_proto::ApplyUserDeletionResponse>, Status> {
        unsupported()
    }
}
fn unsupported<T>() -> Result<Response<T>, Status> {
    Err(Status::unimplemented("outside composition fixture"))
}

struct Running {
    auth: Arc<AuthOwner>,
    profile: Arc<ProfileOwner>,
    reader: ConcertIdentityReader,
    stops: Mutex<Vec<oneshot::Sender<()>>>,
    tasks: Mutex<Vec<JoinHandle<()>>>,
}

impl Running {
    async fn start() -> Self {
        let auth = Arc::new(AuthOwner {
            mode: Mutex::new(Mode::Reply(admission())),
            calls: AtomicUsize::new(0),
            active: AtomicUsize::new(0),
            seen: Mutex::new(Vec::new()),
            remaining: Mutex::new(Vec::new()),
        });
        let profile = Arc::new(ProfileOwner {
            mode: Mutex::new(Mode::Reply(profile(USER, "Mira"))),
            calls: AtomicUsize::new(0),
            seen: Mutex::new(Vec::new()),
            remaining: Mutex::new(Vec::new()),
        });
        let auth_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let profile_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let auth_port = auth_listener.local_addr().unwrap().port();
        let profile_port = profile_listener.local_addr().unwrap().port();
        let auth_service = guard(
            &AUTH_POLICY,
            auth_proto::auth_service_server::AuthServiceServer::new(FakeAuth(auth.clone())),
            public_registry(),
            ActorDelegationVerifier::local_parity(Workload::Auth),
        )
        .unwrap();
        let profile_service = guard(
            &PROFILE_POLICY,
            profile_proto::profile_service_server::ProfileServiceServer::new(FakeProfile(
                profile.clone(),
            )),
            public_registry(),
            ActorDelegationVerifier::local_parity(Workload::Profile),
        )
        .unwrap();
        let (auth_stop, auth_stopped) = oneshot::channel();
        let (profile_stop, profile_stopped) = oneshot::channel();
        let auth_task = tokio::spawn(async move {
            Server::builder()
                .layer(CallDeadlineLayer)
                .add_service(auth_service)
                .serve_with_incoming_shutdown(
                    tokio_stream::wrappers::TcpListenerStream::new(auth_listener),
                    async {
                        let _ = auth_stopped.await;
                    },
                )
                .await
                .unwrap()
        });
        let profile_task = tokio::spawn(async move {
            Server::builder()
                .layer(CallDeadlineLayer)
                .add_service(profile_service)
                .serve_with_incoming_shutdown(
                    tokio_stream::wrappers::TcpListenerStream::new(profile_listener),
                    async {
                        let _ = profile_stopped.await;
                    },
                )
                .await
                .unwrap()
        });
        // A private transparent mapping listener gives each configured client
        // its real endpoint and preserves every expected signed header/body.
        let (map_auth, map_auth_stop) = Self::map_port("auth.v1.AuthService", auth_port).await;
        let (map_profile, map_profile_stop) =
            Self::map_port("profile.v1.ProfileService", profile_port).await;
        let auth_client = GrpcAuthClient::connect_with_metrics(
            format!("http://127.0.0.1:{map_auth}"),
            Workload::Gateway,
            "composition-token",
            None,
            ActorDelegationSigner::local_parity(Workload::Gateway),
            GatewayMetrics::default(),
        )
        .await
        .unwrap();
        let profile_client = GrpcProfileClient::connect_with_metrics(
            format!("http://127.0.0.1:{map_profile}"),
            Workload::Gateway,
            "composition-token",
            None,
            ActorDelegationSigner::local_parity(Workload::Gateway),
            GatewayMetrics::default(),
        )
        .await
        .unwrap();
        Self {
            auth,
            profile,
            reader: ConcertIdentityReader::new(auth_client, profile_client),
            stops: Mutex::new(vec![
                auth_stop,
                profile_stop,
                map_auth_stop.0,
                map_profile_stop.0,
            ]),
            tasks: Mutex::new(vec![
                auth_task,
                profile_task,
                map_auth_stop.1,
                map_profile_stop.1,
            ]),
        }
    }

    async fn map_port(
        prefix: &'static str,
        port: u16,
    ) -> (u16, (oneshot::Sender<()>, JoinHandle<()>)) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mapped = listener.local_addr().unwrap().port();
        let (stop, mut stopped) = oneshot::channel();
        let task = tokio::spawn(async move {
            loop {
                let accepted =
                    tokio::select! { biased; _=&mut stopped => break, x=listener.accept()=>x };
                let (socket, _) = accepted.unwrap();
                tokio::spawn(forward(socket, port, prefix));
            }
        });
        (mapped, (stop, task))
    }

    async fn close(self) {
        let mut stops = self.stops.into_inner().unwrap();
        for stop in stops.drain(..) {
            let _ = stop.send(());
        }
        let mut tasks = self.tasks.into_inner().unwrap();
        for task in tasks.drain(..) {
            tokio::time::timeout(Duration::from_secs(2), task)
                .await
                .unwrap()
                .unwrap();
        }
        assert!(self
            .auth
            .seen
            .lock()
            .unwrap()
            .iter()
            .all(|x| !format!("{x:?}").contains(PRIVATE)));
    }
}

fn public_registry() -> CallerRegistry {
    CallerRegistry::new()
        .register(Workload::Gateway, "composition-token")
        .unwrap()
}

async fn forward(mut downstream: tokio::net::TcpStream, port: u16, _prefix: &'static str) {
    let mut upstream = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .unwrap();
    let (mut dr, mut dw) = downstream.split();
    let (mut ur, mut uw) = upstream.split();
    let a = async { tokio::io::copy(&mut dr, &mut uw).await.map(|_| ()) };
    let b = async { tokio::io::copy(&mut ur, &mut dw).await.map(|_| ()) };
    let _ = tokio::join!(a, b);
}

fn key() -> &'static Key {
    static KEY: OnceLock<Key> = OnceLock::new();
    KEY.get_or_init(|| Key::generate("composition"))
}
struct Key {
    encoding: EncodingKey,
    public: RsaPublicKey,
}
impl Key {
    fn generate(_kid: &'static str) -> Self {
        let private = RsaPrivateKey::new(&mut OsRng, 2048).unwrap();
        let pem = private.to_pkcs8_pem(Default::default()).unwrap();
        Self {
            encoding: EncodingKey::from_rsa_pem(pem.as_bytes()).unwrap(),
            public: RsaPublicKey::from(&private),
        }
    }
    fn jwk(&self) -> Value {
        json!({"kid":"composition","kty":"RSA","alg":"RS256","use":"sig","n":Base64UrlUnpadded::encode_string(&self.public.n().to_bytes_be()),"e":Base64UrlUnpadded::encode_string(&self.public.e().to_bytes_be())})
    }
    fn token(&self, user: &str, exp: i64) -> String {
        jsonwebtoken::encode(&Header { kid:Some("composition".into()), ..Header::new(Algorithm::RS256) },
            &json!({"sub":user,"sid":FAMILY,"iss":"composition-auth","aud":"composition-api","token_type":"access","exp":exp,"roles":[PRIVATE]}),&self.encoding).unwrap()
    }
}
fn context(after: Duration) -> ConcertAccessContext {
    let verifier = JwtVerifier::from_jwks_json(
        "composition-auth",
        "composition-api",
        &json!({"keys":[key().jwk()]}).to_string(),
    )
    .unwrap();
    let token = key().token(USER, Utc::now().timestamp() + 900);
    verifier
        .verify_concert_access(
            &token,
            Some((tokio::time::Instant::now() + after).into_std()),
        )
        .unwrap()
}
fn binding() -> BindingReference {
    BindingReference::new(OTHER, BindingProvider::Google, 17).unwrap()
}

fn deadline_nanos(value: &str) -> u128 {
    let (digits, unit) = value.split_at(value.len() - 1);
    let amount: u128 = digits.parse().unwrap();
    amount
        * match unit {
            "n" => 1,
            "u" => 1_000,
            "m" => 1_000_000,
            "S" => 1_000_000_000,
            _ => panic!("unexpected grpc timeout unit"),
        }
}

#[tokio::test]
async fn email_only_composes_one_active_session_and_fresh_current_self_profile() {
    let h = Running::start().await;
    let ctx = context(Duration::from_secs(4));
    let i = h.reader.read(&ctx, None).await.unwrap();
    assert_eq!(i.user_id().unwrap().to_string(), USER);
    assert_eq!(i.display_name().unwrap(), "Mira");
    assert_eq!(i.bio().unwrap(), None);
    assert_eq!(i.binding_state().unwrap(), BindingState::NotRequested);
    assert_eq!(format!("{i:?}"), "ConcertIdentityObservation([REDACTED])");
    *h.profile.mode.lock().unwrap() = Mode::Reply(profile(USER, "Updated"));
    // A second call must sign fresh Auth and Profile reads, in that order.
    let j = h.reader.read(&ctx, None).await.unwrap();
    assert_eq!(j.display_name().unwrap(), "Updated");
    assert_eq!(h.auth.calls.load(Ordering::SeqCst), 2);
    assert_eq!(h.profile.calls.load(Ordering::SeqCst), 2);
    assert_eq!(h.auth.seen.lock().unwrap()[0].binding, None);
    let auth_remaining = deadline_nanos(&h.auth.remaining.lock().unwrap()[0]);
    let profile_remaining = deadline_nanos(&h.profile.remaining.lock().unwrap()[0]);
    // The owner observes the same captured deadline, after Auth consumes time.
    assert!(profile_remaining > 0);
    // Profile starts after Auth returns, so its remaining time cannot exceed
    // Auth's. Equal is possible when both calls fit within one microsecond.
    assert!(profile_remaining <= auth_remaining);
    assert!(!format!("{:?}", h.auth.seen.lock().unwrap()).contains(PRIVATE));
    assert!(!format!("{:?}", h.profile.seen.lock().unwrap()).contains(PRIVATE));
    h.close().await;
}

#[tokio::test]
async fn exact_requested_bound_result_is_required_but_x_is_not_a_login_prerequisite() {
    let h = Running::start().await;
    let ctx = context(Duration::from_secs(4));
    let mut result = admission();
    result.binding_state = 2;
    *h.auth.mode.lock().unwrap() = Mode::Reply(result);
    let i = h.reader.read(&ctx, Some(binding())).await.unwrap();
    assert_eq!(i.binding_state().unwrap(), BindingState::Bound);
    let b = h.auth.seen.lock().unwrap()[0].binding.clone();
    assert_eq!(b.as_ref().unwrap().provider, "GOOGLE");
    assert_eq!(b.as_ref().unwrap().identity_id, OTHER);
    assert_eq!(b.as_ref().unwrap().binding_epoch, 17);
    let mut result = admission();
    result.binding_state = 3;
    *h.auth.mode.lock().unwrap() = Mode::Reply(result);
    assert_eq!(
        h.reader.read(&ctx, Some(binding())).await.unwrap_err(),
        ConcertIdentityReadError::NotEligible
    );
    assert_eq!(
        h.profile.calls.load(Ordering::SeqCst),
        1,
        "unbound Auth must not dispatch Profile"
    );
    h.close().await;
}

#[tokio::test]
async fn inactive_ended_or_failed_auth_never_dispatches_profile() {
    let h = Running::start().await;
    let ctx = context(Duration::from_secs(4));
    for (account_active, session_live, binding_state) in
        [(false, true, 1), (true, false, 1), (false, false, 1)]
    {
        let mut result = admission();
        result.account_active = account_active;
        result.session_live = session_live;
        result.binding_state = binding_state;
        *h.auth.mode.lock().unwrap() = Mode::Reply(result);
        assert_eq!(
            h.reader.read(&ctx, None).await.unwrap_err(),
            ConcertIdentityReadError::NotEligible
        );
    }
    *h.auth.mode.lock().unwrap() = Mode::Error(Code::Unavailable);
    assert_eq!(
        h.reader.read(&ctx, None).await.unwrap_err(),
        ConcertIdentityReadError::Auth(AdmissionReadError::Unavailable)
    );
    assert_eq!(h.auth.calls.load(Ordering::SeqCst), 4);
    assert_eq!(h.profile.calls.load(Ordering::SeqCst), 0);
    assert!(h
        .auth
        .seen
        .lock()
        .unwrap()
        .iter()
        .all(|v| !format!("{v:?}").contains(PRIVATE)));
    h.close().await;
}

#[tokio::test]
async fn profile_mismatch_outage_and_missing_content_are_fixed_and_auth_is_rechecked() {
    let h = Running::start().await;
    let ctx = context(Duration::from_secs(4));
    *h.profile.mode.lock().unwrap() = Mode::Reply(profile(OTHER, "Foreign"));
    assert!(matches!(
        h.reader.read(&ctx, None).await,
        Err(ConcertIdentityReadError::Profile(
            ConcertProfileReadError::InvalidResponse
        ))
    ));
    *h.profile.mode.lock().unwrap() = Mode::Error(Code::Unavailable);
    assert_eq!(
        h.reader.read(&ctx, None).await.unwrap_err(),
        ConcertIdentityReadError::Profile(ConcertProfileReadError::Unavailable)
    );
    *h.profile.mode.lock().unwrap() = Mode::Error(Code::NotFound);
    assert_eq!(
        h.reader.read(&ctx, None).await.unwrap_err(),
        ConcertIdentityReadError::Profile(ConcertProfileReadError::NotFound)
    );
    assert_eq!(h.auth.calls.load(Ordering::SeqCst), 3);
    assert_eq!(h.profile.calls.load(Ordering::SeqCst), 3);
    let class = ConcertIdentityReadError::Profile(ConcertProfileReadError::NotFound);
    assert!(format!("{class:?}").contains("Profile(NotFound)"));
    assert!(!format!("{class:?} {class}").contains("PRIVATE_COMPOSITION_CANARY"));
    h.close().await;
}

#[tokio::test]
async fn both_observations_are_borrowed_checked_and_cannot_survive_context_expiry() {
    let h = Running::start().await;
    let ctx = context(Duration::from_millis(180));
    let i = h.reader.read(&ctx, None).await.unwrap();
    assert!(i.check().is_ok());
    tokio::time::sleep(Duration::from_millis(200)).await;
    for e in [
        i.user_id().unwrap_err(),
        i.display_name().unwrap_err(),
        i.bio().unwrap_err(),
        i.binding_state().unwrap_err(),
        i.admission_observed_at().unwrap_err(),
        i.profile_observed_at().unwrap_err(),
    ] {
        assert_eq!(e, ConcertIdentityReadError::AccessExpired);
    }
    h.close().await;
}

#[tokio::test]
async fn auth_hold_consumes_one_original_stop_and_delivers_no_profile_result() {
    let h = Running::start().await;
    let ctx = context(Duration::from_millis(120));
    *h.auth.mode.lock().unwrap() = Mode::Hold;
    let started = tokio::time::Instant::now();
    assert!(matches!(
        h.reader.read(&ctx, None).await,
        Err(ConcertIdentityReadError::AccessExpired | ConcertIdentityReadError::DeadlineExceeded)
    ));
    assert!(started.elapsed() < Duration::from_millis(700));
    assert_eq!(h.auth.calls.load(Ordering::SeqCst), 1);
    assert_eq!(h.profile.calls.load(Ordering::SeqCst), 0);
    h.close().await;
}

#[tokio::test]
async fn profile_hold_cannot_outlive_same_original_context_after_auth_pass() {
    let h = Running::start().await;
    let ctx = context(Duration::from_millis(150));
    *h.profile.mode.lock().unwrap() = Mode::Hold;
    let started = tokio::time::Instant::now();
    assert!(matches!(
        h.reader.read(&ctx, None).await,
        Err(ConcertIdentityReadError::AccessExpired | ConcertIdentityReadError::DeadlineExceeded)
    ));
    assert!(started.elapsed() < Duration::from_millis(700));
    assert_eq!(h.auth.calls.load(Ordering::SeqCst), 1);
    assert_eq!(h.profile.calls.load(Ordering::SeqCst), 1);
    h.close().await;
}
