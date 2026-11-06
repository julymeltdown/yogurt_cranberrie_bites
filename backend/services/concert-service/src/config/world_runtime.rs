//! Strict opt-in for actual World serving. Product files retain the bootstrap
//! validator; synthetic metadata is never promoted by this configuration path.
use crate::{
    adapters::out::{
        relationship_authority::{
            grpc::GrpcRelationshipAuthority,
            invalidation::{InvalidationOwner, InvalidationSource},
            world_auth::{GrpcWorldAuthReader, WorldAuthRead},
        },
        world_profile::GrpcWorldProfileReader,
    },
    adapters::r#in::world_ws::{self, SocketTransport, WorldSocketState},
    bootstrap::{BootstrapFeatures, BootstrapService, ProductReleaseBundle},
    world::{
        admission_bridge::AdmissionBridge,
        ingress::OriginPolicy,
        invalidation::runtime::RemoteInvalidationRuntime,
        membership::ClockAnchor,
        production_owners::ProductionOwners,
        recipient_owners::WorldRecipientReader,
        runtime::{RoomSpec, WorldRuntime},
        simulation::ResourcePin,
    },
};
use anyhow::Context;
use concert_application::world_ticket::OwnerIdentity;
use concert_domain::{
    world::{Limits, WorldMap},
    AssetId, EventId, Revision,
};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfiguredRoom {
    room_id: Uuid,
    capacity: usize,
}
/// Opaque production dependencies keep the sealed Auth port inside this crate.
/// No fixture or public subject constructor can manufacture these startup ports.
pub struct WorldProductionPorts {
    profile: GrpcWorldProfileReader,
    relationship: GrpcRelationshipAuthority,
    auth: Arc<dyn WorldAuthRead>,
    auth_invalidation: InvalidationSource,
    friend_invalidation: InvalidationSource,
}
pub struct WorldRuntimeSettings {
    pub address: SocketAddr,
    origins: OriginPolicy,
    owner: Uuid,
    consumer: Uuid,
    event: EventId,
    rooms: Vec<ConfiguredRoom>,
    release: Arc<concert_application::bootstrap::ValidatedProductRelease>,
}
fn read(name: &str) -> anyhow::Result<Option<String>> {
    match std::env::var(name) {
        Ok(v) => Ok(Some(v)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(_) => anyhow::bail!("{name} must contain valid Unicode"),
    }
}
impl WorldRuntimeSettings {
    pub fn load() -> anyhow::Result<Option<Self>> {
        Self::settings(read)
    }
    pub fn settings(
        mut read: impl FnMut(&str) -> anyhow::Result<Option<String>>,
    ) -> anyhow::Result<Option<Self>> {
        let address = read("CONCERT_WORLD_LISTEN_ADDR")?;
        let names = [
            "CONCERT_WORLD_ALLOWED_ORIGINS",
            "CONCERT_WORLD_OWNER_ID",
            "CONCERT_WORLD_EVENT_ID",
            "CONCERT_WORLD_ROOMS_JSON",
            "CONCERT_WORLD_INVALIDATION_CONSUMER_ID",
            "CONCERT_WORLD_RESOURCE_ROOT",
            "CONCERT_WORLD_SOURCE_BINDING_FILE",
        ];
        let values: Vec<_> = names
            .iter()
            .map(|n| read(n))
            .collect::<anyhow::Result<_>>()?;
        let Some(address) = address else {
            anyhow::ensure!(
                values.iter().all(Option::is_none),
                "World settings require CONCERT_WORLD_LISTEN_ADDR"
            );
            return Ok(None);
        };
        let values: Vec<_> = values
            .into_iter()
            .enumerate()
            .map(|(i, v)| {
                v.filter(|v| !v.trim().is_empty())
                    .ok_or_else(|| anyhow::anyhow!("{} is required for World", names[i]))
            })
            .collect::<anyhow::Result<_>>()?;
        let address: SocketAddr = address
            .parse()
            .map_err(|_| anyhow::anyhow!("CONCERT_WORLD_LISTEN_ADDR is invalid"))?;
        let local = match read("CONCERT_RUNTIME_MODE")?.as_deref() {
            Some("local") => true,
            None | Some("production") => false,
            _ => anyhow::bail!("CONCERT_RUNTIME_MODE is invalid"),
        };
        anyhow::ensure!(
            if local {
                address.ip().is_loopback()
            } else {
                address.port() != 0
            },
            "World listener address is unsafe for runtime mode"
        );
        let origins: Vec<String> = serde_json::from_str(&values[0])
            .map_err(|_| anyhow::anyhow!("World origins must be a JSON array"))?;
        let origins = OriginPolicy::new(origins, local)
            .map_err(|_| anyhow::anyhow!("World origins are invalid"))?;
        let canonical = |v: &str| -> anyhow::Result<Uuid> {
            let id: Uuid = v
                .parse()
                .map_err(|_| anyhow::anyhow!("World identity is invalid"))?;
            anyhow::ensure!(
                !id.is_nil() && id.to_string() == v,
                "World identity must be canonical"
            );
            Ok(id)
        };
        let owner = canonical(&values[1])?;
        let consumer = canonical(&values[4])?;
        anyhow::ensure!(
            consumer == owner,
            "World replay consumer must equal the stable World owner identity"
        );
        let event = EventId::from_uuid(canonical(&values[2])?);
        anyhow::ensure!(
            values[3].len() <= 8192,
            "World room configuration is too large"
        );
        let rooms: Vec<ConfiguredRoom> = serde_json::from_str(&values[3])
            .map_err(|_| anyhow::anyhow!("World rooms are invalid"))?;
        let mut seen = BTreeSet::new();
        anyhow::ensure!(
            !rooms.is_empty()
                && rooms.len() <= 16
                && rooms.iter().all(|r| !r.room_id.is_nil()
                    && r.capacity > 0
                    && r.capacity <= 50
                    && seen.insert(r.room_id)),
            "World rooms must be unique and bounded"
        );
        let files: Vec<_> = super::bootstrap_runtime::RELEASE_SETTINGS
            .iter()
            .map(|n| {
                read(n)?
                    .filter(|v| !v.trim().is_empty())
                    .map(std::path::PathBuf::from)
                    .ok_or_else(|| anyhow::anyhow!("{n} is required for World product serving"))
            })
            .collect::<anyhow::Result<_>>()?;
        let release = ProductReleaseBundle::from_source_bound_files(
            &files[0],
            &files[1],
            &files[2],
            [&files[3], &files[4], &files[5]],
            BootstrapFeatures {
                world: true,
                ..Default::default()
            },
            std::path::Path::new(&values[5]),
            std::path::Path::new(&values[6]),
        )
        .map_err(|_| anyhow::anyhow!("World product release is unavailable"))?;
        Ok(Some(Self {
            address,
            origins,
            owner,
            consumer,
            event,
            rooms,
            release,
        }))
    }
    /// Reuse the exact World-featured release for C03, including its source digest.
    pub fn validated_release(
        &self,
    ) -> Arc<concert_application::bootstrap::ValidatedProductRelease> {
        self.release.clone()
    }
    /// Preserve the validated source-bound World release and all owner/listener/
    /// replay identity. C03 and World receive this same final Arc and digest.
    pub fn with_bootstrap_features(
        mut self,
        features: &super::bootstrap_features::ConfiguredBootstrapFeatures,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            features.world_enabled(),
            "Configured World availability is required"
        );
        self.release = features.bind_release(self.release)?;
        Ok(self)
    }
    /// Composes the actual DB bridge, 20Hz owner and socket server with supplied
    /// qualified live owner ports. Absence is rejected before binding a listener.
    pub async fn serve(
        self,
        pool: sqlx::PgPool,
        ports: WorldProductionPorts,
    ) -> anyhow::Result<()> {
        self.serve_inner(pool, ports, None, SocketTransport::default(), None, None)
            .await
    }
    /// Startup barrier for a supervised C03 listener. Failure drops the sender;
    /// success is not a persistent authority grant or an ongoing health signal.
    pub async fn serve_with_ready(
        self,
        pool: sqlx::PgPool,
        ports: WorldProductionPorts,
        ready: tokio::sync::oneshot::Sender<()>,
    ) -> anyhow::Result<()> {
        self.serve_inner(
            pool, ports, Some(ready), SocketTransport::default(), None, None,
        )
        .await
    }
    /// Supply the single incumbent socket registry with its canonical policy
    /// baseline installed before any runtime or listener clones are created.
    pub async fn serve_with_ready_and_transport(
        self,
        pool: sqlx::PgPool,
        ports: WorldProductionPorts,
        ready: tokio::sync::oneshot::Sender<()>,
        transport: SocketTransport,
    ) -> anyhow::Result<()> {
        self.serve_inner(pool, ports, Some(ready), transport, None, None)
            .await
    }
    /// Publish this exact native owner's local invalidation leg only after the
    /// normal startup barrier succeeds. The receiver grants no queue-wide ACK.
    pub async fn serve_with_ready_and_social(
        self,
        pool: sqlx::PgPool,
        ports: WorldProductionPorts,
        ready: tokio::sync::oneshot::Sender<()>,
        transport: SocketTransport,
        local: tokio::sync::oneshot::Sender<
            Arc<crate::world::invalidation::WorldInvalidationConsumer>,
        >,
    ) -> anyhow::Result<()> {
        self.serve_inner(pool, ports, Some(ready), transport, Some(local), None)
            .await
    }
    /// Deletion-only composition for a World without the Social publisher. It
    /// still publishes the exact incumbent bridge/socket registry after the
    /// normal startup checks and under the same serving task supervision.
    pub async fn serve_with_ready_and_deletion(
        self,
        pool: sqlx::PgPool,
        ports: WorldProductionPorts,
        ready: tokio::sync::oneshot::Sender<()>,
        deletion: tokio::sync::oneshot::Sender<super::deletion_world_binding::ServingWorldBinding>,
    ) -> anyhow::Result<()> {
        self.serve_inner(
            pool,
            ports,
            Some(ready),
            SocketTransport::default(),
            None,
            Some(deletion),
        )
        .await
    }
    /// Opt-in account deletion composition receives the exact incumbent bridge
    /// and socket registry after the normal startup checks. The receiver must
    /// also await `ready` and retain the original serving task's supervision.
    /// A dropped receiver is a startup failure and uses that task's cleanup.
    pub async fn serve_with_ready_social_and_deletion(
        self,
        pool: sqlx::PgPool,
        ports: WorldProductionPorts,
        ready: tokio::sync::oneshot::Sender<()>,
        transport: SocketTransport,
        local: tokio::sync::oneshot::Sender<
            Arc<crate::world::invalidation::WorldInvalidationConsumer>,
        >,
        deletion: tokio::sync::oneshot::Sender<super::deletion_world_binding::ServingWorldBinding>,
    ) -> anyhow::Result<()> {
        self.serve_inner(
            pool, ports, Some(ready), transport, Some(local), Some(deletion),
        )
        .await
    }
    async fn serve_inner(
        self,
        pool: sqlx::PgPool,
        ports: WorldProductionPorts,
        ready: Option<tokio::sync::oneshot::Sender<()>>,
        transport: SocketTransport,
        local: Option<
            tokio::sync::oneshot::Sender<
                Arc<crate::world::invalidation::WorldInvalidationConsumer>,
            >,
        >,
        deletion: Option<
            tokio::sync::oneshot::Sender<super::deletion_world_binding::ServingWorldBinding>,
        >,
    ) -> anyhow::Result<()> {
        let template: serde_json::Value =
            serde_json::from_slice(self.release.release().bootstrap_json())?;
        anyhow::ensure!(
            template["event"]["event_id"].as_str() == Some(self.event.uuid().to_string().as_str()),
            "World event and release differ"
        );
        let map = concert_domain::place::parse_stored_release(&serde_json::to_vec(
            &template["map_manifest"],
        )?)?;
        let avatar = &template["avatar_manifest"];
        let asset = |v: &serde_json::Value| {
            AssetId::new(
                v.as_str()
                    .ok_or_else(|| anyhow::anyhow!("World asset is invalid"))?
                    .to_owned(),
            )
            .map_err(anyhow::Error::from)
        };
        let resources = ResourcePin {
            map_id: AssetId::new(map.map_id.clone())?,
            map_revision: Revision::new(map.map_revision)?,
            avatar_manifest_id: asset(&avatar["manifest_id"])?,
            avatar_manifest_version: Revision::new(
                avatar["manifest_version"]
                    .as_i64()
                    .ok_or_else(|| anyhow::anyhow!("World avatar version is invalid"))?,
            )?,
        };
        let mut epochs = BTreeMap::new();
        let mut specs = Vec::new();
        for room in self.rooms {
            let epoch = Uuid::new_v4();
            epochs.insert(room.room_id, epoch);
            specs.push(RoomSpec {
                room_id: room.room_id,
                room_epoch: epoch,
                resources: resources.clone(),
                map: WorldMap::from_geometry(&map.geometry)?,
                limits: Limits::new(160.0, 48.0, 250)?,
                capacity: room.capacity,
                spawn_id: map.geometry.default_spawn_id.clone(),
            });
        }
        let anchor = ClockAnchor {
            monotonic_ms: 0,
            utc: chrono::Utc::now(),
        };
        let started = Instant::now();
        let bootstrap = Arc::new(BootstrapService::new(pool.clone(), self.release));
        let owners = ProductionOwners::new(
            pool.clone(),
            ports.profile,
            WorldRecipientReader::new(pool.clone(), ports.relationship),
            concert_domain::avatar::AvatarCatalog::from_validated_manifest(avatar)
                .map_err(|_| anyhow::anyhow!("World avatar catalog is invalid"))?,
        );
        let bridge = AdmissionBridge::new_production(
            pool.clone(),
            bootstrap,
            owners,
            ports.auth,
            OwnerIdentity::new(self.owner.to_string(), Uuid::new_v4())?,
            self.event,
            epochs,
            resources,
            anchor.clone(),
            started,
        )?;
        let mut remote = RemoteInvalidationRuntime::new(
            pool,
            self.consumer,
            ports.auth_invalidation,
            ports.friend_invalidation,
            vec![bridge.clone()],
        )
        .map_err(|_| anyhow::anyhow!("World replay construction failed"))?;
        remote
            .bootstrap()
            .await
            .map_err(|_| anyhow::anyhow!("World replay bootstrap unavailable"))?;
        let mut runtime = WorldRuntime::new(
            self.event,
            self.owner,
            anchor,
            specs,
            bridge.clone(),
            bridge.clone(),
            transport.clone(),
        )
        .map_err(|_| anyhow::anyhow!("World owner construction failed"))?;
        runtime.configure_motion_profile(&map.geometry_hash)
            .map_err(|_| anyhow::anyhow!("World motion geometry binding failed"))?;
        let (clock, origin_ms) = runtime.clock_origin();
        // Claim only after geometry/runtime construction, so startup's owning
        // lease is fresh when the native owner and public barrier become ready.
        tokio::time::timeout(Duration::from_secs(2), bridge.claim_rooms()).await??;
        let listener = tokio::net::TcpListener::bind(self.address).await?;
        let (handle, owner_task) = runtime.spawn();
        let handle = Arc::new(handle);
        let state = WorldSocketState::new(
            self.origins,
            transport.clone(),
            bridge.clone(),
            handle.clone(),
            clock,
            origin_ms,
        );
        println!("CONCERT_WORLD_LISTENER={}", listener.local_addr()?);
        let authority = async {
            let mut tick = tokio::time::interval(Duration::from_secs(1));
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    biased;
                    _ = platform::shutdown::global().cancelled()=>return Ok::<(),anyhow::Error>(()),
                    _ = tick.tick()=>{
                        tokio::time::timeout(Duration::from_secs(2),bridge.maintain())
                            .await.context("World authority maintenance timed out")?
                            .context("World authority maintenance failed")?;
                    }
                }
            }
        };
        let server = axum::serve(listener, world_ws::router(state.clone()))
            .with_graceful_shutdown(platform::shutdown::global().cancelled());
        let startup = tokio::time::timeout(Duration::from_secs(2), async {
            handle
                .ready()
                .await
                .map_err(|_| anyhow::anyhow!("World owner startup unavailable"))?;
            // Room claims/binding/owner startup may outlast the initial replay
            // freshness window. Both actual journals must be caught up again.
            while !remote
                .poll()
                .await
                .map_err(|_| anyhow::anyhow!("World startup replay unavailable"))?
            {
                tokio::task::yield_now().await;
            }
            bridge
                .startup_ready()
                .map_err(|_| anyhow::anyhow!("World startup authority unavailable"))?;
            anyhow::ensure!(
                !owner_task.is_finished() && !platform::shutdown::global().is_requested(),
                "World stopped during startup"
            );
            if let Some(local) = local {
                local
                    .send(Arc::new(
                        crate::world::invalidation::WorldInvalidationConsumer::new(bridge.clone()),
                    ))
                    .map_err(|_| anyhow::anyhow!("World local invalidation receiver closed"))?;
            }
            if let Some(deletion) = deletion {
                let binding = super::deletion_world_binding::ServingWorldBinding::new(
                    bridge.clone(),
                    state.clone(),
                )
                .map_err(|_| anyhow::anyhow!("World deletion serving ownership is invalid"))?;
                deletion
                    .send(binding)
                    .map_err(|_| anyhow::anyhow!("World deletion serving receiver closed"))?;
            }
            if let Some(ready) = ready {
                ready
                    .send(())
                    .map_err(|_| anyhow::anyhow!("World startup receiver closed"))?;
            }
            Ok::<(), anyhow::Error>(())
        })
        .await
        .map_err(|_| anyhow::anyhow!("World startup readiness timed out"))
        .and_then(|result| result);
        // Startup failure after spawn follows the same explicit native/SQL
        // cleanup path as later server, authority or replay failure.
        let result = match startup {
            Ok(()) => tokio::select! {
                result=async { server.await }=>result.context("World socket server failed"),
                result=authority=>result,
                result=remote.run()=>result.map_err(|_|anyhow::anyhow!("World replay readiness lost")),
            },
            Err(error) => Err(error),
        };
        if let Err(error) = &result {
            // Only the fixed top-level stage is printed, never a response body,
            // credential, identity or SQL error. Preserve it before cleanup.
            eprintln!("World supervised failure before cleanup: {error}");
        }
        let cleanup = async {
            let remote_cleanup = remote.shutdown().await;
            // Dropping all listener states closes sockets and the final handle stops
            // the owner independently of a full command queue. Await its cleanup report.
            transport.close_all(crate::world::ingress::SafeClose::UNAVAILABLE);
            handle.request_stop();
            drop(handle);
            let report = tokio::time::timeout(Duration::from_secs(2), owner_task)
                .await
                .context("World owner shutdown timed out")?
                .context("World owner shutdown task failed")?;
            for b in report.remaining_cleanup {
                let _ = crate::world::membership::CanonicalAuthorityPort::release(&bridge, &b);
            }
            let pending = tokio::time::timeout(Duration::from_secs(2), bridge.cleanup())
                .await
                .context("World persistent cleanup timed out")?
                .context("World persistent cleanup failed")?;
            anyhow::ensure!(
                pending == 0,
                "World shutdown has pending persistent membership cleanup"
            );
            remote_cleanup.map_err(|_| anyhow::anyhow!("World replay shutdown incomplete"))?;
            Ok(())
        }
        .await;
        finish_world_run(result, cleanup)
    }
}
/// Cleanup failure must not erase the supervised failure that initiated shutdown.
fn finish_world_run(result: anyhow::Result<()>, cleanup: anyhow::Result<()>) -> anyhow::Result<()> {
    if let Err(error) = &cleanup {
        eprintln!("World shutdown failure: {error}");
    }
    match (result, cleanup) {
        (Err(error), Err(_)) => {
            Err(error.context("World shutdown also failed; see stage diagnostic"))
        }
        (Err(error), Ok(())) => Err(error),
        (Ok(()), cleanup) => cleanup,
    }
}

#[cfg(test)]
mod shutdown_result_tests {
    use super::finish_world_run;

    #[test]
    fn cleanup_failure_preserves_original_supervised_error() {
        let primary =
            anyhow::Error::new(concert_application::world_ticket::WorldError::OwnerFenced)
                .context("World authority maintenance failed");
        let result = finish_world_run(Err(primary), Err(anyhow::anyhow!("cleanup unavailable")))
            .unwrap_err();
        assert!(result
            .downcast_ref::<concert_application::world_ticket::WorldError>()
            .is_some_and(|error| matches!(
                error,
                concert_application::world_ticket::WorldError::OwnerFenced
            )));
        assert!(format!("{result:#}").contains("World authority maintenance failed"));
    }

    #[test]
    fn successful_runtime_still_reports_cleanup_failure() {
        assert_eq!(
            finish_world_run(Ok(()), Err(anyhow::anyhow!("cleanup unavailable")))
                .unwrap_err()
                .to_string(),
            "cleanup unavailable"
        );
        assert!(finish_world_run(Ok(()), Ok(())).is_ok());
    }
}
fn endpoint(name: &str, value: &str, production: bool) -> anyhow::Result<()> {
    let uri: tonic::codegen::http::Uri = value
        .parse()
        .map_err(|_| anyhow::anyhow!("{name} is invalid"))?;
    anyhow::ensure!(
        matches!(uri.scheme_str(), Some("http" | "https"))
            && uri.authority().is_some_and(|a| !a.as_str().contains('@'))
            && uri.host().is_some_and(|h| !h.is_empty())
            && matches!(uri.path_and_query().map(|p| p.as_str()), None | Some("/")),
        "{name} must be an internal gRPC origin"
    );
    platform::config::require_production_https_url(production, name, value)?;
    Ok(())
}
/// Validate explicit typed owner and sealed replay configuration before any DB
/// or listener opens. Serving bootstraps durable replay with publication closed;
/// only caught-up readiness and exact live SQL permits allow native handoffs.
pub fn production_owners() -> anyhow::Result<WorldProductionPorts> {
    use platform::{
        actor_delegation::{relationship_read::RelationshipReadSigner, ActorDelegationVerifier},
        grpc_auth::Workload,
    };
    let production = match read("CONCERT_RUNTIME_MODE")?.as_deref() {
        Some("local") => false,
        None | Some("production") => true,
        _ => anyhow::bail!("CONCERT_RUNTIME_MODE is invalid"),
    };
    let required = |name| {
        read(name)?
            .filter(|v| !v.trim().is_empty())
            .ok_or_else(|| anyhow::anyhow!("{name} is required for World owner composition"))
    };
    let auth_endpoint = required("CONCERT_AUTH_GRPC_ENDPOINT")?;
    let friend_endpoint = required("CONCERT_FRIEND_GRPC_ENDPOINT")?;
    let profile_endpoint = required("CONCERT_PROFILE_GRPC_ENDPOINT")?;
    for (name, value) in [
        ("CONCERT_AUTH_GRPC_ENDPOINT", &auth_endpoint),
        ("CONCERT_FRIEND_GRPC_ENDPOINT", &friend_endpoint),
        ("CONCERT_PROFILE_GRPC_ENDPOINT", &profile_endpoint),
    ] {
        endpoint(name, value, production)?;
    }
    // No development fallback credential is accepted by this composition.
    let credential = required("CONCERT_SERVICE_CREDENTIAL")?;
    let tls = platform::config::resolve_tls_material(production, "concert")?;
    let gateway = ActorDelegationVerifier::from_env(production, Workload::Concert)?;
    let signer = RelationshipReadSigner::from_env(&gateway)?;
    let replay_tls = tls
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("World replay requires explicit mutual TLS material"))?;
    let ports = WorldProductionPorts {
        auth_invalidation: InvalidationSource::new(
            InvalidationOwner::Auth,
            &auth_endpoint,
            &credential,
            replay_tls,
        )
        .map_err(|_| anyhow::anyhow!("World Auth replay source is unavailable"))?,
        friend_invalidation: InvalidationSource::new(
            InvalidationOwner::Friend,
            &friend_endpoint,
            &credential,
            replay_tls,
        )
        .map_err(|_| anyhow::anyhow!("World Friend replay source is unavailable"))?,
        auth: Arc::new(GrpcWorldAuthReader::connect(
            &auth_endpoint,
            &credential,
            tls.as_ref(),
            signer.clone(),
        )?),
        profile: GrpcWorldProfileReader::connect(
            &profile_endpoint,
            &credential,
            tls.as_ref(),
            signer.clone(),
        )?,
        relationship: GrpcRelationshipAuthority::connect(
            &auth_endpoint,
            &friend_endpoint,
            &credential,
            tls.as_ref(),
            signer,
        )?,
    };
    Ok(ports)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn owner_endpoints_reject_non_origins_and_unsafe_production_hosts() {
        for value in [
            "http://user@owner.internal",
            "http://owner.internal/path",
            "http://owner.internal/?token=x",
            "ftp://owner.internal",
            "http://localhost:9000",
        ] {
            assert!(endpoint("owner", value, true).is_err());
        }
        assert!(endpoint("owner", "https://auth.mcr.internal:443", true).is_ok());
        assert!(endpoint("owner", "http://127.0.0.1:9000", false).is_ok());
    }
}
