//! Explicit qualified Concert reads on the existing Post listener and pool.
use super::AppConfig;
use crate::{
    adapters::r#in::grpc::concert_feed::qualified, concert_feed::read_runtime::QualifiedReadRuntime,
};
use platform::{
    actor_delegation::concert_read::{ConcertPostReadVerifier, PostConcertRootReadSigner},
    grpc_auth::{CallerRegistry, Workload},
    grpc_tls::TlsMaterial,
};

pub struct ReadSettings {
    auth: String,
    friend: String,
    root: String,
}
fn endpoint(value: String) -> anyhow::Result<String> {
    let uri: tonic::codegen::http::Uri = value
        .parse()
        .map_err(|_| anyhow::anyhow!("Invalid qualified Post owner endpoint"))?;
    anyhow::ensure!(
        uri.scheme_str() == Some("https")
            && uri.host().is_some_and(|host| !host.is_empty())
            && uri.port_u16().is_some_and(|port| port > 0)
            && uri.authority().is_some_and(|v| !v.as_str().contains('@'))
            && uri.path_and_query().is_none_or(|v| v.as_str() == "/"),
        "Qualified Post owners require explicit HTTPS host/port without credentials, path or query"
    );
    Ok(value)
}
impl ReadSettings {
    pub fn from_values(
        mut read: impl FnMut(&str) -> anyhow::Result<Option<String>>,
        tls: bool,
        pool_connections: u32,
    ) -> anyhow::Result<Option<Self>> {
        match read("POST_CONCERT_READ_MODE")?.as_deref() {
            None | Some("off") => return Ok(None),
            Some("read") => (),
            _ => anyhow::bail!("POST_CONCERT_READ_MODE must be off or read"),
        }
        anyhow::ensure!(tls, "Qualified Post reads require actual mutual TLS");
        anyhow::ensure!(
            pool_connections >= 2,
            "Qualified Post reads require two pool connections"
        );
        let mut required = |name| {
            read(name)?
                .filter(|v| !v.is_empty())
                .ok_or_else(|| anyhow::anyhow!("{name} is required for qualified Post reads"))
                .and_then(endpoint)
        };
        Ok(Some(Self {
            auth: required("POST_CONCERT_READ_AUTH_GRPC_ENDPOINT")?,
            friend: required("POST_FRIEND_GRPC_ENDPOINT")?,
            root: required("POST_CONCERT_ROOT_GRPC_ENDPOINT")?,
        }))
    }
}
// Credentials and signing material have no Debug/Serialize implementation.
pub struct ReadStartup {
    settings: ReadSettings,
    token: String,
    tls: TlsMaterial,
    callers: CallerRegistry,
    verifier: ConcertPostReadVerifier,
    signer: PostConcertRootReadSigner,
}
impl ReadStartup {
    /// Called before database/listener construction. Off reads only its mode.
    pub fn load(config: &AppConfig) -> anyhow::Result<Option<Self>> {
        let Some(settings) = ReadSettings::from_values(
            |name| match std::env::var(name) {
                Ok(v) => Ok(Some(v)),
                Err(std::env::VarError::NotPresent) => Ok(None),
                Err(_) => anyhow::bail!("Invalid qualified Post configuration encoding"),
            },
            config.tls_material.is_some(),
            config.database_pool_max_connections,
        )?
        else {
            return Ok(None);
        };
        let tls = config
            .tls_material
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Qualified Post TLS required"))?;
        tls.server_config()?;
        tls.endpoint(&settings.auth, Workload::Auth)?;
        tls.endpoint(&settings.friend, Workload::Friend)?;
        tls.endpoint(&settings.root, Workload::Concert)?;
        let name = platform::config::service_credential_env_name(Workload::Post);
        let token = platform::config::resolve_service_credential(
            Some(std::env::var(&name).map_err(|_| anyhow::anyhow!("{name} is required"))?),
            &name,
        )?
        .ok_or_else(|| anyhow::anyhow!("Explicit Post credential required"))?;
        let callers = platform::config::build_caller_registry(
            true,
            &[Workload::Concert, Workload::Auth, Workload::Friend],
            &[Workload::Concert, Workload::Auth, Workload::Friend],
        )?;
        let verifier = ConcertPostReadVerifier::configured_from_env(&config.actor_delegation)?
            .ok_or_else(|| anyhow::anyhow!("Concert Post read verifying keys are required"))?;
        let signer = PostConcertRootReadSigner::from_env(&config.actor_delegation)?;
        Ok(Some(Self {
            settings,
            token,
            tls,
            callers,
            verifier,
            signer,
        }))
    }
    pub fn compose(self, pool: sqlx::PgPool) -> anyhow::Result<qualified::Services> {
        let runtime = QualifiedReadRuntime::connect(
            pool,
            &self.settings.auth,
            &self.settings.friend,
            &self.settings.root,
            &self.token,
            &self.tls,
            self.signer,
        )?;
        qualified::services(runtime, self.callers, self.verifier)
    }
}
