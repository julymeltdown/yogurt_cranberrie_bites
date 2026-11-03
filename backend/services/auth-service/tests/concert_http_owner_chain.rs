//! Real HTTP identity -> configured Gateway clients -> guarded Auth/Profile
//! services -> three isolated PostgreSQL databases. Auth and Profile own their
//! schemas; Gateway uses its own test-only migration database.
//!
//! This is a test-only route and private observation. It exposes no public
//! endpoint, does not claim a Concert event decision and cannot issue a
//! portable participation grant.

use api_gateway::{
    adapters::{
        out::concert_identity::ConcertIdentityReadError,
        r#in::http::{self, concert_access::HttpConcertAccess},
    },
    config::{AppConfig as GatewayConfig, AppState as GatewayState},
};
use auth_service::{
    adapters::{
        out::security::admitted_argon2::hash_password,
        r#in::grpc::{
            method_policy::AUTH_METHOD_POLICY,
            server::{proto as auth_proto, AuthGrpcServer},
        },
    },
    config::{AppConfig as AuthConfig, AppState as AuthState},
};
use axum::{
    body::{to_bytes, Body},
    extract::FromRef,
    extract::State,
    http::{header, Request, StatusCode},
    middleware::{self, Next},
    routing::get,
    Router,
};
use platform::{
    actor_delegation::ActorDelegationVerifier,
    call_deadline::CallDeadlineLayer,
    grpc_auth::{CallerRegistry, Workload},
};
use profile_service::{
    adapters::r#in::grpc::{
        method_policy::PROFILE_METHOD_POLICY,
        server::{proto as profile_proto, ProfileGrpcServer},
    },
    config::{AppConfig as ProfileConfig, AppState as ProfileState},
};
use sqlx::PgPool;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::{net::TcpListener, sync::oneshot, task::JoinHandle};
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;
use tower::ServiceExt;
use uuid::Uuid;

const CANARY_EMAIL: &str = "owner-chain-private@example.invalid";
const CANARY_PASSWORD: &str = "OwnerChain-H3a-Private-Password!";
const CANARY_NICKNAME: &str = "Private owner nickname";
const CANARY_BIO: &str = "private-owner-bio-canary";
const CANARY_USERNAME: &str = "private-owner-username";
const CANARY_AVATAR: &str = "https://private-owner-avatar.invalid/secret";
const CANARY_DELETE_REASON: &str = "private-owner-delete-reason-canary";

#[derive(Clone)]
struct RouteState {
    app: Arc<GatewayState>,
}

impl FromRef<RouteState> for Arc<GatewayState> {
    fn from_ref(state: &RouteState) -> Self {
        state.app.clone()
    }
}

async fn owner_probe(State(route): State<RouteState>, access: HttpConcertAccess) -> StatusCode {
    let Ok(context) = access.borrowed() else {
        return StatusCode::UNAUTHORIZED;
    };
    let Some(reader) = route.app.concert_identity.as_ref() else {
        return StatusCode::SERVICE_UNAVAILABLE;
    };
    let result = reader.read(context, None).await;
    if let Err(error) = &result {
        eprintln!("owner read diagnostic: {error:?}");
    }
    identity_status(result)
}

fn identity_status(
    result: Result<
        api_gateway::adapters::out::concert_identity::ConcertIdentityObservation<'_>,
        ConcertIdentityReadError,
    >,
) -> StatusCode {
    match result {
        Ok(identity) => {
            let Ok(user) = identity.user_id() else {
                return StatusCode::GATEWAY_TIMEOUT;
            };
            match (user, identity.display_name()) {
                (_, Ok(name)) if name.len() <= 40 => StatusCode::OK,
                _ => StatusCode::SERVICE_UNAVAILABLE,
            }
        }
        Err(ConcertIdentityReadError::NotEligible) => StatusCode::NOT_FOUND,
        Err(ConcertIdentityReadError::DeadlineExceeded) => StatusCode::GATEWAY_TIMEOUT,
        Err(ConcertIdentityReadError::AccessExpired) => StatusCode::UNAUTHORIZED,
        Err(ConcertIdentityReadError::Profile(
            api_gateway::adapters::out::profile_client::concert_identity::ConcertProfileReadError::NotFound,
        )) => StatusCode::SERVICE_UNAVAILABLE,
        Err(_) => StatusCode::SERVICE_UNAVAILABLE,
    }
}

fn owner_router(route: RouteState) -> Router {
    Router::new()
        .route("/private-owner-probe", get(owner_probe))
        .with_state(route.clone())
        .layer(middleware::from_fn_with_state(
            route.app.clone(),
            http::timeout_middleware,
        ))
        .layer(middleware::from_fn(normalize_rejection))
}

async fn normalize_rejection(request: Request<Body>, next: Next) -> axum::response::Response {
    next.run(request).await
}

struct Running<T> {
    endpoint: String,
    client: T,
    stop: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<Result<(), tonic::transport::Error>>>,
}

#[derive(Clone)]
struct DelayedProfileService<S>(S, Arc<AtomicUsize>, Duration);

impl<S> tonic::server::NamedService for DelayedProfileService<S>
where
    S: tonic::server::NamedService,
{
    const NAME: &'static str = S::NAME;
}

impl<S> tower::Service<::http::Request<tonic::body::Body>> for DelayedProfileService<S>
where
    S: tower::Service<
            ::http::Request<tonic::body::Body>,
            Response = ::http::Response<tonic::body::Body>,
        > + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
    S::Error: Send + 'static,
{
    type Response = ::http::Response<tonic::body::Body>;
    type Error = S::Error;
    type Future =
        std::pin::Pin<Box<dyn std::future::Future<Output = Result<S::Response, S::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        context: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        tower::Service::poll_ready(&mut self.0, context)
    }

    fn call(&mut self, request: ::http::Request<tonic::body::Body>) -> Self::Future {
        let owner_read =
            request.uri().path() == "/profile.v1.ProfileService/ReadConcertSelfProfile";
        if owner_read {
            self.1.fetch_add(1, Ordering::SeqCst);
        }
        let delay = self.2;
        let mut inner = self.0.clone();
        Box::pin(async move {
            if owner_read {
                tokio::time::sleep(delay).await;
            }
            tower::Service::call(&mut inner, request).await
        })
    }
}

impl<T> Drop for Running<T> {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

struct AuthService {
    running: Running<auth_proto::auth_service_client::AuthServiceClient<tonic::transport::Channel>>,
    pool: PgPool,
    service_token: String,
}

impl AuthService {
    async fn start(config: &AuthConfig, pool: PgPool, state: AuthState) -> anyhow::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = format!("http://{}", listener.local_addr()?);
        let (stop, stopped) = oneshot::channel();
        let guarded = platform::grpc_method_policy::guard(
            &AUTH_METHOD_POLICY,
            auth_proto::auth_service_server::AuthServiceServer::new(AuthGrpcServer::new(Arc::new(
                state,
            ))),
            config.caller_registry.clone(),
            ActorDelegationVerifier::local_parity(Workload::Auth),
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
            auth_proto::auth_service_client::AuthServiceClient::connect(endpoint.clone()).await?;
        Ok(Self {
            running: Running {
                endpoint,
                client,
                stop: Some(stop),
                task: Some(task),
            },
            pool,
            service_token: config.outbound_identity.clone(),
        })
    }

    async fn login(&mut self, email: &str) -> anyhow::Result<auth_proto::AuthResponse> {
        let mut request = tonic::Request::new(auth_proto::LoginEmailRequest {
            email: email.into(),
            password: CANARY_PASSWORD.into(),
            ip_address: "127.0.0.1".into(),
            user_agent: "owner-chain-test".into(),
        });
        request
            .metadata_mut()
            .insert("x-service-caller", "gateway".parse()?);
        request
            .metadata_mut()
            .insert("x-service-token", self.service_token.parse()?);
        Ok(self.running.client.login_email(request).await?.into_inner())
    }

    async fn stop(mut self) -> anyhow::Result<()> {
        let _ = self.running.stop.take().unwrap().send(());
        tokio::time::timeout(Duration::from_secs(3), self.running.task.take().unwrap()).await???;
        Ok(())
    }
}

struct ProfileService {
    running: Running<
        profile_proto::profile_service_client::ProfileServiceClient<tonic::transport::Channel>,
    >,
    pool: PgPool,
}

impl ProfileService {
    async fn start(
        config: &ProfileConfig,
        pool: PgPool,
        state: ProfileState,
        calls: Arc<AtomicUsize>,
        delay: Duration,
    ) -> anyhow::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = format!("http://{}", listener.local_addr()?);
        let (stop, stopped) = oneshot::channel();
        let generated = profile_proto::profile_service_server::ProfileServiceServer::new(
            ProfileGrpcServer::new(Arc::new(state)),
        );
        let counted = DelayedProfileService(generated, calls, delay);
        let guarded = platform::grpc_method_policy::guard(
            &PROFILE_METHOD_POLICY,
            counted,
            config.caller_registry.clone(),
            ActorDelegationVerifier::local_parity(Workload::Profile),
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
            profile_proto::profile_service_client::ProfileServiceClient::connect(endpoint.clone())
                .await?;
        Ok(Self {
            running: Running {
                endpoint,
                client,
                stop: Some(stop),
                task: Some(task),
            },
            pool,
        })
    }

    async fn stop(mut self) -> anyhow::Result<()> {
        let _ = self.running.stop.take().unwrap().send(());
        tokio::time::timeout(Duration::from_secs(3), self.running.task.take().unwrap()).await???;
        Ok(())
    }
}

fn auth_config(url: String) -> anyhow::Result<AuthConfig> {
    let mut config = auth_service::config::AppConfig::load()?;
    config.database_url = url;
    config.runtime_mode = auth_service::config::RuntimeMode::Parity;
    config.issuer = "owner-chain-auth".into();
    config.access_audience = "owner-chain-api".into();
    config.public_application_routes_enabled = false;
    config.lifecycle_cleanup_worker_enabled = false;
    config.user_deletion_worker_enabled = false;
    config.jwt_private_key_pem =
        Some(std::env::var("MCR_HTTP_OWNER_JWT_PRIVATE_KEY_PEM").expect("explicit fixture key"));
    config.jwt_key_id = Some("owner-chain-fixture".into());
    Ok(config)
}

fn profile_config(url: String) -> ProfileConfig {
    let mut config = ProfileConfig::load().expect("profile configuration");
    config.database_url = url;
    config.retention_purge_worker_enabled = false;
    config.mutation_receipt_retention_worker_enabled = false;
    config
}

fn configure_gateway(
    auth_endpoint: String,
    profile_endpoint: String,
    database_url: String,
    jwks: &str,
) -> anyhow::Result<GatewayConfig> {
    let mut config = GatewayConfig::load()?;
    config.database_url = database_url;
    config.auth_grpc_endpoint = auth_endpoint;
    config.profile_grpc_endpoint = profile_endpoint;
    config.friend_grpc_endpoint = "http://127.0.0.1:1".into();
    config.notification_grpc_endpoint = "http://127.0.0.1:1".into();
    config.post_grpc_endpoint = "http://127.0.0.1:1".into();
    config.jwt_issuer = "owner-chain-auth".into();
    config.jwt_audience = "owner-chain-api".into();
    config.jwt_jwks_json = jwks.into();
    config.jwt_keys_refresh_seconds = 0;
    config.request_timeout_ms = 1_000;
    config.journal_client_timeout_ms = 900;
    config.validate()?;
    Ok(config)
}

async fn seed_owner(auth: &PgPool, profile: &PgPool) -> anyhow::Result<Uuid> {
    let user = Uuid::new_v4();
    let password = hash_password(CANARY_PASSWORD)?;
    sqlx::query(
        "insert into users(id,primary_email,password_hash,status) values($1,$2,$3,'ACTIVE')",
    )
    .bind(user)
    .bind(CANARY_EMAIL)
    .bind(&password)
    .execute(auth)
    .await?;
    sqlx::query(
        "insert into profiles(user_id,username,nickname,bio,avatar_url) values($1,$2,$3,$4,$5)",
    )
    .bind(user)
    .bind(CANARY_USERNAME)
    .bind(CANARY_NICKNAME)
    .bind(CANARY_BIO)
    .bind(CANARY_AVATAR)
    .execute(profile)
    .await?;
    Ok(user)
}

async fn request(router: &Router, token: &str) -> anyhow::Result<(StatusCode, String)> {
    let request = Request::builder()
        .uri("/private-owner-probe")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())?;
    let response = router.clone().oneshot(request).await?;
    let status = response.status();
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    Ok((status, String::from_utf8_lossy(&body).into_owned()))
}

fn assert_no_canary(body: &str) {
    for canary in [
        CANARY_EMAIL,
        CANARY_PASSWORD,
        CANARY_NICKNAME,
        CANARY_BIO,
        CANARY_USERNAME,
        CANARY_AVATAR,
        CANARY_DELETE_REASON,
        "$argon2",
    ] {
        assert!(!body.contains(canary), "private canary leaked in response");
    }
}

fn assert_profile_calls(counter: &AtomicUsize, expected: usize) {
    assert_eq!(counter.load(Ordering::SeqCst), expected);
}

#[tokio::test]
#[ignore = "requires three explicit isolated PostgreSQL URLs from the native controller"]
async fn native_http_real_owner_chain_success_negatives_and_safe_failures() -> anyhow::Result<()> {
    let auth_url = std::env::var("MCR_HTTP_OWNER_AUTH_URL")?;
    let profile_url = std::env::var("MCR_HTTP_OWNER_PROFILE_URL")?;
    let gateway_url = std::env::var("MCR_HTTP_OWNER_GATEWAY_URL")?;
    anyhow::ensure!(!auth_url.eq_ignore_ascii_case(&profile_url));
    anyhow::ensure!(!auth_url.eq_ignore_ascii_case(&gateway_url));
    anyhow::ensure!(!profile_url.eq_ignore_ascii_case(&gateway_url));

    let auth_config = auth_config(auth_url)?;
    let profile_config = profile_config(profile_url);
    let auth_pool = auth_service::config::connect_pool(&auth_config).await?;
    let profile_pool = profile_service::config::connect_pool(&profile_config).await?;
    let user = seed_owner(&auth_pool, &profile_pool).await?;
    let auth_state = auth_service::config::build_state(&auth_config, auth_pool.clone()).await?;
    let mut auth_service = AuthService::start(&auth_config, auth_pool.clone(), auth_state).await?;
    let profile_state =
        profile_service::config::build_state(&profile_config, profile_pool.clone()).await?;
    let profile_requests = Arc::new(AtomicUsize::new(0));
    let mut profile_service = ProfileService::start(
        &profile_config,
        profile_pool.clone(),
        profile_state,
        profile_requests.clone(),
        Duration::ZERO,
    )
    .await?;
    let first_profile_endpoint = profile_service.running.endpoint.clone();
    let _ = first_profile_endpoint;

    let session = auth_service.login(CANARY_EMAIL).await?;
    assert_eq!(session.user_id, user.to_string());
    let mut access = session.access_token;
    let _session_family: Uuid = sqlx::query_scalar(
        "select family_id from refresh_tokens where user_id=$1 and revoked_at is null order by expires_at desc limit 1",
    )
    .bind(user)
    .fetch_one(&auth_pool)
    .await?;
    let private_token_hash: String = sqlx::query_scalar(
        "select token_hash from refresh_tokens where user_id=$1 order by expires_at desc limit 1",
    )
    .bind(user)
    .fetch_one(&auth_pool)
    .await?;
    assert!(!private_token_hash.is_empty());
    assert!(!body_contains_private_token_hash(
        &access,
        &private_token_hash
    ));

    let jwks: String = auth_service
        .running
        .client
        .get_signing_keys({
            let mut request = tonic::Request::new(auth_proto::GetSigningKeysRequest {});
            request
                .metadata_mut()
                .insert("x-service-caller", "gateway".parse()?);
            request
                .metadata_mut()
                .insert("x-service-token", auth_service.service_token.parse()?);
            request
        })
        .await?
        .into_inner()
        .jwks_json;
    let gateway_config = configure_gateway(
        auth_service.running.endpoint.clone(),
        profile_service.running.endpoint.clone(),
        gateway_url,
        &jwks,
    )?;
    let gateway_pool = api_gateway::config::connect_pool(&gateway_config).await?;
    let mut app = Arc::new(api_gateway::config::build_state(&gateway_config, gateway_pool).await?);
    assert!(app.concert_identity.is_some());
    let mut router = owner_router(RouteState { app: app.clone() });

    let (status, body) = request(&router, &access).await?;
    assert_eq!(status, StatusCode::OK, "owner read failed: {body}");
    assert_no_canary(&body);
    assert!(!body.contains(&private_token_hash));
    assert_profile_calls(&profile_requests, 1);

    sqlx::query("update refresh_tokens set revoked_at=clock_timestamp() where user_id=$1")
        .bind(user)
        .execute(&auth_pool)
        .await?;
    let (status, body) = request(&router, &access).await?;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "revoked Auth session: {body}"
    );
    assert_no_canary(&body);
    assert_profile_calls(&profile_requests, 1);
    // Session revocation is not undone to prepare another case. A real login
    // issues the fresh family needed by the subsequent Profile checks.
    access = auth_service.login(CANARY_EMAIL).await?.access_token;

    sqlx::query(
        "update profiles set deleted_at=clock_timestamp(),delete_reason=$2 where user_id=$1",
    )
    .bind(user)
    .bind(CANARY_DELETE_REASON)
    .execute(&profile_pool)
    .await?;
    let (status, body) = request(&router, &access).await?;
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "deleted Profile: {body}"
    );
    assert_no_canary(&body);
    assert_profile_calls(&profile_requests, 2);

    sqlx::query("update profiles set deleted_at=null,delete_reason=null where user_id=$1")
        .bind(user)
        .execute(&profile_pool)
        .await?;
    sqlx::query("update users set status='PENDING_VERIFICATION' where id=$1")
        .bind(user)
        .execute(&auth_pool)
        .await?;
    let (status, body) = request(&router, &access).await?;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "inactive Auth account: {body}"
    );
    assert_no_canary(&body);
    assert_profile_calls(&profile_requests, 2);

    sqlx::query("update users set status='ACTIVE' where id=$1")
        .bind(user)
        .execute(&auth_pool)
        .await?;
    let renewed = auth_service.login(CANARY_EMAIL).await?;
    assert_eq!(renewed.user_id, user.to_string());
    access = renewed.access_token;
    let live_family: Uuid = sqlx::query_scalar(
        "select family_id from refresh_tokens where user_id=$1 and revoked_at is null order by expires_at desc limit 1",
    )
    .bind(user)
    .fetch_one(&auth_pool)
    .await?;
    assert!(!live_family.is_nil());
    let (status, body) = request(&router, &access).await?;
    assert_eq!(status, StatusCode::OK, "renewed owner read failed: {body}");
    assert_profile_calls(&profile_requests, 3);
    sqlx::query("delete from profiles where user_id=$1")
        .bind(user)
        .execute(&profile_pool)
        .await?;
    let (status, body) = request(&router, &access).await?;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_no_canary(&body);
    assert_profile_calls(&profile_requests, 4);

    sqlx::query(
        "insert into profiles(user_id,username,nickname,bio,avatar_url) values($1,$2,$3,$4,$5)",
    )
    .bind(user)
    .bind(CANARY_USERNAME)
    .bind(CANARY_NICKNAME)
    .bind(CANARY_BIO)
    .bind(CANARY_AVATAR)
    .execute(&profile_pool)
    .await?;

    profile_service.pool.close().await;
    profile_service.stop().await?;
    let (status, body) = request(&router, &access).await?;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_no_canary(&body);

    let profile_pool = profile_service::config::connect_pool(&profile_config).await?;
    let profile_state =
        profile_service::config::build_state(&profile_config, profile_pool.clone()).await?;
    profile_service = ProfileService::start(
        &profile_config,
        profile_pool.clone(),
        profile_state,
        profile_requests.clone(),
        Duration::from_millis(700),
    )
    .await?;
    assert_ne!(profile_service.running.endpoint, first_profile_endpoint);
    let restarted_gateway = configure_gateway(
        auth_service.running.endpoint.clone(),
        profile_service.running.endpoint.clone(),
        gateway_config.database_url.clone(),
        &jwks,
    )?;
    let mut limited_gateway = restarted_gateway.clone();
    limited_gateway.request_timeout_ms = 200;
    limited_gateway.journal_client_timeout_ms = 150;
    limited_gateway.validate()?;
    let short_gateway_pool = api_gateway::config::connect_pool(&limited_gateway).await?;
    app = Arc::new(api_gateway::config::build_state(&limited_gateway, short_gateway_pool).await?);
    router = owner_router(RouteState { app: app.clone() });
    let before_slow_profile = profile_requests.load(Ordering::SeqCst);
    let started = Instant::now();
    let (status, body) = request(&router, &access).await?;
    // Both guards share the original stop. Depending on poll ordering, the
    // checked access context rejects first (401), or the outer HTTP timeout
    // wins (504). Neither may return the delayed owner's successful content.
    assert!(
        matches!(
            status,
            StatusCode::UNAUTHORIZED | StatusCode::GATEWAY_TIMEOUT
        ),
        "original request stop must refuse late Profile success: {status} {body}"
    );
    assert_no_canary(&body);
    let elapsed = started.elapsed();
    assert!(elapsed >= Duration::from_millis(150));
    assert!(elapsed < Duration::from_millis(700));
    assert_eq!(
        profile_requests.load(Ordering::SeqCst),
        before_slow_profile + 1,
        "the timeout must follow an actual Profile dispatch"
    );

    auth_service.pool.close().await;
    auth_service.stop().await?;
    let before_outage = profile_requests.load(Ordering::SeqCst);
    let (status, body) = request(&router, &access).await?;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_no_canary(&body);
    assert_eq!(profile_requests.load(Ordering::SeqCst), before_outage);

    profile_service.pool.close().await;
    profile_service.stop().await?;
    let _ = (auth_config, profile_config);
    Ok(())
}

fn body_contains_private_token_hash(body: &str, token_hash: &str) -> bool {
    body.contains(token_hash)
}
