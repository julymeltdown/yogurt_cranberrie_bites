//! Strict opt-in for actual World serving. Product files retain the bootstrap
//! validator; synthetic metadata is never promoted by this configuration path.
use crate::{
    adapters::r#in::world_ws::{self, SocketTransport, WorldSocketState},
    bootstrap::{BootstrapFeatures, BootstrapService, ProductReleaseBundle},
    world::{
        admission_bridge::{AdmissionBridge, LiveWorldOwners},
        ingress::OriginPolicy,
        membership::ClockAnchor,
        runtime::{RoomSpec, WorldRuntime},
        simulation::ResourcePin,
    },
};
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
pub struct WorldRuntimeSettings {
    pub address: SocketAddr,
    origins: OriginPolicy,
    owner: Uuid,
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
        let release = ProductReleaseBundle::from_product_files(
            &files[0],
            &files[1],
            &files[2],
            [&files[3], &files[4], &files[5]],
            BootstrapFeatures {
                world: true,
                ..Default::default()
            },
        )
        .map_err(|_| anyhow::anyhow!("World product release is unavailable"))?;
        Ok(Some(Self {
            address,
            origins,
            owner,
            event,
            rooms,
            release,
        }))
    }
    /// Composes the actual DB bridge, 20Hz owner and socket server with supplied
    /// qualified live owner ports. Absence is rejected before binding a listener.
    pub async fn serve(
        self,
        pool: sqlx::PgPool,
        owners: Arc<dyn LiveWorldOwners>,
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
        let bridge = AdmissionBridge::new(
            pool,
            bootstrap,
            owners,
            OwnerIdentity::new(self.owner.to_string(), Uuid::new_v4())?,
            self.event,
            epochs,
            resources,
            anchor.clone(),
            started,
        )?;
        tokio::time::timeout(Duration::from_secs(2), bridge.claim_rooms()).await??;
        let transport = SocketTransport::default();
        let runtime = WorldRuntime::new(
            self.event,
            self.owner,
            anchor,
            specs,
            bridge.clone(),
            bridge.clone(),
            transport.clone(),
        )
        .map_err(|_| anyhow::anyhow!("World owner construction failed"))?;
        let (clock, origin_ms) = runtime.clock_origin();
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
                tokio::select! {biased;_ = platform::shutdown::global().cancelled()=>return Ok::<(),anyhow::Error>(()),_ = tick.tick()=>{tokio::time::timeout(Duration::from_secs(2),bridge.maintain()).await??;}}
            }
        };
        let server = axum::serve(listener, world_ws::router(state))
            .with_graceful_shutdown(platform::shutdown::global().cancelled());
        let result = tokio::select! {result=async { server.await }=>result.map_err(anyhow::Error::from),result=authority=>result};
        // Dropping all listener states closes sockets and the final handle stops
        // the owner independently of a full command queue. Await its cleanup report.
        transport.close_all(crate::world::ingress::SafeClose::UNAVAILABLE);
        handle.request_stop();
        drop(handle);
        let report = tokio::time::timeout(Duration::from_secs(2), owner_task).await??;
        for b in report.remaining_cleanup {
            let _ = crate::world::membership::CanonicalAuthorityPort::release(&bridge, &b);
        }
        let pending = tokio::time::timeout(Duration::from_secs(2), bridge.cleanup()).await??;
        anyhow::ensure!(
            pending == 0,
            "World shutdown has pending persistent membership cleanup"
        );
        result
    }
}
/// Production transport is intentionally not synthesized from configuration flags.
/// Until the actual owner producer is linked, explicit opt-in reports this exact
/// unavailable dependency before it opens any world socket.
pub fn production_owners() -> anyhow::Result<Arc<dyn LiveWorldOwners>> {
    anyhow::bail!("World live Auth/Profile and fenced recipient authority transport is unavailable")
}
