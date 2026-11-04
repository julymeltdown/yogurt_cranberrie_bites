//! Explicit target callback opt-in over Root's existing canonical composition.
//! Parse before opening the database; this creates no listener or background job.
use super::canonical_runtime::CanonicalRuntimeCore;
use crate::notification_live::target_authority::TargetAuthorityService;
use platform::{
    grpc_auth::{CallerRegistry, ServiceIdentityClientInterceptor, Workload},
    grpc_tls::TlsMaterial,
};
use sqlx::PgPool;
use std::collections::BTreeSet;

pub use crate::notification_live::target_authority::POLICY;
const MODE: &str = "CONCERT_NOTIFICATION_TARGET_MODE";

// No Debug/Serialize: settings retain the real caller's accepted credentials.
pub struct NotificationTargetRuntimeSettings {
    callers: CallerRegistry,
}
impl NotificationTargetRuntimeSettings {
    /// `inbox_enabled` and `shared_intro` come from the existing validated Inbox
    /// and shared Intro/Participant startup settings. TLS is the material used
    /// by the existing Concert listener, never an inferred plaintext fallback.
    pub fn from_values(
        mut read: impl FnMut(&str) -> anyhow::Result<Option<String>>,
        inbox_enabled: bool,
        shared_intro: bool,
        tls: Option<&TlsMaterial>,
    ) -> anyhow::Result<Option<Self>> {
        let live = match read(MODE)?.as_deref() {
            None | Some("off") => false,
            Some("live") => true,
            _ => anyhow::bail!("{MODE} must be off or live"),
        };
        if !live {
            anyhow::ensure!(
                !inbox_enabled,
                "Live Notification Inbox requires live {MODE} on the same canonical core"
            );
            // Global workload credentials may belong to independent features.
            // Off mode does not read or validate them.
            return Ok(None);
        }
        anyhow::ensure!(
            inbox_enabled && shared_intro,
            "Live Notification target requires Inbox and shared live Intro/Participant"
        );
        let tls = tls.ok_or_else(|| {
            anyhow::anyhow!("Live Notification target requires actual Concert mutual TLS")
        })?;
        // Parse the actual certificate/key/CA now, without binding a socket.
        // Merely constructing ServerTlsConfig would defer malformed PEM errors.
        tonic::transport::Server::builder().tls_config(tls.server_config()?)?;
        let name = platform::config::service_credential_env_name(Workload::Notification);
        let previous_name =
            platform::config::previous_service_credential_env_name(Workload::Notification);
        let current = read(&name)?;
        let previous = read(&previous_name)?;
        let current = platform::config::resolve_service_credential(current, &name)?
            .ok_or_else(|| anyhow::anyhow!("{name} is required for live Notification target"))?;
        let previous = platform::config::resolve_service_credential(previous, &previous_name)?;
        let mut target_credentials = BTreeSet::new();
        for token in std::iter::once(&current).chain(previous.iter()) {
            ServiceIdentityClientInterceptor::new(Workload::Notification, token)?;
            anyhow::ensure!(
                target_credentials.insert(token.clone()),
                "Notification target current and previous credentials must differ"
            );
        }
        // Participant validates its own role set. Reject reuse with this newly
        // admitted caller without broadening or replacing that registry.
        for role in Workload::ALL {
            if role == Workload::Notification {
                continue;
            }
            for name in [
                platform::config::service_credential_env_name(role),
                platform::config::previous_service_credential_env_name(role),
            ] {
                if let Some(token) =
                    platform::config::resolve_service_credential(read(&name)?, &name)?
                {
                    anyhow::ensure!(
                        !target_credentials.contains(&token),
                        "Notification target credentials must differ from other workload credentials"
                    );
                }
            }
        }
        Ok(Some(Self {
            callers: CallerRegistry::new().register_with_previous(
                Workload::Notification,
                &current,
                previous.as_deref(),
            )?,
        }))
    }
    pub fn load(
        inbox_enabled: bool,
        shared_intro: bool,
        tls: Option<&TlsMaterial>,
    ) -> anyhow::Result<Option<Self>> {
        Self::from_values(
            |name| match std::env::var(name) {
                Ok(value) => Ok(Some(value)),
                Err(std::env::VarError::NotPresent) => Ok(None),
                Err(_) => anyhow::bail!("{name} must contain valid Unicode"),
            },
            inbox_enabled,
            shared_intro,
            tls,
        )
    }
    /// Pass the SAME pool used to compose this existing canonical core. Root
    /// retains listener/mTLS mounting and supervises the existing recovery worker.
    /// The callback resolves the original registry; no second core is constructed.
    pub fn compose(self, pool: PgPool, core: &CanonicalRuntimeCore) -> NotificationTargetRuntime {
        NotificationTargetRuntime {
            service: TargetAuthorityService::new(pool, core.registry()),
            callers: self.callers,
        }
    }
}
pub struct NotificationTargetRuntime {
    pub service: TargetAuthorityService,
    pub callers: CallerRegistry,
}
