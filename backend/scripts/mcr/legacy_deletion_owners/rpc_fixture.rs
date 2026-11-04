use chrono::{DateTime, Utc};
use platform::{
    grpc_auth::{CallerRegistry, ServiceIdentityClientInterceptor, Workload},
    grpc_tls::TlsMaterial,
};
use sqlx::PgPool;
use std::{os::unix::fs::OpenOptionsExt, path::PathBuf, sync::Arc, time::Duration};
use tonic::{transport::Server, Request};
use uuid::Uuid;
const AUTH: &str = "be42-legacy-auth";
const GATEWAY: &str = "be42-legacy-gateway";
fn root() -> anyhow::Result<PathBuf> {
    Ok(std::env::var("LEGACY_TEST_OWNER_ROOT")?.into())
}
fn tls(role: Workload) -> anyhow::Result<TlsMaterial> {
    let p = root()?.join("pki");
    TlsMaterial::load(
        p.join(format!("{}.crt", role.as_str())),
        p.join(format!("{}.key", role.as_str())),
        p.join("ca.crt"),
    )
}
fn bounded(c: proto::ApplyUserDeletionRequest) -> Request<proto::ApplyUserDeletionRequest> {
    let mut r = Request::new(c);
    r.set_timeout(Duration::from_secs(3));
    r
}
fn command() -> proto::ApplyUserDeletionRequest {
    proto::ApplyUserDeletionRequest {
        saga_id: Uuid::new_v4().to_string(),
        user_id: Uuid::new_v4().to_string(),
        actor_id: Uuid::new_v4().to_string(),
        retention_until: "2027-01-15T08:00:00.123456789+09:00".into(),
    }
}
struct Fixture {
    pool: PgPool,
    endpoint: String,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<Result<(), tonic::transport::Error>>>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(s) = self.stop.take() {
            let _ = s.send(());
        }
    }
}
impl Fixture {
    async fn start() -> anyhow::Result<Self> {
        let pool = owned::owned_pool(&format!("mcr_legacy_{OWNER}")).await?;
        migration_runtime::prepare_database(
            &pool,
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations"),
            migration_runtime::MigrationMode::Startup,
            &[],
        )
        .await?;
        let registry = CallerRegistry::new()
            .register(Workload::Auth, AUTH)?
            .register(Workload::Gateway, GATEWAY)?;
        let mut config = AppConfig::load()?;
        config.seed_enabled = false;
        config.caller_registry = registry.clone();
        config.tls_material = Some(tls(ROLE)?);
        config.actor_delegation =
            platform::actor_delegation::ActorDelegationVerifier::local_parity(ROLE);
        let delegation = config.actor_delegation.clone();
        let state = Arc::new(build_state(&config, pool.clone()).await?);
        let service = platform::grpc_method_policy::guard(
            &POLICY,
            Service::new(GrpcServer::new(state)),
            registry,
            delegation,
        )?;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = format!("https://{}", listener.local_addr()?);
        let (stop, shutdown) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(
            Server::builder()
                .tls_config(tls(ROLE)?.server_config()?)?
                .layer(platform::call_deadline::CallDeadlineLayer)
                .add_service(service)
                .serve_with_incoming_shutdown(
                    tonic::codegen::tokio_stream::wrappers::TcpListenerStream::new(listener),
                    async {
                        let _ = shutdown.await;
                    },
                ),
        );
        Ok(Self {
            pool,
            endpoint,
            stop: Some(stop),
            task: Some(task),
        })
    }
    async fn client(&self, role: Workload, token: &str) -> anyhow::Result<Client> {
        Ok(Client::new(
            tonic::service::interceptor::InterceptedService::new(
                tls(role)?.connect(&self.endpoint, ROLE).await?,
                ServiceIdentityClientInterceptor::new(role, token)?,
            ),
        ))
    }
    async fn finish(mut self) -> anyhow::Result<()> {
        let _ = self.stop.take().unwrap().send(());
        tokio::time::timeout(Duration::from_secs(3), self.task.take().unwrap()).await???;
        self.pool.close().await;
        Ok(())
    }
}
async fn proof_count(p: &PgPool, user: Uuid) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar(&format!(
        "select count(*) from {OWNER}_deletion_receipts_v1 where user_id=$1"
    ))
    .bind(user)
    .fetch_one(p)
    .await?)
}
async fn clean(p: &PgPool, user: Uuid) -> anyhow::Result<bool> {
    Ok(
        sqlx::query_scalar(&format!("select {OWNER}_deletion_clean_v1($1)"))
            .bind(user)
            .fetch_one(p)
            .await?,
    )
}

#[tokio::test]
#[ignore = "fresh bounded private PG16/Redis/PKI offer required"]
async fn populated_actual_rpc_atomic_replay_rebound_fence_and_original_clock() -> anyhow::Result<()>
{
    let f = Fixture::start().await?;
    let mut auth = f.client(Workload::Auth, AUTH).await?;
    let mut wrong = f.client(Workload::Gateway, GATEWAY).await?;
    let c = command();
    let user = Uuid::parse_str(&c.user_id)?;
    let peer = seed::populate(&f.pool, OWNER, user).await?;
    assert!(wrong.apply_user_deletion(bounded(c.clone())).await.is_err());
    assert!(auth
        .apply_user_deletion(Request::new(c.clone()))
        .await
        .is_err()); // unknown clock
    assert_eq!(proof_count(&f.pool, user).await?, 0);
    assert!(!clean(&f.pool, user).await?);
    let mut concurrent = auth.clone();
    let (a, b) = tokio::join!(
        auth.apply_user_deletion(bounded(c.clone())),
        concurrent.apply_user_deletion(bounded(c.clone()))
    );
    let a = a?.into_inner();
    let b = b?.into_inner();
    assert_eq!(a, b);
    assert!(a.applied);
    let r = a.committed_receipt.as_ref().unwrap();
    assert_eq!(r.version, 1);
    assert_eq!(r.owner, OWNER.to_uppercase());
    assert_eq!(
        (&r.saga_id, &r.user_id, &r.actor_id),
        (&c.saga_id, &c.user_id, &c.actor_id)
    );
    let deadline = DateTime::parse_from_rfc3339(&c.retention_until)?;
    assert_eq!(r.retention_seconds, Some(deadline.timestamp()));
    assert_eq!(r.retention_nanos, Some(123456789));
    assert!(r.fence_revision > 0 && r.effect_xid.parse::<u64>()? > 0);
    assert!(DateTime::parse_from_rfc3339(&r.recorded_at).is_ok());
    assert!(clean(&f.pool, user).await?);
    seed::assert_peer_survives(&f.pool, OWNER, peer).await?;
    if OWNER == "post" {
        // A delayed local completion must not restore its pre-deletion claim body.
        sqlx::query("update event_publications set payload='{\"content\":\"stale private body\"}' where event_id in(select event_id from post_event_outbox where actor_id=$1)").bind(user).execute(&f.pool).await?;
        assert!(clean(&f.pool, user).await?);
        let pending: bool = sqlx::query_scalar(
            "select bool_and(not published) from post_event_outbox where actor_id=$1",
        )
        .bind(user)
        .fetch_one(&f.pool)
        .await?;
        assert!(pending); // No fabricated publication success in canonical outbox.
    }

    for part in 0..4 {
        let mut bad = c.clone();
        match part {
            0 => bad.saga_id = Uuid::new_v4().to_string(),
            1 => bad.user_id = Uuid::new_v4().to_string(),
            2 => bad.actor_id = Uuid::new_v4().to_string(),
            _ => bad.retention_until = "2027-01-15T08:00:00.123456790+09:00".into(),
        };
        assert!(auth.apply_user_deletion(bounded(bad)).await.is_err());
    }
    assert_eq!(proof_count(&f.pool, user).await?, 1);
    for sql in [format!("update {OWNER}_deletion_receipts_v1 set effect_xid=pg_current_xact_id() where user_id='{user}'"),format!("delete from {OWNER}_deletion_fences_v1 where user_id='{user}'"),format!("delete from {OWNER}_user_deletion_tombstones where user_id='{user}'"),format!("delete from {OWNER}_audit_events where id='{}'",c.saga_id)]{assert!(sqlx::query(&sql).execute(&f.pool).await.is_err());}
    assert!(seed::populate(&f.pool, OWNER, user).await.is_err()); // stale producer cannot recreate
    assert_eq!(
        auth.apply_user_deletion(bounded(c.clone()))
            .await?
            .into_inner(),
        a
    );

    // A receipt INSERT failure rolls back real populated cleanup, fence and audit.
    let rollback = command();
    let u = Uuid::parse_str(&rollback.user_id)?;
    seed::populate(&f.pool, OWNER, u).await?;
    sqlx::raw_sql(&format!("create function legacy_fixture_reject_receipt() returns trigger language plpgsql as $$ begin if NEW.user_id='{u}'::uuid then raise exception 'controlled rollback'; end if; return NEW; end $$; create trigger legacy_fixture_reject_receipt before insert on {OWNER}_deletion_receipts_v1 for each row execute function legacy_fixture_reject_receipt();")).execute(&f.pool).await?;
    assert!(auth
        .apply_user_deletion(bounded(rollback.clone()))
        .await
        .is_err());
    assert!(!clean(&f.pool, u).await?);
    assert_eq!(proof_count(&f.pool, u).await?, 0);
    let n: i64 = sqlx::query_scalar(&format!(
        "select count(*) from {OWNER}_user_deletion_tombstones where user_id=$1"
    ))
    .bind(u)
    .fetch_one(&f.pool)
    .await?;
    assert_eq!(n, 0);
    let n: i64 = sqlx::query_scalar(&format!(
        "select count(*) from {OWNER}_audit_events where id=$1"
    ))
    .bind(Uuid::parse_str(&rollback.saga_id)?)
    .fetch_one(&f.pool)
    .await?;
    assert_eq!(n, 0);
    sqlx::raw_sql(&format!("drop trigger legacy_fixture_reject_receipt on {OWNER}_deletion_receipts_v1; drop function legacy_fixture_reject_receipt();")).execute(&f.pool).await?;
    auth.apply_user_deletion(bounded(rollback)).await?;
    assert!(clean(&f.pool, u).await?);

    // A standalone permanent fence cannot commit without the same effect receipt.
    let forged = command();
    let u = Uuid::parse_str(&forged.user_id)?;
    let mut tx = f.pool.begin().await?;
    sqlx::query(&format!("insert into {OWNER}_user_deletion_tombstones(user_id,saga_id,actor_id,retention_until) values($1,$2,$3,$4)"))
        .bind(u).bind(Uuid::parse_str(&forged.saga_id)?).bind(Uuid::parse_str(&forged.actor_id)?)
        .bind(DateTime::parse_from_rfc3339(&forged.retention_until)?.with_timezone(&Utc)).execute(&mut *tx).await?;
    sqlx::query(&format!(
        "insert into {OWNER}_deletion_fences_v1(user_id) values($1)"
    ))
    .bind(u)
    .execute(&mut *tx)
    .await?;
    assert!(tx.commit().await.is_err());
    assert_eq!(proof_count(&f.pool, u).await?, 0);

    // Hold the actual subject lock beyond caller expiry. No late owner ACK/effect.
    let expired = command();
    let u = Uuid::parse_str(&expired.user_id)?;
    seed::populate(&f.pool, OWNER, u).await?;
    let mut lock = f.pool.begin().await?;
    sqlx::query(&format!("select {OWNER}_lock_subject_ids($1::uuid[])"))
        .bind(vec![u])
        .execute(&mut *lock)
        .await?;
    let mut request = Request::new(expired.clone());
    request.set_timeout(Duration::from_millis(40));
    assert!(auth.apply_user_deletion(request).await.is_err());
    lock.rollback().await?;
    tokio::time::sleep(Duration::from_millis(60)).await;
    assert_eq!(proof_count(&f.pool, u).await?, 0);
    assert!(!clean(&f.pool, u).await?);
    auth.apply_user_deletion(bounded(expired)).await?;
    assert!(clean(&f.pool, u).await?);
    f.finish().await
}

#[tokio::test]
#[ignore = "external bounded controller owns lifetime and actual restart"]
async fn serve_actual_owner_for_auth_worker() -> anyhow::Result<()> {
    let f = Fixture::start().await?;
    use std::io::Write;
    let endpoint = root()?.join(format!("{OWNER}-endpoint"));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(endpoint)?;
    file.write_all(f.endpoint.as_bytes())?;
    file.sync_all()?;
    while !root()?.join(format!("{OWNER}-stop")).exists() {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    f.finish().await
}
