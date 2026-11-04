use concert_service::{
    bootstrap::{BootstrapFeatures, BootstrapService, ProductReleaseBundle},
    config, health, rpc, world_rpc,
};
mod in01_catalog;
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
    if matches!(
        command,
        migration_runtime::Command::Migrate | migration_runtime::Command::Verify
    ) {
        config::database_options(std::env::var("CONCERT_DATABASE_URL").ok().as_deref())?;
    }
    if migration_runtime::handle_command(
        command_arguments.iter().cloned(),
        "CONCERT_DATABASE_URL",
        &config::migration_directory(),
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
    let wall_b_settings = config::mural_tools_b::WallBSettings::load(intro_settings.is_some())?;
    let report_settings = config::report_runtime::ReportStartupSettings::load(
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
    let social_settings = config::social_startup::SocialStartupSettings::load(
        intro_settings.is_some(),
        world_settings.is_some(),
        inbox_settings.enabled() && target_settings.is_some(),
        runtime.as_ref().and_then(|runtime| runtime.tls.as_ref()),
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
            ..product_settings
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
    let mut child_recovery = None;
    let mut notification_target = None;
    let mut feed_callback = None;
    let mut social_delivery = None;
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
                    let owners = inbox_settings.compose(&shared.owners, intro_owner);
                    let owners = match wall_b_settings { Some(settings) => settings.compose(pool.clone(), &shared.owners, owners), None => owners };
                    let owners = match feed_settings {
                        Some(settings) => {
                            let (owners, callback) = settings.compose(&shared.owners, owners)?;
                            feed_callback = Some(callback);
                            owners
                        }
                        None => owners,
                    };
                    let owners = match social_settings {
                        Some(settings) => {
                            let policy = config::social_policy_runtime::SocialWorldPolicy::compose(
                                pool.clone(),
                            );
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
                    product_owner = Some(match report_settings {
                        Some(settings) => {
                            settings.compose_shared(pool.clone(), &shared.owners, owners)?
                        }
                        None => owners,
                    });
                    Ok(shared.participant)
                }
                None => settings.compose(pool.clone(), release),
            }
        })
        .transpose()?;
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
    let (world_ready_tx, world_ready_rx) = if world_settings.is_some() {
        let (tx, rx) = tokio::sync::oneshot::channel();
        (Some(tx), Some(rx))
    } else {
        (None, None)
    };
    let _application = config::build_application(pool.clone());
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
            if let Some(ready) = world_ready_rx {
                tokio::select! {
                    result = ready => result.map_err(|_| anyhow::anyhow!("World startup readiness failed"))?,
                    _ = platform::shutdown::global().cancelled() => return Ok(()),
                }
            }
            match (runtime, bootstrap) {
                (Some(runtime), Some(release)) => {
                    let mut server = tonic::transport::Server::builder()
                        .layer(platform::actor_delegation::response_scope::RetainedScopeLayer)
                        .layer(platform::correlation::CorrelationLayer)
                        .layer(platform::call_deadline::CallDeadlineLayer)
                        .layer(platform::trace_context::TraceContextLayer);
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
                    server
                        .add_service(bootstrap)
                        .add_service(world)
                        .add_optional_service(participant)
                        .add_optional_service(decision)
                        .add_optional_service(settlement)
                        .add_optional_service(product)
                        .add_optional_service(notification_target)
                        .add_optional_service(feed_callback)
                        .add_optional_service(delivery)
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
        let world = async move {
            match (world_settings, world_owners, world_ready_tx) {
                (Some(settings), Some(owners), Some(ready)) => {
                    match (world_transport, world_local) {
                        (Some(transport), Some(local)) => settings
                            .serve_with_ready_and_social(world_pool, owners, ready, transport, local)
                            .await,
                        (None, None) => settings.serve_with_ready(world_pool, owners, ready).await,
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
        // Each component requests shared shutdown on failure. Join rather than
        // dropping peer futures so native replay, membership and recovery drain.
        let (http, grpc, world, recovery, children, social) = tokio::join!(
            component("health", http, platform::shutdown::global()),
            component("gRPC", grpc, platform::shutdown::global()),
            component("World", world, platform::shutdown::global()),
            component("Participant recovery", recovery, platform::shutdown::global()),
            component("canonical child recovery", children, platform::shutdown::global()),
            component("Social delivery", social, platform::shutdown::global()),
        );
        http?;
        grpc?;
        world?;
        recovery?;
        children?;
        social?;
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
    use super::component;
    use platform::shutdown::Shutdown;
    use std::sync::atomic::{AtomicBool, Ordering};

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
