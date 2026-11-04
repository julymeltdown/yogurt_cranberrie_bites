//! Auth's real dispatcher and durable worker. Scripted peer cases are negative
//! transport components; only the separately paired real Notification RPC may
//! qualify a positive owning COMMIT. No other BE42 target is acknowledged here.
#[path = "../../notification-service/tests/concert_inbox_support/mod.rs"]
mod owned;
use auth_service::{
    adapters::{
        out::{
            grpc::user_deletion_dispatcher::{
                notification_proto as proto, verify_notification_deletion_receipt,
                GrpcUserDeletionDispatcher, UserDeletionDispatchRequest,
            },
            persistence::{
                user_deletion_saga_repository::PgUserDeletionSagaRepository,
                user_repository::{PgAuthAuditRepository, PgUserRepository},
            },
            runtime::SystemAuthRuntime,
        },
        r#in::messaging::user_deletion_worker::{UserDeletionWorker, UserDeletionWorkerPolicy},
    },
    application::{
        dto::{auth_dto::DeleteUserDto, deletion_dto::AuthUserDeletionTargetDto as Target},
        errors::AuthError,
        ports::{UserDeletionDispatcherPort, UserDeletionSagaPort},
        use_cases::delete_user::DeleteUserUseCase,
    },
    domain::user_deletion::{UserDeletionSaga, UserDeletionStepStatus, UserDeletionTarget},
};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use platform::{
    grpc_auth::{CallerRegistry, Workload},
    grpc_tls::TlsMaterial,
};
use sqlx::PgPool;
use std::{
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::sync::oneshot;
use tonic::{transport::Server, Request, Response, Status};
use uuid::Uuid;
const AUTH: &str = "be33-transport-auth";
fn root() -> anyhow::Result<PathBuf> {
    Ok(PathBuf::from(std::env::var("CONCERT_TEST_OWNER_ROOT")?))
}
fn tls(role: Workload) -> anyhow::Result<TlsMaterial> {
    let pki = root()?.join("pki");
    TlsMaterial::load(
        pki.join(format!("{}.crt", role.as_str())),
        pki.join(format!("{}.key", role.as_str())),
        pki.join("ca.crt"),
    )
}
fn dispatch(endpoint: &str) -> anyhow::Result<GrpcUserDeletionDispatcher> {
    GrpcUserDeletionDispatcher::connect(
        endpoint.into(),
        endpoint.into(),
        endpoint.into(),
        endpoint.into(),
        AUTH,
        Some(&tls(Workload::Auth)?),
    )
}
fn command() -> UserDeletionDispatchRequest {
    UserDeletionDispatchRequest {
        saga_id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        actor_id: Uuid::new_v4(),
        retention_until: DateTime::from_timestamp(1_800_000_000, 123456789).unwrap(),
    }
}
fn synthetic_receipt(c: &UserDeletionDispatchRequest) -> proto::ApplyUserDeletionResponse {
    proto::ApplyUserDeletionResponse {
        applied: true,
        committed_receipt: Some(proto::NotificationDeletionReceipt {
            version: 1,
            saga_id: c.saga_id.to_string(),
            user_id: c.user_id.to_string(),
            actor_id: c.actor_id.to_string(),
            retention_seconds: Some(c.retention_until.timestamp()),
            retention_nanos: Some(c.retention_until.timestamp_subsec_nanos()),
            fence_revision: 7,
            effect_xid: "12345".into(),
            recorded_at: "2026-11-04T15:00:00Z".into(),
        }),
    }
}
#[test]
fn exact_receipt_binding_rejects_boolean_partial_and_every_rebound_field() {
    let c = command();
    let valid = synthetic_receipt(&c);
    assert!(verify_notification_deletion_receipt(&c, &valid).is_ok()); // data validator only
    for kind in 0..21 {
        let mut r = valid.clone();
        match kind {
            0 => r.applied = false,
            1 => r.committed_receipt = None,
            2 => r.committed_receipt = Some(Default::default()),
            n => {
                let v = r.committed_receipt.as_mut().unwrap();
                match n {
                    3 => v.version = 0,
                    4 => v.version = 2,
                    5 => v.saga_id = Uuid::new_v4().to_string(),
                    6 => v.user_id = Uuid::new_v4().to_string(),
                    7 => v.actor_id = Uuid::new_v4().to_string(),
                    8 => v.retention_seconds = Some(c.retention_until.timestamp() + 1),
                    9 => v.retention_nanos = Some(c.retention_until.timestamp_subsec_nanos() + 1),
                    10 => v.retention_nanos = Some(1_000_000_000),
                    11 => v.fence_revision = 0,
                    12 => v.fence_revision = -1,
                    13 => v.effect_xid.clear(),
                    14 => v.effect_xid = "0".into(),
                    15 => v.effect_xid = "18446744073709551616".into(),
                    16 => v.effect_xid = "012345".into(),
                    17 => v.recorded_at.clear(),
                    18 => v.saga_id = Uuid::nil().to_string(),
                    19 => v.retention_seconds = None,
                    _ => v.retention_nanos = None,
                }
            }
        }
        let error = verify_notification_deletion_receipt(&c, &r).unwrap_err();
        assert_eq!(
            error.downcast_ref::<AuthError>(),
            Some(&AuthError::DependencyUnavailable),
            "case {kind}"
        );
        assert!(!format!("{error}").contains(&c.user_id.to_string()));
    }
    for field in 0..3 {
        let mut nil = c.clone();
        match field {
            0 => nil.saga_id = Uuid::nil(),
            1 => nil.user_id = Uuid::nil(),
            _ => nil.actor_id = Uuid::nil(),
        };
        assert!(verify_notification_deletion_receipt(&nil, &synthetic_receipt(&nil)).is_err());
    }
}
#[test]
fn retention_binding_preserves_nanoseconds_before_and_after_sql_epoch() {
    for (seconds, nanos) in [
        (-1, 999999999),
        (946684799, 123456789),
        (946684800, 1),
        (1800000000, 0),
    ] {
        let mut c = command();
        c.retention_until = DateTime::from_timestamp(seconds, nanos).unwrap();
        let r = synthetic_receipt(&c);
        assert!(verify_notification_deletion_receipt(&c, &r).is_ok());
        c.retention_until += ChronoDuration::nanoseconds(1);
        assert!(verify_notification_deletion_receipt(&c, &r).is_err());
    }
}
#[derive(Clone)]
struct Hostile {
    mode: Arc<AtomicUsize>,
    calls: Arc<AtomicUsize>,
}
#[tonic::async_trait]
impl proto::notification_service_server::NotificationService for Hostile {
    async fn create_notification(
        &self,
        _: Request<proto::CreateNotificationRequest>,
    ) -> Result<Response<proto::CreateNotificationResponse>, Status> {
        Err(Status::unimplemented("not a product owner"))
    }
    async fn list_notifications(
        &self,
        _: Request<proto::ListNotificationsRequest>,
    ) -> Result<Response<proto::ListNotificationsResponse>, Status> {
        Err(Status::unimplemented("not a product owner"))
    }
    async fn mark_notification_read(
        &self,
        _: Request<proto::MarkNotificationReadRequest>,
    ) -> Result<Response<proto::MarkNotificationReadResponse>, Status> {
        Err(Status::unimplemented("not a product owner"))
    }
    async fn mark_all_notifications_read(
        &self,
        _: Request<proto::MarkAllNotificationsReadRequest>,
    ) -> Result<Response<proto::MarkAllNotificationsReadResponse>, Status> {
        Err(Status::unimplemented("not a product owner"))
    }
    async fn delete_notification(
        &self,
        _: Request<proto::DeleteNotificationRequest>,
    ) -> Result<Response<proto::DeleteNotificationResponse>, Status> {
        Err(Status::unimplemented("not a product owner"))
    }
    async fn apply_user_deletion(
        &self,
        r: Request<proto::ApplyUserDeletionRequest>,
    ) -> Result<Response<proto::ApplyUserDeletionResponse>, Status> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let r = r.into_inner();
        let c = UserDeletionDispatchRequest {
            saga_id: Uuid::parse_str(&r.saga_id).unwrap(),
            user_id: Uuid::parse_str(&r.user_id).unwrap(),
            actor_id: Uuid::parse_str(&r.actor_id).unwrap(),
            retention_until: DateTime::parse_from_rfc3339(&r.retention_until)
                .unwrap()
                .with_timezone(&Utc),
        };
        let mut response = synthetic_receipt(&c);
        match self.mode.load(Ordering::SeqCst) {
            0 => response.committed_receipt = None,
            1 => response.applied = false,
            2 => response.committed_receipt = Some(Default::default()),
            3 => {
                response.committed_receipt.as_mut().unwrap().retention_nanos =
                    Some((c.retention_until.timestamp_subsec_nanos() + 1) % 1_000_000_000)
            }
            _ => {
                // Late, syntactically valid test data still cannot settle a
                // cancelled worker. It is never counted as owning COMMIT proof.
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
        }
        Ok(Response::new(response))
    }
}
struct HostileServer {
    endpoint: String,
    peer: Hostile,
    stop: Option<oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<Result<(), tonic::transport::Error>>>,
}
impl Drop for HostileServer {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}
impl HostileServer {
    async fn new() -> anyhow::Result<Self> {
        // Test-only service/caller table; the positive path below uses the actual
        // Notification handler and its unchanged production method policy.
        static POLICY: platform::grpc_method_policy::MethodPolicy =
            platform::grpc_method_policy::MethodPolicy {
                service: "notification.v1.NotificationService",
                workload: Some(Workload::Notification),
                methods: &[("ApplyUserDeletion", &[Workload::Auth])],
                actor_fields: &[],
            };
        let peer = Hostile {
            mode: Arc::new(AtomicUsize::new(0)),
            calls: Arc::new(AtomicUsize::new(0)),
        };
        let service = platform::grpc_method_policy::guard_without_delegation(
            &POLICY,
            proto::notification_service_server::NotificationServiceServer::new(peer.clone()),
            CallerRegistry::new().register(Workload::Auth, AUTH)?,
        )?;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = format!("https://{}", listener.local_addr()?);
        let (stop, shutdown) = oneshot::channel();
        let server = Server::builder()
            .tls_config(tls(Workload::Notification)?.server_config()?)?
            .layer(platform::call_deadline::CallDeadlineLayer)
            .add_service(service);
        let task = tokio::spawn(server.serve_with_incoming_shutdown(
            tokio_stream::wrappers::TcpListenerStream::new(listener),
            async {
                let _ = shutdown.await;
            },
        ));
        Ok(Self {
            endpoint,
            peer,
            stop: Some(stop),
            task: Some(task),
        })
    }
    async fn finish(mut self) -> anyhow::Result<()> {
        let _ = self.stop.take().unwrap().send(());
        tokio::time::timeout(Duration::from_secs(3), self.task.take().unwrap()).await???;
        Ok(())
    }
}
async fn pool() -> anyhow::Result<PgPool> {
    // The existing marker/inode/socket guard authorizes this second isolated DB;
    // it contains only Auth's migrations, never Concert tables or live data.
    let p = owned::owned_pool("mcr_concert_notification_source").await?;
    migration_runtime::prepare_database(
        &p,
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations"),
        migration_runtime::MigrationMode::Startup,
        &[],
    )
    .await?;
    Ok(p)
}
async fn begin(p: &PgPool) -> anyhow::Result<UserDeletionDispatchRequest> {
    let user = Uuid::new_v4();
    sqlx::query("insert into users(id,primary_email,password_hash,status) values($1,$2,'synthetic','ACTIVE')").bind(user).bind(format!("{user}@example.invalid")).execute(p).await?;
    let response = DeleteUserUseCase::new(
        Arc::new(PgUserRepository::new(
            p.clone(),
            Arc::new(SystemAuthRuntime),
        )),
        Arc::new(PgAuthAuditRepository::new(p.clone())),
        PgUserDeletionSagaRepository::new(p.clone()),
        Arc::new(SystemAuthRuntime),
    )
    .execute(DeleteUserDto {
        user_id: user.to_string(),
        actor_id: user.to_string(),
        request_id: None,
        ip_address: None,
        user_agent: None,
    })
    .await?;
    let repo = PgUserDeletionSagaRepository::new(p.clone());
    let saga: UserDeletionSaga = repo
        .find_by_saga_id(Uuid::parse_str(&response.saga_id)?)
        .await?
        .unwrap()
        .into();
    assert_eq!(saga.steps.len(), 8);
    // Bounded scheduling fixture only: keep the other seven real required steps
    // pending. This does not fake their owner responses or remove inventory.
    sqlx::query("update user_deletion_steps set next_attempt_at=clock_timestamp()+interval '1 day' where saga_id=$1 and target<>'NOTIFICATION'").bind(saga.id).execute(p).await?;
    Ok(UserDeletionDispatchRequest {
        saga_id: saga.id,
        user_id: saga.user_id,
        actor_id: saga.actor_id,
        retention_until: saga.retention_until,
    })
}
async fn saga(p: &PgPool, id: Uuid) -> anyhow::Result<UserDeletionSaga> {
    Ok(PgUserDeletionSagaRepository::new(p.clone())
        .find_by_saga_id(id)
        .await?
        .unwrap()
        .into())
}
fn worker(
    p: &PgPool,
    d: GrpcUserDeletionDispatcher,
) -> UserDeletionWorker<PgUserDeletionSagaRepository, GrpcUserDeletionDispatcher> {
    UserDeletionWorker::new(
        PgUserDeletionSagaRepository::new(p.clone()),
        d,
        UserDeletionWorkerPolicy {
            batch_size: 100,
            max_attempts: 8,
            lease_timeout: ChronoDuration::seconds(1),
            base_retry: ChronoDuration::seconds(1),
        },
    )
}
async fn apply(
    d: &GrpcUserDeletionDispatcher,
    c: &UserDeletionDispatchRequest,
) -> Result<(), AuthError> {
    d.apply(
        Target::Notification,
        c.saga_id,
        c.user_id,
        c.actor_id,
        c.retention_until,
    )
    .await
}
#[tokio::test]
#[ignore = "fresh owned PG16/PKI offer required; hostile replies are negative fixtures"]
async fn actual_auth_worker_refuses_hostile_receipts_and_cancellation_keeps_step_unacknowledged(
) -> anyhow::Result<()> {
    let p = pool().await?;
    let server = HostileServer::new().await?;
    let d = dispatch(&server.endpoint)?;
    for mode in 0..4 {
        server.peer.mode.store(mode, Ordering::SeqCst);
        let c = begin(&p).await?;
        let result = worker(&p, d.clone()).run_once(Utc::now()).await?;
        assert_eq!(result.succeeded, 0);
        let s = saga(&p, c.saga_id).await?;
        assert!(s
            .steps
            .iter()
            .all(|s| s.status != UserDeletionStepStatus::Succeeded));
        assert!(s.completed_at.is_none());
        sqlx::query("update user_deletion_steps set next_attempt_at=clock_timestamp()+interval '1 day' where saga_id=$1").bind(c.saga_id).execute(&p).await?;
    }
    server.peer.mode.store(4, Ordering::SeqCst);
    let c = begin(&p).await?;
    let before = server.peer.calls.load(Ordering::SeqCst);
    let w = worker(&p, d.clone());
    let task = tokio::spawn(async move { w.run_once(Utc::now()).await });
    tokio::time::timeout(Duration::from_secs(3), async {
        while server.peer.calls.load(Ordering::SeqCst) == before {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    tokio::time::sleep(Duration::from_millis(280)).await;
    assert!(saga(&p, c.saga_id)
        .await?
        .steps
        .iter()
        .all(|s| s.status != UserDeletionStepStatus::Succeeded));
    let short = Instant::now() + Duration::from_millis(40);
    assert_eq!(
        platform::call_deadline::scope(short, apply(&d, &command())).await,
        Err(AuthError::DependencyUnavailable)
    );
    let sent = server.peer.calls.load(Ordering::SeqCst);
    assert_eq!(
        platform::call_deadline::scope(Instant::now(), apply(&d, &command())).await,
        Err(AuthError::DependencyUnavailable)
    );
    assert_eq!(sent, server.peer.calls.load(Ordering::SeqCst));
    // Fixture scheduling cleanup preserves every failed/pending step without
    // advancing any target, and prevents this cancelled lease leaking into the
    // separately qualified actual-owner case.
    sqlx::query("update user_deletion_steps set status='RETRY',locked_at=null,next_attempt_at=clock_timestamp()+interval '1 day' where saga_id=$1 and target='NOTIFICATION'")
        .bind(c.saga_id).execute(&p).await?;
    server.finish().await?;
    p.close().await;
    Ok(())
}
#[tokio::test]
#[ignore = "paired actual Notification RPC; external controller freezes proof and restarts PG"]
async fn actual_auth_worker_accepts_only_committed_notification_receipt() -> anyhow::Result<()> {
    let p = pool().await?;
    let endpoint = std::fs::read_to_string(root()?.join("notification-receipt-endpoint"))?;
    let d = dispatch(&endpoint)?;
    let c = begin(&p).await?;
    let result = worker(&p, d.clone()).run_once(Utc::now()).await?;
    assert_eq!(result.succeeded, 1);
    let s = saga(&p, c.saga_id).await?;
    assert_eq!(
        s.steps
            .iter()
            .filter(|s| s.status == UserDeletionStepStatus::Succeeded)
            .count(),
        1
    );
    assert!(s
        .steps
        .iter()
        .find(|s| s.target == UserDeletionTarget::Notification)
        .unwrap()
        .completed_at
        .is_some());
    assert!(s.completed_at.is_none());
    apply(&d, &c).await?;
    let n = owned::owned_pool("mcr_concert_notification").await?;
    let receipt: serde_json::Value = sqlx::query_scalar(
        "select to_jsonb(r) from notification_deletion_receipts_v1 r where saga_id=$1",
    )
    .bind(c.saga_id)
    .fetch_one(&n)
    .await?;
    let checkpoint = serde_json::json!({"saga":c.saga_id,"user":c.user_id,"actor":c.actor_id,"retention":c.retention_until.to_rfc3339(),"receipt":receipt});
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(root()?.join("auth-notification-receipt-checkpoint.json"))?;
    file.write_all(&serde_json::to_vec(&checkpoint)?)?;
    file.sync_all()?;
    n.close().await;
    p.close().await;
    Ok(())
}
#[tokio::test]
#[ignore = "run only after controller's actual PG restart and real RPC restart"]
async fn actual_restart_recovers_identical_receipt_without_completing_other_auth_steps(
) -> anyhow::Result<()> {
    let p = pool().await?;
    let cp: serde_json::Value = serde_json::from_slice(&std::fs::read(
        root()?.join("auth-notification-receipt-checkpoint.json"),
    )?)?;
    let c = UserDeletionDispatchRequest {
        saga_id: Uuid::parse_str(cp["saga"].as_str().unwrap())?,
        user_id: Uuid::parse_str(cp["user"].as_str().unwrap())?,
        actor_id: Uuid::parse_str(cp["actor"].as_str().unwrap())?,
        retention_until: DateTime::parse_from_rfc3339(cp["retention"].as_str().unwrap())?
            .with_timezone(&Utc),
    };
    let d = dispatch(&std::fs::read_to_string(
        root()?.join("notification-receipt-endpoint"),
    )?)?;
    apply(&d, &c).await?;
    let n = owned::owned_pool("mcr_concert_notification").await?;
    let actual: serde_json::Value = sqlx::query_scalar(
        "select to_jsonb(r) from notification_deletion_receipts_v1 r where saga_id=$1",
    )
    .bind(c.saga_id)
    .fetch_one(&n)
    .await?;
    assert_eq!(actual, cp["receipt"]);
    let mut rebound = c.clone();
    rebound.retention_until += ChronoDuration::nanoseconds(1);
    assert!(apply(&d, &rebound).await.is_err());
    let s = saga(&p, c.saga_id).await?;
    assert_eq!(
        s.steps
            .iter()
            .filter(|s| s.status == UserDeletionStepStatus::Succeeded)
            .count(),
        1
    );
    assert_eq!(s.steps.len(), 8);
    assert!(s.completed_at.is_none());
    n.close().await;
    p.close().await;
    Ok(())
}
