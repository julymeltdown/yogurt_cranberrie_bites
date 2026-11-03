//! Explicit opt-in for the guarded bootstrap listener. Product release loading
//! never uses the synthetic-fixture constructor, including in local mode.
use platform::{
    actor_delegation::ActorDelegationVerifier,
    grpc_auth::{CallerRegistry, Workload},
};
use std::{net::SocketAddr, path::PathBuf};

pub struct BootstrapRuntime {
    pub address: SocketAddr,
    pub files: [PathBuf; 6],
    pub callers: CallerRegistry,
    pub actors: ActorDelegationVerifier,
    pub tls: Option<platform::grpc_tls::TlsMaterial>,
}

pub const RELEASE_SETTINGS: [&str; 6] = [
    "CONCERT_BOOTSTRAP_FILE",
    "CONCERT_AVATAR_MANIFEST_FILE",
    "CONCERT_MAP_MANIFEST_FILE",
    "CONCERT_REVENGE_MANIFEST_FILE",
    "CONCERT_PARADE_MANIFEST_FILE",
    "CONCERT_DANGER_MANIFEST_FILE",
];

/// Pure configuration boundary so invalid opt-ins can be tested without changing
/// process-wide environment or initializing credentials, TLS or a database.
pub fn settings(
    mut read: impl FnMut(&str) -> anyhow::Result<Option<String>>,
) -> anyhow::Result<Option<(SocketAddr, [PathBuf; 6], bool)>> {
    let Some(address) = read("CONCERT_GRPC_ADDR")? else {
        return Ok(None);
    };
    let address: SocketAddr = address
        .parse()
        .map_err(|_| anyhow::anyhow!("CONCERT_GRPC_ADDR is invalid"))?;
    let production = match read("CONCERT_RUNTIME_MODE")?.as_deref() {
        None | Some("production") => true,
        Some("local") => false,
        _ => anyhow::bail!("CONCERT_RUNTIME_MODE must be production or local"),
    };
    anyhow::ensure!(
        if production {
            address.port() != 0 && !address.ip().is_loopback()
        } else {
            address.ip().is_loopback()
        },
        "CONCERT_GRPC_ADDR is unsafe for this runtime mode"
    );
    let mut files = Vec::with_capacity(6);
    for name in RELEASE_SETTINGS {
        let value = read(name)?
            .filter(|v| !v.trim().is_empty())
            .ok_or_else(|| anyhow::anyhow!("{name} is required for bootstrap serving"))?;
        files.push(PathBuf::from(value));
    }
    Ok(Some((
        address,
        files.try_into().expect("six release settings"),
        production,
    )))
}

impl BootstrapRuntime {
    pub fn load() -> anyhow::Result<Option<Self>> {
        let Some((address, files, production)) = settings(|name| match std::env::var(name) {
            Ok(value) => Ok(Some(value)),
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(_) => anyhow::bail!("{name} must contain valid Unicode"),
        })?
        else {
            return Ok(None);
        };
        Ok(Some(Self {
            address,
            files,
            callers: platform::config::build_caller_registry(
                production,
                &[Workload::Gateway],
                &[Workload::Gateway],
            )?,
            actors: ActorDelegationVerifier::from_env(production, Workload::Concert)?,
            tls: platform::config::resolve_tls_material(production, "concert")?,
        }))
    }
}
