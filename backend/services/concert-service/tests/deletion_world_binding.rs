//! Object-custody/native-task regressions, not canonical Auth/deletion proof.
//! No database is started or contacted, and no subject is admitted. Production
//! all-category deletion is qualified separately by the real Auth/Concert join.
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::{
    bootstrap::{BootstrapDefaults, BootstrapRelease, ValidatedProductRelease},
    world_ticket::{OwnerIdentity, WorldError, WorldSubject},
};
use concert_domain::EventId;
use concert_service::{
    adapters::r#in::world_ws::{SocketTransport, WorldSocketState},
    bootstrap::BootstrapService,
    config::{
        deletion_world_binding::ServingWorldBinding,
        world_runtime::{WorldProductionPorts, WorldRuntimeSettings},
    },
    world::{
        account_deletion::DeletionFailure,
        admission_bridge::{AdmissionBridge, LiveOwnerView, LiveWorldOwners},
        ingress::OriginPolicy,
        membership::ClockAnchor,
        runtime::{ShutdownReport, WorldRuntime},
    },
};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::{sync::oneshot, task::JoinHandle};
use uuid::Uuid;

// Retained geometry/avatar helpers only, from Root7d74's existing World tests.
// These synthetic assets are not product artwork or authority provenance.
#[path = "world_runtime_support/mod.rs"]
mod geometry;

struct DeniedOwners(Arc<Mutex<()>>);
#[async_trait]
impl LiveWorldOwners for DeniedOwners {
    fn publication_fence(&self) -> Arc<Mutex<()>> {
        self.0.clone()
    }
    fn generation(&self) -> Uuid {
        Uuid::from_u128(1)
    }
    async fn current(
        &self,
        _: &WorldSubject,
        _: DateTime<Utc>,
    ) -> Result<LiveOwnerView, WorldError> {
        Err(WorldError::Unavailable)
    }
    async fn visible(
        &self,
        _: &WorldSubject,
        _: &[WorldSubject],
        _: DateTime<Utc>,
    ) -> Result<BTreeSet<Uuid>, WorldError> {
        Err(WorldError::Unavailable)
    }
}
struct Fixture {
    pool: PgPool,
    bootstrap: Arc<BootstrapService>,
    bridge: Arc<AdmissionBridge>,
    state: Arc<WorldSocketState>,
    owner: OwnerIdentity,
    event: EventId,
    room: Uuid,
    anchor: ClockAnchor,
    started: Instant,
    task: Option<JoinHandle<ShutdownReport>>,
}
impl Fixture {
    fn bridge(
        pool: PgPool,
        bootstrap: Arc<BootstrapService>,
        owner: OwnerIdentity,
        event: EventId,
        room: Uuid,
        anchor: ClockAnchor,
        started: Instant,
    ) -> Arc<AdmissionBridge> {
        AdmissionBridge::new_fixture(
            pool,
            bootstrap,
            Arc::new(DeniedOwners(Arc::new(Mutex::new(())))),
            owner,
            event,
            BTreeMap::from([(room, geometry::id(6))]),
            geometry::pins(),
            anchor,
            started,
        )
        .unwrap()
    }
    fn new() -> Self {
        // Lazy, zero-minimum pool: the missing private socket prevents any
        // accidental successful DB access. None of these tests issues SQL.
        let pool = PgPoolOptions::new()
            .min_connections(0)
            .max_connections(1)
            .connect_lazy(
                "postgresql://fixture@localhost/fixture?host=/mcr-deletion-binding-no-database",
            )
            .unwrap();
        let release = Arc::new(
            ValidatedProductRelease::from_validated(
                BootstrapRelease::new(
                    BootstrapDefaults {
                        theme: "REVENGE".parse().unwrap(),
                        avatar: geometry::identity().avatar,
                    },
                    b"{}".to_vec(),
                )
                .unwrap(),
                "a".repeat(64),
            )
            .unwrap(),
        );
        let bootstrap = Arc::new(BootstrapService::new(pool.clone(), release));
        let owner = OwnerIdentity::new(Uuid::new_v4().to_string(), Uuid::new_v4()).unwrap();
        let event = EventId::from_uuid(Uuid::new_v4());
        let room = Uuid::new_v4();
        let anchor = ClockAnchor {
            monotonic_ms: 0,
            utc: Utc::now(),
        };
        let started = Instant::now();
        let bridge = Self::bridge(
            pool.clone(),
            bootstrap.clone(),
            owner.clone(),
            event,
            room,
            anchor.clone(),
            started,
        );
        let transport = SocketTransport::default();
        let runtime = WorldRuntime::new(
            event,
            Uuid::parse_str(&owner.process_id).unwrap(),
            anchor.clone(),
            vec![geometry::spec(room, 1)],
            bridge.clone(),
            bridge.clone(),
            transport.clone(),
        )
        .unwrap();
        let (clock, origin) = runtime.clock_origin();
        let (handle, task) = runtime.spawn();
        let state = WorldSocketState::new(
            OriginPolicy::new(vec!["http://localhost:9000".into()], true).unwrap(),
            transport,
            bridge.clone(),
            Arc::new(handle),
            clock,
            origin,
        );
        Self {
            pool,
            bootstrap,
            bridge,
            state,
            owner,
            event,
            room,
            anchor,
            started,
            task: Some(task),
        }
    }
    fn reconstructed_bridge(&self) -> Arc<AdmissionBridge> {
        Self::bridge(
            self.pool.clone(),
            self.bootstrap.clone(),
            self.owner.clone(),
            self.event,
            self.room,
            self.anchor.clone(),
            self.started,
        )
    }
    async fn stop(&mut self) {
        self.state.owner.request_stop();
        let report = tokio::time::timeout(Duration::from_secs(2), self.task.take().unwrap())
            .await
            .unwrap()
            .unwrap();
        assert!(report.remaining_cleanup.is_empty());
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.state.owner.request_stop();
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

#[tokio::test]
async fn borrowed_adapter_preserves_the_exact_serving_objects() {
    let mut f = Fixture::new();
    let binding = ServingWorldBinding::new(f.bridge.clone(), f.state.clone()).unwrap();
    let serving = binding.as_serving_world();
    assert!(Arc::ptr_eq(&serving.bridge, &f.bridge));
    assert!(std::ptr::eq(serving.sockets, f.state.as_ref()));
    assert!(Arc::ptr_eq(&serving.sockets.owner, &f.state.owner));
    drop(serving);
    f.stop().await;
}

#[tokio::test]
async fn reconstructed_bridge_with_identical_ids_is_rejected() {
    let mut f = Fixture::new();
    let result = ServingWorldBinding::new(f.reconstructed_bridge(), f.state.clone());
    assert!(matches!(result, Err(DeletionFailure::Unavailable)));
    f.stop().await;
}

#[tokio::test]
async fn retained_binding_does_not_prevent_supervised_owner_shutdown() {
    let mut f = Fixture::new();
    let binding = ServingWorldBinding::new(f.bridge.clone(), f.state.clone()).unwrap();
    f.stop().await;
    // A retained object is not readiness; the stopped owner's queue is closed.
    assert!(binding
        .as_serving_world()
        .sockets
        .owner
        .ready()
        .await
        .is_err());
}

#[tokio::test]
async fn one_shot_transfers_custody_and_releases_the_same_objects() {
    let mut f = Fixture::new();
    let weak_bridge = Arc::downgrade(&f.bridge);
    let weak_state = Arc::downgrade(&f.state);
    let (sender, receiver) = oneshot::channel();
    assert!(sender
        .send(ServingWorldBinding::new(f.bridge.clone(), f.state.clone()).unwrap())
        .is_ok());
    let binding = receiver.await.unwrap();
    f.stop().await;
    drop(f);
    assert!(weak_bridge.upgrade().is_some());
    assert!(weak_state.upgrade().is_some());
    drop(binding);
    assert!(weak_state.upgrade().is_none());
    assert!(weak_bridge.upgrade().is_none());
}

#[tokio::test]
async fn dropped_receiver_retains_no_hidden_binding_after_failure() {
    let mut f = Fixture::new();
    let weak_bridge = Arc::downgrade(&f.bridge);
    let weak_state = Arc::downgrade(&f.state);
    let (sender, receiver) = oneshot::channel();
    drop(receiver);
    let failed = sender.send(ServingWorldBinding::new(f.bridge.clone(), f.state.clone()).unwrap());
    assert!(failed.is_err());
    drop(failed);
    f.stop().await;
    drop(f);
    assert!(weak_state.upgrade().is_none());
    assert!(weak_bridge.upgrade().is_none());
}

// Compile the actual opt-in API as a Send future, without constructing fake
// production ports or opening a listener to pretend the startup path passed.
#[allow(dead_code)]
fn typed_startup_api(
    settings: WorldRuntimeSettings,
    pool: PgPool,
    ports: WorldProductionPorts,
    ready: oneshot::Sender<()>,
    transport: SocketTransport,
    local: oneshot::Sender<Arc<concert_service::world::invalidation::WorldInvalidationConsumer>>,
    deletion: oneshot::Sender<ServingWorldBinding>,
) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
    settings.serve_with_ready_social_and_deletion(pool, ports, ready, transport, local, deletion)
}
