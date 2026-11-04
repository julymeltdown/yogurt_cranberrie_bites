use crate::adapters::out::{repository::PgConcertRepository, SystemClock, UnavailablePolicy};
use concert_application::ConcertCore;
use migration_runtime::{MigrationMode, SchemaProbe};
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    ConnectOptions, PgPool,
};
use std::{net::SocketAddr, path::PathBuf, str::FromStr, time::Duration};

pub mod bootstrap_runtime;
pub mod canonical_runtime;
pub mod inbox_runtime;
pub mod intro_runtime;
pub mod native_resolver;
pub mod notification_target_runtime;
pub mod participant_runtime;
pub mod party_runtime;
pub mod product_runtime;
pub mod report_runtime;
pub mod social_delivery;
pub mod social_policy_runtime;
pub mod social_runtime;
pub mod social_startup;

pub struct AppConfig {
    pub database: PgConnectOptions,
    pub metrics_addr: SocketAddr,
    pub migration_mode: MigrationMode,
}
impl AppConfig {
    pub fn load() -> anyhow::Result<Self> {
        let read = |name: &'static str| -> anyhow::Result<Option<String>> {
            match std::env::var(name) {
                Ok(value) => Ok(Some(value)),
                Err(std::env::VarError::NotPresent) => Ok(None),
                Err(_) => anyhow::bail!("{name} must contain valid Unicode"),
            }
        };
        Self::from_values(
            read("CONCERT_DATABASE_URL")?.as_deref(),
            read("CONCERT_METRICS_ADDR")?.as_deref(),
            read("CONCERT_RUNTIME_MODE")?.as_deref(),
            read("CONCERT_MIGRATION_MODE")?.as_deref(),
        )
    }
    pub fn from_values(
        url: Option<&str>,
        address: Option<&str>,
        runtime: Option<&str>,
        migration: Option<&str>,
    ) -> anyhow::Result<Self> {
        let local = match runtime {
            None | Some("production") => false,
            Some("local") => true,
            _ => anyhow::bail!("CONCERT_RUNTIME_MODE must be production or local"),
        };
        let database = database_options(url)?;
        let metrics_addr: SocketAddr = address
            .ok_or_else(|| anyhow::anyhow!("CONCERT_METRICS_ADDR is required"))?
            .parse()
            .map_err(|_| anyhow::anyhow!("CONCERT_METRICS_ADDR is invalid"))?;
        if local {
            anyhow::ensure!(
                metrics_addr.ip().is_loopback(),
                "local Concert health must bind loopback"
            );
        } else {
            anyhow::ensure!(
                metrics_addr.port() != 0,
                "production Concert health requires an explicit port"
            );
        }
        let migration_mode = match migration {
            None | Some("external") => MigrationMode::External,
            Some("startup") if local => MigrationMode::Startup,
            _ => anyhow::bail!(
                "Concert production requires external migrations; startup is explicit local only"
            ),
        };
        Ok(Self {
            database,
            metrics_addr,
            migration_mode,
        })
    }
}
/// Operator-packaged runtime SQL, shared by standalone commands and serving.
/// Selecting a directory grants no authority and does not freeze its contents;
/// deployment/qualification custody must retain and verify the exact SQL set.
pub fn migration_directory() -> anyhow::Result<PathBuf> {
    migration_directory_from(std::env::var_os("CONCERT_MIGRATION_DIRECTORY").as_deref())
}
fn migration_directory_from(value: Option<&std::ffi::OsStr>) -> anyhow::Result<PathBuf> {
    use std::{fs, path::Component};
    let invalid = || anyhow::anyhow!("Concert migration directory must be an absolute nonsymlink directory containing regular SQL files");
    let directory = value.map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations")
    });
    anyhow::ensure!(directory.is_absolute(), invalid());
    anyhow::ensure!(
        !directory.as_os_str().as_encoded_bytes().split(|b| *b == b'/')
            .any(|part| part == b"." || part == b".."),
        invalid()
    );
    let mut prefix = PathBuf::new();
    for component in directory.components() {
        anyhow::ensure!(matches!(component, Component::RootDir | Component::Normal(_)), invalid());
        prefix.push(component.as_os_str());
        let metadata = fs::symlink_metadata(&prefix).map_err(|_| invalid())?;
        anyhow::ensure!(metadata.is_dir() && !metadata.file_type().is_symlink(), invalid());
    }
    let mut has_sql = false;
    for entry in fs::read_dir(&directory).map_err(|_| invalid())? {
        let entry = entry.map_err(|_| invalid())?;
        let metadata = fs::symlink_metadata(entry.path()).map_err(|_| invalid())?;
        anyhow::ensure!(metadata.is_file() && !metadata.file_type().is_symlink(), invalid());
        if entry.path().extension().is_some_and(|extension| extension == "sql") {
            anyhow::ensure!(metadata.is_file(), invalid());
            has_sql = true;
        }
    }
    anyhow::ensure!(has_sql, invalid());
    directory.canonicalize().map_err(|_| invalid())
}
pub const SCHEMA_PROBES: &[SchemaProbe] = &[SchemaProbe {
    name: "concert-core-constraints",
    sql: include_str!("schema-probe.sql"),
}];
pub async fn connect_pool(config: &AppConfig) -> anyhow::Result<PgPool> {
    let migration_directory = migration_directory()?;
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .acquire_timeout(Duration::from_secs(3))
        .connect_with(migration_runtime::serving_options(
            config
                .database
                .clone()
                .options([("statement_timeout", "3000"), ("lock_timeout", "1000")]),
            config.migration_mode,
        ))
        .await
        .map_err(|_| anyhow::anyhow!("Concert database connection failed"))?;
    if migration_runtime::prepare_database(
        &pool,
        &migration_directory,
        config.migration_mode,
        SCHEMA_PROBES,
    )
    .await
    .is_err()
    {
        pool.close().await;
        anyhow::bail!("Concert database schema verification failed");
    }
    Ok(pool)
}
pub type Core = ConcertCore<PgConcertRepository, SystemClock, UnavailablePolicy>;
pub fn build_application(pool: PgPool) -> Core {
    ConcertCore::new(
        PgConcertRepository::new(pool),
        SystemClock,
        UnavailablePolicy,
    )
}

pub fn database_options(url: Option<&str>) -> anyhow::Result<PgConnectOptions> {
    let url = url
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("CONCERT_DATABASE_URL is required"))?;
    // SQLx initializes absent URL components from PG* variables. Require the DB
    // path explicitly so PGDATABASE can never turn an incomplete URL into a target.
    let rest = url
        .strip_prefix("postgresql://")
        .or_else(|| url.strip_prefix("postgres://"))
        .ok_or_else(|| {
            anyhow::anyhow!("CONCERT_DATABASE_URL must be an explicit PostgreSQL URL")
        })?;
    let (authority, path) = rest
        .split_once('/')
        .ok_or_else(|| anyhow::anyhow!("CONCERT_DATABASE_URL must include its database path"))?;
    anyhow::ensure!(
        !authority.is_empty()
            && !authority.contains(['?', '#'])
            && !authority.rsplit('@').next().unwrap_or_default().is_empty()
            && !path.contains('#'),
        "CONCERT_DATABASE_URL requires an explicit host and database path"
    );
    let explicit_database = path.split('?').next().unwrap_or_default();
    anyhow::ensure!(
        !explicit_database.is_empty() && !explicit_database.contains('/'),
        "CONCERT_DATABASE_URL requires one database path"
    );
    let database = PgConnectOptions::from_str(url)
        .map_err(|_| anyhow::anyhow!("CONCERT_DATABASE_URL is invalid"))?
        .disable_statement_logging();
    anyhow::ensure!(
        database.get_database() == Some(explicit_database),
        "Concert database must be explicit and unambiguous"
    );
    anyhow::ensure!(
        database.get_options().is_none(),
        "CONCERT_DATABASE_URL must not override server options"
    );
    let name = database.get_database().unwrap_or_default();
    anyhow::ensure!(name.len() <= 63, "Concert database name is too long");
    anyhow::ensure!(
        name == "mcr_concert"
            || name
                .strip_prefix("mcr_concert_")
                .is_some_and(|suffix| !suffix.is_empty()
                    && suffix
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')),
        "CONCERT_DATABASE_URL requires a dedicated mcr_concert database"
    );
    Ok(database)
}

pub mod world_runtime;

pub mod event_cleanup;

#[cfg(test)]
mod migration_directory_tests {
    use super::migration_directory_from;
    use std::{fs, path::PathBuf};

    struct Package(PathBuf);
    impl Package {
        fn new() -> Self {
            let parent = std::env::temp_dir().canonicalize().unwrap();
            let path = parent.join(format!("mcr-migration-path-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn sql(&self) -> PathBuf {
            let path = self.0.join("0001_fixture.sql");
            fs::write(&path, "select 1;\n").unwrap();
            path
        }
    }
    impl Drop for Package {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn accepts_explicit_regular_sql_package_and_retains_absent_default() {
        let package = Package::new();
        let sql = package.sql();
        let before = fs::read(&sql).unwrap();
        assert_eq!(migration_directory_from(Some(package.0.as_os_str())).unwrap(), package.0);
        assert_eq!(fs::read(sql).unwrap(), before);
        let default = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations");
        assert_eq!(migration_directory_from(None).unwrap(), default.canonicalize().unwrap());
    }

    #[test]
    fn refuses_empty_relative_parent_missing_file_and_empty_package_paths() {
        let package = Package::new();
        assert!(migration_directory_from(Some(package.0.as_os_str())).is_err());
        let sql = package.sql();
        for path in [
            PathBuf::new(),
            PathBuf::from("migrations"),
            package.0.join(".."),
            package.0.join("."),
            package.0.join("missing"),
            sql,
        ] {
            assert!(migration_directory_from(Some(path.as_os_str())).is_err(), "invalid package path accepted");
        }
    }

    #[cfg(unix)]
    #[test]
    fn refuses_symlink_directory_ancestor_and_sql_entry() {
        use std::os::unix::fs::symlink;
        let package = Package::new();
        package.sql();
        let aliases = Package::new();
        let alias = aliases.0.join("alias");
        symlink(&package.0, &alias).unwrap();
        assert!(migration_directory_from(Some(alias.as_os_str())).is_err());
        let nested = package.0.join("nested");
        fs::create_dir(&nested).unwrap();
        fs::write(nested.join("0001_fixture.sql"), "select 1;\n").unwrap();
        assert!(migration_directory_from(Some(alias.join("nested").as_os_str())).is_err());
        fs::remove_dir_all(nested).unwrap();
        let linked_sql = package.0.join("0002_linked.sql");
        symlink(package.0.join("0001_fixture.sql"), linked_sql).unwrap();
        assert!(migration_directory_from(Some(package.0.as_os_str())).is_err());
    }

    #[test]
    fn refuses_directory_named_as_sql_and_nested_nonregular_entries() {
        let package = Package::new();
        package.sql();
        fs::create_dir(package.0.join("0002_directory.sql")).unwrap();
        assert!(migration_directory_from(Some(package.0.as_os_str())).is_err());
    }
}
