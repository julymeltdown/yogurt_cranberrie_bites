use concert_service::{
    bootstrap::{BootstrapFeatures, BootstrapService, ProductReleaseBundle},
    config, health, rpc, world_rpc,
};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use tonic::body::Body;
use tonic::codegen::http::{Request, Response};
use tower::{Layer, Service};
mod in01_catalog;

/// The only service this listener answers before World readiness. Auth and
/// Friend settle owner-authority scopes against it during their own startup,
/// so refusing it would keep both sides from ever reaching World readiness.
const EARLY_SERVICES: &[&str] = &["authority.lease.v1.ProducerSettlementService"];
const WORLD_NOT_READY: &str = "World not ready";

/// Deny-by-default gate over the whole served gRPC router. The server binds
/// immediately so Auth/Friend can settle, while every capability that reads or
/// writes World-owned state refuses with `UNAVAILABLE` until the supervised
/// World publishes its startup barrier. Allowing the settlement service by
/// exact gRPC service name keeps the exception one operation family wide.
#[derive(Clone)]
struct WorldReadinessGate {
    ready: Arc<AtomicBool>,
}

impl WorldReadinessGate {
    /// Without a configured World there is no readiness to wait for.
    fn new(world_configured: bool) -> Self {
        Self {
            ready: Arc::new(AtomicBool::new(!world_configured)),
        }
    }

    fn state(&self) -> Arc<AtomicBool> {
        self.ready.clone()
    }
}

#[derive(Clone)]
struct WorldReadinessGateService<S> {
    inner: S,
    ready: Arc<AtomicBool>,
}

impl<S> Layer<S> for WorldReadinessGate {
    type Service = WorldReadinessGateService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        WorldReadinessGateService {
            inner,
            ready: self.ready.clone(),
        }
    }
}

impl<S> Service<Request<Body>> for WorldReadinessGateService<S>
where
    S: Service<Request<Body>, Response = Response<Body>> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response<Body>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        let early = request
            .uri()
            .path()
            .strip_prefix('/')
            .and_then(|rest| rest.split_once('/'))
            .is_some_and(|(service, method)| {
                !method.is_empty() && EARLY_SERVICES.contains(&service)
            });
        if self.ready.load(Ordering::Acquire) || early {
            Box::pin(self.inner.call(request))
        } else {
            Box::pin(async { Ok(tonic::Status::unavailable(WORLD_NOT_READY).into_http::<Body>()) })
        }
    }
}

/// Account deletion can only be composed from the World's serving binding,
/// which the supervised World publishes during its own startup. The listener
/// mounts this slot immediately and refuses with `UNAVAILABLE` until that exact
/// binding arrives, so the early settlement path never waits on it.
#[derive(Clone)]
struct DeferredAccountDeletion {
    service: Arc<tokio::sync::OnceCell<config::account_deletion_runtime::AccountDeletionService>>,
}

impl tonic::server::NamedService for DeferredAccountDeletion {
    const NAME: &'static str = "concert.account_deletion.v1.AccountDeletionService";
}

impl Service<Request<Body>> for DeferredAccountDeletion {
    type Response = Response<Body>;
    type Error = std::convert::Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        match self.service.get() {
            Some(service) => {
                let mut service = service.clone();
                Box::pin(async move { service.call(request).await })
            }
            None => Box::pin(async {
                Ok(tonic::Status::unavailable("Account deletion is starting").into_http::<Body>())
            }),
        }
    }
}

/// Wait for the supervised World to publish its startup barrier. A dropped
/// sender is a startup failure: the process must exit instead of half-serving
/// with every capability gated forever.
async fn await_world_ready(
    ready: tokio::sync::oneshot::Receiver<()>,
    gate: Arc<AtomicBool>,
) -> anyhow::Result<()> {
    ready
        .await
        .map_err(|_| anyhow::anyhow!("World startup readiness failed"))?;
    gate.store(true, Ordering::Release);
    Ok(())
}
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    if arguments.len() == 1 && arguments[0] == "--prepare-in01-catalog" {
        return in01_catalog::run().await;
    }
    let inspect_release = arguments.len() == 1 && arguments[0] == "--inspect-product-release";
    let command_arguments = if inspect_release {
        Vec::new()
    } else {
        arguments
    };
    let command = migration_runtime::parse_command(command_arguments.iter().cloned())?;
    let mut migration_directory = std::path::PathBuf::new();
    if matches!(
        command,
        migration_runtime::Command::Migrate | migration_runtime::Command::Verify
    ) {
        config::database_options(std::env::var("CONCERT_DATABASE_URL").ok().as_deref())?;
        migration_directory = config::migration_directory()?;
    }
    if migration_runtime::handle_command(
        command_arguments.iter().cloned(),
        "CONCERT_DATABASE_URL",
        &migration_directory,
    )
    .await?
    {
        return Ok(());
    }
    platform::shutdown::install_signal_handlers()?;
    let config = config::AppConfig::load()?;
    // Validate all security and product inputs before opening the database or
    // listeners. Absence retains the explicit health-only operational mode.
    let runtime = config::bootstrap_runtime::BootstrapRuntime::load()?;
    // Account deletion serving is an explicit opt-in. It reuses the incumbent
    // guarded listener, its mTLS material and the canonical Auth endpoint and
    // credential; it never opens a second registry or listener.
    let deletion_settings = config::account_deletion_runtime::AccountDeletionSettings::load()?;
    anyhow::ensure!(
        deletion_settings == config::account_deletion_runtime::AccountDeletionSettings::Off
            || runtime.is_some(),
        "Managed account deletion requires the guarded Concert bootstrap listener"
    );
    let product_settings = config::product_runtime::ProductSettings::load(runtime.is_some())?;
    let mut world_settings = config::world_runtime::WorldRuntimeSettings::load()?;
    anyhow::ensure!(
        world_settings.is_none() || runtime.is_some(),
        "World requires the guarded Concert bootstrap listener"
    );
    let bootstrap = runtime
        .as_ref()
        .map(|runtime| {
            if let Some(world) = world_settings.as_ref() {
                return Ok(world.validated_release());
            }
            let files = &runtime.files;
            ProductReleaseBundle::from_product_files(
                &files[0],
                &files[1],
                &files[2],
                [&files[3], &files[4], &files[5]],
                BootstrapFeatures::default(),
            )
            .map_err(|_| anyhow::anyhow!("Concert product release is unavailable"))
        })
        .transpose()?;
    let participant_settings = if inspect_release {
        let source = bootstrap
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Product release inspection requires bootstrap"))?;
        config::participant_runtime::ParticipantRuntimeSettings::load_for_release_inspection(
            runtime.is_some(),
            source,
        )?
    } else {
        config::participant_runtime::ParticipantRuntimeSettings::load(runtime.is_some())?
    };
    let intro_settings = config::intro_runtime::IntroRuntimeSettings::load(
        runtime.is_some(),
        participant_settings.is_some(),
    )?;
    let party_settings = config::party_runtime::PartyRuntimeSettings::load(
        runtime.is_some(),
        intro_settings.is_some(),
    )?;
    let gift_settings = config::gift_runtime::GiftRuntimeSettings::load(
        runtime.is_some(),
        intro_settings.is_some(),
    )?;
    let gift_sanitizer = if gift_settings.is_some() {
        let workers = platform::cpu::WorkerPool::new(2)?;
        Some(Arc::new(
            concert_service::adapters::out::media::codec::BoundedImageSanitizer::new(
                workers.class(16)?,
            ),
        ))
    } else {
        None
    };
    let wall_b_settings = config::mural_tools_b::WallBSettings::load(intro_settings.is_some())?;
    let report_settings = config::report_runtime::ReportStartupSettings::load(
        runtime.is_some(),
        intro_settings.is_some(),
    )?;
    let venue_settings = config::venue_runtime::VenueRuntimeSettings::load(
        runtime.is_some(),
        intro_settings.is_some(),
    )?;
    let inbox_settings =
        config::inbox_runtime::InboxRuntimeSettings::load(intro_settings.is_some())?;
    let target_settings =
        config::notification_target_runtime::NotificationTargetRuntimeSettings::load(
            inbox_settings.enabled(),
            intro_settings.is_some(),
            runtime.as_ref().and_then(|runtime| runtime.tls.as_ref()),
        )?;
    let feed_settings = config::product_runtime::feed::FeedRuntimeSettings::load(
        product_settings.public_feed_read,
        intro_settings.is_some(),
        runtime.as_ref().and_then(|runtime| runtime.tls.as_ref()),
        runtime.as_ref().map(|runtime| &runtime.actors),
    )?;
    let owner_media_settings = config::owner_media_runtime::OwnerMediaSettings::load(
        runtime.is_some(),
        intro_settings.is_some(),
    )?;
    anyhow::ensure!(
        !product_settings.owner_media || owner_media_settings.is_some(),
        "Uploader media mode requires the shared canonical composition"
    );
    let social_settings = config::social_startup::SocialStartupSettings::load(
        intro_settings.is_some(),
        world_settings.is_some(),
        inbox_settings.enabled() && target_settings.is_some(),
        runtime.as_ref().and_then(|runtime| runtime.tls.as_ref()),
    )?;
    let closure_settings = config::closure_runtime::ClosureSettings::load(
        world_settings.is_some(),
        participant_settings.is_some(),
        runtime.is_some(),
    )?;
    let features = config::bootstrap_features::ConfiguredBootstrapFeatures::from_settings(
        runtime.as_ref(),
        world_settings.as_ref(),
        participant_settings.as_ref(),
        intro_settings.as_ref(),
        feed_settings.as_ref(),
        social_settings.as_ref(),
    )?;
    // Keep one computed Arc/digest for C03, Participant and the incumbent World.
    let bootstrap = match world_settings.take() {
        Some(world) => {
            let world = world.with_bootstrap_features(&features)?;
            let release = world.validated_release();
            world_settings = Some(world);
            Some(release)
        }
        None => bootstrap
            .map(|source| features.bind_release(source))
            .transpose()?,
    };
    if inspect_release {
        let release = bootstrap
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Product release inspection requires bootstrap"))?;
        println!("CONCERT_PRODUCT_RELEASE_DIGEST={}", release.source_digest());
        return Ok(());
    }
    // The standalone constructor still rejects unsupported Feed composition.
    // Only the validated shared settings above consume the explicit read mode.
    let mut product_owner =
        config::product_runtime::production_owners(config::product_runtime::ProductSettings {
            public_feed_read: false,
            owner_media: false,
        })?;
    anyhow::ensure!(
        product_owner.is_none() || participant_settings.is_none(),
        "Product and Participant require a shared canonical scope composition"
    );
    let world_owners = world_settings
        .as_ref()
        .map(|_| config::world_runtime::production_owners())
        .transpose()?;
    let pool = config::connect_pool(&config).await?;
    let (closure_handoff, closure_bridge) = config::closure_runtime::BridgeHandoff::channel();
    let mut archive_service = None;
    let mut child_recovery = None;
    let mut notification_target = None;
    let mut feed_callback = None;
    let mut social_delivery = None;
    // Populated only by the incumbent configured Social runtime. The single
    // deletion composer owns consuming this typed handle and mounting Apply.
    let mut deletion_sharing_publisher: Option<
        std::sync::Arc<dyn concert_service::social_live::invalidation::SocialPolicyPublisher>,
    > = None;
    let mut world_transport = None;
    let mut world_local = None;
    let participant_runtime = participant_settings
        .map(|settings| {
            let release = bootstrap
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Participant release is unavailable"))?
                .clone();
            match intro_settings {
                Some(intro) => {
                    let shared = settings.compose_shared(
                        pool.clone(),
                        release,
                        intro.participant_owners(),
                    )?;
                    child_recovery = Some(
                        shared
                            .owners
                            .core
                            .child_recovery_worker(intro.recovery_settings())?,
                    );
                    notification_target = target_settings
                        .map(|settings| settings.compose(pool.clone(), &shared.owners.core));
                    let intro_owner = intro.compose(pool.clone(), &shared.owners);
                    let owners = match party_settings {
                        Some(settings) => {
                            settings.compose(pool.clone(), &shared.owners, intro_owner)?
                        }
                        None => intro_owner,
                    };
                    let owners = match gift_settings {
                        Some(settings) => settings.compose(
                            pool.clone(),
                            &shared.owners,
                            gift_sanitizer.clone().ok_or_else(|| {
                                anyhow::anyhow!("Gift sanitizer composition is unavailable")
                            })?,
                            owners,
                        )?,
                        None => owners,
                    };
                    let owners = inbox_settings.compose(&shared.owners, owners);
                    let owners = match wall_b_settings {
                        Some(settings) => settings.compose(pool.clone(), &shared.owners, owners),
                        None => owners,
                    };
                    let owners = match feed_settings {
                        Some(settings) => {
                            let (owners, callback) =
                                settings.compose(pool.clone(), &shared.owners, owners)?;
                            feed_callback = Some(callback);
                            owners
                        }
                        None => owners,
                    };
                    let owners = match owner_media_settings {
                        Some(settings) => settings.compose(pool.clone(), &shared.owners, owners)?,
                        None => owners,
                    };
                    let owners = match social_settings {
                        Some(settings) => {
                            let policy = config::social_policy_runtime::SocialWorldPolicy::compose(
                                pool.clone(),
                            );
                            deletion_sharing_publisher = Some(policy.publisher.clone());
                            let (sender, receiver) = tokio::sync::oneshot::channel();
                            social_delivery = Some(
                                config::social_delivery::SocialDeliveryRuntime::new(
                                    pool.clone(),
                                    policy.sink,
                                    settings.notification,
                                )
                                .with_world(receiver),
                            );
                            world_transport = Some(policy.transport);
                            world_local = Some(sender);
                            settings
                                .config
                                .compose_shared(pool.clone(), &shared.owners, owners)?
                        }
                        None => owners,
                    };
                    let owners = match venue_settings {
                        Some(settings) => settings.compose(pool.clone(), &shared.owners, owners)?,
                        None => owners,
                    };
                    product_owner = Some(match report_settings {
                        Some(settings) => {
                            settings.compose_shared(pool.clone(), &shared.owners, owners)?
                        }
                        None => owners,
                    });
                    if let Some(settings) = closure_settings {
                        let (owners, archive) = settings.compose(
                            pool.clone(),
                            closure_handoff.clone(),
                            &shared.owners,
                            product_owner.take().ok_or_else(|| {
                                anyhow::anyhow!("Closure composition is unavailable")
                            })?,
                        )?;
                        product_owner = Some(owners);
                        archive_service = Some(archive);
                    }
                    Ok(shared.participant)
                }
                None => settings.compose(pool.clone(), release),
            }
        })
        .transpose()?;
    let read_service_policy = config::product_runtime::service_policy_read()?;
    if read_service_policy {
        let runtime = participant_runtime.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Service policy read requires the shared participant composition")
        })?;
        let producer: Arc<dyn concert_service::product_rpc::scoped::ScopedProductProducer> =
            Arc::new(concert_service::service_policy::ServicePolicyProducer::new(
                pool.clone(),
            ));
        let policy: Arc<dyn concert_service::product_rpc::ProductOwner> = Arc::new(
            concert_service::product_rpc::scoped::ScopedProductOwner::new(
                runtime.fence.clone(),
                runtime.delivery.clone(),
                producer,
            ),
        );
        // The read is independent of the other product modes: an Intro,
        // Social, Report or Closure surface already composed above is retained
        // as this layer's base instead of suppressing the policy read.
        let delivery = product_owner
            .as_ref()
            .map(|owners| owners.delivery.clone())
            .unwrap_or_else(|| runtime.delivery.clone());
        let product: Arc<dyn concert_service::product_rpc::ProductOwner> =
            concert_service::service_policy::compose(
                read_service_policy,
                product_owner.take().map(|owners| owners.product),
                policy,
            )
            .ok_or_else(|| anyhow::anyhow!("Service policy surface composition is unavailable"))?;
        product_owner = Some(config::product_runtime::ProductOwners { product, delivery });
    }
    let (participant_owners, participant_recovery) = match participant_runtime {
        Some(config::participant_runtime::ParticipantRuntime {
            participant,
            decision,
            decision_verifier,
            settlement,
            delivery,
            recovery,
            callers,
            gateway_verifier,
            ..
        }) => (
            Some((
                participant,
                decision,
                decision_verifier,
                settlement,
                delivery,
                callers,
                gateway_verifier,
            )),
            Some(recovery),
        ),
        None => (None, None),
    };
    let world_configured = world_settings.is_some();
    let (world_ready_tx, world_ready_rx) = if world_configured {
        let (tx, rx) = tokio::sync::oneshot::channel();
        (Some(tx), Some(rx))
    } else {
        (None, None)
    };
    // Built before the server so the listener starts closed and opens exactly
    // once, from the supervised World barrier.
    let world_gate = WorldReadinessGate::new(world_configured);
    let world_ready_gate = world_gate.state();
    // The deletion composer borrows the exact supervised World bridge/socket
    // registry. It is only requested when a World is actually served.
    let (deletion_world_tx, deletion_world_rx) = if deletion_settings
        != config::account_deletion_runtime::AccountDeletionSettings::Off
        && world_settings.is_some()
    {
        let (sender, receiver) =
            tokio::sync::oneshot::channel::<config::deletion_world_binding::ServingWorldBinding>();
        (Some(sender), Some(receiver))
    } else {
        (None, None)
    };
    let _application = config::build_application(pool.clone());
    let receipt_maintenance =
        concert_service::workers::receipts::ReceiptMaintenance::new(pool.clone());
    // The discovery projection worker is opt-in. When it is off, discovery keeps
    // whatever the projection already holds rather than starting a worker that
    // would fail every round against an unconfigured endpoint.
    let discovery_projection = config::discovery_projection::DiscoveryProjectionSettings::load()?;
    // A dedicated handle for the discovery worker, so the closure below does not
    // have to move the pool the serving listeners also use.
    let discovery_pool = pool.clone();
    let health = health::router(health::readiness(pool.clone()));
    let listener = tokio::net::TcpListener::bind(config.metrics_addr)
        .await
        .map_err(|_| anyhow::anyhow!("Concert health listener bind failed"))?;
    println!("CONCERT_HEALTH_LISTENER={}", listener.local_addr()?);
    let grpc_pool = pool.clone();
    let world_pool = pool.clone();
    let result = platform::shutdown::supervise(async move {
        let http = async move {
            axum::serve(listener, health)
                .with_graceful_shutdown(platform::shutdown::global().cancelled())
                .await?;
            Ok::<(), anyhow::Error>(())
        };
        let grpc = async move {
            match (runtime, bootstrap) {
                (Some(runtime), Some(release)) => {
                    // Compose on the actual serving pool, canonical Auth endpoint,
                    // listener mTLS and the exact supervised World objects. Missing
                    // optional owners stay unavailable for their own target.
                    let deletion = match deletion_settings {
                        config::account_deletion_runtime::AccountDeletionSettings::Off => None,
                        config::account_deletion_runtime::AccountDeletionSettings::Managed => {
                            let endpoint = std::env::var("CONCERT_AUTH_GRPC_ENDPOINT")
                                .map_err(|_| anyhow::anyhow!("CONCERT_AUTH_GRPC_ENDPOINT is required for managed account deletion"))?;
                            let credential = std::env::var("CONCERT_SERVICE_CREDENTIAL")
                                .map_err(|_| anyhow::anyhow!("CONCERT_SERVICE_CREDENTIAL is required for managed account deletion"))?;
                            let callers = config::account_deletion_runtime::AuthDeletionCallers::configured()?;
                            let tls = runtime.tls.clone();
                            let pool = grpc_pool.clone();
                            let sharing_publisher = deletion_sharing_publisher.take();
                            let cell = Arc::new(tokio::sync::OnceCell::new());
                            let managed = cell.clone();
                            tokio::spawn(async move {
                                let binding = match deletion_world_rx {
                                    Some(binding) => binding.await,
                                    None => return,
                                };
                                let Ok(binding) = binding else {
                                    if !platform::shutdown::global().is_requested() {
                                        eprintln!("CONCERT_COMPONENT_FAILURE=account deletion World binding failed");
                                        platform::shutdown::global().request();
                                    }
                                    return;
                                };
                                let composed = config::account_deletion_runtime::AccountDeletionRuntime::compose(
                                    config::account_deletion_runtime::AccountDeletionSettings::Managed,
                                    pool,
                                    &endpoint,
                                    tls.as_ref(),
                                    &credential,
                                    callers,
                                    config::account_deletion_runtime::LiveDeletionOwners {
                                        worlds: Some(vec![binding.as_serving_world()]),
                                        sharing_publisher,
                                    },
                                )
                                .and_then(|runtime| {
                                    runtime.ok_or_else(|| {
                                        anyhow::anyhow!("Managed account deletion composer is unavailable")
                                    })
                                })
                                .and_then(|runtime| runtime.service());
                                match composed {
                                    Ok(service) => {
                                        let _ = managed.set(service);
                                    }
                                    Err(error) => {
                                        eprintln!("CONCERT_COMPONENT_FAILURE=account deletion composition {error:?}");
                                        platform::shutdown::global().request();
                                    }
                                }
                            });
                            Some(DeferredAccountDeletion { service: cell })
                        }
                    };
                    let mut server = tonic::transport::Server::builder()
                        .layer(platform::actor_delegation::response_scope::RetainedScopeLayer)
                        .layer(platform::correlation::CorrelationLayer)
                        .layer(platform::call_deadline::CallDeadlineLayer)
                        .layer(platform::trace_context::TraceContextLayer)
                        // Outermost: a closed gate answers every non-settlement
                        // service without composing or touching any owner.
                        .layer(world_gate);
                    if let Some(tls) = runtime.tls {
                        server = server.tls_config(tls.server_config()?)?;
                    }
                    let bootstrap =
                        std::sync::Arc::new(BootstrapService::new(grpc_pool.clone(), release));
                    let world = world_rpc::WorldRpcService::new(
                        grpc_pool.clone(),
                        bootstrap.clone(),
                        world_rpc::PgWorldCanonicalAuthority::new(
                            bootstrap.clone(),
                            grpc_pool.clone(),
                        ),
                        std::sync::Arc::new(world_rpc::CsprngTicketEntropy),
                    );
                    let bootstrap = platform::grpc_method_policy::guard(
                        &rpc::POLICY,
                        rpc::proto::concert_service_server::ConcertServiceServer::new(
                            BootstrapService::new(grpc_pool.clone(), bootstrap.release().clone()),
                        ),
                        runtime.callers.clone(),
                        runtime.actors.clone(),
                    )?;
                    let world = platform::grpc_method_policy::guard(
                        &world_rpc::POLICY,
                        world_rpc::proto::world_service_server::WorldServiceServer::new(world),
                        runtime.callers.clone(),
                        runtime.actors.clone(),
                    )?;
                    let (participant, decision, settlement, participant_delivery) = participant_owners
                        .map(|(participant, decision, decision_verifier, settlement, delivery, callers, gateway_verifier)| {
                            let participant = platform::grpc_method_policy::guard(
                                &concert_service::participant::POLICY,
                                participant.server(),
                                callers.clone(), gateway_verifier.clone(),
                            )?;
                            let decision = platform::grpc_method_policy::guard(
                                &concert_service::participant::live::decision_rpc::DECISION_POLICY,
                                decision.server(),
                                callers.clone(), decision_verifier,
                            )?;
                            let settlement = platform::grpc_method_policy::guard(
                                &concert_service::product_rpc::settlement::POLICY,
                                settlement.server(),
                                callers.clone(), gateway_verifier.clone(),
                            )?;
                            let delivery = platform::grpc_method_policy::guard(
                                &concert_service::product_rpc::delivery::POLICY,
                                concert_service::product_rpc::proto::product_delivery_service_server::ProductDeliveryServiceServer::new(
                                    concert_service::product_rpc::delivery::DeliveryService::new(delivery),
                                ).max_decoding_message_size(4096).max_encoding_message_size(4096),
                                callers, gateway_verifier,
                            )?;
                            Ok::<_, anyhow::Error>((Some(participant), Some(decision), Some(settlement), Some(delivery)))
                        }).transpose()?.unwrap_or((None, None, None, None));
                    // Shared mode already constructed the exact canonical
                    // Participant delivery service. Mount it once for both paths.
                    let delivery = match participant_delivery {
                        Some(delivery) => Some(delivery),
                        None => product_owner.as_ref().map(|owners| {
                        platform::grpc_method_policy::guard(
                            &concert_service::product_rpc::delivery::POLICY,
                            concert_service::product_rpc::proto::product_delivery_service_server::ProductDeliveryServiceServer::new(
                                concert_service::product_rpc::delivery::DeliveryService::new(owners.delivery.clone()),
                            ).max_decoding_message_size(4096).max_encoding_message_size(4096),
                            runtime.callers.clone(),
                            runtime.actors.clone(),
                        )
                        }).transpose()?,
                    };
                    let product = product_owner.map(|owners| {
                        platform::grpc_method_policy::guard(
                            &concert_service::product_rpc::POLICY,
                            concert_service::product_rpc::proto::product_service_server::ProductServiceServer::new(
                                concert_service::product_rpc::ProductRpcService::new(owners.product),
                            )
                            .max_decoding_message_size(concert_service::product_rpc::MAX_FRAME)
                            .max_encoding_message_size(concert_service::product_rpc::MAX_FRAME),
                            runtime.callers.clone(),
                            runtime.actors.clone(),
                        )
                    }).transpose()?;
                    // Fifteen frozen profile operations read tables that live in
                    // THIS database (`profile_posts`, `profile_post_attachments`,
                    // `profile_media`, `profile_rooms`, `profile_avatars`). The
                    // profile service runs against `mcr_profile` and cannot reach
                    // them, and a cross-database join is forbidden, so the
                    // Gateway routes those operations here. This transport is the
                    // event-free profile analogue of `concert.product.v1`: same
                    // authority discipline, keyed by `profile_id`.
                    let profile_product = concert_service::profile_product::runtime(
                        grpc_pool.clone(),
                        runtime.callers.clone(),
                        runtime.actors.clone(),
                    )?;
                    // This callback uses the original shared registry and the
                    // dedicated Notification workload identity. It accepts no
                    // Gateway actor delegation and shares this listener's TLS
                    // and call-deadline layers.
                    let notification_target = notification_target.map(|target| {
                        platform::grpc_method_policy::guard_without_delegation(
                            &config::notification_target_runtime::POLICY,
                            target.service.server(),
                            target.callers,
                        )
                    }).transpose()?;
                    let feed_callback = feed_callback.map(|callback| {
                        platform::grpc_method_policy::guard(
                            &concert_service::product_rpc::root_read::POLICY,
                            callback.service.server(),
                            callback.callers,
                            callback.verifier,
                        )
                    }).transpose()?;
                    let archive = archive_service.map(|service| {
                        platform::grpc_method_policy::guard_without_delegation(
                            &config::closure_runtime::ARCHIVE_POLICY,
                            concert_service::product_rpc::proto::product_archive_service_server::ProductArchiveServiceServer::new(service),
                            runtime.callers.clone(),
                        )
                    }).transpose()?;
                    server
                        .add_service(bootstrap)
                        .add_service(world)
                        .add_optional_service(participant)
                        .add_optional_service(decision)
                        .add_optional_service(settlement)
                        .add_optional_service(product)
                        .add_optional_service(notification_target)
                        .add_optional_service(feed_callback)
                        .add_optional_service(archive)
                        .add_optional_service(delivery)
                        .add_service(concert_service::profile_product::retain_scopes(
                            profile_product.dispatch,
                        ))
                        .add_service(profile_product.settlement)
                        .add_optional_service(deletion)
                        .serve_with_shutdown(
                            runtime.address,
                            platform::shutdown::global().cancelled(),
                        )
                        .await?;
                }
                _ => platform::shutdown::idle().await?,
            }
            Ok::<(), anyhow::Error>(())
        };
        // The barrier is awaited beside the serving listener rather than before
        // it. Success opens the gate; failure requests shared shutdown with the
        // original error through the component supervisor below.
        let readiness = async move {
            match world_ready_rx {
                Some(ready) => {
                    await_world_ready(ready, world_ready_gate).await?;
                    platform::shutdown::idle().await
                }
                None => platform::shutdown::idle().await,
            }
        };
        let world = async move {
            match (world_settings, world_owners, world_ready_tx) {
                (Some(settings), Some(owners), Some(ready)) => {
                    match (world_transport, world_local, deletion_world_tx) {
                        (Some(transport), Some(local), Some(deletion)) => settings
                            .serve_with_ready_social_and_deletion_and_bridge(
                                world_pool, owners, ready, transport, local, deletion, closure_bridge,
                            )
                            .await,
                        (None, None, Some(deletion)) => settings
                            .serve_with_ready_and_deletion_and_bridge(world_pool, owners, ready, deletion, closure_bridge)
                            .await,
                        (Some(transport), Some(local), None) => settings
                            .serve_with_ready_and_social_and_bridge(world_pool, owners, ready, transport, local, closure_bridge)
                            .await,
                        (None, None, None) => settings.serve_with_ready_and_bridge(world_pool, owners, ready, closure_bridge).await,
                        _ => anyhow::bail!("World Social policy composition is incomplete"),
                    }
                }
                _ => platform::shutdown::idle().await,
            }
        };
        let recovery = async move {
            match participant_recovery {
                Some(recovery) => {
                    let (stop, receiver) = tokio::sync::watch::channel(false);
                    let worker = recovery.run(receiver);
                    tokio::pin!(worker);
                    tokio::select! {
                        _ = &mut worker => {
                            anyhow::ensure!(platform::shutdown::global().is_requested(), "Participant recovery stopped unexpectedly");
                        }
                        _ = platform::shutdown::global().cancelled() => {
                            stop.send_replace(true);
                            worker.await;
                        }
                    }
                    Ok(())
                }
                None => platform::shutdown::idle().await,
            }
        };
        let children = async move {
            match child_recovery {
                Some(worker) => worker.run().await.map_err(anyhow::Error::from),
                None => platform::shutdown::idle().await,
            }
        };
        let social = async move {
            match social_delivery {
                Some(worker) => worker.run().await,
                None => platform::shutdown::idle().await,
            }
        };
        let receipts = async move {
            receipt_maintenance
                .run_until(platform::shutdown::global().cancelled())
                .await
                .map_err(anyhow::Error::from)
        };
        // The discovery projection worker. It is idle when unconfigured, so the
        // composition has one shape either way. A round failure is logged and
        // retried by the worker itself; it never stops the process, because a
        // stale projection is better than a stopped concert service.
        let discovery = async move {
            match discovery_projection {
                Some(settings) => {
                    let endpoint = settings.endpoint().to_owned();
                    let interval = settings.interval();
                    let channel = tonic::transport::Channel::from_shared(endpoint)
                        .map_err(|_| anyhow::anyhow!("Discovery source endpoint is invalid"))?
                        .connect_lazy();
                    let source = Arc::new(concert_service::discovery::GrpcDiscoverySource::new(
                        concert_service::discovery::projection::proto::discovery_source_service_client::DiscoverySourceServiceClient::new(channel),
                    ));
                    let writer = Arc::new(concert_service::discovery::ProjectionWriter::new(
                        discovery_pool,
                        source,
                    ));
                    let worker = concert_service::discovery::ProjectionWorker::new(
                        writer,
                        None,
                        interval,
                        concert_service::discovery::ROUND_DEADLINE,
                    )
                    .map_err(|_| anyhow::anyhow!("Discovery projection worker configuration is invalid"))?;
                    worker
                        .run_until(platform::shutdown::global().cancelled())
                        .await
                        .map_err(anyhow::Error::from)
                }
                None => platform::shutdown::idle().await,
            }
        };
        // Each component requests shared shutdown on failure. Join rather than
        // dropping peer futures so native replay, membership and recovery drain.
        let (http, grpc, readiness, world, recovery, children, social, receipts, discovery) = tokio::join!(
            component("health", http, platform::shutdown::global()),
            component("gRPC", grpc, platform::shutdown::global()),
            component("World readiness", readiness, platform::shutdown::global()),
            component("World", world, platform::shutdown::global()),
            component("Participant recovery", recovery, platform::shutdown::global()),
            component("canonical child recovery", children, platform::shutdown::global()),
            component("Social delivery", social, platform::shutdown::global()),
            component("BE15 receipt maintenance", receipts, platform::shutdown::global()),
            component("discovery projection", discovery, platform::shutdown::global()),
        );
        http?;
        grpc?;
        readiness?;
        world?;
        recovery?;
        children?;
        social?;
        receipts?;
        discovery?;
        Ok(())
    })
    .await;
    pool.close().await;
    result
}

async fn component(
    name: &'static str,
    future: impl std::future::Future<Output = anyhow::Result<()>>,
    shutdown: platform::shutdown::Shutdown,
) -> anyhow::Result<()> {
    let result = future.await;
    if let Err(error) = &result {
        eprintln!("CONCERT_COMPONENT_FAILURE={name} {error:?}");
    }
    let result = if result.is_ok() && !shutdown.is_requested() {
        Err(anyhow::anyhow!("{name} stopped unexpectedly"))
    } else {
        result
    };
    if result.is_err() {
        shutdown.request();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::{
        await_world_ready, component, WorldReadinessGate, EARLY_SERVICES, WORLD_NOT_READY,
    };
    use platform::shutdown::Shutdown;
    use std::future::Ready;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::task::{Context, Poll};
    use tonic::body::Body;
    use tonic::codegen::http::{Request, Response};
    use tower::{Layer, Service};

    /// Every service this binary mounts except owner-authority settlement.
    /// The gate is deny-by-default; this list documents and exercises the
    /// registration list without turning it into a second allowlist.
    const GATED_SERVICES: &[&str] = &[
        "concert.v1.ConcertService",
        "concert.world.v1.WorldService",
        "concert.participant.v1.ConcertParticipantService",
        "concert.participant.v1.ConcertParticipantDecisionService",
        "concert.product.v1.ProductService",
        "concert.product.v1.ProductDeliveryService",
        "concert.product.v1.ProductArchiveService",
        "authority.lease.v1.ConcertRootReadService",
        "notification.concert.v1.ConcertNotificationTargetAuthority",
        "concert.account_deletion.v1.AccountDeletionService",
    ];

    #[derive(Clone)]
    struct PassThrough {
        calls: Arc<AtomicUsize>,
    }

    impl Service<Request<Body>> for PassThrough {
        type Response = Response<Body>;
        type Error = std::convert::Infallible;
        type Future = Ready<Result<Self::Response, Self::Error>>;

        fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, _: Request<Body>) -> Self::Future {
            self.calls.fetch_add(1, Ordering::SeqCst);
            std::future::ready(Ok(Response::new(Body::empty())))
        }
    }

    fn rpc(path: &str) -> Request<Body> {
        Request::builder()
            .uri(format!("/{path}/Probe"))
            .body(Body::empty())
            .unwrap()
    }

    fn status_of(response: &Response<Body>) -> Option<&str> {
        response
            .headers()
            .get(tonic::Status::GRPC_STATUS)
            .and_then(|value| value.to_str().ok())
    }

    #[test]
    fn early_allowlist_is_exactly_owner_authority_settlement() {
        assert_eq!(
            EARLY_SERVICES,
            ["authority.lease.v1.ProducerSettlementService"]
        );
        for service in GATED_SERVICES {
            assert!(
                !EARLY_SERVICES.contains(service),
                "{service} must stay gated"
            );
        }
    }

    #[tokio::test]
    async fn closed_gate_serves_settlement_and_refuses_every_gated_service() {
        let calls = Arc::new(AtomicUsize::new(0));
        let gate = WorldReadinessGate::new(true);
        let mut service = gate.layer(PassThrough {
            calls: calls.clone(),
        });
        // The settlement service is the one early answer.
        service
            .call(rpc("authority.lease.v1.ProducerSettlementService"))
            .await
            .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        // Every other mounted capability is refused without reaching the owner.
        for name in GATED_SERVICES {
            let response = service.call(rpc(name)).await.unwrap();
            assert_eq!(status_of(&response), Some("14"), "{name} was not gated");
            let status = response
                .extensions()
                .get::<tonic::Status>()
                .expect("trailers-only status");
            assert_eq!(status.code(), tonic::Code::Unavailable);
            assert_eq!(status.message(), WORLD_NOT_READY);
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        // Removing the gate would let the gated services reach the owner, so
        // this count is what fails when the gate stops short-circuiting.
    }

    #[tokio::test]
    async fn opening_the_gate_after_world_readiness_serves_gated_services() {
        let calls = Arc::new(AtomicUsize::new(0));
        let gate = WorldReadinessGate::new(true);
        let state = gate.state();
        let mut service = gate.layer(PassThrough {
            calls: calls.clone(),
        });
        let refused = service
            .call(rpc("concert.v1.ConcertService"))
            .await
            .unwrap();
        assert_eq!(status_of(&refused), Some("14"));
        state.store(true, Ordering::Release);
        let served = service
            .call(rpc("concert.v1.ConcertService"))
            .await
            .unwrap();
        assert_eq!(status_of(&served), None);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_configured_world_starts_closed_and_an_absent_world_serves() {
        assert!(!WorldReadinessGate::new(true)
            .state()
            .load(Ordering::Acquire));
        assert!(WorldReadinessGate::new(false)
            .state()
            .load(Ordering::Acquire));
    }

    #[tokio::test]
    async fn world_failure_requests_shutdown_with_the_original_error() {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        drop(sender);
        let gate = Arc::new(AtomicBool::new(false));
        let stop = Shutdown::new();
        let result = component(
            "World readiness",
            await_world_ready(receiver, gate.clone()),
            stop.clone(),
        )
        .await;
        assert!(stop.is_requested());
        let error = result.unwrap_err().to_string();
        assert!(error.contains("World startup readiness failed"), "{error}");
        assert!(!gate.load(Ordering::Acquire));
    }

    /// The serving binary, not only the test fixture, must build the listener
    /// behind the gate and await World readiness beside it, never before it.
    #[test]
    fn serving_binary_builds_the_listener_behind_the_world_gate() {
        let source = include_str!("main.rs");
        for required in [
            "WorldReadinessGate::new(world_configured)",
            ".layer(world_gate)",
            "await_world_ready(ready, world_ready_gate)",
            "component(\"World readiness\", readiness",
        ] {
            assert!(source.contains(required), "serving binary omits {required}");
        }
        let grpc = source
            .split("let grpc = async move {")
            .nth(1)
            .and_then(|rest| rest.split("let readiness = async move {").next())
            .expect("gRPC component is present");
        assert!(
            !grpc.contains("world_ready_rx"),
            "the gRPC listener must not await World readiness before serving"
        );
    }

    /// The serving binary, not only a test fixture, must compose the account
    /// deletion coordinator on the incumbent listener behind the Auth-only
    /// caller registry and mount the composed service.
    #[test]
    fn serving_binary_composes_and_mounts_account_deletion() {
        let source = include_str!("main.rs");
        for required in [
            "AccountDeletionSettings::load",
            "AccountDeletionRuntime::compose",
            "AuthDeletionCallers::configured",
            "LiveDeletionOwners",
            "as_serving_world",
            ".add_optional_service(deletion)",
            "serve_with_ready_social_and_deletion",
            "serve_with_ready_and_deletion",
        ] {
            assert!(source.contains(required), "serving binary omits {required}");
        }
    }

    #[tokio::test]
    async fn component_failure_requests_stop_and_allows_peer_cleanup_to_finish() {
        let stop = Shutdown::new();
        let cleaned = AtomicBool::new(false);
        let peer = async {
            stop.cancelled().await;
            tokio::task::yield_now().await;
            cleaned.store(true, Ordering::SeqCst);
            Ok(())
        };
        let (failed, drained) = tokio::join!(
            component(
                "failed",
                async { anyhow::bail!("owned failure") },
                stop.clone()
            ),
            component("peer", peer, stop.clone()),
        );
        assert!(failed.is_err());
        assert!(drained.is_ok());
        assert!(cleaned.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn unexpected_success_stops_peers_but_normal_shutdown_is_successful() {
        let stop = Shutdown::new();
        assert!(component("early", async { Ok(()) }, stop.clone())
            .await
            .is_err());
        assert!(stop.is_requested());
        assert!(component("drained", async { Ok(()) }, stop).await.is_ok());
    }
}
