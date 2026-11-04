use super::*;
use async_trait::async_trait;
use platform::grpc_auth::Workload;
use prost::Message;
use sqlx::{PgPool, Row};
use std::sync::Arc;

const OWNER: &str = "auth";
const REVISION: &str = "auth_concert_relationship_revision";

pub(super) async fn pool(producer: bool) -> anyhow::Result<PgPool> {
    let name = if producer {
        format!("mcr_owner_{OWNER}_producer")
    } else {
        format!("mcr_owner_{OWNER}")
    };
    let key = if producer {
        "MCR_AUTHORITY_PRODUCER_URL"
    } else {
        "MCR_AUTHORITY_OWNER_URL"
    };
    let p = sqlx::postgres::PgPoolOptions::new()
        .max_connections(12)
        .connect(&std::env::var(key)?)
        .await?;
    let row=sqlx::query("select current_database() db,current_setting('listen_addresses') listeners,(select system_identifier::text from pg_control_system()) system").fetch_one(&p).await?;
    anyhow::ensure!(
        row.get::<String, _>("db") == name
            && row.get::<String, _>("listeners").is_empty()
            && row.get::<String, _>("system") == std::env::var("MCR_AUTHORITY_SYSTEM")?,
        "foreign fixture"
    );
    Ok(p)
}
pub(super) async fn setup(p: &PgPool, producer: &PgPool) -> anyhow::Result<()> {
    let m = sqlx::migrate::Migrator::new(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("migrations")
            .as_path(),
    )
    .await?;
    m.run(p).await?;
    sqlx::raw_sql("create table if not exists fixture_scopes(id uuid primary key,digest bytea not null,finished bool not null default false);create table if not exists fixture_registrations(id uuid primary key,identity bytea not null,digest bytea not null);create table if not exists fixture_receipts(xid text primary key,digest bytea not null);").execute(producer).await?;
    Ok(())
}
/// Fixture producer uses its own database. The production owner only calls the
/// SettlementResolver interface, never a foreign database. No guessed outcomes.
#[derive(Clone)]
pub(super) struct ActualProducer(pub(super) PgPool);
#[async_trait]
impl SettlementResolver for ActualProducer {
    fn configured(&self, p: proto::Producer) -> bool {
        p == proto::Producer::Concert
    }
    async fn resolve_scope(
        &self,
        q: proto::ResolveScopeRequest,
        _: &OperationBudget,
    ) -> Result<proto::ResolveScopeResponse, tonic::Status> {
        let scope = q
            .scope
            .ok_or_else(|| tonic::Status::invalid_argument("scope"))?;
        let found: Option<(Vec<u8>, bool)> =
            sqlx::query_as("select digest,finished from fixture_scopes where id=$1")
                .bind(Uuid::parse_str(&scope.scope_id).unwrap())
                .fetch_optional(&self.0)
                .await
                .map_err(|_| tonic::Status::unavailable("fixture"))?;
        let finished =
            found.is_some_and(|(digest, finished)| digest == scope.binding_digest && finished);
        Ok(proto::ResolveScopeResponse {
            scope: Some(scope),
            sealed_and_finished: finished,
        })
    }
    async fn resolve_transaction(
        &self,
        q: proto::ResolveTransactionRequest,
        _: &OperationBudget,
    ) -> Result<proto::ResolveTransactionResponse, tonic::Status> {
        let identity = q
            .transaction
            .ok_or_else(|| tonic::Status::invalid_argument("transaction"))?;
        let registered: Option<(Vec<u8>, Vec<u8>)> =
            sqlx::query_as("select identity,digest from fixture_registrations where id=$1")
                .bind(Uuid::parse_str(&q.registration_id).unwrap())
                .fetch_optional(&self.0)
                .await
                .map_err(|_| tonic::Status::unavailable("fixture"))?;
        let Some((bytes, digest)) = registered else {
            return Err(tonic::Status::unavailable("unknown registration"));
        };
        if bytes != identity.encode_to_vec() {
            return Err(tonic::Status::unavailable("identity mismatch"));
        }
        let status: Option<String> = sqlx::query_scalar("select pg_xact_status($1::text::xid8)")
            .bind(&identity.full_xid)
            .fetch_one(&self.0)
            .await
            .map_err(|_| tonic::Status::unavailable("fixture"))?;
        let receipt: Option<Vec<u8>> =
            sqlx::query_scalar("select digest from fixture_receipts where xid=$1")
                .bind(&identity.full_xid)
                .fetch_optional(&self.0)
                .await
                .map_err(|_| tonic::Status::unavailable("fixture"))?;
        let status = match (status.as_deref(), receipt.as_ref()) {
            (Some("in progress"), _) => proto::TransactionStatus::InProgress,
            (Some("aborted"), _) => proto::TransactionStatus::Aborted,
            (Some("committed"), Some(r)) if r == &identity.effect_binding_digest => {
                proto::TransactionStatus::Committed
            }
            _ => proto::TransactionStatus::Unknown,
        };
        Ok(proto::ResolveTransactionResponse {
            transaction: Some(identity),
            binding_digest: digest,
            status: status as i32,
            receipt_evidence: receipt.unwrap_or_default(),
        })
    }
}
pub(super) async fn fixture(p: &PgPool) -> anyhow::Result<(Uuid, Uuid, Uuid)> {
    let actor = Uuid::new_v4();
    let subject = Uuid::new_v4();
    let family = Uuid::new_v4();
    for id in [actor, subject] {
        sqlx::query("insert into users(id,primary_email,password_hash,status) values($1,$2,'fixture-password','ACTIVE')").bind(id).bind(format!("{id}@example.invalid")).execute(p).await?;
    }
    sqlx::query("insert into refresh_tokens(jti,token_hash,user_id,family_id,expires_at) values($1,'fixture-refresh',$2,$3,clock_timestamp()+interval '1 hour')").bind(Uuid::new_v4().to_string()).bind(actor).bind(family).execute(p).await?;
    Ok((actor, subject, family))
}
pub(super) fn binding(actor: Uuid, family: Uuid) -> proto::GuardBinding {
    let event = Uuid::new_v4();
    proto::GuardBinding {
        actor_id: actor.to_string(),
        event_id: event.to_string(),
        session_family_id: family.to_string(),
        request_id: Uuid::new_v4().to_string(),
        operation: "test.owner".into(),
        subject_ids: vec![],
        expires_unix_micros: chrono::Utc::now().timestamp_micros() + 4_800_000,
        parent_digest: vec![1; 32],
        parent_body_digest: vec![2; 32],
        body_digest: vec![3; 32],
        parent_method: "/concert.product.v1.ProductService/Dispatch".into(),
        http_method: "POST".into(),
        http_target: format!("/api/concert/events/{event}/test"),
    }
}
fn digest(b: &proto::GuardBinding) -> Vec<u8> {
    use platform::actor_delegation::owner_binding::{binding_digest, OwnerBinding};
    binding_digest(&OwnerBinding {
        actor_id: Uuid::parse_str(&b.actor_id).unwrap(),
        event_id: Uuid::parse_str(&b.event_id).unwrap(),
        session_family_id: Uuid::parse_str(&b.session_family_id).unwrap(),
        request_id: Uuid::parse_str(&b.request_id).unwrap(),
        operation: b.operation.clone(),
        expires_unix_micros: b.expires_unix_micros,
        parent_digest: b.parent_digest.clone().try_into().unwrap(),
        parent_body_digest: b.parent_body_digest.clone().try_into().unwrap(),
        body_digest: b.body_digest.clone().try_into().unwrap(),
        parent_method: b.parent_method.clone(),
        http_method: b.http_method.clone(),
        http_target: b.http_target.clone(),
    })
    .unwrap()
    .to_vec()
}
pub(super) async fn scope(
    p: &PgPool,
    b: &proto::GuardBinding,
) -> anyhow::Result<proto::ProducerScope> {
    let id = Uuid::new_v4();
    let digest = digest(b);
    sqlx::query("insert into fixture_scopes(id,digest) values($1,$2)")
        .bind(id)
        .bind(&digest)
        .execute(p)
        .await?;
    Ok(proto::ProducerScope {
        scope_id: id.to_string(),
        instance_id: Uuid::new_v4().to_string(),
        binding_digest: digest,
    })
}
fn clock() -> OperationBudget {
    OperationBudget::new(Instant::now() + Duration::from_secs(2)).unwrap()
}
async fn grant(
    store: &PgOwnerAuthority,
    producer: &PgPool,
    b: &proto::GuardBinding,
) -> anyhow::Result<proto::AcquireResponse> {
    Ok(store
        .acquire(
            proto::AcquireRequest {
                binding: Some(b.clone()),
                response_scope: Some(scope(producer, b).await?),
            },
            &clock(),
        )
        .await?)
}
pub(super) fn completion(
    b: &proto::GuardBinding,
    g: &proto::AcquireResponse,
) -> proto::CompleteRequest {
    proto::CompleteRequest {
        view: g.view.clone(),
        binding: Some(b.clone()),
        completion_secret: g.completion_secret.clone(),
    }
}
pub(super) async fn finish_scopes(p: &PgPool) -> anyhow::Result<()> {
    sqlx::query("update fixture_scopes set finished=true")
        .execute(p)
        .await?;
    Ok(())
}
async fn pending(p: &PgPool) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar(
        "select count(*) from pg_prepared_xacts where database=current_database()",
    )
    .fetch_one(p)
    .await?)
}

#[tokio::test]
#[ignore = "requires owned private PG16 and frozen artifact runner"]
async fn actual_transactions_enrollment_sealing_and_quarantine() -> anyhow::Result<()> {
    let p = pool(false).await?;
    let prod = pool(true).await?;
    setup(&p, &prod).await?;
    let (actor, subject, family) = fixture(&p).await?;
    let b = binding(actor, family);
    let (store, _producer) = super::transport::rpc_store(&p, &prod).await?;
    let g = grant(&store, &prod, &b).await?;
    assert_eq!(pending(&p).await?, 1);
    assert_eq!(g.subjects.len(), 1);
    // A real mutation waits on the prepared source transaction (blocking PID 0).
    let wp = p.clone();
    let mutation = tokio::spawn(async move {
        sqlx::query("update users set password_hash='changed-by-waiting-writer' where id=$1")
            .bind(actor)
            .execute(&wp)
            .await
    });
    let stop = Instant::now() + Duration::from_secs(1);
    loop {
        let waits:bool=sqlx::query_scalar("select exists(select 1 from pg_stat_activity where datname=current_database() and 0=any(pg_blocking_pids(pid)))").fetch_one(&p).await?;
        if waits {
            break;
        }
        anyhow::ensure!(Instant::now() < stop, "writer did not wait");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let enrolled = tokio::time::timeout(
        Duration::from_millis(500),
        store.enroll(
            proto::EnrollRequest {
                view: g.view.clone(),
                binding: Some(b.clone()),
                subject_ids: vec![subject.to_string()],
            },
            &clock(),
        ),
    )
    .await??;
    assert_eq!(enrolled.subjects.len(), 2);
    assert!(!mutation.is_finished());
    let mut bad = g.view.clone().unwrap();
    bad.view_secret[0] ^= 1;
    assert_eq!(
        store
            .check(
                proto::CheckRequest {
                    view: Some(bad),
                    binding: Some(b.clone())
                },
                &clock()
            )
            .await
            .unwrap_err()
            .code(),
        tonic::Code::PermissionDenied
    );
    let mut effect = prod.begin().await?;
    let row=sqlx::query("select pg_current_xact_id()::text xid,pg_backend_pid() pid,(extract(epoch from backend_start)*1000000)::bigint started,(select system_identifier::text from pg_control_system()) system,(select oid::bigint from pg_database where datname=current_database()) dboid from pg_stat_activity where pid=pg_backend_pid()").fetch_one(&mut *effect).await?;
    let identity = proto::TransactionIdentity {
        producer: proto::Producer::Concert as i32,
        full_xid: row.get("xid"),
        backend_pid: row.get("pid"),
        backend_started_unix_micros: row.get("started"),
        effect_binding_digest: vec![9; 32],
        system_identifier: row.get("system"),
        database_oid: row.get::<i64, _>("dboid") as u32,
        settlement_reference: Uuid::new_v4().as_bytes().to_vec(),
        producer_instance_id: Uuid::new_v4().to_string(),
    };
    let q = proto::TrackRequest {
        view: g.view.clone(),
        binding: Some(b.clone()),
        transaction: Some(identity.clone()),
    };
    assert_eq!(
        store
            .track(q.clone(), Workload::Post, &clock())
            .await
            .unwrap_err()
            .code(),
        tonic::Code::PermissionDenied
    );
    let tracked = store.track(q.clone(), Workload::Concert, &clock()).await?;
    assert_eq!(
        tracked,
        store.track(q.clone(), Workload::Concert, &clock()).await?
    );
    sqlx::query("insert into fixture_registrations(id,identity,digest) values($1,$2,$3)")
        .bind(Uuid::parse_str(&tracked.registration_id)?)
        .bind(identity.encode_to_vec())
        .bind(digest(&b))
        .execute(&prod)
        .await?;
    let mut aborting = prod.begin().await?;
    let row=sqlx::query("select pg_current_xact_id()::text xid,pg_backend_pid() pid,(extract(epoch from backend_start)*1000000)::bigint started from pg_stat_activity where pid=pg_backend_pid()").fetch_one(&mut *aborting).await?;
    let aborted_identity = proto::TransactionIdentity {
        full_xid: row.get("xid"),
        backend_pid: row.get("pid"),
        backend_started_unix_micros: row.get("started"),
        settlement_reference: Uuid::new_v4().as_bytes().to_vec(),
        ..identity.clone()
    };
    let aborted = store
        .track(
            proto::TrackRequest {
                transaction: Some(aborted_identity.clone()),
                ..q.clone()
            },
            Workload::Concert,
            &clock(),
        )
        .await?;
    sqlx::query("insert into fixture_registrations(id,identity,digest) values($1,$2,$3)")
        .bind(Uuid::parse_str(&aborted.registration_id)?)
        .bind(aborted_identity.encode_to_vec())
        .bind(digest(&b))
        .execute(&prod)
        .await?;
    let mut forged_completion = completion(&b, &g);
    forged_completion.completion_secret[0] ^= 1;
    assert_eq!(
        store
            .complete(forged_completion, &clock())
            .await
            .unwrap_err()
            .code(),
        tonic::Code::PermissionDenied
    );
    // Root is still assembling: even zero committed effects cannot release.
    assert!(
        !store
            .complete(completion(&b, &g), &clock())
            .await?
            .completed
    );
    assert_eq!(pending(&p).await?, 1);
    assert!(store.track(q, Workload::Concert, &clock()).await.is_err());
    assert!(store
        .enroll(
            proto::EnrollRequest {
                view: g.view.clone(),
                binding: Some(b.clone()),
                subject_ids: vec![]
            },
            &clock()
        )
        .await
        .is_err());
    finish_scopes(&prod).await?;
    assert!(
        !store
            .complete(completion(&b, &g), &clock())
            .await?
            .completed
    );
    assert!(!mutation.is_finished());
    sqlx::query("insert into fixture_receipts(xid,digest) values($1,$2)")
        .bind(&identity.full_xid)
        .bind(&identity.effect_binding_digest)
        .execute(&mut *effect)
        .await?;
    effect.commit().await?;
    aborting.rollback().await?;
    assert!(
        store
            .complete(completion(&b, &g), &clock())
            .await?
            .completed
    );
    tokio::time::timeout(Duration::from_secs(1), mutation).await???;
    assert_eq!(pending(&p).await?, 0);
    assert!(
        store
            .complete(completion(&b, &g), &clock())
            .await?
            .completed
    );
    println!("actual-owner={OWNER}: prepared exclusion, writer wait, enrollment without relock, capability rejection, real xid IN_PROGRESS/COMMITTED receipt, scope seal, registration rejection, idempotent completion PASS");
    p.close().await;
    prod.close().await;
    Ok(())
}

#[tokio::test]
#[ignore = "requires owned private PG16 and frozen artifact runner"]
async fn cancellation_and_capacity_survive_expiry() -> anyhow::Result<()> {
    let p = pool(false).await?;
    let prod = pool(true).await?;
    setup(&p, &prod).await?;
    let (a, _, f) = fixture(&p).await?;
    let store = PgOwnerAuthority::new(p.clone(), Arc::new(ActualProducer(prod.clone())))?;
    let mut writer = p.begin().await?;
    sqlx::query(&format!(
        "select * from {REVISION} where singleton for update"
    ))
    .execute(&mut *writer)
    .await?;
    let mut b = binding(a, f);
    b.expires_unix_micros = chrono::Utc::now().timestamp_micros() + 350_000;
    let sc = scope(&prod, &b).await?;
    let short = OperationBudget::new(Instant::now() + Duration::from_millis(150))?;
    assert!(store
        .acquire(
            proto::AcquireRequest {
                binding: Some(b),
                response_scope: Some(sc)
            },
            &short
        )
        .await
        .is_err());
    writer.rollback().await?;
    tokio::time::sleep(Duration::from_millis(400)).await;
    store.recover(&clock()).await?;
    assert_eq!(pending(&p).await?, 0);
    let count: i64 = sqlx::query_scalar(
        "select count(*) from concert_owner_guard_leases where state='PREPARING'",
    )
    .fetch_one(&p)
    .await?;
    assert_eq!(count, 0);
    let mut grants = Vec::new();
    for _ in 0..MAX_GUARDS {
        let b = binding(a, f);
        let g = grant(&store, &prod, &b).await?;
        grants.push((b, g));
    }
    assert_eq!(pending(&p).await?, 16);
    let extra = binding(a, f);
    assert_eq!(
        grant(&store, &prod, &extra)
            .await
            .unwrap_err()
            .downcast_ref::<tonic::Status>()
            .unwrap()
            .code(),
        tonic::Code::ResourceExhausted
    );
    // Expiry alone does not release any of the sixteen still-open root scopes.
    tokio::time::sleep(Duration::from_secs(5)).await;
    store.recover(&clock()).await?;
    assert_eq!(pending(&p).await?, 16);
    finish_scopes(&prod).await?;
    store.recover(&clock()).await?;
    assert_eq!(pending(&p).await?, 0);
    println!("actual-owner={OWNER}: cancelled acquisition recovered with exact preparer identity; 16 durable quarantines counted after expiry and released only after actual scope finish PASS");
    p.close().await;
    prod.close().await;
    Ok(())
}

#[tokio::test]
#[ignore = "runner phase before physical PG restart"]
async fn prepare_restart() -> anyhow::Result<()> {
    let p = pool(false).await?;
    let prod = pool(true).await?;
    setup(&p, &prod).await?;
    let (a, _, f) = fixture(&p).await?;
    let b = binding(a, f);
    let store = PgOwnerAuthority::new(p.clone(), Arc::new(ActualProducer(prod.clone())))?;
    let _g = grant(&store, &prod, &b).await?;
    assert_eq!(pending(&p).await?, 1);
    sqlx::query(&format!(
        "create table fixture_restart as select owner_epoch,revision from {REVISION}"
    ))
    .execute(&p)
    .await?;
    p.close().await;
    prod.close().await;
    Ok(())
}
#[tokio::test]
#[ignore = "runner phase after physical PG restart"]
async fn recover_restart() -> anyhow::Result<()> {
    let p = pool(false).await?;
    let prod = pool(true).await?;
    let store = PgOwnerAuthority::new(p.clone(), Arc::new(ActualProducer(prod.clone())))?;
    assert_eq!(pending(&p).await?, 1);
    let same:bool=sqlx::query_scalar(&format!("select exists(select 1 from fixture_restart r join {REVISION} s using(owner_epoch,revision))")).fetch_one(&p).await?;
    assert!(same);
    store.verify_ready(&clock()).await?;
    tokio::time::sleep(Duration::from_secs(5)).await;
    store.recover(&clock()).await?;
    assert_eq!(pending(&p).await?, 1);
    finish_scopes(&prod).await?;
    store.recover(&clock()).await?;
    assert_eq!(pending(&p).await?, 0);
    println!("actual-owner={OWNER}: physical restart retained GID and epoch; unknown root quarantined; actual scope finish recovered PASS");
    p.close().await;
    prod.close().await;
    Ok(())
}

#[tokio::test]
#[ignore = "requires owned private PG16"]
async fn actual_security_generation_distinguishes_rotation_and_invalidation() -> anyhow::Result<()>
{
    use auth_service::{
        adapters::out::{cache::session_store::PgRefreshTokenStore, runtime::SystemAuthRuntime},
        application::ports::RefreshTokenStorePort,
        domain::session::RefreshTokenRecord,
    };
    let p = pool(false).await?;
    let prod = pool(true).await?;
    setup(&p, &prod).await?;
    let (actor, other, family) = fixture(&p).await?;
    async fn generation(p: &PgPool, id: Uuid) -> anyhow::Result<i64> {
        Ok(sqlx::query_scalar(
            "select generation from auth_concert_user_security_generation where user_id=$1",
        )
        .bind(id)
        .fetch_one(p)
        .await?)
    }
    let initial = generation(&p, actor).await?;
    let other_initial = generation(&p, other).await?;
    let sessions = PgRefreshTokenStore::new(p.clone(), Arc::new(SystemAuthRuntime));
    let old: String = sqlx::query_scalar(
        "select jti from refresh_tokens where user_id=$1 and family_id=$2 and revoked_at is null",
    )
    .bind(actor)
    .bind(family)
    .fetch_one(&p)
    .await?;
    let next = RefreshTokenRecord {
        user_id: actor,
        family_id: family,
        jti: Uuid::new_v4().to_string(),
        token_hash: "next-refresh".into(),
        expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
    };
    sessions
        .rotate(&old, "fixture-refresh", &(&next).into())
        .await?;
    assert_eq!(
        generation(&p, actor).await?,
        initial,
        "actual valid refresh rotation must remain stable"
    );
    let login = RefreshTokenRecord {
        jti: Uuid::new_v4().to_string(),
        family_id: Uuid::new_v4(),
        ..next.clone()
    };
    sessions.store(&(&login).into()).await?;
    assert_eq!(
        generation(&p, actor).await?,
        initial,
        "unrelated login stable"
    );
    sessions.revoke(&next.jti).await?;
    let revoked = generation(&p, actor).await?;
    assert!(revoked > initial);
    assert_eq!(generation(&p, other).await?, other_initial);
    let mut rollback = p.begin().await?;
    sqlx::query("update users set password_hash='rolled-back' where id=$1")
        .bind(actor)
        .execute(&mut *rollback)
        .await?;
    rollback.rollback().await?;
    assert_eq!(generation(&p, actor).await?, revoked);
    sqlx::query("update users set password_hash='changed' where id=$1")
        .bind(actor)
        .execute(&p)
        .await?;
    let password = generation(&p, actor).await?;
    assert!(password > revoked);
    sqlx::query("update users set status='SUSPENDED' where id=$1")
        .bind(actor)
        .execute(&p)
        .await?;
    let suspended = generation(&p, actor).await?;
    assert!(suspended > password);
    // A ROTATED label with no real same-family successor cannot bypass it.
    // Password changes correctly revoked the earlier login. Use a fresh live
    // record so the fake rotation tests a security transition, not a no-op on
    // an already invalid token.
    let fake = RefreshTokenRecord {
        jti: Uuid::new_v4().to_string(),
        family_id: Uuid::new_v4(),
        ..next.clone()
    };
    sessions.store(&(&fake).into()).await?;
    let before_fake = generation(&p, actor).await?;
    sqlx::query("update refresh_tokens set revoked_at=transaction_timestamp(),rotated_at=transaction_timestamp(),revoked_reason='ROTATED',successor_jti=$2 where jti=$1").bind(&fake.jti).bind(Uuid::new_v4().to_string()).execute(&p).await?;
    assert!(generation(&p, actor).await? > before_fake);
    sqlx::query("delete from users where id=$1")
        .bind(actor)
        .execute(&p)
        .await?;
    let deleted = generation(&p, actor).await?;
    assert!(deleted > suspended);
    sqlx::query("insert into users(id,primary_email,password_hash,status) values($1,$2,'recreated','ACTIVE')").bind(actor).bind(format!("{actor}@example.invalid")).execute(&p).await?;
    assert!(
        generation(&p, actor).await? > deleted,
        "UUID reuse cannot reset tombstone"
    );
    assert!(
        sqlx::query("delete from auth_concert_user_security_generation where user_id=$1")
            .bind(actor)
            .execute(&p)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("truncate auth_concert_user_security_generation")
            .execute(&p)
            .await
            .is_err()
    );
    assert_eq!(generation(&p, other).await?, other_initial);
    let active = RefreshTokenRecord {
        user_id: actor,
        jti: Uuid::new_v4().to_string(),
        family_id: Uuid::new_v4(),
        ..next.clone()
    };
    sessions.store(&(&active).into()).await?;
    let before_truncate = generation(&p, actor).await?;
    sqlx::query("truncate refresh_tokens").execute(&p).await?;
    assert!(generation(&p, actor).await? > before_truncate);
    let before_users = generation(&p, actor).await?;
    sqlx::query("truncate users cascade").execute(&p).await?;
    assert!(generation(&p, actor).await? > before_users);
    println!("Auth per-user generations: actual rotation/login stable; logout/password/suspension/delete/recreate monotonic; rollback and unrelated user stable; tamper rejected PASS");
    p.close().await;
    prod.close().await;
    Ok(())
}
