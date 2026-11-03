//! Transport fixture adapted from Auth d901b133; all mutations/scopes are Friend-owned.
use super::*;
use friend_service::{
    adapters::out::persistence::follow_repository::PgFollowRepository,
    application::ports::{RelationshipPolicyRepositoryPort, UserLifecycleRepositoryPort},
};
use platform::grpc_tls::TlsMaterial;
use sqlx::{PgPool, Row};
use std::{os::unix::fs::MetadataExt, path::PathBuf, str::FromStr, sync::Arc};
use tokio_stream::wrappers::TcpListenerStream;
type Client =
    proto::friend_concert_invalidation_service_client::FriendConcertInvalidationServiceClient<
        tonic::transport::Channel,
    >;

// Actual HTTP/2 transport with deliberately truncated protobuf payloads. The
// generated decoder must reject them before any owning database read.
#[derive(Default)]
struct RawCodec;
struct RawEncoder;
impl tonic::codec::Encoder for RawEncoder {
    type Item = tonic::codegen::Bytes;
    type Error = tonic::Status;
    fn encode(
        &mut self,
        item: Self::Item,
        dst: &mut tonic::codec::EncodeBuf<'_>,
    ) -> Result<(), Self::Error> {
        use prost::bytes::BufMut;
        dst.put_slice(&item);
        Ok(())
    }
}
impl tonic::codec::Codec for RawCodec {
    type Encode = tonic::codegen::Bytes;
    type Decode = proto::ReadInvalidationHeadResponse;
    type Encoder = RawEncoder;
    type Decoder = <tonic_prost::ProstCodec<
        proto::ReadInvalidationHeadRequest,
        proto::ReadInvalidationHeadResponse,
    > as tonic::codec::Codec>::Decoder;
    fn encoder(&mut self) -> Self::Encoder {
        RawEncoder
    }
    fn decoder(&mut self) -> Self::Decoder {
        tonic::codec::Codec::decoder(&mut tonic_prost::ProstCodec::<
            proto::ReadInvalidationHeadRequest,
            proto::ReadInvalidationHeadResponse,
        >::default())
    }
}

async fn pool(max: u32) -> anyhow::Result<PgPool> {
    let root = PathBuf::from(std::env::var("MCR_INVALIDATION_RPC_ROOT")?);
    anyhow::ensure!(
        root.canonicalize()? == root && root.starts_with("/private/tmp"),
        "unsafe root"
    );
    let meta = std::fs::symlink_metadata(&root)?;
    anyhow::ensure!(
        meta.is_dir()
            && meta.mode() & 0o077 == 0
            && format!("{}:{}", meta.dev(), meta.ino())
                == std::env::var("MCR_INVALIDATION_RPC_IDENTITY")?,
        "foreign root"
    );
    let marker_meta = std::fs::symlink_metadata(root.join("mcr-owner.json"))?;
    anyhow::ensure!(
        marker_meta.is_file() && marker_meta.uid() == meta.uid() && marker_meta.mode() & 0o077 == 0,
        "unsafe marker"
    );
    for name in ["data", "socket"] {
        let path = root.join(name);
        anyhow::ensure!(
            std::fs::symlink_metadata(&path)?.is_dir() && path.canonicalize()? == path,
            "foreign directory"
        );
    }
    let marker: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("mcr-owner.json"))?)?;
    anyhow::ensure!(
        marker["token"].as_str() == Some(std::env::var("MCR_INVALIDATION_RPC_TOKEN")?.as_str())
            && marker["uid"].as_u64() == Some(u64::from(meta.uid())),
        "foreign marker"
    );
    let options = sqlx::postgres::PgConnectOptions::from_str(&std::env::var(
        "MCR_INVALIDATION_RPC_DATABASE_URL",
    )?)?;
    anyhow::ensure!(
        options.get_username() == "mcr_owner"
            && options.get_database() == Some("mcr_friend_invalidation_rpc")
            && options.get_socket() == Some(&root.join("socket"))
            && options.get_options().is_none()
            && options.get_port() == 5432,
        "unsafe database options"
    );
    let p = sqlx::postgres::PgPoolOptions::new()
        .max_connections(max)
        .connect_with(options.application_name("mcr_friend_invalidation_rpc_reader"))
        .await?;
    let r=sqlx::query("select system_identifier::text identity,current_database() db,current_setting('server_version_num')::int version,current_setting('listen_addresses') listeners,current_setting('data_directory') data from pg_control_system()").fetch_one(&p).await?;
    anyhow::ensure!(
        r.get::<String, _>("identity") == std::env::var("MCR_INVALIDATION_RPC_SYSTEM_IDENTIFIER")?
            && r.get::<String, _>("db") == "mcr_friend_invalidation_rpc"
            && r.get::<i32, _>("version") == 160014
            && r.get::<String, _>("listeners").is_empty()
            && r.get::<String, _>("data") == root.join("data").to_str().unwrap(),
        "foreign cluster"
    );
    Ok(p)
}
fn tls(w: Workload) -> anyhow::Result<TlsMaterial> {
    let leaf = test_support::tls::issue(w.as_str())?;
    TlsMaterial::load(&leaf.cert, &leaf.key, &leaf.ca)
}
// Harness-only observation: a dropped pending service future proves HTTP/2
// cancellation independently of SQL's eventual original statement timeout.
#[derive(Clone)]
struct Observe<S> {
    inner: S,
    dropped: Arc<tokio::sync::Notify>,
}
impl<S: tonic::server::NamedService> tonic::server::NamedService for Observe<S> {
    const NAME: &'static str = S::NAME;
}
impl<S> tonic::codegen::Service<http::Request<tonic::body::Body>> for Observe<S>
where
    S: tonic::codegen::Service<http::Request<tonic::body::Body>>,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = DropAware<S::Future>;
    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
    fn call(&mut self, r: http::Request<tonic::body::Body>) -> Self::Future {
        DropAware {
            future: Some(Box::pin(self.inner.call(r))),
            dropped: self.dropped.clone(),
            pending: true,
        }
    }
}
struct DropAware<F> {
    future: Option<std::pin::Pin<Box<F>>>,
    dropped: Arc<tokio::sync::Notify>,
    pending: bool,
}
impl<F: std::future::Future> std::future::Future for DropAware<F> {
    type Output = F::Output;
    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.get_mut();
        let result = this.future.as_mut().unwrap().as_mut().poll(cx);
        if result.is_ready() {
            this.pending = false;
        }
        result
    }
}
impl<F> Drop for DropAware<F> {
    fn drop(&mut self) {
        drop(self.future.take());
        if self.pending {
            self.dropped.notify_one();
        }
    }
}
struct Running {
    endpoint: String,
    dropped: Arc<tokio::sync::Notify>,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<Result<(), tonic::transport::Error>>,
}
impl Drop for Running {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Running {
    async fn start(p: PgPool) -> anyhow::Result<Self> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = format!("https://{}", listener.local_addr()?);
        let (stop, rx) = tokio::sync::oneshot::channel();
        let dropped = Arc::new(tokio::sync::Notify::new());
        let server = tonic::transport::Server::builder()
            .tls_config(tls(Workload::Friend)?.server_config()?)?
            .layer(platform::call_deadline::CallDeadlineLayer)
            .add_service(Observe {
                inner: guarded_server(p, registry())?,
                dropped: dropped.clone(),
            });
        let task = tokio::spawn(server.serve_with_incoming_shutdown(
            TcpListenerStream::new(listener),
            async {
                let _ = rx.await;
            },
        ));
        Ok(Self {
            endpoint,
            dropped,
            stop: Some(stop),
            task,
        })
    }
    async fn client(&self, w: Workload) -> anyhow::Result<Client> {
        let channel = tokio::time::timeout(
            Duration::from_secs(1),
            tls(w)?.connect(&self.endpoint, Workload::Friend),
        )
        .await??;
        Ok(Client::new(channel)
            .max_decoding_message_size(128 * 1024)
            .max_encoding_message_size(4096))
    }
    async fn close(mut self) -> anyhow::Result<()> {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        tokio::time::timeout(Duration::from_secs(3), &mut self.task).await???;
        anyhow::ensure!(
            tokio::net::TcpStream::connect(self.endpoint.trim_start_matches("https://"))
                .await
                .is_err(),
            "listener remains"
        );
        Ok(())
    }
}
fn request<T>(query: T, who: Workload) -> Request<T> {
    let mut r = Request::new(query);
    r.metadata_mut()
        .insert("x-call-deadline-remaining", "900m".parse().unwrap());
    r.metadata_mut()
        .insert("x-service-caller", who.as_str().parse().unwrap());
    r.metadata_mut().insert(
        "x-service-token",
        format!("fixture-{}", who.as_str()).parse().unwrap(),
    );
    r
}
async fn head(c: &mut Client) -> anyhow::Result<proto::InvalidationCursor> {
    let stop = Instant::now() + Duration::from_millis(950);
    tokio::time::timeout_at(stop.into(), async {
        let q = head_query();
        let r = c.read_head(request(q.clone(), Workload::Concert)).await?;
        anyhow::ensure!(
            r.metadata().get("cache-control").unwrap() == "no-store",
            "cache"
        );
        let r = r.into_inner();
        assert_eq!(r.query, Some(q));
        Ok(r.head.unwrap())
    })
    .await?
}
async fn page(
    c: &mut Client,
    after: proto::InvalidationCursor,
    limit: u32,
) -> anyhow::Result<proto::ReadInvalidationsResponse> {
    let stop = Instant::now() + Duration::from_millis(950);
    tokio::time::timeout_at(stop.into(), async {
        let mut q = after_query();
        q.after = Some(after.clone());
        q.limit = limit;
        let r = c
            .read_after(request(q.clone(), Workload::Concert))
            .await?
            .into_inner();
        assert_eq!(r.query, Some(q));
        assert!(r.encoded_len() <= 128 * 1024 && r.signals.len() <= limit as usize);
        let mut sequence = after.sequence;
        for s in &r.signals {
            sequence += 1;
            let pos = s.cursor.as_ref().unwrap();
            assert_eq!(pos.owner_epoch, after.owner_epoch);
            assert_eq!(pos.sequence, sequence);
            assert_eq!(
                Uuid::parse_str(&s.signal_id).unwrap().to_string(),
                s.signal_id
            );
            match (
                proto::InvalidationReason::try_from(s.reason).unwrap(),
                s.scope.as_ref().unwrap(),
            ) {
                (
                    proto::InvalidationReason::FriendRelationshipChanged,
                    proto::invalidation_signal::Scope::Pair(pair),
                ) => {
                    let first = Uuid::parse_str(&pair.actor_id).unwrap();
                    let second = Uuid::parse_str(&pair.subject_id).unwrap();
                    assert!(!first.is_nil() && first < second);
                    assert_eq!(first.to_string(), pair.actor_id);
                    assert_eq!(second.to_string(), pair.subject_id);
                }
                (
                    proto::InvalidationReason::FriendUserRemoved,
                    proto::invalidation_signal::Scope::User(user),
                ) => {
                    let id = Uuid::parse_str(&user.user_id).unwrap();
                    assert!(!id.is_nil());
                    assert_eq!(id.to_string(), user.user_id);
                }
                _ => panic!("foreign owner or malformed scope"),
            }
        }
        assert_eq!(r.next.as_ref().unwrap().sequence, sequence);
        assert!(sequence <= r.head.as_ref().unwrap().sequence);
        Ok(r)
    })
    .await?
}
async fn save_replay(p: &PgPool, c: &mut Client) -> anyhow::Result<()> {
    let h = Box::pin(head(c)).await?;
    let mut after = h.clone();
    after.sequence = 0;
    let mut encoded = Vec::new();
    loop {
        let r = Box::pin(page(c, after, 256)).await?;
        for s in r.signals {
            encoded.push(s.encode_to_vec());
        }
        after = r.next.unwrap();
        if after == h {
            break;
        }
    }
    sqlx::query("create table if not exists invalidation_rpc_restart_fixture (singleton bool primary key,epoch text,head bigint,signals jsonb)").execute(p).await?;
    sqlx::query("insert into invalidation_rpc_restart_fixture values(true,$1,$2,$3)")
        .bind(&h.owner_epoch)
        .bind(h.sequence)
        .bind(serde_json::to_value(encoded)?)
        .execute(p)
        .await?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires friend_invalidation_rpc_smoke.py private PG16"]
async fn owning_mutations_replay_over_guarded_mutual_tls() -> anyhow::Result<()> {
    let p = pool(6).await?;
    let mut conn = p.acquire().await?.detach();
    sqlx::migrate::Migrator::new(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("migrations")
            .as_path(),
    )
    .await?
    .run(&mut conn)
    .await?;
    drop(conn);
    let server = Running::start(p.clone()).await?;
    let mut c = Box::pin(server.client(Workload::Concert)).await?;
    let channel = tls(Workload::Concert)?
        .connect(&server.endpoint, Workload::Friend)
        .await?;
    let mut raw = tonic::client::Grpc::new(channel);
    for bytes in [vec![0x0a, 0xff], vec![0xff], vec![0; 5000]] {
        let result = tokio::time::timeout(Duration::from_secs(1), async {
            raw.ready().await.unwrap();
            raw.unary(
                request(tonic::codegen::Bytes::from(bytes), Workload::Concert),
                http::uri::PathAndQuery::from_static(
                    "/relationship.v1.FriendConcertInvalidationService/ReadHead",
                ),
                RawCodec,
            )
            .await
        })
        .await?;
        assert_ne!(result.unwrap_err().code(), Code::Ok);
    }
    drop(raw);
    let ca = test_support::tls::issue("concert")?;
    let public_endpoint = tonic::transport::Endpoint::from_shared(server.endpoint.clone())?
        .tls_config(
            tonic::transport::ClientTlsConfig::new()
                .ca_certificate(tonic::transport::Certificate::from_pem(std::fs::read(
                    &ca.ca,
                )?))
                .domain_name("friend.svc.internal"),
        )?;
    if let Ok(Ok(channel)) =
        tokio::time::timeout(Duration::from_secs(1), public_endpoint.connect()).await
    {
        let public_result = tokio::time::timeout(
            Duration::from_secs(1),
            Client::new(channel).read_head(head_query()),
        )
        .await;
        assert!(!matches!(public_result, Ok(Ok(_))));
    }
    let empty = Box::pin(head(&mut c)).await?;
    assert_eq!(empty.sequence, 0);
    let e = Box::pin(page(&mut c, empty.clone(), 256)).await?;
    assert!(e.signals.is_empty());
    assert_eq!(e.next, e.head);
    // Correct TLS and credential authenticate other workloads, but both methods
    // still deny them. A sibling certificate cannot present Concert metadata.
    for w in Workload::ALL
        .into_iter()
        .filter(|w| *w != Workload::Concert)
    {
        let mut sibling = Box::pin(server.client(w)).await?;
        assert_eq!(
            sibling
                .read_head(request(head_query(), w))
                .await
                .unwrap_err()
                .code(),
            Code::PermissionDenied
        );
        assert_eq!(
            sibling
                .read_after(request(after_query(), w))
                .await
                .unwrap_err()
                .code(),
            Code::PermissionDenied
        );
        assert_eq!(
            sibling
                .read_head(request(head_query(), Workload::Concert))
                .await
                .unwrap_err()
                .code(),
            Code::Unauthenticated
        );
    }
    assert_eq!(
        c.read_head(head_query()).await.unwrap_err().code(),
        Code::Unauthenticated
    );
    let mut forged = request(head_query(), Workload::Concert);
    forged
        .metadata_mut()
        .insert("x-service-token", "wrong".parse()?);
    assert_eq!(
        c.read_head(forged).await.unwrap_err().code(),
        Code::Unauthenticated
    );
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let repo = PgFollowRepository::new(p.clone());
    let before = Box::pin(head(&mut c)).await?;
    repo.block(a, b, chrono::Utc::now()).await?;
    assert!(repo.policy(a, b).await?.blocked_by_viewer);
    let blocked = Box::pin(page(&mut c, before.clone(), 256)).await?;
    assert_eq!(blocked.signals.len(), 1);
    assert_eq!(
        blocked.signals[0].reason,
        proto::InvalidationReason::FriendRelationshipChanged as i32
    );
    assert_eq!(
        Box::pin(page(&mut c, before, 256)).await?.signals,
        blocked.signals
    );
    repo.unblock(a, b).await?;
    assert!(!repo.policy(a, b).await?.blocked_by_viewer);
    repo.block(b, a, chrono::Utc::now()).await?;
    let before_delete = Box::pin(head(&mut c)).await?;
    repo.apply_user_deletion(
        Uuid::new_v4(),
        a,
        a,
        chrono::Utc::now() + chrono::Duration::days(30),
    )
    .await?;
    let deleted = Box::pin(page(&mut c, before_delete, 256)).await?;
    assert!(deleted.signals.iter().any(|s|s.reason==proto::InvalidationReason::FriendUserRemoved as i32 && matches!(s.scope.as_ref(),Some(proto::invalidation_signal::Scope::User(u)) if u.user_id==a.to_string())));
    assert!(repo.block(a, b, chrono::Utc::now()).await.is_err());
    let checkpoint = Box::pin(head(&mut c)).await?;
    let mut tx = p.begin().await?;
    sqlx::query("INSERT INTO user_blocks(blocker_id,blocked_id,created_at) VALUES($1,$2,now())")
        .bind(Uuid::new_v4())
        .bind(Uuid::new_v4())
        .execute(&mut *tx)
        .await?;
    assert_eq!(Box::pin(head(&mut c)).await?, checkpoint);
    tx.rollback().await?;
    assert_eq!(Box::pin(head(&mut c)).await?, checkpoint);
    // Force the actual owner mutation's trigger to fail; both block and journal
    // allocation must roll back. No private token/actor proof is needed to replay.
    let fail_a = Uuid::new_v4();
    let fail_b = Uuid::new_v4();
    sqlx::query("ALTER TABLE friend_world_invalidation_journal ADD CONSTRAINT rpc_fixture_failure CHECK(false) NOT VALID").execute(&p).await?;
    assert!(repo
        .block(fail_a, fail_b, chrono::Utc::now())
        .await
        .is_err());
    assert_eq!(Box::pin(head(&mut c)).await?, checkpoint);
    assert!(!repo.policy(fail_a, fail_b).await?.is_blocked());
    sqlx::query(
        "ALTER TABLE friend_world_invalidation_journal DROP CONSTRAINT rpc_fixture_failure",
    )
    .execute(&p)
    .await?;
    sqlx::query("INSERT INTO user_blocks(blocker_id,blocked_id,created_at) SELECT gen_random_uuid(),gen_random_uuid(),now() FROM generate_series(1,300)").execute(&p).await?;
    let first = Box::pin(page(&mut c, empty.clone(), 256)).await?;
    assert_eq!(first.signals.len(), 256);
    assert!(first.next.as_ref().unwrap().sequence < first.head.as_ref().unwrap().sequence);
    let second = Box::pin(page(&mut c, first.next.unwrap(), 256)).await?;
    assert_eq!(second.next, second.head);
    let caught = Box::pin(page(&mut c, second.next.unwrap(), 17)).await?;
    assert!(caught.signals.is_empty());
    assert_eq!(caught.next, caught.head);
    for foreign in [
        proto::InvalidationCursor {
            owner_epoch: Uuid::new_v4().to_string(),
            sequence: 0,
        },
        proto::InvalidationCursor {
            owner_epoch: empty.owner_epoch.clone(),
            sequence: Box::pin(head(&mut c)).await?.sequence + 1,
        },
    ] {
        let mut q = after_query();
        q.after = Some(foreign);
        assert_eq!(
            c.read_after(request(q, Workload::Concert))
                .await
                .unwrap_err()
                .code(),
            Code::FailedPrecondition
        );
    }
    save_replay(&p, &mut c).await?;
    drop(c);
    server.close().await?;
    p.close().await;
    Ok(())
}

async fn wait_locked(observer: &PgPool) -> anyhow::Result<i32> {
    let stop = Instant::now() + Duration::from_millis(700);
    while Instant::now() < stop {
        let pid:Option<i32>=sqlx::query_scalar("SELECT pid FROM pg_stat_activity WHERE application_name='mcr_friend_invalidation_rpc_reader' AND state='active' AND wait_event_type='Lock' AND query LIKE 'SELECT owner_epoch,sequence FROM public.friend_world_invalidation_head%' LIMIT 1").fetch_optional(observer).await?;
        if let Some(pid) = pid {
            return Ok(pid);
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    anyhow::bail!("expected actual reader lock wait")
}
async fn drained(observer: &PgPool) -> anyhow::Result<()> {
    tokio::time::timeout(Duration::from_millis(1100),async {loop {
        let n:i64=sqlx::query_scalar("SELECT count(*) FROM pg_stat_activity WHERE application_name='mcr_friend_invalidation_rpc_reader' AND query LIKE 'SELECT owner_epoch,sequence FROM public.friend_world_invalidation_head%' AND state<>'idle'").fetch_one(observer).await?;
        if n==0 {return Ok::<_,anyhow::Error>(());}
        tokio::time::sleep(Duration::from_millis(5)).await;
    }}).await??;
    Ok(())
}
#[tokio::test]
#[ignore = "requires friend_invalidation_rpc_smoke.py private PG16"]
async fn deadline_includes_pool_sql_and_cancellation_releases_work() -> anyhow::Result<()> {
    let observer = pool(3).await?;
    let readers = pool(1).await?;
    let server = Running::start(readers.clone()).await?;
    let mut c = Box::pin(server.client(Workload::Concert)).await?;
    let mut lock = observer.begin().await?;
    sqlx::query("lock table friend_world_invalidation_head in access exclusive mode")
        .execute(&mut *lock)
        .await?;
    let held = readers.acquire().await?;
    let mut r = request(head_query(), Workload::Concert);
    r.metadata_mut()
        .insert("x-call-deadline-remaining", "250m".parse()?);
    let start = Instant::now();
    let mut original = c.clone();
    let call = tokio::spawn(async move { original.read_head(r).await });
    tokio::time::sleep(Duration::from_millis(150)).await;
    drop(held);
    assert_eq!(call.await?.unwrap_err().code(), Code::DeadlineExceeded);
    assert!(start.elapsed() < Duration::from_millis(380));
    drained(&observer).await?;
    // A caller's long header never expands the local one-second operation cap.
    let mut r = request(head_query(), Workload::Concert);
    r.metadata_mut()
        .insert("x-call-deadline-remaining", "30S".parse()?);
    let start = Instant::now();
    assert_eq!(
        c.read_head(r).await.unwrap_err().code(),
        Code::DeadlineExceeded
    );
    assert!(start.elapsed() < Duration::from_millis(1400));
    drained(&observer).await?;
    let dropped = server.dropped.clone();
    let notification = dropped.notified();
    let original_stop = Instant::now() + Duration::from_millis(900);
    let mut cancel = c.clone();
    let call = tokio::spawn(async move {
        cancel
            .read_head(request(head_query(), Workload::Concert))
            .await
    });
    let blocked_pid = wait_locked(&observer).await?;
    assert!(Instant::now() + Duration::from_millis(300) < original_stop);
    let cancel_start = Instant::now();
    call.abort();
    assert!(call.await.unwrap_err().is_cancelled());
    tokio::time::timeout(Duration::from_millis(200), notification).await?;
    assert!(Instant::now() < original_stop - Duration::from_millis(100));
    // Keep the blocker held until the exact cancelled reader backend vanishes;
    // an active wait or idle transaction must not hide behind releasing the lock.
    loop {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE pid=$1)")
                .bind(blocked_pid)
                .fetch_one(&observer)
                .await?;
        if !exists {
            break;
        }
        anyhow::ensure!(
            cancel_start.elapsed() < Duration::from_millis(1100),
            "cancelled reader exceeded original operation bound"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    drained(&observer).await?;
    lock.rollback().await?;
    Box::pin(head(&mut c)).await?;
    let checkpoint = Box::pin(head(&mut c)).await?;
    let mut break_storage = observer.begin().await?;
    sqlx::query("alter table friend_world_invalidation_head rename to rpc_fixture_hidden_head")
        .execute(&mut *break_storage)
        .await?;
    // Commit the temporary rename so another connection observes unavailability;
    // restore it before final success. No migration source is edited.
    break_storage.commit().await?;
    assert_eq!(
        c.read_head(request(head_query(), Workload::Concert))
            .await
            .unwrap_err()
            .code(),
        Code::Unavailable
    );
    sqlx::query("alter table rpc_fixture_hidden_head rename to friend_world_invalidation_head")
        .execute(&observer)
        .await?;
    assert_eq!(Box::pin(head(&mut c)).await?, checkpoint);
    drop(c);
    server.close().await?;
    readers.close().await;
    observer.close().await;
    Ok(())
}

#[tokio::test]
#[ignore = "requires friend_invalidation_rpc_smoke.py private PG16 after real restart"]
async fn restart_replays_same_epoch_positions_and_signal_identity() -> anyhow::Result<()> {
    let p = pool(3).await?;
    let saved: (String, i64, serde_json::Value) = sqlx::query_as(
        "select epoch,head,signals from invalidation_rpc_restart_fixture where singleton",
    )
    .fetch_one(&p)
    .await?;
    let server = Running::start(p.clone()).await?;
    let mut c = Box::pin(server.client(Workload::Concert)).await?;
    let h = Box::pin(head(&mut c)).await?;
    assert_eq!(h.owner_epoch, saved.0);
    assert_eq!(h.sequence, saved.1);
    let mut after = h.clone();
    after.sequence = 0;
    let mut signals = Vec::<Vec<u8>>::new();
    loop {
        let r = Box::pin(page(&mut c, after, 23)).await?;
        for s in r.signals {
            signals.push(s.encode_to_vec());
        }
        after = r.next.unwrap();
        if after == h {
            break;
        }
    }
    assert_eq!(serde_json::to_value(signals)?, saved.2);
    drop(c);
    server.close().await?;
    p.close().await;
    Ok(())
}
