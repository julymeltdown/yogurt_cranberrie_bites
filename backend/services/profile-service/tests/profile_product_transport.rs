//! WP-02 T-101: profile product transport against a real PostgreSQL cluster.
//!
//! These are native acceptance cases, not doubles: the transport is served over
//! loopback gRPC with the real method policy, the real owner fence, the real
//! durable scope ledger and a real producer that reads the real schema. The
//! database is the runner-owned private cluster described by
//! `backend/scripts/mcr/migration_smoke.py`; the connection URL is supplied
//! through `PROFILE_TEST_DATABASE_URL` (and the owned-root variables the concert
//! native harness uses) so no test ever attaches to a foreign server.
//!
//! What each case proves is named in its doc comment. The three that the task
//! document calls out explicitly are:
//!
//! * a nil `profile_id` is rejected;
//! * a target containing `/events/` is rejected;
//! * a delivery token is issued and settlement is required before the response
//!   is complete.
use chrono::Utc;
use platform::{
    call_deadline::CallDeadlineLayer,
    grpc_auth::{CallerRegistry, Workload},
};
use profile_service::profile_product::{self, proto};
use tokio::{net::TcpListener, sync::oneshot, task::JoinHandle};
use tokio_stream::wrappers::TcpListenerStream;
use tonic::{
    transport::{Channel, Server},
    Code, Request,
};
use uuid::Uuid;

const TOKEN: &str = "wp02-profile-product-gateway";

// ---------------------------------------------------------------------------
// Native database fixture
// ---------------------------------------------------------------------------

async fn pool() -> anyhow::Result<sqlx::PgPool> {
    let url = std::env::var("PROFILE_TEST_DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("PROFILE_TEST_DATABASE_URL is required"))?;
    let options: sqlx::postgres::PgConnectOptions = url.parse()?;
    anyhow::ensure!(
        options.get_username() == "mcr_owner",
        "profile product fixture must use the owned cluster"
    );
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await?;
    // The fixture database is created empty by the harness and migrated here so
    // the test owns the exact schema it asserts against.
    profile_service_migrations(&pool).await?;
    Ok(pool)
}

async fn profile_service_migrations(pool: &sqlx::PgPool) -> anyhow::Result<()> {
    // Every test in this binary shares one fixture database. The migrations are
    // idempotent per schema, so a second application is a no-op; the advisory
    // lock serializes concurrent test startup.
    let mut tx = pool.begin().await?;
    sqlx::query("select pg_advisory_xact_lock(hashtextextended('wp02-profile-migrations',0))")
        .execute(&mut *tx)
        .await?;
    let applied: bool =
        sqlx::query_scalar("select to_regclass('public.profile_product_scopes') is not null")
            .fetch_one(&mut *tx)
            .await?;
    if applied {
        tx.commit().await?;
        return Ok(());
    }
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let mut entries: Vec<_> = std::fs::read_dir(&directory)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().is_some_and(|extension| extension == "sql")
                && !path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.ends_with(".down.sql"))
        })
        .collect();
    entries.sort();
    for path in entries {
        let sql = std::fs::read_to_string(&path)?;
        // Migrations are applied as one script each, exactly as the service
        // migrator does, so a DO block or a multi-statement body is one unit.
        sqlx::raw_sql(&sql).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Count the durable scope rows written for one actor.
///
/// Every test in this binary shares one fixture database and runs concurrently,
/// so a global count would observe other tests' scopes. Scoping by actor makes
/// the "no scope was opened" assertion about this test's own request only.
async fn scopes_for(pool: &sqlx::PgPool, actor: Uuid) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar(
        "select count(*) from profile_product_scopes where actor_id=$1",
    )
    .bind(actor)
    .fetch_one(pool)
    .await?)
}

/// Insert one account profile and return its canonical profile id.
async fn seed_profile(pool: &sqlx::PgPool, account: Uuid) -> anyhow::Result<Uuid> {
    let profile = Uuid::new_v4();
    sqlx::query(
        "insert into profile_identities(profile_id,account_id,nickname,bio,visibility) \
         values($1,$2,$3,$4,'PUBLIC')",
    )
    .bind(profile)
    .bind(account)
    .bind("native-profile")
    .bind(Some("native bio".to_owned()))
    .execute(pool)
    .await?;
    Ok(profile)
}

// ---------------------------------------------------------------------------
// Loopback harness
// ---------------------------------------------------------------------------

struct Harness {
    channel: Channel,
    /// The live scope registry. A test that asserts "the scope is still held"
    /// must keep the transport's registry alive, exactly as the running service
    /// does; the gRPC server alone does not own it.
    registry: std::sync::Arc<profile_service::adapters::out::profile_owner_guard::ProfileScopeRegistry>,
    stop: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<Result<(), tonic::transport::Error>>>,
}

impl Harness {
    async fn start(pool: sqlx::PgPool) -> anyhow::Result<Self> {
        let runtime = profile_product::runtime(
            pool.clone(),
            CallerRegistry::new().register(Workload::Gateway, TOKEN)?,
            platform::actor_delegation::ActorDelegationVerifier::local_parity(Workload::Profile),
        )?;
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let registry = runtime.registry.clone();
        let (stop, stopped) = oneshot::channel();
        let task = tokio::spawn(
            Server::builder()
                .layer(CallDeadlineLayer)
                .add_service(profile_product::retain_scopes(runtime.dispatch))
                .add_service(runtime.settlement)
                .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async {
                    let _ = stopped.await;
                }),
        );
        let channel = Channel::from_shared(format!("http://{address}"))?
            .connect()
            .await?;
        Ok(Self {
            channel,
            registry,
            stop: Some(stop),
            task: Some(task),
        })
    }

    /// The dispatch client presents the Gateway workload identity and signs the
    /// actor delegation exactly as the real Gateway client does.
    fn dispatch_client(
        &self,
    ) -> proto::profile_product_service_client::ProfileProductServiceClient<
        tonic::service::interceptor::InterceptedService<
            platform::actor_delegation::ActorDelegationClient<Channel>,
            platform::grpc_auth::ServiceIdentityClientInterceptor,
        >,
    > {
        proto::profile_product_service_client::ProfileProductServiceClient::with_interceptor(
            platform::actor_delegation::ActorDelegationClient::new(
                self.channel.clone(),
                platform::actor_delegation::ActorDelegationSigner::local_parity(Workload::Gateway),
                Workload::Profile,
            ),
            platform::grpc_auth::ServiceIdentityClientInterceptor::new(Workload::Gateway, TOKEN)
                .expect("static gateway identity"),
        )
        .max_decoding_message_size(profile_product::MAX_BODY + 16 * 1024)
    }

    fn delivery_client(
        &self,
    ) -> proto::profile_product_delivery_service_client::ProfileProductDeliveryServiceClient<
        tonic::service::interceptor::InterceptedService<
            Channel,
            platform::grpc_auth::ServiceIdentityClientInterceptor,
        >,
    > {
        proto::profile_product_delivery_service_client::ProfileProductDeliveryServiceClient::with_interceptor(
            self.channel.clone(),
            platform::grpc_auth::ServiceIdentityClientInterceptor::new(Workload::Gateway, TOKEN)
                .expect("static gateway identity"),
        )
    }

    async fn close(mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(task) = self.task.take() {
            let _ = tokio::time::timeout(std::time::Duration::from_secs(3), task).await;
        }
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

/// One actor's signed request, assembled the way the Gateway assembles it.
struct Actor {
    user: Uuid,
    profile: Uuid,
    family: Uuid,
    request: Uuid,
}

fn actor(profile: Uuid) -> Actor {
    Actor {
        user: Uuid::new_v4(),
        profile,
        family: Uuid::new_v4(),
        request: Uuid::new_v4(),
    }
}

impl Actor {
    fn message(&self, method: &str, target: &str) -> proto::ProfileRequest {
        let now = Utc::now().timestamp_micros();
        proto::ProfileRequest {
            user_id: self.user.to_string(),
            profile_id: self.profile.to_string(),
            session_family_id: self.family.to_string(),
            request_id: self.request.to_string(),
            http_method: method.into(),
            http_target: target.into(),
            http_body: Vec::new(),
            content_type: String::new(),
            idempotency_key: String::new(),
            authority_expires_unix_micros: now + 1_500_000,
            auth_observed_at_unix_micros: now,
            profile_observed_at_unix_micros: now,
            access_expires_unix_micros: now + 1_500_000,
            trusted_network_bucket: Vec::new(),
        }
    }
}

#[tokio::test]
async fn nil_profile_id_is_rejected_before_any_owner_work() -> anyhow::Result<()> {
    let pool = pool().await?;
    let harness = Harness::start(pool.clone()).await?;
    let mut actor = actor(Uuid::new_v4());
    actor.profile = Uuid::nil();
    let before = scopes_for(&pool, actor.user).await?;
    let request = actor.message("GET", "/api/me/profile");
    // The signed delegation names the actor, so the guard passes and the
    // handler's own profile check is what refuses this request.
    let mut signed = platform::actor_delegation::request_for(request, &actor.user.to_string())?;
    signed.set_timeout(std::time::Duration::from_secs(2));
    let status = harness
        .dispatch_client()
        .dispatch(signed)
        .await
        .expect_err("a nil profile id must be rejected");
    // A nil profile identity is an argument error, not an authentication one.
    assert_eq!(status.code(), Code::InvalidArgument);
    // Nothing was registered: the refusal happens before acquisition.
    assert_eq!(
        scopes_for(&pool, actor.user).await?,
        before,
        "a rejected request must not open a scope"
    );
    harness.close().await;
    Ok(())
}

#[tokio::test]
async fn event_path_target_is_rejected_by_the_parser_and_the_binding() -> anyhow::Result<()> {
    // The parser is the first guard: a profile operation never carries an event
    // path, so it is refused before route classification.
    for target in [
        "/api/concert/events/00000000-0000-4000-8000-000000000001/feed",
        "/api/me/profile/events/1",
        "/api/profiles/x/events/",
    ] {
        let status = profile_product::targets::parse("GET", target)
            .expect_err("an event path must be rejected");
        assert_eq!(
            status.code(),
            Code::InvalidArgument,
            "target {target} must be refused by the profile parser"
        );
        assert!(
            profile_product::rejects_event_path(target),
            "target {target} must be recognised as an event path"
        );
    }
    // And a well-formed profile target is not mistaken for an event path.
    for target in ["/api/me/profile", "/api/profiles/00000000-0000-4000-8000-000000000001"] {
        assert!(!profile_product::rejects_event_path(target));
    }
    // The loopback path proves the same rejection survives the real guard.
    let pool = pool().await?;
    let harness = Harness::start(pool.clone()).await?;
    let actor = actor(Uuid::new_v4());
    let event = Uuid::new_v4();
    let before = scopes_for(&pool, actor.user).await?;
    let request = actor.message(
        "GET",
        &format!("/api/concert/events/{event}/feed"),
    );
    let mut signed = platform::actor_delegation::request_for(request, &actor.user.to_string())?;
    signed.set_timeout(std::time::Duration::from_secs(2));
    let status = harness
        .dispatch_client()
        .dispatch(signed)
        .await
        .expect_err("an event path must be rejected");
    assert_eq!(status.code(), Code::InvalidArgument);
    assert!(
        status.message().contains("event path"),
        "the refusal must name the event path: {}",
        status.message()
    );
    assert_eq!(
        scopes_for(&pool, actor.user).await?,
        before,
        "an event target must never open a scope"
    );
    harness.close().await;
    Ok(())
}

#[tokio::test]
async fn delivery_token_is_issued_and_settlement_is_required_before_completion(
) -> anyhow::Result<()> {
    let pool = pool().await?;
    let harness = Harness::start(pool.clone()).await?;
    let account = Uuid::new_v4();
    let profile = seed_profile(&pool, account).await?;
    let mut actor = actor(profile);
    actor.user = account;
    let request = actor.message("GET", "/api/me/profile");
    let mut signed = platform::actor_delegation::request_for(request, &actor.user.to_string())?;
    signed.set_timeout(std::time::Duration::from_secs(2));
    let response = harness.dispatch_client().dispatch(signed).await?.into_inner();
    assert_eq!(response.status, 200);
    assert_eq!(response.content_type, "application/json");
    let body: serde_json::Value = serde_json::from_slice(&response.body)?;
    assert_eq!(body["profile_id"], profile.to_string());
    // The transport held the owner exclusion through the producer and released
    // it only at the gRPC body terminal. A scope that was never settled is
    // therefore recorded as unresolved in the durable ledger; see
    // `an_unsettled_scope_stays_unresolved`.
    // The delivery capability is present and complete.
    let delivery = response
        .delivery
        .as_ref()
        .expect("a delivery capability must be issued");
    assert_eq!(delivery.completion_secret.len(), 32);
    assert!(!delivery.binding_digest.iter().all(|byte| *byte == 0));
    assert!(response.content_expires_unix_micros > 0);
    // The durable ledger recorded the scope as published but NOT finished.
    let (phase, published): (String, bool) = sqlx::query_as(
        "select phase, delivery_secret_hash is not null from profile_product_scopes \
         where scope_id=$1",
    )
    .bind(Uuid::parse_str(&delivery.scope_id)?)
    .fetch_one(&pool)
    .await?;
    // Published, and therefore sealed against a second publication, but not
    // finished: only the Gateway's body terminal may finish it.
    assert_eq!(phase, "SEALED");
    assert!(published);
    // A forged secret never settles the scope, and never completes it.
    let mut forged = delivery.clone();
    forged.completion_secret = vec![7; 32];
    let status = harness
        .delivery_client()
        .complete_response(forged)
        .await
        .expect_err("a forged capability must not settle");
    assert_eq!(status.code(), Code::PermissionDenied);
    let phase: String =
        sqlx::query_scalar("select phase from profile_product_scopes where scope_id=$1")
            .bind(Uuid::parse_str(&delivery.scope_id)?)
            .fetch_one(&pool)
            .await?;
    assert_ne!(phase, "ROOT_FINISHED", "a forged capability must not complete");
    harness.close().await;
    Ok(())
}

#[tokio::test]
async fn an_unsettled_scope_stays_unresolved() -> anyhow::Result<()> {
    let pool = pool().await?;
    let harness = Harness::start(pool.clone()).await?;
    let account = Uuid::new_v4();
    let profile = seed_profile(&pool, account).await?;
    let mut actor = actor(profile);
    actor.user = account;
    let request = actor.message("GET", "/api/me/profile");
    let mut signed = platform::actor_delegation::request_for(request, &actor.user.to_string())?;
    signed.set_timeout(std::time::Duration::from_secs(2));
    let response = harness.dispatch_client().dispatch(signed).await?.into_inner();
    let delivery = response.delivery.expect("delivery capability");
    // The Gateway never reported the HTTP body terminal.
    let unresolved: i64 = sqlx::query_scalar(
        "select count(*) from profile_product_scopes where scope_id=$1 and phase<>'ROOT_FINISHED'",
    )
    .bind(Uuid::parse_str(&delivery.scope_id)?)
    .fetch_one(&pool)
    .await?;
    assert_eq!(unresolved, 1, "an unsettled scope must stay unresolved");
    harness.close().await;
    Ok(())
}

#[tokio::test]
async fn a_private_profile_is_unavailable_to_another_actor() -> anyhow::Result<()> {
    let pool = pool().await?;
    let harness = Harness::start(pool.clone()).await?;
    let owner_account = Uuid::new_v4();
    let profile = seed_profile(&pool, owner_account).await?;
    sqlx::query("update profile_identities set visibility='PRIVATE', revision=revision+1 where profile_id=$1")
        .bind(profile)
        .execute(&pool)
        .await?;
    let viewer_account = Uuid::new_v4();
    seed_profile(&pool, viewer_account).await?;
    let mut viewer = actor(profile);
    viewer.user = viewer_account;
    let request = viewer.message("GET", &format!("/api/profiles/{profile}"));
    let mut signed = platform::actor_delegation::request_for(request, &viewer.user.to_string())?;
    signed.set_timeout(std::time::Duration::from_secs(2));
    let response = harness.dispatch_client().dispatch(signed).await?.into_inner();
    // A refusal is a normal product response, not a transport error, and it is
    // indistinguishable from absence.
    assert_eq!(response.status, 404);
    let body: serde_json::Value = serde_json::from_slice(&response.body)?;
    assert_eq!(body["details"]["reason"], "RESOURCE_UNAVAILABLE");
    // The owner still sees their own private profile.
    let mut owner = actor(profile);
    owner.user = owner_account;
    let request = owner.message("GET", &format!("/api/profiles/{profile}"));
    let mut signed = platform::actor_delegation::request_for(request, &owner.user.to_string())?;
    signed.set_timeout(std::time::Duration::from_secs(2));
    let response = harness.dispatch_client().dispatch(signed).await?.into_inner();
    assert_eq!(response.status, 200);
    harness.close().await;
    Ok(())
}

/// The visitor profile carries the three fields the visitor card renders.
///
/// `shared_items` is the catalog overlap between the viewer and the owner,
/// computed from `profile_favorites` in this database — not from the concert
/// service's `discovery_favorite_items` projection, which is a display copy
/// written by a background worker.
#[tokio::test]
async fn a_visitor_profile_reports_the_shared_catalog_overlap() -> anyhow::Result<()> {
    let pool = pool().await?;
    let harness = Harness::start(pool.clone()).await?;
    let owner_account = Uuid::new_v4();
    let owner = seed_profile(&pool, owner_account).await?;
    let viewer_account = Uuid::new_v4();
    let viewer = seed_profile(&pool, viewer_account).await?;
    // Two catalog items the owner selected, one of which the viewer also did.
    let shared = Uuid::new_v4();
    let owner_only = Uuid::new_v4();
    // Labels are unique across the shared fixture database, so each run scopes
    // its own with the item id.
    for (id, label, category) in [
        (shared, format!("Visitor Overlap Shared {shared}"), "music"),
        (owner_only, format!("Visitor Overlap Owner Only {owner_only}"), "anime"),
    ] {
        // A music row must carry an explicit exclusion review (0016); the
        // non-music row must not claim one.
        let review: Option<&str> = (category == "music").then_some("reviewed: not K-pop");
        sqlx::query(
            "insert into catalog_items(catalog_item_id,category,label,exclusion_reason) \
             values($1,$2,$3,$4)",
        )
        .bind(id)
        .bind(category)
        .bind(&label)
        .bind(review)
        .execute(&pool)
        .await?;
    }
    for (profile, item, category, position) in [
        (owner, shared, "music", 0i16),
        (owner, owner_only, "anime", 0i16),
        (viewer, shared, "music", 0i16),
    ] {
        sqlx::query(
            "insert into profile_favorites(profile_id,category,catalog_item_id,position) \
             values($1,$2,$3,$4)",
        )
        .bind(profile)
        .bind(category)
        .bind(item)
        .bind(position)
        .execute(&pool)
        .await?;
    }
    let mut as_viewer = actor(viewer);
    as_viewer.user = viewer_account;
    let request = as_viewer.message("GET", &format!("/api/profiles/{owner}"));
    let mut signed = platform::actor_delegation::request_for(request, &as_viewer.user.to_string())?;
    signed.set_timeout(std::time::Duration::from_secs(2));
    let response = harness.dispatch_client().dispatch(signed).await?.into_inner();
    assert_eq!(response.status, 200);
    let body: serde_json::Value = serde_json::from_slice(&response.body)?;
    let items = body["shared_items"].as_array().expect("shared_items is an array");
    assert_eq!(items.len(), 1, "only the overlapping item is shared");
    assert_eq!(items[0]["catalog_item_id"], shared.to_string());
    assert_eq!(
        items[0]["label"],
        format!("Visitor Overlap Shared {shared}")
    );
    assert_eq!(items[0]["category"], "MUSIC");
    // No age is ever exposed on a visitor profile.
    assert!(body.get("age").is_none());
    assert!(body.get("age_at_verification").is_none());
    harness.close().await;
    Ok(())
}

/// A viewer with no profile of their own sees no overlap, not an error.
#[tokio::test]
async fn a_viewer_without_a_profile_has_no_shared_items() -> anyhow::Result<()> {
    let pool = pool().await?;
    let harness = Harness::start(pool.clone()).await?;
    let owner_account = Uuid::new_v4();
    let owner = seed_profile(&pool, owner_account).await?;
    let item = Uuid::new_v4();
    sqlx::query(
        "insert into catalog_items(catalog_item_id,category,label,exclusion_reason) \
         values($1,'music',$2,'reviewed: not K-pop')",
    )
    .bind(item)
    .bind(format!("Visitor Overlap Solo {item}"))
    .execute(&pool)
    .await?;
    sqlx::query(
        "insert into profile_favorites(profile_id,category,catalog_item_id,position) \
         values($1,'music',$2,0)",
    )
    .bind(owner)
    .bind(item)
    .execute(&pool)
    .await?;
    // An account with no profile row at all.
    let mut stranger = actor(owner);
    stranger.user = Uuid::new_v4();
    let request = stranger.message("GET", &format!("/api/profiles/{owner}"));
    let mut signed = platform::actor_delegation::request_for(request, &stranger.user.to_string())?;
    signed.set_timeout(std::time::Duration::from_secs(2));
    let response = harness.dispatch_client().dispatch(signed).await?.into_inner();
    assert_eq!(response.status, 200);
    let body: serde_json::Value = serde_json::from_slice(&response.body)?;
    assert_eq!(body["shared_items"].as_array().map(Vec::len), Some(0));
    harness.close().await;
    Ok(())
}

/// `gender_bucket` is absent unless the owner opted in, and `room_available`
/// never claims a room this service cannot verify.
#[tokio::test]
async fn an_unverified_owner_shows_no_gender_bucket() -> anyhow::Result<()> {
    let pool = pool().await?;
    let harness = Harness::start(pool.clone()).await?;
    let owner_account = Uuid::new_v4();
    let owner = seed_profile(&pool, owner_account).await?;
    let viewer_account = Uuid::new_v4();
    seed_profile(&pool, viewer_account).await?;
    let mut as_viewer = actor(owner);
    as_viewer.user = viewer_account;
    let request = as_viewer.message("GET", &format!("/api/profiles/{owner}"));
    let mut signed = platform::actor_delegation::request_for(request, &as_viewer.user.to_string())?;
    signed.set_timeout(std::time::Duration::from_secs(2));
    let response = harness.dispatch_client().dispatch(signed).await?.into_inner();
    assert_eq!(response.status, 200);
    let body: serde_json::Value = serde_json::from_slice(&response.body)?;
    // No claim, and no opt-in: the value is null, never a guess.
    assert_eq!(body["gender_bucket"], serde_json::Value::Null);
    // `profile_rooms` lives in `mcr_concert`; this service cannot verify it, so
    // it reports the closed answer rather than asserting a room exists.
    assert_eq!(body["room_available"], serde_json::Value::Bool(false));
    harness.close().await;
    Ok(())
}

#[tokio::test]
async fn an_unauthenticated_caller_never_reaches_the_producer() -> anyhow::Result<()> {
    let pool = pool().await?;
    let harness = Harness::start(pool.clone()).await?;
    let actor = actor(Uuid::new_v4());
    let before = scopes_for(&pool, actor.user).await?;
    // No delegation header at all: the method policy refuses before the handler.
    let mut request = Request::new(actor.message("GET", "/api/me/profile"));
    request.set_timeout(std::time::Duration::from_secs(2));
    let status = harness
        .dispatch_client()
        .dispatch(request)
        .await
        .expect_err("an unauthenticated call must be refused");
    assert_eq!(status.code(), Code::Unauthenticated);
    assert_eq!(scopes_for(&pool, actor.user).await?, before);
    harness.close().await;
    Ok(())
}

#[tokio::test]
async fn the_catalog_producer_answers_search_catalog() -> anyhow::Result<()> {
    let pool = pool().await?;
    let harness = Harness::start(pool.clone()).await?;
    // Two music rows and one anime row, so a category filter is observable.
    // Every music row must carry an explicit review decision, and a music row
    // can never be marked excluded: the K-pop exclusion is a catalog policy
    // enforced by the schema, not a flag a writer sets.
    for (category, label) in [
        ("music", "My Chemical Romance"),
        ("music", "NANA"),
        ("anime", "NANA"),
    ] {
        // The fixture database is shared across runs, so the seed is idempotent.
        sqlx::query(
            "insert into catalog_items(category,label,excluded,exclusion_reason) \
             values($1,$2,false,$3) on conflict do nothing",
        )
        .bind(category)
        .bind(label)
        .bind((category == "music").then_some("reviewed: not K-pop"))
        .execute(&pool)
        .await?;
    }
    let account = Uuid::new_v4();
    let profile = seed_profile(&pool, account).await?;
    let mut actor = actor(profile);
    actor.user = account;
    let request = actor.message("GET", "/api/catalog/music/search?q=NANA");
    let mut signed = platform::actor_delegation::request_for(request, &actor.user.to_string())?;
    signed.set_timeout(std::time::Duration::from_secs(2));
    let response = harness.dispatch_client().dispatch(signed).await?.into_inner();
    assert_eq!(response.status, 200, "catalog search must be served");
    let body: serde_json::Value = serde_json::from_slice(&response.body)?;
    assert_eq!(body["category"], "MUSIC");
    let items = body["items"].as_array().expect("items");
    assert_eq!(items.len(), 1, "only the music NANA row matches: {body}");
    assert_eq!(items[0]["label"], "NANA");
    assert_eq!(items[0]["category"], "MUSIC");
    // The anime row of the same label is a different category and is excluded.
    let request = actor.message("GET", "/api/catalog/anime/search?q=NANA");
    let mut signed = platform::actor_delegation::request_for(request, &actor.user.to_string())?;
    signed.set_timeout(std::time::Duration::from_secs(2));
    let response = harness.dispatch_client().dispatch(signed).await?.into_inner();
    let body: serde_json::Value = serde_json::from_slice(&response.body)?;
    assert_eq!(body["category"], "ANIME");
    assert_eq!(body["items"].as_array().expect("items").len(), 1);
    // An unknown category is refused, not defaulted. The refusal is a closed
    // product response (the producer's error projection), not a transport error.
    let request = actor.message("GET", "/api/catalog/kpop/search?q=NANA");
    let mut signed = platform::actor_delegation::request_for(request, &actor.user.to_string())?;
    signed.set_timeout(std::time::Duration::from_secs(2));
    let response = harness.dispatch_client().dispatch(signed).await?.into_inner();
    assert_eq!(response.status, 404, "an unknown category must be unavailable");
    let body: serde_json::Value = serde_json::from_slice(&response.body)?;
    assert_eq!(body["details"]["reason"], "RESOURCE_UNAVAILABLE");
    // An unknown query field is refused rather than ignored. Like every other
    // producer refusal, it is a closed product response with the frozen status.
    let request = actor.message("GET", "/api/catalog/music/search?score=1");
    let mut signed = platform::actor_delegation::request_for(request, &actor.user.to_string())?;
    signed.set_timeout(std::time::Duration::from_secs(2));
    let response = harness.dispatch_client().dispatch(signed).await?.into_inner();
    assert_eq!(response.status, 400, "an unknown query field must be refused");
    let body: serde_json::Value = serde_json::from_slice(&response.body)?;
    assert_eq!(body["details"]["reason"], "INVALID_INPUT");
    harness.close().await;
    Ok(())
}

#[tokio::test]
async fn the_transport_serves_only_the_closed_profile_route_set() -> anyhow::Result<()> {
    // Routes outside the closed set are refused, never forwarded.
    for (method, target) in [
        ("GET", "/api/concert/events/x/feed"),
        ("POST", "/api/me/profile"),
        ("GET", "/api/unknown"),
        ("GET", "/api/profiles/not-a-uuid"),
    ] {
        assert!(
            profile_product::targets::parse(method, target).is_err(),
            "{method} {target} must not be served"
        );
    }
    Ok(())
}
