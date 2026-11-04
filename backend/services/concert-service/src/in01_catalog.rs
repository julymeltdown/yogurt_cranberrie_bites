//! Explicit local startup preparation through the real source-bound repository.
//! This command creates no principal, participant, admission, ticket or listener.
use concert_application::place::PlaceRepository;
use concert_domain::EventId;
use concert_service::{
    adapters::out::repository::place::PgPlaceRepository,
    bootstrap::{spatial::load_candidate, BootstrapFeatures, ProductReleaseBundle},
    config,
};
use std::path::PathBuf;

pub async fn run() -> anyhow::Result<()> {
    anyhow::ensure!(
        std::env::var("CONCERT_RUNTIME_MODE").as_deref() == Ok("local"),
        "Catalog preparation requires explicit local mode"
    );
    let required = |name| {
        std::env::var(name)
            .ok()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow::anyhow!("Missing catalog preparation setting"))
    };
    let event_text = required("CONCERT_WORLD_EVENT_ID")?;
    let event = uuid::Uuid::parse_str(&event_text)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == event_text)
        .ok_or_else(|| anyhow::anyhow!("Invalid catalog event"))?;
    let files = config::bootstrap_runtime::RELEASE_SETTINGS
        .map(|name| required(name).map(PathBuf::from))
        .into_iter()
        .collect::<anyhow::Result<Vec<_>>>()?;
    let binding = PathBuf::from(required("CONCERT_SOURCE_BINDING_FILE")?);
    let assets = PathBuf::from(required("CONCERT_ASSET_ROOT")?);
    anyhow::ensure!(
        files
            .iter()
            .chain([&binding, &assets])
            .all(|path| path.is_absolute()),
        "Catalog paths must be absolute"
    );
    let release = ProductReleaseBundle::from_source_bound_files(
        &files[0],
        &files[1],
        &files[2],
        [&files[3], &files[4], &files[5]],
        BootstrapFeatures::default(),
        &assets,
        &binding,
    )
    .map_err(|_| anyhow::anyhow!("Source-bound product release unavailable"))?;
    let template: serde_json::Value = serde_json::from_slice(release.release().bootstrap_json())?;
    anyhow::ensure!(
        template["event"]["event_id"].as_str() == Some(event_text.as_str()),
        "Catalog event and product release differ"
    );
    let candidate = load_candidate(&files[2], &binding, &assets)
        .map_err(|_| anyhow::anyhow!("Source-bound map unavailable"))?;
    anyhow::ensure!(
        candidate.catalog.manifest.map_revision == 4,
        "IN01 preparation requires the selected map revision"
    );
    let options = config::database_options(Some(&required("CONCERT_DATABASE_URL")?))?;
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect_with(options)
        .await
        .map_err(|_| anyhow::anyhow!("Catalog database unavailable"))?;
    let repository = PgPlaceRepository::new(pool.clone());
    let event = EventId::from_uuid(event);
    repository
        .publish_release(event, candidate.catalog.clone())
        .await
        .map_err(|_| anyhow::anyhow!("Catalog publication unavailable"))?;
    let persisted = repository
        .catalog(event)
        .await
        .map_err(|_| anyhow::anyhow!("Catalog readback unavailable"))?
        .ok_or_else(|| anyhow::anyhow!("Catalog missing after publication"))?;
    anyhow::ensure!(
        persisted.release == candidate.catalog,
        "Catalog readback mismatch"
    );
    pool.close().await;
    println!("IN01_CATALOG_PUBLISHED=4");
    Ok(())
}
