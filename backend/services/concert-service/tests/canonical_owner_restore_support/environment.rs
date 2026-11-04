//! Restore harness derived from backend/services/concert-service/tests/canonical_owner_composition_support/environment.rs at 3afb8111d8ef206f3e73c63186699c98d9fd4997; original SHA256 7d1b8a3a51be7c63a6aaeaa0a82a25dbb62f9e7ae46737a89ea0905166dd49f6.
//! CF01 adapted from process_child_recovery_support/environment.rs at 591d908a85a01fe0a494e04c0109bb0bec64a3e1; original SHA256 adde0dc16f21112e866bea96f11334d6033f6669fad408569de492f5c392c352.
//! Adapted read-only from backend/services/concert-service/tests/owner_authority_support/environment.rs at 3e22615eb39a8912c71c7bbcb003930cfc24f373.
//! Source SHA-256: d02ed1917ed25ea635dd4187a3b862cead97b52942c951cb0a25786e30a21a33.
//! Public native fixture changes: crate-local paths; Profile allowed in exact owned DB list.
//! Owned fixture checks adapted from product_scope_ledger (root b59fea237d)
//! and owner-store native fixture (store d8c6381b). No ambient database is used.
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool, Row,
};
use std::{
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::PathBuf,
    str::FromStr,
};
use uuid::Uuid;

pub(super) fn root() -> anyhow::Result<PathBuf> {
    let root = PathBuf::from(std::env::var("MCR_CANONICAL_ROOT")?);
    let m = std::fs::symlink_metadata(&root)?;
    anyhow::ensure!(
        root.is_absolute()
            && m.is_dir()
            && !m.file_type().is_symlink()
            && m.permissions().mode() & 0o077 == 0,
        "Private owned directory required"
    );
    anyhow::ensure!(
        format!("{}:{}", m.dev(), m.ino()) == std::env::var("MCR_CANONICAL_ROOT_IDENTITY")?,
        "Replaced fixture root"
    );
    let marker = root.join("mcr-owner.json");
    let mm = std::fs::symlink_metadata(&marker)?;
    anyhow::ensure!(
        mm.is_file() && !mm.file_type().is_symlink() && mm.permissions().mode() & 0o077 == 0,
        "Private marker required"
    );
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(marker)?)?;
    anyhow::ensure!(
        value["uid"].as_u64() == Some(m.uid() as u64)
            && value["token"].as_str() == Some(std::env::var("MCR_CANONICAL_ROOT_TOKEN")?.as_str()),
        "Foreign fixture marker"
    );
    Ok(root)
}
pub(super) async fn pool(owner: &str) -> anyhow::Result<PgPool> {
    anyhow::ensure!(
        matches!(
            owner,
            "auth" | "friend" | "concert" | "profile" | "notification"
        ),
        "Unknown fixture owner"
    );
    let root = root()?;
    let name = format!("mcr_canonical_{owner}");
    let options = PgConnectOptions::from_str(&std::env::var(format!(
        "MCR_CANONICAL_{}_URL",
        owner.to_ascii_uppercase()
    ))?)?;
    anyhow::ensure!(
        options.get_database() == Some(name.as_str())
            && options.get_username() == "mcr_owner"
            && options.get_socket() == Some(&root.join("socket"))
            && options.get_port() == 5432
            && options.get_options().is_none(),
        "Foreign database options"
    );
    let pool = PgPoolOptions::new()
        .max_connections(12)
        .connect_with(options)
        .await?;
    let row=sqlx::query("select current_database() db,current_setting('data_directory') data,current_setting('listen_addresses') listeners,(select system_identifier::text from pg_control_system()) system").fetch_one(&pool).await?;
    anyhow::ensure!(
        row.get::<String, _>("db") == name
            && row.get::<String, _>("data") == root.join("data").to_string_lossy()
            && row.get::<String, _>("listeners").is_empty()
            && row.get::<String, _>("system") == std::env::var("MCR_CANONICAL_SYSTEM")?,
        "Foreign PostgreSQL identity"
    );
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(format!("{owner}-service"))
        .join("migrations");
    sqlx::migrate::Migrator::new(path.as_path())
        .await?
        .run(&pool)
        .await?;
    Ok(pool)
}
pub(super) async fn identities(
    auth: &PgPool,
    count: usize,
) -> anyhow::Result<(Uuid, Uuid, Vec<Uuid>)> {
    let actor = Uuid::new_v4();
    let family = Uuid::new_v4();
    let subjects = (0..count).map(|_| Uuid::new_v4()).collect::<Vec<_>>();
    for id in std::iter::once(&actor).chain(subjects.iter()) {
        sqlx::query("insert into users(id,primary_email,password_hash,status) values($1,$2,'canonical-fixture','ACTIVE')").bind(id).bind(format!("{id}@example.invalid")).execute(auth).await?;
    }
    sqlx::query("insert into refresh_tokens(jti,token_hash,user_id,family_id,expires_at) values($1,$2,$3,$4,clock_timestamp()+interval '1 hour')").bind(Uuid::new_v4().to_string()).bind(Uuid::new_v4().to_string()).bind(actor).bind(family).execute(auth).await?;
    Ok((actor, family, subjects))
}
pub(super) fn tls(
    role: platform::grpc_auth::Workload,
) -> anyhow::Result<platform::grpc_tls::TlsMaterial> {
    let p = root()?.join("pki");
    platform::grpc_tls::TlsMaterial::load(
        p.join(format!("{}.crt", role.as_str())),
        p.join(format!("{}.key", role.as_str())),
        p.join("ca.crt"),
    )
}
