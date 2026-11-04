//! Single-listener account-deletion join. Consumers supply only incumbent owners.
use super::{
    account_deletion_runtime::{
        AccountDeletionRuntime, AccountDeletionService, AccountDeletionSettings,
        AuthDeletionCallers, LiveDeletionOwners,
    },
    bootstrap_runtime::BootstrapRuntime,
    world_runtime::WorldDeletionHandle,
};
use crate::social_live::invalidation::SocialPolicyPublisher;
use std::sync::Arc;

pub struct DeletionComposition {
    mode: AccountDeletionSettings,
    auth_endpoint: Option<String>,
    credential: Option<String>,
    callers: Option<AuthDeletionCallers>,
}
impl DeletionComposition {
    /// Capture the already canonical configuration before opening DB/listeners.
    /// Off does not require credentials or create an outbound client.
    pub fn load(runtime: Option<&BootstrapRuntime>) -> anyhow::Result<Self> {
        let mode = AccountDeletionSettings::load()?;
        if mode == AccountDeletionSettings::Off {
            return Ok(Self {
                mode,
                auth_endpoint: None,
                credential: None,
                callers: None,
            });
        }
        let runtime = runtime.ok_or_else(|| {
            anyhow::anyhow!("Managed account deletion requires the canonical Concert listener")
        })?;
        anyhow::ensure!(
            runtime.tls.is_some(),
            "Managed account deletion requires canonical internal mutual TLS"
        );
        let required = |name: &str| {
            std::env::var(name)
                .ok()
                .filter(|v| !v.trim().is_empty())
                .ok_or_else(|| anyhow::anyhow!("{name} required for managed account deletion"))
        };
        let auth_endpoint = required("CONCERT_AUTH_GRPC_ENDPOINT")?;
        let uri: tonic::codegen::http::Uri = auth_endpoint.parse()?;
        anyhow::ensure!(
            uri.scheme_str() == Some("https")
                && uri
                    .authority()
                    .is_some_and(|a| !a.host().is_empty() && !a.as_str().contains('@'))
                && uri.path_and_query().is_none_or(|p| p.as_str() == "/"),
            "Canonical Auth deletion endpoint must be an HTTPS origin"
        );
        Ok(Self {
            mode,
            auth_endpoint: Some(auth_endpoint),
            credential: Some(required("CONCERT_SERVICE_CREDENTIAL")?),
            callers: Some(AuthDeletionCallers::configured()?),
        })
    }
    pub fn managed(&self) -> bool {
        self.mode == AccountDeletionSettings::Managed
    }
    pub fn compose(
        self,
        pool: sqlx::PgPool,
        tls: Option<&platform::grpc_tls::TlsMaterial>,
        world: Option<WorldDeletionHandle>,
        sharing_publisher: Option<Arc<dyn SocialPolicyPublisher>>,
    ) -> anyhow::Result<Option<AccountDeletionService>> {
        if !self.managed() {
            return Ok(None);
        }
        let worlds = world.as_ref().map(|world| vec![world.serving_world()]);
        AccountDeletionRuntime::compose(
            self.mode,
            pool,
            self.auth_endpoint
                .as_deref()
                .expect("captured managed endpoint"),
            tls,
            self.credential
                .as_deref()
                .expect("captured managed credential"),
            self.callers.expect("captured managed callers"),
            LiveDeletionOwners {
                worlds,
                sharing_publisher,
            },
        )?
        .map(AccountDeletionRuntime::service)
        .transpose()
    }
}
