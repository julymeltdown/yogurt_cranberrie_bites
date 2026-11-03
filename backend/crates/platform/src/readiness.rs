//! Bounded local and authenticated gRPC health checks on the metrics listener.
//! Peers report only their local health, so probing does not recursively fan out.

use crate::edge::Edge;
use crate::grpc_auth::{
    CallerRegistry, RequireRegisteredCallerInterceptor, ServiceIdentityClientInterceptor,
};
use crate::grpc_client_trace::GrpcClientTraceService;
use crate::grpc_tls::TlsMaterial;
use crate::shutdown::Shutdown;
use futures::future::join_all;
use serde::Serialize;
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::Instant;
use tonic::{Request, Response, Status};
use tonic_health::pb::{
    health_check_response::ServingStatus,
    health_client::HealthClient,
    health_server::{Health, HealthServer},
    HealthCheckRequest, HealthCheckResponse,
};

pub const DATABASE_PROBE_TIMEOUT: Duration = Duration::from_millis(250);
pub const GRPC_PROBE_TIMEOUT: Duration = Duration::from_millis(500);
pub const PROBE_CACHE_TTL: Duration = Duration::from_secs(1);

type CheckFuture = Pin<Box<dyn Future<Output = bool> + Send>>;
type CachedStatus = Arc<Mutex<Option<(Instant, ProbeStatus)>>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeStatus {
    Up,
    Down,
    TimedOut,
    NotChecked,
}

#[derive(Clone)]
pub struct Probe {
    name: &'static str,
    required: bool,
    deadline: Duration,
    run: Arc<dyn Fn() -> CheckFuture + Send + Sync>,
    cached: CachedStatus,
}

impl Probe {
    pub fn new<F, Fut>(name: &'static str, required: bool, deadline: Duration, run: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = bool> + Send + 'static,
    {
        Self {
            name,
            required,
            deadline,
            run: Arc::new(move || Box::pin(run())),
            cached: Arc::new(Mutex::new(None)),
        }
    }

    /// Constructs a lazy channel; reachability is checked only when probed.
    pub fn grpc(
        edge: Edge,
        endpoint: &str,
        credential: &str,
        tls: Option<&TlsMaterial>,
    ) -> anyhow::Result<Self> {
        let channel = crate::grpc_tls::lazy_channel_for_edge_with_optional_tls(
            tls,
            endpoint,
            edge.peer(),
            edge.budget(),
        )?;
        let client = HealthClient::with_interceptor(
            GrpcClientTraceService::new(channel),
            ServiceIdentityClientInterceptor::new(edge.caller(), credential)?,
        );
        let deadline = GRPC_PROBE_TIMEOUT.min(edge.budget().deadline);
        Ok(Self::new(
            edge.budget().edge,
            edge.required_for_readiness(),
            deadline,
            move || {
                let mut client = client.clone();
                async move {
                    let mut request = Request::new(HealthCheckRequest {
                        service: String::new(),
                    });
                    request.set_timeout(deadline);
                    match client.check(request).await {
                        Ok(response) => response.get_ref().status == ServingStatus::Serving as i32,
                        Err(error) => {
                            tracing::debug!(
                                edge = edge.budget().edge,
                                peer = edge.peer().as_str(),
                                %error,
                                "readiness gRPC dependency probe failed"
                            );
                            false
                        }
                    }
                }
            },
        ))
    }

    async fn check(&self) -> ProbeStatus {
        // Clones share both the cached result and the in-flight check. Frequent
        // health requests cannot multiply the work against a slow dependency.
        let mut cached = self.cached.lock().await;
        if let Some((checked_at, status)) = *cached {
            if checked_at.elapsed() < PROBE_CACHE_TTL {
                return status;
            }
        }
        let status = match tokio::time::timeout(self.deadline, (self.run)()).await {
            Ok(true) => ProbeStatus::Up,
            Ok(false) => ProbeStatus::Down,
            Err(_) => ProbeStatus::TimedOut,
        };
        *cached = Some((Instant::now(), status));
        status
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct DependencyReport {
    pub required: bool,
    pub status: ProbeStatus,
}

#[derive(Clone, Debug, Serialize)]
pub struct ReadinessReport {
    pub ready: bool,
    pub degraded: bool,
    pub draining: bool,
    pub checks: BTreeMap<&'static str, DependencyReport>,
}

#[derive(Clone)]
pub struct Readiness {
    local: Probe,
    dependencies: Vec<Probe>,
    shutdown: Shutdown,
}

impl Readiness {
    pub fn new(local: Probe) -> Self {
        Self::with_shutdown(local, crate::shutdown::global())
    }

    pub fn with_shutdown(mut local: Probe, shutdown: Shutdown) -> Self {
        local.required = true;
        Self {
            local,
            dependencies: Vec::new(),
            shutdown,
        }
    }

    pub fn with_dependencies(mut self, dependencies: Vec<Probe>) -> anyhow::Result<Self> {
        let mut names = std::collections::HashSet::from([self.local.name]);
        for probe in &dependencies {
            anyhow::ensure!(
                names.insert(probe.name),
                "Duplicate readiness dependency name"
            );
        }
        self.dependencies = dependencies;
        Ok(self)
    }

    pub async fn report(&self) -> ReadinessReport {
        let draining = self.shutdown.is_requested();
        let checks = join_all(std::iter::once(&self.local).chain(&self.dependencies).map(
            |probe| async move {
                let status = if draining {
                    ProbeStatus::NotChecked
                } else {
                    probe.check().await
                };
                (
                    probe.name,
                    DependencyReport {
                        required: probe.required,
                        status,
                    },
                )
            },
        ))
        .await
        .into_iter()
        .collect::<BTreeMap<_, _>>();
        // Shutdown overrides even a just-cached successful result.
        let draining = self.shutdown.is_requested();
        ReadinessReport {
            ready: !draining
                && checks
                    .values()
                    .all(|check| !check.required || check.status == ProbeStatus::Up),
            degraded: checks.values().any(|check| check.status != ProbeStatus::Up),
            draining,
            checks,
        }
    }

    pub fn grpc_service(
        &self,
        registry: CallerRegistry,
    ) -> anyhow::Result<AuthenticatedHealthService> {
        Ok(HealthServer::with_interceptor(
            LocalHealth {
                readiness: self.clone(),
            },
            RequireRegisteredCallerInterceptor::new(registry)?,
        ))
    }
}

pub type AuthenticatedHealthService = tonic::service::interceptor::InterceptedService<
    HealthServer<LocalHealth>,
    RequireRegisteredCallerInterceptor,
>;

#[derive(Clone)]
pub struct LocalHealth {
    readiness: Readiness,
}

#[tonic::async_trait]
impl Health for LocalHealth {
    async fn check(
        &self,
        request: Request<HealthCheckRequest>,
    ) -> Result<Response<HealthCheckResponse>, Status> {
        if !request.get_ref().service.is_empty() {
            return Err(Status::not_found("Unknown health service"));
        }
        let ready = !self.readiness.shutdown.is_requested()
            && self.readiness.local.check().await == ProbeStatus::Up
            && !self.readiness.shutdown.is_requested();
        Ok(Response::new(HealthCheckResponse {
            status: if ready {
                ServingStatus::Serving
            } else {
                ServingStatus::NotServing
            } as i32,
        }))
    }

    type WatchStream = tokio_stream::Empty<Result<HealthCheckResponse, Status>>;

    async fn watch(
        &self,
        _: Request<HealthCheckRequest>,
    ) -> Result<Response<Self::WatchStream>, Status> {
        Err(Status::unimplemented("Use the bounded unary health check"))
    }
}
