use concert_service::{config, health};
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
    let pool = config::connect_pool(&config).await?;
    let _application = config::build_application(pool.clone());
    let health = health::router(health::readiness(pool.clone()));
    let listener = tokio::net::TcpListener::bind(config.metrics_addr)
        .await
        .map_err(|_| anyhow::anyhow!("Concert health listener bind failed"))?;
    println!("CONCERT_HEALTH_LISTENER={}", listener.local_addr()?);
    let result = platform::shutdown::supervise(async move {
        axum::serve(listener, health)
            .with_graceful_shutdown(platform::shutdown::global().cancelled())
            .await?;
        Ok(())
    })
    .await;
    pool.close().await;
    result
}
