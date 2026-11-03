use concert_service::{
    bootstrap::{BootstrapFeatures, BootstrapService, ProductReleaseBundle},
    config, health, rpc, world_rpc,
};
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let command = migration_runtime::parse_command(std::env::args_os().skip(1))?;
    if matches!(
        command,
        migration_runtime::Command::Migrate | migration_runtime::Command::Verify
    ) {
        config::database_options(std::env::var("CONCERT_DATABASE_URL").ok().as_deref())?;
    }
    if migration_runtime::handle_command(
        std::env::args_os().skip(1),
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
    let bootstrap = runtime
        .as_ref()
        .map(|runtime| {
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
    let pool = config::connect_pool(&config).await?;
    let _application = config::build_application(pool.clone());
    let health = health::router(health::readiness(pool.clone()));
    let listener = tokio::net::TcpListener::bind(config.metrics_addr)
        .await
        .map_err(|_| anyhow::anyhow!("Concert health listener bind failed"))?;
    println!("CONCERT_HEALTH_LISTENER={}", listener.local_addr()?);
    let grpc_pool = pool.clone();
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
                    let mut server = tonic::transport::Server::builder()
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
                        runtime.callers,
                        runtime.actors,
                    )?;
                    server
                        .add_service(bootstrap)
                        .add_service(world)
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
        tokio::try_join!(http, grpc)?;
        Ok(())
    })
    .await;
    pool.close().await;
    result
}
