//! Production composition for the existing durable eight-target Auth worker.
//! Configuration is explicit; off leaves all four required Concert steps retryable.
use super::AppConfig;
use crate::{
    adapters::out::grpc::{
        concert_deletion_client::ConcertDeletionClient,
        user_deletion_dispatcher::GrpcUserDeletionDispatcher,
    },
    concert_deletion::{ConcertDeletionDispatcher, ConcertDeletionOwners},
};
use platform::{
    edge_metrics::EdgeDeadlineMetrics,
    grpc_auth::{CallerRegistry, Workload},
};
use std::time::Duration;

pub type RuntimeDispatcher = ConcertDeletionDispatcher<GrpcUserDeletionDispatcher>;

#[derive(Clone, Debug)]
pub struct ConcertDeletionSettings {
    endpoint: Option<String>,
    deadline: Duration,
}
fn read(name: &str) -> anyhow::Result<Option<String>> {
    match std::env::var(name) {
        Ok(v) => Ok(Some(v)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(_) => anyhow::bail!("{name} must contain valid Unicode"),
    }
}
impl ConcertDeletionSettings {
    pub fn load() -> anyhow::Result<Self> {
        Self::from_values(
            read("AUTH_CONCERT_DELETION_MODE")?.as_deref(),
            read("AUTH_CONCERT_GRPC_ENDPOINT")?.as_deref(),
            read("AUTH_CONCERT_DELETION_DEADLINE_MS")?.as_deref(),
        )
    }
    pub fn from_values(
        mode: Option<&str>,
        endpoint: Option<&str>,
        deadline_ms: Option<&str>,
    ) -> anyhow::Result<Self> {
        let deadline = match deadline_ms {
            None => 5000,
            Some(v) => platform::config::parse_u64_setting(
                "AUTH_CONCERT_DELETION_DEADLINE_MS",
                v,
                1,
                5000,
            )?,
        };
        let endpoint = match mode {
            None | Some("off") => {
                anyhow::ensure!(
                    endpoint.is_none(),
                    "AUTH_CONCERT_GRPC_ENDPOINT requires managed deletion mode"
                );
                None
            }
            Some("managed") => {
                let value = endpoint.ok_or_else(|| {
                    anyhow::anyhow!("AUTH_CONCERT_GRPC_ENDPOINT is required for managed deletion")
                })?;
                https_endpoint(value)?;
                Some(value.to_owned())
            }
            _ => anyhow::bail!("AUTH_CONCERT_DELETION_MODE must be off or managed"),
        };
        Ok(Self {
            endpoint,
            deadline: Duration::from_millis(deadline),
        })
    }
    pub fn managed(&self) -> bool {
        self.endpoint.is_some()
    }
    pub fn deadline(&self) -> Duration {
        self.deadline
    }
    pub fn validate(&self, config: &AppConfig) -> anyhow::Result<()> {
        if let Some(endpoint) = &self.endpoint {
            let tls = config.tls_material.as_ref().ok_or_else(|| {
                anyhow::anyhow!("Managed Concert deletion requires internal mTLS")
            })?;
            // Validate the actual Auth credential/TLS channel eagerly; connection
            // reachability remains lazy and cannot produce a successful no-op.
            ConcertDeletionClient::connect(endpoint, tls, &config.outbound_identity)?;
            for endpoint in [
                &config.profile_grpc_endpoint,
                &config.friend_grpc_endpoint,
                &config.post_grpc_endpoint,
                &config.notification_grpc_endpoint,
            ] {
                https_endpoint(endpoint)?;
            }
        }
        Ok(())
    }
    pub fn dispatcher(
        &self,
        config: &AppConfig,
        metrics: EdgeDeadlineMetrics,
    ) -> anyhow::Result<RuntimeDispatcher> {
        self.validate(config)?;
        let inherited = GrpcUserDeletionDispatcher::connect_with_metrics(
            config.profile_grpc_endpoint.clone(),
            config.friend_grpc_endpoint.clone(),
            config.post_grpc_endpoint.clone(),
            config.notification_grpc_endpoint.clone(),
            &config.outbound_identity,
            config.tls_material.as_ref(),
            metrics,
        )?;
        let owners = match &self.endpoint {
            None => ConcertDeletionOwners::default(),
            Some(endpoint) => ConcertDeletionClient::connect(
                endpoint,
                config.tls_material.as_ref().expect("validated mTLS"),
                &config.outbound_identity,
            )?
            .owners(),
        };
        Ok(ConcertDeletionDispatcher::new(
            inherited,
            owners,
            self.deadline,
        )?)
    }
    /// Run before database connection or listener binding. The source service is
    /// the canonical reverse leg of managed deletion, never implicitly enabled.
    pub fn startup_source_registry(
        &self,
        config: &AppConfig,
    ) -> anyhow::Result<Option<CallerRegistry>> {
        self.validate(config)?;
        let enabled = match read("AUTH_CONCERT_DELETION_SOURCE_ENABLED")?.as_deref() {
            None | Some("false") => false,
            Some("true") => true,
            _ => anyhow::bail!("AUTH_CONCERT_DELETION_SOURCE_ENABLED must be true or false"),
        };
        anyhow::ensure!(
            !self.managed() || enabled,
            "Managed Concert deletion requires explicit Auth deletion source opt-in"
        );
        if !enabled {
            return Ok(None);
        }
        anyhow::ensure!(
            config.tls_material.is_some(),
            "Auth deletion source requires internal mTLS"
        );
        Ok(Some(platform::config::build_caller_registry(
            true,
            &[Workload::Concert],
            &[Workload::Concert],
        )?))
    }
}
fn https_endpoint(value: &str) -> anyhow::Result<()> {
    let uri: http::Uri = value
        .parse()
        .map_err(|_| anyhow::anyhow!("Deletion endpoint must be an explicit HTTPS origin"))?;
    anyhow::ensure!(
        uri.scheme_str() == Some("https")
            && uri
                .authority()
                .is_some_and(|a| !a.host().is_empty() && !a.as_str().contains('@'))
            && uri.path_and_query().is_none_or(|p| p.as_str() == "/"),
        "Deletion endpoint must be an explicit HTTPS origin"
    );
    Ok(())
}
