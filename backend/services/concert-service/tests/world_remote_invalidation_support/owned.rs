//! Ownership checks adapted from world_owner_composition/mod.rs at a4ae8555.
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool,
};
use std::{os::unix::fs::MetadataExt, path::PathBuf, str::FromStr};
pub(super) async fn pool(owner: &str) -> anyhow::Result<PgPool> {
    anyhow::ensure!(
        matches!(owner, "auth" | "concert" | "friend" | "profile"),
        "foreign owner refused"
    );
    let root = PathBuf::from(std::env::var("MCR_WORLD_REMOTE_ROOT")?);
    let info = std::fs::symlink_metadata(&root)?;
    anyhow::ensure!(
        root.is_absolute()
            && root.canonicalize()? == root
            && info.is_dir()
            && info.mode() & 0o077 == 0,
        "private owned fixture required"
    );
    anyhow::ensure!(
        format!("{}:{}", info.dev(), info.ino()) == std::env::var("MCR_WORLD_REMOTE_IDENTITY")?,
        "fixture inode changed"
    );
    let marker = root.join("mcr-owner.json");
    let mi = std::fs::symlink_metadata(&marker)?;
    anyhow::ensure!(
        mi.is_file()
            && !mi.file_type().is_symlink()
            && mi.mode() & 0o077 == 0
            && mi.uid() == info.uid(),
        "private marker required"
    );
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(marker)?)?;
    anyhow::ensure!(
        value["uid"].as_u64() == Some(info.uid() as u64)
            && value["token"].as_str() == Some(std::env::var("MCR_WORLD_REMOTE_TOKEN")?.as_str()),
        "fixture owner mismatch"
    );
    for name in ["data", "socket"] {
        let child = root.join(name);
        let ci = std::fs::symlink_metadata(&child)?;
        anyhow::ensure!(
            ci.is_dir() && !ci.file_type().is_symlink() && child.canonicalize()? == child,
            "foreign fixture child refused"
        );
    }
    let options = PgConnectOptions::from_str(&std::env::var(format!(
        "MCR_WORLD_REMOTE_{}_URL",
        owner.to_uppercase()
    ))?)?;
    let database = format!("mcr_{owner}_world_remote");
    anyhow::ensure!(
        options.get_database() == Some(database.as_str())
            && options.get_username() == "mcr_owner"
            && options.get_socket() == Some(&root.join("socket"))
            && options.get_options().is_none()
            && options.get_port() == 5432,
        "foreign database refused"
    );
    let pool = PgPoolOptions::new()
        .max_connections(6)
        .connect_with(options)
        .await?;
    let data: String = sqlx::query_scalar("show data_directory")
        .fetch_one(&pool)
        .await?;
    anyhow::ensure!(
        data == root.join("data").to_string_lossy(),
        "foreign data directory refused"
    );
    let identity: (String, String, String, i32) = sqlx::query_as(
        "select current_database(),current_user,current_setting('listen_addresses'),current_setting('server_version_num')::int"
    ).fetch_one(&pool).await?;
    anyhow::ensure!(
        identity == (database, "mcr_owner".into(), "".into(), 160014),
        "foreign server refused"
    );
    Ok(pool)
}
