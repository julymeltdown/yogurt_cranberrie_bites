//! Drop or withhold the actual PostgreSQL COMMIT reply, before SQLx sees it.
use super::*;
use notification_service::{
    adapters::out::{
        persistence::notification_repository::PgNotificationRepository,
        runtime::SystemNotificationRuntime,
    },
    application::{
        dto::repository_dto::NotificationRecordDto,
        errors::NotificationError,
        notification_contracts::NotificationCreateOutcome,
        notification_creation::{NotificationAdmittedCreatePort, NotificationWriteDeadlineDto},
        use_cases::create_notification::CreateNotificationUseCase,
    },
    config::AppState,
};
use post_service::application::{
    claimed_notification::ClaimedNotificationDeliveryPort,
    delivery_effect::{DeliveryEffectDeadlineDto, DeliveryEffectFailure},
    ports::ReplayEventDeliveryPort,
};

struct ObservedDelivery {
    inner: Arc<dyn ClaimedNotificationDeliveryPort>,
    claims: std::sync::Mutex<Vec<TargetDeliveryClaimDto>>,
    failures: std::sync::Mutex<Vec<DeliveryEffectFailure>>,
}
#[async_trait::async_trait]
impl ClaimedNotificationDeliveryPort for ObservedDelivery {
    async fn deliver_claimed_notifications(
        &self,
        claim: &TargetDeliveryClaimDto,
        deadline: DeliveryEffectDeadlineDto,
    ) -> anyhow::Result<()> {
        self.claims.lock().unwrap().push(claim.clone());
        let result = self
            .inner
            .deliver_claimed_notifications(claim, deadline)
            .await;
        if let Err(error) = &result {
            self.failures
                .lock()
                .unwrap()
                .push(DeliveryEffectFailure::classify(error));
        }
        result
    }
}
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot, Notify};

const DROP_REPLY: u8 = 1;
const HOLD_REPLY: u8 = 2;

pub(super) struct CommitAckProxy {
    pub pool: PgPool,
    mode: Arc<AtomicU8>,
    observed: mpsc::UnboundedReceiver<u8>,
    release: Arc<Notify>,
    stop: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<anyhow::Result<()>>,
    port: u16,
}
impl Drop for CommitAckProxy {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl CommitAckProxy {
    pub(super) async fn start(direct: &PgPool) -> anyhow::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        let backend = direct.connect_options().get_port();
        let mode = Arc::new(AtomicU8::new(0));
        let release = Arc::new(Notify::new());
        let (send, observed) = mpsc::unbounded_channel();
        let (stop, mut stopped) = oneshot::channel();
        let task_mode = mode.clone();
        let task_release = release.clone();
        let task = tokio::spawn(async move {
            let mut connections = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    _ = &mut stopped => break,
                    accepted = listener.accept() => {
                        let (front, _) = accepted?;
                        connections.spawn(forward(front, backend, task_mode.clone(), send.clone(), task_release.clone()));
                    }
                    joined = connections.join_next(), if !connections.is_empty() => {
                        // EOF is normal when SQLx closes a connection after a fault.
                        if let Some(result) = joined { result??; }
                    }
                }
            }
            connections.abort_all();
            while let Some(result) = connections.join_next().await {
                match result {
                    Err(e) if e.is_cancelled() => {}
                    result => {
                        result??;
                    }
                }
            }
            Ok(())
        });
        let options = direct
            .connect_options()
            .as_ref()
            .clone()
            .host("127.0.0.1")
            .port(port)
            .ssl_mode(sqlx::postgres::PgSslMode::Disable);
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        Ok(Self {
            pool,
            mode,
            observed,
            release,
            stop: Some(stop),
            task,
            port,
        })
    }
    pub(super) fn destination_state(&self, original: &AppState) -> Arc<AppState> {
        let mut state = original.clone();
        // Only the concrete creation repository's transport changes. Admission,
        // application construction, SQL and all other state remain production code.
        state.create_notification = CreateNotificationUseCase::new(
            Arc::new(PgNotificationRepository::new(self.pool.clone())),
            Arc::new(SystemNotificationRuntime),
        );
        Arc::new(state)
    }
    fn arm(&self, mode: u8) {
        assert!(matches!(mode, DROP_REPLY | HOLD_REPLY));
        assert_eq!(self.mode.swap(mode, Ordering::SeqCst), 0);
    }
    async fn observed(&mut self, expected: u8) -> anyhow::Result<()> {
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), self.observed.recv()).await?,
            Some(expected)
        );
        Ok(())
    }
    pub(super) async fn shutdown(mut self) -> anyhow::Result<()> {
        self.release.notify_waiters();
        self.pool.close().await;
        let _ = self.stop.take().unwrap().send(());
        tokio::time::timeout(Duration::from_secs(2), &mut self.task).await???;
        assert!(tokio::net::TcpStream::connect(("127.0.0.1", self.port))
            .await
            .is_err());
        println!("NOTIFICATION_COMMIT_PROXY_DISPOSED: {}", self.port);
        Ok(())
    }
    pub(super) async fn repository_faults(&mut self, direct: &PgPool) -> anyhow::Result<()> {
        let repo = PgNotificationRepository::new(self.pool.clone());
        for mode in [DROP_REPLY, HOLD_REPLY] {
            let now =
                chrono::DateTime::from_timestamp_micros(chrono::Utc::now().timestamp_micros())
                    .unwrap();
            let record = NotificationRecordDto {
                id: Uuid::new_v4(),
                user_id: Uuid::new_v4(),
                notification_type: "POST_COMMENTED".into(),
                payload_json: "{\"payloadVersion\":1}".into(),
                created_at: now,
                read_at: None,
                event_id: Uuid::new_v4(),
                actor_id: Some(Uuid::new_v4()),
                occurred_at: now,
            };
            self.arm(mode);
            let expires_at = chrono::Utc::now() + chrono::Duration::milliseconds(700);
            let started = std::time::Instant::now();
            let result = tokio::time::timeout(
                Duration::from_secs(2),
                repo.create_admitted(&record, NotificationWriteDeadlineDto { expires_at }),
            )
            .await?;
            self.observed(mode).await?;
            if mode == HOLD_REPLY {
                assert!(chrono::Utc::now() >= expires_at);
                assert!(started.elapsed() >= Duration::from_millis(650));
            }
            // A lost or late COMMIT reply is dependency uncertainty, never the
            // AdmissionExpired decision that asserts a known pre-COMMIT denial.
            assert!(
                matches!(result, Err(NotificationError::DependencyUnavailable)),
                "mode {mode}: {result:?}"
            );
            let actual: (Uuid, String, Option<Uuid>, chrono::DateTime<chrono::Utc>) = sqlx::query_as(
                "select id,payload_json,actor_id,occurred_at from notifications where user_id=$1 and event_id=$2 and type=$3",
            ).bind(record.user_id).bind(record.event_id).bind(&record.notification_type).fetch_one(direct).await?;
            assert_eq!(
                actual,
                (
                    record.id,
                    record.payload_json.clone(),
                    record.actor_id,
                    record.occurred_at
                )
            );
            self.release.notify_waiters();
            // Recover through the real deduplication transaction and a new
            // admission budget: the same event retains its original identity.
            let outcome = repo
                .create_admitted(
                    &record,
                    NotificationWriteDeadlineDto {
                        expires_at: chrono::Utc::now() + chrono::Duration::seconds(2),
                    },
                )
                .await?;
            assert!(
                matches!(outcome, NotificationCreateOutcome::Existing(ref row) if row.id == record.id)
            );
            let count: i64 = sqlx::query_scalar(
                "select count(*) from notifications where user_id=$1 and event_id=$2 and type=$3",
            )
            .bind(record.user_id)
            .bind(record.event_id)
            .bind(&record.notification_type)
            .fetch_one(direct)
            .await?;
            assert_eq!(count, 1);
        }
        println!("NOTIFICATION_PG_COMMIT_ACK: actual COMMIT CommandComplete dropped/withheld before SQLx; DependencyUnavailable; independent durable identity/payload; actual retry Existing and single row");
        Ok(())
    }
    pub(super) async fn worker_faults(
        &mut self,
        post: &post_support::TestApp,
        destination: &notification_support::TestApp,
        selected: Arc<
            dyn post_service::application::claimed_notification::ClaimedNotificationDeliveryPort,
        >,
    ) -> anyhow::Result<()> {
        for replay in [false, true] {
            for mode in [DROP_REPLY, HOLD_REPLY] {
                let c = seed(&post.pool, Uuid::new_v4(), Uuid::new_v4(), "POST_COMMENTED").await?;
                workers::ready(post, &c).await?;
                let observed = Arc::new(ObservedDelivery {
                    inner: selected.clone(),
                    claims: Default::default(),
                    failures: Default::default(),
                });
                self.arm(mode);
                if replay {
                    assert!(post
                        .fenced_replay_delivery(Default::default())?
                        .with_claimed_notifications(observed.clone())
                        .replay_target(c.event.event_id, DeliveryTarget::Internal)
                        .await
                        .is_err());
                } else {
                    workers::automatic(post, observed.clone()).await?;
                }
                self.observed(mode).await?;
                let count: i64 =
                    sqlx::query_scalar("select count(*) from notifications where event_id=$1")
                        .bind(c.event.event_id)
                        .fetch_one(&destination.pool)
                        .await?;
                assert_eq!(count, 1);
                let row = sqlx::query("select state,attempt_count,lease_token,lease_expires_at from post_event_delivery_jobs where event_id=$1 and target='INTERNAL'")
                    .bind(c.event.event_id).fetch_one(&post.pool).await?;
                assert_eq!(row.get::<String, _>("state"), "LEASED");
                let (id, source, phases) = workers::attempt(&post.pool, c.event.event_id).await?;
                assert_ne!(id, c.attempt_id);
                assert_eq!(
                    source,
                    if replay {
                        "ADMIN_REPLAY"
                    } else {
                        "OUTBOX_WORKER"
                    }
                );
                assert_eq!(phases, vec!["STARTED"]);
                assert_eq!(
                    *observed.failures.lock().unwrap(),
                    vec![DeliveryEffectFailure::Uncertain]
                );
                let frozen = {
                    let captured = observed.claims.lock().unwrap();
                    assert_eq!(captured.len(), 1);
                    captured[0].clone()
                };
                assert_eq!(id, frozen.attempt_id);
                assert_eq!(row.get::<i32, _>("attempt_count"), frozen.attempt_number);
                assert_eq!(row.get::<Uuid, _>("lease_token"), frozen.lease_token);
                assert_eq!(
                    row.get::<chrono::DateTime<chrono::Utc>, _>("lease_expires_at"),
                    frozen.lease_expires_at
                );
                let lease = row.get::<Uuid, _>("lease_token");
                let expiry = row.get::<chrono::DateTime<chrono::Utc>, _>("lease_expires_at");
                assert_eq!(row.get::<i32, _>("attempt_count"), c.attempt_number + 1);
                assert!(expiry > chrono::Utc::now());
                self.release.notify_waiters();
                let again: (Uuid, chrono::DateTime<chrono::Utc>) = sqlx::query_as("select lease_token,lease_expires_at from post_event_delivery_jobs where event_id=$1 and target='INTERNAL'")
                    .bind(c.event.event_id).fetch_one(&post.pool).await?;
                assert_eq!(again, (lease, expiry));
            }
        }
        println!("NOTIFICATION_WORKER_PG_COMMIT_ACK: automatic/replay x drop/withhold actual COMMIT reply; durable destination row; lease retained and only STARTED; no immediate failed/succeeded attempt");
        Ok(())
    }
}

async fn forward(
    front: tokio::net::TcpStream,
    backend_port: u16,
    mode: Arc<AtomicU8>,
    observed: mpsc::UnboundedSender<u8>,
    release: Arc<Notify>,
) -> std::io::Result<()> {
    let backend = tokio::net::TcpStream::connect(("127.0.0.1", backend_port)).await?;
    let (mut fr, mut fw) = front.into_split();
    let (mut br, mut bw) = backend.into_split();
    let to_backend = tokio::io::copy(&mut fr, &mut bw);
    let to_front = async {
        loop {
            let tag = br.read_u8().await?;
            let len = br.read_u32().await?;
            if !(4..=32 * 1024 * 1024).contains(&len) {
                return Err(std::io::Error::other("invalid owned PostgreSQL frame"));
            }
            let mut body = vec![0; (len - 4) as usize];
            br.read_exact(&mut body).await?;
            if tag == b'C' && body == b"COMMIT\0" {
                let fault = mode.swap(0, Ordering::SeqCst);
                if fault != 0 {
                    let _ = observed.send(fault);
                    if fault == HOLD_REPLY {
                        release.notified().await;
                    }
                    // Withholding ends by discarding the late reply too; a
                    // cancelled transaction cannot return a stale pooled ACK.
                    return Ok(());
                }
            }
            fw.write_u8(tag).await?;
            fw.write_u32(len).await?;
            fw.write_all(&body).await?;
        }
    };
    let result = tokio::select! { r=to_backend => r.map(|_|()), r=to_front => r };
    match result {
        Err(e)
            if matches!(
                e.kind(),
                std::io::ErrorKind::UnexpectedEof
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::BrokenPipe
            ) =>
        {
            Ok(())
        }
        other => other,
    }
}
