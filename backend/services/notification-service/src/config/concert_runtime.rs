//! Explicit Concert opt-in, parsed before database or listener acquisition.
//! Inherited Notification callers and their method policy remain unchanged.
use crate::concert::{authority::NotificationInboxService, delivery::NotificationDeliveryIngress};
use platform::{
    grpc_auth::{CallerRegistry, ServiceIdentityClientInterceptor, Workload},
    grpc_tls::TlsMaterial,
};
use sqlx::PgPool;
use std::{collections::BTreeSet, sync::Arc};

// No Debug: runtime settings contain workload credentials.
pub struct ConcertSettings {
    auth: String,
    friend: String,
    root: String,
    credential: String,
    pub callers: CallerRegistry,
}
impl ConcertSettings {
    pub fn parse(
        mut read: impl FnMut(&str) -> anyhow::Result<Option<String>>,
        has_tls: bool,
    ) -> anyhow::Result<Option<Self>> {
        match read("NOTIFICATION_CONCERT_MODE")?.as_deref() {
            None | Some("off") => return Ok(None),
            Some("live") => {}
            _ => anyhow::bail!("NOTIFICATION_CONCERT_MODE must be off or live"),
        }
        anyhow::ensure!(has_tls, "Concert Notification serving requires mutual TLS");
        let auth = endpoint(&mut read, "NOTIFICATION_CONCERT_AUTH_GRPC_ENDPOINT")?;
        let friend = endpoint(&mut read, "NOTIFICATION_CONCERT_FRIEND_GRPC_ENDPOINT")?;
        let root = endpoint(&mut read, "NOTIFICATION_CONCERT_ROOT_GRPC_ENDPOINT")?;
        let credential = credential_for(&mut read, Workload::Notification)?;
        let mut secrets = BTreeSet::from([credential.clone()]);
        let mut callers = CallerRegistry::new();
        // Concert alone invokes inbox/ingress; only Auth/Friend recover their
        // registered native children. Never add Concert to inherited policy.
        for role in [Workload::Concert, Workload::Auth, Workload::Friend] {
            let current = credential_for(&mut read, role)?;
            let previous_name = platform::config::previous_service_credential_env_name(role);
            let previous = platform::config::resolve_service_credential(
                read(&previous_name)?,
                &previous_name,
            )?;
            anyhow::ensure!(
                secrets.insert(current.clone()),
                "Live Concert workload credentials must differ"
            );
            if let Some(previous) = &previous {
                ServiceIdentityClientInterceptor::new(role, previous)?;
                anyhow::ensure!(
                    secrets.insert(previous.clone()),
                    "Live Concert workload credentials must differ"
                );
            }
            callers = callers.register_with_previous(role, &current, previous.as_deref())?;
        }
        Ok(Some(Self {
            auth,
            friend,
            root,
            credential,
            callers,
        }))
    }
    pub fn load(has_tls: bool) -> anyhow::Result<Option<Self>> {
        Self::parse(
            |key| match std::env::var(key) {
                Ok(value) => Ok(Some(value)),
                Err(std::env::VarError::NotPresent) => Ok(None),
                Err(_) => anyhow::bail!("{key} must be Unicode"),
            },
            has_tls,
        )
    }
    pub async fn connect(
        &self,
        pool: PgPool,
        tls: &TlsMaterial,
    ) -> anyhow::Result<ConcertServices> {
        let targets = Arc::new(crate::concert::target_client::RootTargetClient::connect(
            &self.root,
            &self.credential,
            tls,
        )?);
        let inbox = NotificationInboxService::connect(
            pool.clone(),
            &self.auth,
            &self.friend,
            &self.credential,
            Some(tls),
            Some(targets),
        )
        .await?;
        Ok(ConcertServices {
            inbox,
            delivery: NotificationDeliveryIngress::new(pool),
        })
    }
}
pub struct ConcertServices {
    pub inbox: NotificationInboxService,
    pub delivery: NotificationDeliveryIngress,
}
fn credential_for(
    read: &mut impl FnMut(&str) -> anyhow::Result<Option<String>>,
    role: Workload,
) -> anyhow::Result<String> {
    let name = platform::config::service_credential_env_name(role);
    let value = platform::config::resolve_service_credential(read(&name)?, &name)?
        .ok_or_else(|| anyhow::anyhow!("{name} is required for live Concert Notification"))?;
    ServiceIdentityClientInterceptor::new(role, &value)?;
    Ok(value)
}
fn endpoint(
    read: &mut impl FnMut(&str) -> anyhow::Result<Option<String>>,
    name: &str,
) -> anyhow::Result<String> {
    let value = read(name)?
        .ok_or_else(|| anyhow::anyhow!("{name} is required for live Concert Notification"))?;
    let uri: axum::http::Uri = value
        .parse()
        .map_err(|_| anyhow::anyhow!("Invalid {name}"))?;
    anyhow::ensure!(
        uri.scheme_str() == Some("https")
            && uri.authority().is_some_and(|a| !a.as_str().contains('@'))
            && uri.path() == "/"
            && uri.query().is_none(),
        "{name} requires an HTTPS service origin"
    );
    Ok(value)
}
