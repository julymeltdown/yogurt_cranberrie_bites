//! Real PostgreSQL storage qualification only. Owner acknowledgements below
//! are explicit storage fixtures, never canonical guard/HTTP qualification.
use super::*;
use crate::{
    adapters::r#in::grpc::concert_feed::proto, concert_feed::read_admission::ReadOperation,
};
use chrono::{Duration as ChronoDuration, Utc};
use platform::actor_delegation::authority_deadline::AuthorityRpcDeadline;
use post_application::concert_feed::Context;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::{os::unix::fs::MetadataExt, path::PathBuf, str::FromStr, time::Duration};

struct Database {
    pool: PgPool,
    admin: PgPool,
    schema: String,
}
impl Database {
    async fn start() -> anyhow::Result<Self> {
        let url = std::env::var("MCR_FEED_NATIVE_DATABASE_URL")?;
        let root = PathBuf::from(std::env::var("MCR_FEED_NATIVE_OWNER_ROOT")?);
        let info = std::fs::symlink_metadata(&root)?;
        let marker = root.join("mcr-owner.json");
        let marker_info = std::fs::symlink_metadata(&marker)?;
        let proof: serde_json::Value = serde_json::from_slice(&std::fs::read(&marker)?)?;
        anyhow::ensure!(
            root.is_absolute()
                && root.canonicalize()? == root
                && info.is_dir()
                && marker_info.is_file()
                && info.uid() == marker_info.uid()
                && proof
                    == serde_json::json!({"token":std::env::var("MCR_FEED_NATIVE_OWNER_TOKEN")?,"uid":info.uid()})
                && std::env::var("MCR_FEED_NATIVE_OWNER_IDENTITY")?
                    == format!("{}:{}", info.dev(), info.ino()),
            "owned native controller required"
        );
        let options = PgConnectOptions::from_str(&url)
            .map_err(|_| anyhow::anyhow!("native PostgreSQL configuration invalid"))?;
        anyhow::ensure!(
            options.get_socket() == Some(&root.join("socket"))
                && root.join("socket").canonicalize()? == root.join("socket")
                && options.get_options().is_none()
                && options.get_port() == 5432
                && options.get_database() == Some("mcr_feed_native")
                && options.get_username() == "mcr_owner",
            "owned native socket required"
        );
        let admin = PgPoolOptions::new()
            .max_connections(1)
            .connect_with(options.clone())
            .await?;
        let settings=sqlx::query("select current_setting('data_directory') as data,current_setting('listen_addresses') as listen,current_setting('unix_socket_directories') as socket")
            .fetch_one(&admin).await?;
        anyhow::ensure!(
            settings.try_get::<String, _>("data")? == root.join("data").to_string_lossy()
                && settings.try_get::<String, _>("listen")?.is_empty()
                && settings.try_get::<String, _>("socket")?
                    == root.join("socket").to_string_lossy(),
            "native PostgreSQL ownership mismatch"
        );
        let schema = format!("feed_read_{}", Uuid::new_v4().simple());
        sqlx::query(&format!("create schema {schema}"))
            .execute(&admin)
            .await?;
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect_with(options.options([("search_path", schema.clone())]))
            .await?;
        assert_eq!(
            include_str!("read_tracking.sql"),
            include_str!("../../migrations/0032_concert_feed_read_tracking.sql")
        );
        sqlx::raw_sql(include_str!("read_tracking.sql"))
            .execute(&pool)
            .await?;
        Ok(Self {
            pool,
            admin,
            schema,
        })
    }
    async fn finish(self) -> anyhow::Result<()> {
        self.pool.close().await;
        sqlx::query(&format!("drop schema {} cascade", self.schema))
            .execute(&self.admin)
            .await?;
        self.admin.close().await;
        Ok(())
    }
}
fn admission() -> ReadAdmission {
    let context = Context {
        actor_id: Uuid::new_v4(),
        event_id: Uuid::new_v4(),
        session_family_id: Uuid::new_v4(),
        request_id: Uuid::new_v4(),
        original_authority: [1; 32],
        expires_at: Utc::now() + ChronoDuration::seconds(4),
    };
    let binding = wire::GuardBinding {
        actor_id: context.actor_id.to_string(),
        event_id: context.event_id.to_string(),
        session_family_id: context.session_family_id.to_string(),
        request_id: context.request_id.to_string(),
        operation: "listConcertFeed".into(),
        subject_ids: vec![],
        expires_unix_micros: context.expires_at.timestamp_micros(),
        parent_digest: vec![1; 32],
        parent_body_digest: vec![2; 32],
        parent_method: "/concert.product.v1.ProductService/Dispatch".into(),
        http_method: "GET".into(),
        http_target: format!("/api/concert/events/{}/feed", context.event_id),
        body_digest: vec![3; 32],
    };
    let view = |secret| wire::GuardView {
        guard_id: Uuid::new_v4().to_string(),
        binding_digest: vec![4; 32],
        view_secret: vec![secret; 32],
        expires_unix_micros: context.expires_at.timestamp_micros(),
    };
    let authority = proto::ReadAuthority {
        binding: Some(binding.clone()),
        auth: Some(view(5)),
        friend: Some(view(6)),
        root: Some(wire::RootReadView {
            scope: Some(wire::ProducerScope {
                instance_id: Uuid::new_v4().to_string(),
                scope_id: Uuid::new_v4().to_string(),
                binding_digest: vec![4; 32],
            }),
            view_secret: vec![7; 32],
            binding: Some(binding),
        }),
    };
    ReadAdmission {
        deadline: AuthorityRpcDeadline::new(
            context.expires_at.timestamp_micros(),
            Duration::from_secs(5),
        )
        .unwrap(),
        context,
        authority,
        operation: ReadOperation::Feed,
        target_post: None,
    }
}
async fn captured(
    tx: &mut Transaction<'_, Postgres>,
    store: &TrackingStore,
) -> CapturedReadTransaction {
    CapturedReadTransaction::capture(
        tx,
        store.instance(),
        [8; 32],
        crate::concert_feed::digest(&(Uuid::new_v4(), Uuid::new_v4())).unwrap(),
    )
    .await
    .unwrap()
}
fn request(
    a: &ReadAdmission,
    identity: &CapturedReadTransaction,
    registration: Uuid,
) -> wire::ResolveTransactionRequest {
    wire::ResolveTransactionRequest {
        binding: a.authority.binding.clone(),
        transaction: Some(identity.wire()),
        registration_id: registration.to_string(),
    }
}
fn stop() -> tokio::time::Instant {
    tokio::time::Instant::now() + Duration::from_secs(2)
}
async fn acknowledge_both(t: TrackedRead) -> RegisteredRead {
    for owner in [RegistrationOwner::Auth, RegistrationOwner::Friend] {
        t.attempt(owner).await.unwrap();
        t.acknowledge(owner, Uuid::new_v4()).await.unwrap();
    }
    RegisteredRead { tracked: t }
}

#[tokio::test]
#[ignore = "requires explicitly leased native PostgreSQL; storage evidence only"]
async fn native_lost_track_ack_uses_actual_aborted_transaction_and_exact_owner_binding(
) -> anyhow::Result<()> {
    let db = Database::start().await?;
    let store = TrackingStore::new(db.pool.clone(), Uuid::new_v4())?;
    let a = admission();
    let mut tx = db.pool.begin().await?;
    let identity = captured(&mut tx, &store).await;
    let tracked = store.begin(&a, identity.clone()).await?;
    tracked.attempt(RegistrationOwner::Auth).await?;
    // Simulate lost Track response: no local acknowledgement is manufactured.
    let r = request(&a, &identity, Uuid::new_v4());
    assert_eq!(
        store
            .resolve(Workload::Auth, r.clone(), stop())
            .await?
            .status,
        wire::TransactionStatus::InProgress as i32
    );
    assert!(matches!(
        store.resolve(Workload::Concert, r.clone(), stop()).await,
        Err(Error::Authority)
    ));
    assert!(matches!(
        store.resolve(Workload::Friend, r.clone(), stop()).await,
        Err(Error::Authority)
    ));
    let mut forged = r.clone();
    forged.binding.as_mut().unwrap().event_id = Uuid::new_v4().to_string();
    assert!(matches!(
        store.resolve(Workload::Auth, forged, stop()).await,
        Err(Error::Authority)
    ));
    let mut forged = r.clone();
    forged.transaction.as_mut().unwrap().backend_pid += 1;
    assert!(matches!(
        store.resolve(Workload::Auth, forged, stop()).await,
        Err(Error::Authority)
    ));
    tx.rollback().await?;
    let terminal = store.resolve(Workload::Auth, r, stop()).await?;
    assert_eq!(terminal.status, wire::TransactionStatus::Aborted as i32);
    assert!(terminal.receipt_evidence.is_empty());
    db.finish().await
}

#[tokio::test]
#[ignore = "requires explicitly leased native PostgreSQL; storage evidence only"]
async fn native_committed_transaction_without_exact_same_transaction_witness_stays_unknown(
) -> anyhow::Result<()> {
    let db = Database::start().await?;
    let store = TrackingStore::new(db.pool.clone(), Uuid::new_v4())?;
    let a = admission();
    let mut tx = db.pool.begin().await?;
    let identity = captured(&mut tx, &store).await;
    let tracked = store.begin(&a, identity.clone()).await?;
    tracked.attempt(RegistrationOwner::Auth).await?;
    let ack = Uuid::new_v4();
    tracked.acknowledge(RegistrationOwner::Auth, ack).await?;
    tx.commit().await?;
    assert_eq!(
        store
            .resolve(Workload::Auth, request(&a, &identity, ack), stop())
            .await?
            .status,
        wire::TransactionStatus::Unknown as i32
    );
    // A different real connection/transaction cannot forge this witness.
    tracked.attempt(RegistrationOwner::Friend).await?;
    tracked
        .acknowledge(RegistrationOwner::Friend, Uuid::new_v4())
        .await?;
    let registered = RegisteredRead { tracked };
    let mut wrong = db.pool.begin().await?;
    assert!(matches!(
        registered.witness(&mut wrong).await,
        Err(Error::Authority)
    ));
    wrong.rollback().await?;
    assert_eq!(
        store
            .resolve(Workload::Auth, request(&a, &identity, ack), stop())
            .await?
            .status,
        wire::TransactionStatus::Unknown as i32
    );
    db.finish().await
}

#[tokio::test]
#[ignore = "requires explicitly leased native PostgreSQL; storage evidence only"]
async fn native_both_durable_acknowledgements_and_exact_committed_witness_are_required(
) -> anyhow::Result<()> {
    let db = Database::start().await?;
    let store = TrackingStore::new(db.pool.clone(), Uuid::new_v4())?;
    let a = admission();
    let mut tx = db.pool.begin().await?;
    let identity = captured(&mut tx, &store).await;
    let tracked = store.begin(&a, identity.clone()).await?;
    // Friend cannot begin before Auth's acknowledgement has committed.
    assert!(matches!(
        tracked.attempt(RegistrationOwner::Friend).await,
        Err(Error::Authority)
    ));
    let registered = acknowledge_both(tracked).await;
    registered.witness(&mut tx).await?;
    let ack: Uuid = sqlx::query_scalar(
        "select auth_registration_id from post_concert_read_tracking where settlement_reference=$1",
    )
    .bind(identity.settlement_reference().to_vec())
    .fetch_one(&db.pool)
    .await?;
    let r = request(&a, &identity, ack);
    assert_eq!(
        store
            .resolve(Workload::Auth, r.clone(), stop())
            .await?
            .status,
        wire::TransactionStatus::InProgress as i32
    );
    tx.commit().await?;
    let terminal = store.resolve(Workload::Auth, r.clone(), stop()).await?;
    assert_eq!(terminal.status, wire::TransactionStatus::Committed as i32);
    assert_eq!(terminal.receipt_evidence.len(), 32);
    let restarted = TrackingStore::new(db.pool.clone(), Uuid::new_v4())?;
    assert_eq!(
        restarted
            .resolve(Workload::Auth, r.clone(), stop())
            .await?
            .status,
        wire::TransactionStatus::Unknown as i32
    );
    let mut wrong = r;
    wrong.registration_id = Uuid::new_v4().to_string();
    assert!(matches!(
        store.resolve(Workload::Auth, wrong, stop()).await,
        Err(Error::Authority)
    ));
    db.finish().await
}

#[tokio::test]
#[ignore = "requires explicitly leased native PostgreSQL; storage evidence only"]
async fn native_expiry_reclaims_only_proven_terminal_work_never_a_live_or_unknown_transaction(
) -> anyhow::Result<()> {
    let db = Database::start().await?;
    let store = TrackingStore::new(db.pool.clone(), Uuid::new_v4())?;
    let a = admission();
    let mut aborted = db.pool.begin().await?;
    let aborted_id = captured(&mut aborted, &store).await;
    store.begin(&a, aborted_id.clone()).await?;
    aborted.rollback().await?;
    let mut live = db.pool.begin().await?;
    let live_id = captured(&mut live, &store).await;
    store.begin(&a, live_id.clone()).await?;
    let mut committed = db.pool.begin().await?;
    let unknown_id = captured(&mut committed, &store).await;
    store.begin(&a, unknown_id.clone()).await?;
    committed.commit().await?;
    // Wait through the actual original deadline; expiry alone cannot turn the
    // still-open transaction or a commit lacking its witness into an abort.
    tokio::time::sleep(Duration::from_secs(5)).await;
    store.reap(stop()).await?;
    for (identity, expected) in [
        (&aborted_id, Some("ABORTED")),
        (&live_id, None),
        (&unknown_id, None),
    ] {
        let status: Option<String> = sqlx::query_scalar(
            "select terminal_status from post_concert_read_tracking where settlement_reference=$1",
        )
        .bind(identity.settlement_reference().to_vec())
        .fetch_one(&db.pool)
        .await?;
        assert_eq!(status.as_deref(), expected);
    }
    live.rollback().await?;
    db.finish().await
}

#[tokio::test]
#[ignore = "requires explicitly leased native PostgreSQL; storage evidence only"]
async fn native_comment_subject_budget_and_filtered_boundary_do_not_skip_or_duplicate_records(
) -> anyhow::Result<()> {
    let db = Database::start().await?;
    sqlx::raw_sql(include_str!("../../migrations/0031_concert_feed_owner.sql"))
        .execute(&db.pool)
        .await?;
    let a = admission();
    let post = Uuid::new_v4();
    let at = Utc::now() - ChronoDuration::minutes(5);
    sqlx::query("insert into post_concert_posts(post_id,event_id,author_id,content,revision,state,created_at,comment_count) values($1,$2,$3,$4,1,'PUBLISHED',$5,101)")
        .bind(post).bind(a.context.event_id).bind(Uuid::from_u128(4000))
        .bind(serde_json::json!({"text":"Real stored parent","links":[],"media_ids":[]})).bind(at)
        .execute(&db.pool).await?;
    // Equal timestamps exercise the UUID component of the DESC keyset.
    sqlx::query("insert into post_concert_comments(comment_id,event_id,post_id,author_id,text,revision,state,created_at) select ('00000000-0000-0000-0000-'||lpad(to_hex(1000+n),12,'0'))::uuid,$1,$2,('00000000-0000-0000-0000-'||lpad(to_hex(2000+n),12,'0'))::uuid,'Current stored comment',1,'PUBLISHED',$3 from generate_series(0,100) n")
        .bind(a.context.event_id).bind(post).bind(at+ChronoDuration::seconds(1)).execute(&db.pool).await?;
    let owner = crate::concert_feed::tests::paging_owner(
        db.pool.clone(),
        vec![
            (a.context.actor_id, Uuid::from_u128(2100)),
            (Uuid::from_u128(2002), a.context.actor_id),
        ],
    );
    let upper = Utc::now();
    let limit = crate::concert_feed::read_runtime::comment_scan_limit(100);
    let first = owner
        .comments_in(&a.context, post, upper, None, limit, db.pool.begin().await?)
        .await?;
    assert_eq!(first.items.len(), 97);
    assert!(first.has_more);
    let boundary = first.scanned_through.unwrap();
    assert_eq!(boundary.id, Uuid::from_u128(1002));
    assert!(!first.items.iter().any(|c| c.comment_id == boundary.id));
    let second = owner
        .comments_in(
            &a.context,
            post,
            upper,
            Some(boundary),
            limit,
            db.pool.begin().await?,
        )
        .await?;
    assert_eq!(second.items.len(), 2);
    assert!(!second.has_more);
    let actual: Vec<_> = first
        .items
        .iter()
        .chain(second.items.iter())
        .map(|c| c.comment_id)
        .collect();
    let expected: Vec<_> = (1000..=1100)
        .rev()
        .filter(|n| *n != 1100 && *n != 1002)
        .map(Uuid::from_u128)
        .collect();
    assert_eq!(actual, expected);
    db.finish().await
}
