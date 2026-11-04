//! Composition only. Root mounts the service on its existing mTLS listener and
//! supplies the actual serving World/socket and Sharing publication handles.
use crate::{
    account_private_data::PrivateDataAggregateCoordinator,
    adapters::{
        out::relationship_authority::deletion_source::AuthDeletionSource,
        r#in::{
            deletion_rpc::{self, ApplyServer, DeletionOwner, DeletionOwners},
            world_ws::WorldSocketState,
        },
    },
    social_live::{
        account_deletion::SharingDeletion,
        invalidation::{SocialInvalidationSink, SocialPolicyPublisher},
    },
    world::{
        account_deletion::{
            DeletionAcknowledgement, DeletionAuthority, DeletionFailure, TicketDeletion,
            WorldDeletionCommand,
        },
        account_sessions::{SessionDeletion, SessionOwner},
        admission_bridge::AdmissionBridge,
    },
};
use platform::{
    grpc_auth::{CallerRegistry, Workload},
    grpc_method_policy::GuardedService,
    grpc_tls::TlsMaterial,
};
use sqlx::PgPool;
use std::sync::Arc;

pub type AccountDeletionService = deletion_rpc::Deadline<
    GuardedService<
        deletion_rpc::proto::account_deletion_service_server::AccountDeletionServiceServer<
            ApplyServer,
        >,
    >,
>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountDeletionSettings {
    Off,
    Managed,
}
impl AccountDeletionSettings {
    pub fn load() -> anyhow::Result<Self> {
        match std::env::var("CONCERT_ACCOUNT_DELETION_MODE") {
            Ok(v) => Self::from_value(Some(&v)),
            Err(std::env::VarError::NotPresent) => Self::from_value(None),
            Err(_) => anyhow::bail!("CONCERT_ACCOUNT_DELETION_MODE must contain valid Unicode"),
        }
    }
    pub fn from_value(value: Option<&str>) -> anyhow::Result<Self> {
        match value {
            None | Some("off") => Ok(Self::Off),
            Some("managed") => Ok(Self::Managed),
            _ => anyhow::bail!("CONCERT_ACCOUNT_DELETION_MODE must be off or managed"),
        }
    }
}

/// A borrow of the existing serving state, never a separately created registry.
/// SessionOwner::new verifies that these are the same bridge/runtime/transport.
pub struct ServingWorld<'a> {
    pub bridge: Arc<AdmissionBridge>,
    pub sockets: &'a WorldSocketState,
}
/// Missing dependencies remain unavailable. A configured PrivateData coordinator
/// still refuses completion until its real category owners are all terminal.
#[derive(Default)]
pub struct LiveDeletionOwners<'a> {
    pub worlds: Option<Vec<ServingWorld<'a>>>,
    pub sharing_publisher: Option<Arc<dyn SocialPolicyPublisher>>,
    pub private_data: Option<Arc<PrivateDataAggregateCoordinator>>,
}

/// An Auth-only registry built from the canonical configured credentials, with
/// the same bounded current/previous rotation as other internal owner services.
pub struct AuthDeletionCallers(CallerRegistry);
impl AuthDeletionCallers {
    pub fn configured() -> anyhow::Result<Self> {
        Ok(Self(platform::config::build_caller_registry(
            true,
            &[Workload::Auth],
            &[Workload::Auth],
        )?))
    }
    pub fn from_credentials(current: &str, previous: Option<&str>) -> anyhow::Result<Self> {
        Ok(Self(CallerRegistry::new().register_with_previous(
            Workload::Auth,
            current,
            previous,
        )?))
    }
}

pub struct AccountDeletionRuntime {
    owners: DeletionOwners,
    authority: Arc<dyn DeletionAuthority>,
    callers: CallerRegistry,
}
impl AccountDeletionRuntime {
    /// Reuse the actual canonical Auth endpoint, Concert identity/TLS and pool.
    /// CallerRegistry must be the configured Auth registry (rotation preserved);
    /// the service's method policy accepts only authenticated Auth regardless of
    /// any other service registered on the shared listener.
    pub fn compose(
        settings: AccountDeletionSettings,
        pool: PgPool,
        auth_endpoint: &str,
        tls: Option<&TlsMaterial>,
        concert_credential: &str,
        callers: AuthDeletionCallers,
        live: LiveDeletionOwners<'_>,
    ) -> anyhow::Result<Option<Self>> {
        Self::compose_with_private_data(
            settings,
            pool,
            auth_endpoint,
            tls,
            concert_credential,
            callers,
            live,
            |_, _| Ok(None),
        )
    }

    /// The single composition owner may construct the real coordinator using
    /// this SAME pool and canonical Auth source. The closure never runs in off
    /// mode, and cannot replace an already supplied coordinator.
    pub fn compose_with_private_data<F>(
        settings: AccountDeletionSettings,
        pool: PgPool,
        auth_endpoint: &str,
        tls: Option<&TlsMaterial>,
        concert_credential: &str,
        callers: AuthDeletionCallers,
        live: LiveDeletionOwners<'_>,
        private_data: F,
    ) -> anyhow::Result<Option<Self>>
    where
        F: FnOnce(
            PgPool,
            Arc<dyn DeletionAuthority>,
        ) -> anyhow::Result<Option<Arc<PrivateDataAggregateCoordinator>>>,
    {
        if settings == AccountDeletionSettings::Off {
            return Ok(None);
        }
        let uri: tonic::codegen::http::Uri = auth_endpoint.parse().map_err(|_| {
            anyhow::anyhow!("Canonical Auth deletion endpoint must be an explicit HTTPS origin")
        })?;
        anyhow::ensure!(
            uri.scheme_str() == Some("https")
                && uri
                    .authority()
                    .is_some_and(|a| !a.host().is_empty() && !a.as_str().contains('@'))
                && uri.path_and_query().is_none_or(|p| p.as_str() == "/"),
            "Canonical Auth deletion endpoint must be an explicit HTTPS origin"
        );
        let tls = tls.ok_or_else(|| {
            anyhow::anyhow!("Managed account deletion requires canonical internal mTLS")
        })?;
        let authority: Arc<dyn DeletionAuthority> = Arc::new(AuthDeletionSource::connect(
            auth_endpoint,
            tls,
            concert_credential,
        )?);
        let constructed = private_data(pool.clone(), authority.clone())?;
        anyhow::ensure!(
            live.private_data.is_none() || constructed.is_none(),
            "PrivateData coordinator supplied twice"
        );
        let private_data = live
            .private_data
            .or(constructed)
            .map(|owner| Arc::new(PrivateDataAdapter(owner)) as Arc<dyn DeletionOwner>);
        let tickets: Arc<dyn DeletionOwner> =
            Arc::new(TicketDeletion::new(pool.clone(), authority.clone()));
        let sessions = match live.worlds {
            None => None,
            Some(worlds) => {
                let owners = worlds
                    .into_iter()
                    .map(|w| SessionOwner::new(w.bridge, w.sockets))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|_| anyhow::anyhow!("Serving World deletion ownership is invalid"))?;
                let owner = SessionDeletion::new(pool.clone(), authority.clone(), owners)
                    .map_err(|_| anyhow::anyhow!("Serving World deletion ownership is invalid"))?;
                Some(Arc::new(SessionAdapter(owner)) as Arc<dyn DeletionOwner>)
            }
        };
        let sharing = live.sharing_publisher.map(|publisher| {
            Arc::new(SharingDeletion::new(
                pool.clone(),
                authority.clone(),
                SocialInvalidationSink::new(pool.clone(), publisher),
            )) as Arc<dyn DeletionOwner>
        });
        Ok(Some(Self {
            owners: DeletionOwners {
                tickets: Some(tickets),
                sessions,
                sharing,
                private_data,
            },
            authority,
            callers: callers.0,
        }))
    }
    pub fn service(self) -> anyhow::Result<AccountDeletionService> {
        deletion_rpc::guarded_server(self.owners, self.authority, self.callers)
    }
}
struct PrivateDataAdapter(Arc<PrivateDataAggregateCoordinator>);
#[async_trait::async_trait]
impl DeletionOwner for PrivateDataAdapter {
    async fn apply(
        &self,
        command: WorldDeletionCommand,
        original_deadline: tokio::time::Instant,
    ) -> Result<DeletionAcknowledgement, DeletionFailure> {
        crate::world::account_deletion::check_deadline(original_deadline)?;
        let receipt = self.0.apply(command.clone(), original_deadline).await?;
        crate::world::account_deletion::check_deadline(original_deadline)?;
        if !receipt.verifies(&command) {
            return Err(DeletionFailure::Unavailable);
        }
        Ok(DeletionAcknowledgement {
            command,
            fence_revision: receipt.fence_revision,
        })
    }
}
struct SessionAdapter(SessionDeletion);
#[async_trait::async_trait]
impl DeletionOwner for SessionAdapter {
    async fn apply(
        &self,
        command: WorldDeletionCommand,
        original_deadline: tokio::time::Instant,
    ) -> Result<DeletionAcknowledgement, DeletionFailure> {
        self.0.apply(command, original_deadline).await
    }
}
