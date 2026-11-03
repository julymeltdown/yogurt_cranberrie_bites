use super::*;
use auth_service::{
    adapters::out::{
        cache::session_store::PgRefreshTokenStore, persistence::user_repository::PgUserRepository,
        runtime::SystemAuthRuntime,
    },
    application::ports::{RefreshTokenStorePort, UserRepositoryPort},
    domain::session::RefreshTokenRecord,
};
use platform::grpc_tls::TlsMaterial;
use sqlx::{PgPool, Row};
use std::{os::unix::fs::MetadataExt, path::PathBuf, str::FromStr, sync::Arc};
use tokio_stream::wrappers::TcpListenerStream;
type Client = proto::auth_concert_invalidation_service_client::AuthConcertInvalidationServiceClient<
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
            && options.get_database() == Some("mcr_auth_invalidation_rpc")
            && options.get_socket() == Some(&root.join("socket"))
            && options.get_options().is_none(),
        "unsafe database options"
    );
    let p = sqlx::postgres::PgPoolOptions::new()
        .max_connections(max)
        .connect_with(options.application_name("mcr_auth_invalidation_rpc_reader"))
        .await?;
    let r=sqlx::query("select system_identifier::text identity,current_database() db,current_setting('server_version_num')::int version,current_setting('listen_addresses') listeners,current_setting('data_directory') data from pg_control_system()").fetch_one(&p).await?;
    anyhow::ensure!(
        r.get::<String, _>("identity") == std::env::var("MCR_INVALIDATION_RPC_SYSTEM_IDENTIFIER")?
            && r.get::<String, _>("db") == "mcr_auth_invalidation_rpc"
            && r.get::<i32, _>("version") / 10000 == 16
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
struct Running {
    endpoint: String,
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
        let server = tonic::transport::Server::builder()
            .tls_config(tls(Workload::Auth)?.server_config()?)?
            .layer(platform::call_deadline::CallDeadlineLayer)
            .add_service(guarded_server(p, registry())?);
        let task = tokio::spawn(server.serve_with_incoming_shutdown(
            TcpListenerStream::new(listener),
            async {
                let _ = rx.await;
            },
        ));
        Ok(Self {
            endpoint,
            stop: Some(stop),
            task,
        })
    }
    async fn client(&self, w: Workload) -> anyhow::Result<Client> {
        let channel = tokio::time::timeout(
            Duration::from_secs(1),
            tls(w)?.connect(&self.endpoint, Workload::Auth),
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
        .insert("x-service-caller", who.as_str().parse().unwrap());
    r.metadata_mut().insert(
        "x-service-token",
        format!("fixture-{}", who.as_str()).parse().unwrap(),
    );
    r
}
async fn head(c: &mut Client) -> anyhow::Result<proto::InvalidationCursor> {
    let q = head_query();
    let r = c.read_head(request(q.clone(), Workload::Concert)).await?;
    anyhow::ensure!(
        r.metadata().get("cache-control").unwrap() == "no-store",
        "cache"
    );
    let r = r.into_inner();
    assert_eq!(r.query, Some(q));
    Ok(r.head.unwrap())
}
async fn page(
    c: &mut Client,
    after: proto::InvalidationCursor,
    limit: u32,
) -> anyhow::Result<proto::ReadInvalidationsResponse> {
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
                proto::InvalidationReason::AuthSessionChanged,
                proto::invalidation_signal::Scope::Family(f),
            ) => {
                assert!(!Uuid::parse_str(&f.user_id).unwrap().is_nil());
                assert!(!Uuid::parse_str(&f.session_family_id).unwrap().is_nil());
            }
            (
                proto::InvalidationReason::AuthUserAuthorityChanged
                | proto::InvalidationReason::AuthCredentialsChanged
                | proto::InvalidationReason::AuthUserRemoved,
                proto::invalidation_signal::Scope::User(u),
            ) => assert!(!Uuid::parse_str(&u.user_id).unwrap().is_nil()),
            _ => panic!("foreign owner or malformed scope"),
        }
    }
    assert_eq!(r.next.as_ref().unwrap().sequence, sequence);
    assert!(sequence <= r.head.as_ref().unwrap().sequence);
    Ok(r)
}
fn store(p: &PgPool) -> PgRefreshTokenStore {
    PgRefreshTokenStore::new(p.clone(), Arc::new(SystemAuthRuntime))
}
async fn user(p: &PgPool) -> anyhow::Result<Uuid> {
    let u = Uuid::new_v4();
    sqlx::query("insert into users(id,primary_email,password_hash,status) values($1,$2,'fixture-secret','ACTIVE')").bind(u).bind(format!("{u}@fixture.invalid")).execute(p).await?;
    Ok(u)
}
async fn session(p: &PgPool, u: Uuid, f: Uuid) -> anyhow::Result<String> {
    let s = RefreshTokenRecord {
        user_id: u,
        family_id: f,
        jti: Uuid::new_v4().to_string(),
        token_hash: "fixture-refresh-secret".into(),
        expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
    };
    store(p).store(&(&s).into()).await?;
    Ok(s.jti)
}
async fn save_replay(p: &PgPool, c: &mut Client) -> anyhow::Result<()> {
    let h = head(c).await?;
    let mut after = h.clone();
    after.sequence = 0;
    let mut encoded = Vec::new();
    loop {
        let r = page(c, after, 256).await?;
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
#[ignore = "requires auth_invalidation_rpc_smoke.py private PG16"]
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
    let mut c = server.client(Workload::Concert).await?;
    let channel = tls(Workload::Concert)?
        .connect(&server.endpoint, Workload::Auth)
        .await?;
    let mut raw = tonic::client::Grpc::new(channel);
    for bytes in [vec![0x0a, 0xff], vec![0xff], vec![0; 5000]] {
        let result = tokio::time::timeout(Duration::from_secs(1), async {
            raw.ready().await.unwrap();
            raw.unary(
                request(tonic::codegen::Bytes::from(bytes), Workload::Concert),
                http::uri::PathAndQuery::from_static(
                    "/relationship.v1.AuthConcertInvalidationService/ReadHead",
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
                .domain_name("auth.svc.internal"),
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
    let empty = head(&mut c).await?;
    assert_eq!(empty.sequence, 0);
    let e = page(&mut c, empty.clone(), 256).await?;
    assert!(e.signals.is_empty());
    assert_eq!(e.next, e.head);
    // Correct TLS and credential authenticate other workloads, but both methods
    // still deny them. A sibling certificate cannot present Concert metadata.
    for w in Workload::ALL
        .into_iter()
        .filter(|w| *w != Workload::Concert)
    {
        let mut sibling = server.client(w).await?;
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
    let u = user(&p).await?;
    let f = Uuid::new_v4();
    let jti = session(&p, u, f).await?;
    let before = head(&mut c).await?;
    store(&p).revoke(&jti).await?;
    let logout = page(&mut c, before.clone(), 256).await?;
    assert!(logout.signals.iter().any(|s|matches!(s.scope.as_ref(),Some(proto::invalidation_signal::Scope::Family(x)) if x.user_id==u.to_string()&&x.session_family_id==f.to_string())));
    let repeated = page(&mut c, before.clone(), 256).await?;
    // Each call has a fresh request ID and checks its own exact echo in page().
    // Redelivery stability applies to retained signals and committed positions.
    assert_eq!(repeated.signals, logout.signals);
    assert_eq!(repeated.next, logout.next);
    assert_eq!(repeated.head, logout.head);
    assert!(!store(&p).session_is_live(u, f, chrono::Utc::now()).await?);
    let f2 = Uuid::new_v4();
    session(&p, u, f2).await?;
    store(&p).revoke_family(f2).await?;
    session(&p, u, Uuid::new_v4()).await?;
    PgUserRepository::new(p.clone(), Arc::new(SystemAuthRuntime))
        .update_password_hash(u, "fixture-changed-secret")
        .await?;
    let prior = head(&mut c).await?;
    let mut tx = p.begin().await?;
    sqlx::query("update users set status='SUSPENDED' where id=$1")
        .bind(u)
        .execute(&mut *tx)
        .await?;
    assert_eq!(head(&mut c).await?, prior);
    tx.rollback().await?;
    assert_eq!(head(&mut c).await?, prior);
    sqlx::query("update users set status='SUSPENDED' where id=$1")
        .bind(u)
        .execute(&p)
        .await?;
    sqlx::query("delete from users where id=$1")
        .bind(u)
        .execute(&p)
        .await?;
    // No JWT is needed to replay a deleted person's revocation.
    let all = page(&mut c, empty.clone(), 256).await?;
    for reason in [
        proto::InvalidationReason::AuthSessionChanged,
        proto::InvalidationReason::AuthCredentialsChanged,
        proto::InvalidationReason::AuthUserAuthorityChanged,
        proto::InvalidationReason::AuthUserRemoved,
    ] {
        assert!(all.signals.iter().any(|s| s.reason == reason as i32));
    }
    let rollback_user = user(&p).await?;
    let rollback_jti = session(&p, rollback_user, Uuid::new_v4()).await?;
    let checkpoint = head(&mut c).await?;
    sqlx::query("alter table auth_concert_invalidation_journal add constraint rpc_fixture_failure check(false) not valid").execute(&p).await?;
    assert!(store(&p).revoke(&rollback_jti).await.is_err());
    assert_eq!(head(&mut c).await?, checkpoint);
    sqlx::query(
        "alter table auth_concert_invalidation_journal drop constraint rpc_fixture_failure",
    )
    .execute(&p)
    .await?;
    let paging_user = user(&p).await?;
    sqlx::query("insert into refresh_tokens(jti,token_hash,user_id,family_id,expires_at) select gen_random_uuid()::text,'fixture-secret',$1,gen_random_uuid(),clock_timestamp()+interval '1 hour' from generate_series(1,300)").bind(paging_user).execute(&p).await?;
    let first = page(&mut c, empty.clone(), 256).await?;
    assert_eq!(first.signals.len(), 256);
    assert!(first.next.as_ref().unwrap().sequence < first.head.as_ref().unwrap().sequence);
    let second = page(&mut c, first.next.unwrap(), 256).await?;
    assert_eq!(second.next, second.head);
    let caught = page(&mut c, second.next.unwrap(), 17).await?;
    assert!(caught.signals.is_empty());
    assert_eq!(caught.next, caught.head);
    for foreign in [
        proto::InvalidationCursor {
            owner_epoch: Uuid::new_v4().to_string(),
            sequence: 0,
        },
        proto::InvalidationCursor {
            owner_epoch: empty.owner_epoch.clone(),
            sequence: head(&mut c).await?.sequence + 1,
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

async fn wait_locked(observer: &PgPool) -> anyhow::Result<()> {
    let stop = Instant::now() + Duration::from_millis(700);
    while Instant::now() < stop {
        let n:i64=sqlx::query_scalar("select count(*) from pg_stat_activity where application_name='mcr_auth_invalidation_rpc_reader' and wait_event_type='Lock' and query like '%auth_concert_invalidation_head%'").fetch_one(observer).await?;
        if n > 0 {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    anyhow::bail!("expected actual reader lock wait")
}
#[tokio::test]
#[ignore = "requires auth_invalidation_rpc_smoke.py private PG16"]
async fn deadline_includes_pool_sql_and_cancellation_releases_work() -> anyhow::Result<()> {
    let observer = pool(3).await?;
    let readers = pool(1).await?;
    let server = Running::start(readers.clone()).await?;
    let mut c = server.client(Workload::Concert).await?;
    let mut lock = observer.begin().await?;
    sqlx::query("lock table auth_concert_invalidation_head in access exclusive mode")
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
    let mut cancel = c.clone();
    let call = tokio::spawn(async move {
        cancel
            .read_head(request(head_query(), Workload::Concert))
            .await
    });
    wait_locked(&observer).await?;
    let cancel_start = Instant::now();
    call.abort();
    assert!(call.await.unwrap_err().is_cancelled());
    // Observe server work, not just client error. SQLx queues transaction
    // rollback on drop; the existing SQL statement timer must still bound any
    // in-flight read to the original one-second operation, without fresh work.
    loop {
        let n:i64=sqlx::query_scalar("select count(*) from pg_stat_activity where application_name='mcr_auth_invalidation_rpc_reader' and state='active' and wait_event_type='Lock'").fetch_one(&observer).await?;
        if n == 0 {
            break;
        }
        anyhow::ensure!(
            cancel_start.elapsed() < Duration::from_millis(1100),
            "cancelled reader exceeded original operation bound"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    lock.rollback().await?;
    head(&mut c).await?;
    let checkpoint = head(&mut c).await?;
    let mut break_storage = observer.begin().await?;
    sqlx::query("alter table auth_concert_invalidation_head rename to rpc_fixture_hidden_head")
        .execute(&mut *break_storage)
        .await?;
    // Roll back the fixture DDL before final success; no migration was edited.
    break_storage.commit().await?;
    assert_eq!(
        c.read_head(request(head_query(), Workload::Concert))
            .await
            .unwrap_err()
            .code(),
        Code::Unavailable
    );
    sqlx::query("alter table rpc_fixture_hidden_head rename to auth_concert_invalidation_head")
        .execute(&observer)
        .await?;
    assert_eq!(head(&mut c).await?, checkpoint);
    drop(c);
    server.close().await?;
    readers.close().await;
    observer.close().await;
    Ok(())
}

#[tokio::test]
#[ignore = "requires auth_invalidation_rpc_smoke.py private PG16 after real restart"]
async fn restart_replays_same_epoch_positions_and_signal_identity() -> anyhow::Result<()> {
    let p = pool(3).await?;
    let saved: (String, i64, serde_json::Value) = sqlx::query_as(
        "select epoch,head,signals from invalidation_rpc_restart_fixture where singleton",
    )
    .fetch_one(&p)
    .await?;
    let server = Running::start(p.clone()).await?;
    let mut c = server.client(Workload::Concert).await?;
    let h = head(&mut c).await?;
    assert_eq!(h.owner_epoch, saved.0);
    assert_eq!(h.sequence, saved.1);
    let mut after = h.clone();
    after.sequence = 0;
    let mut signals = Vec::<Vec<u8>>::new();
    loop {
        let r = page(&mut c, after, 23).await?;
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
