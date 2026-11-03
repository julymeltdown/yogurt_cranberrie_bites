//! Exact native qualification is ignored unless invoked by the owned-cluster harness.
use chrono::Utc;
use friend_service::adapters::out::persistence::follow_repository::PgFollowRepository;
use friend_service::adapters::r#in::grpc::concert_relationship::proto;
use friend_service::application::ports::{
    RelationshipPolicyRepositoryPort, UserLifecycleRepositoryPort,
};
use friend_service::concert_invalidation::*;
use sqlx::PgPool;
use std::time::{Duration, Instant};
use uuid::Uuid;
#[path = "concert_invalidation_support/mod.rs"]
mod support;

fn identity() -> HeadQuery {
    HeadQuery::new(Uuid::new_v4(), Uuid::new_v4()).unwrap()
}
fn budget() -> ReadBudget {
    ReadBudget::new(
        Instant::now() + Duration::from_millis(950),
        ReadCancellation::default(),
    )
    .unwrap()
}
async fn head(pool: &PgPool) -> Cursor {
    Box::pin(PgJournal::new(pool.clone()).read_head(identity(), &budget()))
        .await
        .unwrap()
        .cursor()
}
async fn page(pool: &PgPool, after: Cursor, limit: u32) -> Page {
    Box::pin(
        PgJournal::new(pool.clone())
            .read_after(PageQuery::new(identity(), after, limit).unwrap(), &budget()),
    )
    .await
    .unwrap()
}
async fn block(pool: &PgPool, a: Uuid, b: Uuid) {
    sqlx::query("INSERT INTO user_blocks(blocker_id,blocked_id,created_at) VALUES($1,$2,now())")
        .bind(a)
        .bind(b)
        .execute(pool)
        .await
        .unwrap();
}

#[test]
fn closed_wire_shape_and_budget_validation() {
    let good = proto::ReadInvalidationHeadRequest {
        consumer_id: Uuid::new_v4().to_string(),
        request_id: Uuid::new_v4().to_string(),
    };
    let query = HeadQuery::try_from(good.clone()).unwrap();
    assert_eq!(proto::ReadInvalidationHeadRequest::from(query), good);
    for value in [
        "",
        "secret-token",
        "00000000-0000-0000-0000-000000000000",
        "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
    ] {
        let mut bad = good.clone();
        bad.consumer_id = value.into();
        assert_eq!(HeadQuery::try_from(bad), Err(ReadError::InvalidInput));
    }
    let cursor = Cursor::new(Uuid::new_v4(), 0).unwrap();
    for limit in [0, 257, u32::MAX] {
        assert_eq!(
            PageQuery::new(query, cursor, limit),
            Err(ReadError::InvalidInput)
        );
    }
    for sequence in [-1, MAX_SEQUENCE + 1] {
        assert_eq!(
            Cursor::new(cursor.epoch(), sequence),
            Err(ReadError::InvalidInput)
        );
    }
    assert!(Cursor::new(Uuid::nil(), 0).is_err());
    assert!(ReadBudget::new(
        Instant::now() + Duration::from_secs(2),
        ReadCancellation::default()
    )
    .is_err());
    let cancellation = ReadCancellation::default();
    cancellation.cancel();
    assert_eq!(
        ReadBudget::new(Instant::now() + Duration::from_millis(50), cancellation).unwrap_err(),
        ReadError::Cancelled
    );
    assert_eq!(
        tonic::Status::from(ReadError::Cancelled).code(),
        tonic::Code::Cancelled
    );
    assert_eq!(
        tonic::Status::from(ReadError::DeadlineExceeded).code(),
        tonic::Code::DeadlineExceeded
    );
    assert!(!format!("{query:?}").contains(&good.consumer_id));
}

#[tokio::test]
#[ignore = "new owned private cluster required"]
async fn native_upgrade_0015_to_0016() {
    let pool = support::owned_pool().await;
    let migrator = migration_runtime::load_migrations(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations"),
    )
    .await
    .unwrap();
    let before = sqlx::migrate::Migrator {
        migrations: std::borrow::Cow::Owned(
            migrator
                .iter()
                .filter(|m| m.version <= 15)
                .cloned()
                .collect(),
        ),
        ..sqlx::migrate::Migrator::DEFAULT
    };
    before.run(&pool).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM _sqlx_migrations")
            .fetch_one(&pool)
            .await
            .unwrap(),
        15
    );
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let repo = PgFollowRepository::new(pool.clone());
    repo.block(a, b, Utc::now()).await.unwrap();
    let revision: Uuid =
        sqlx::query_scalar("SELECT owner_epoch FROM friend_concert_relationship_revision")
            .fetch_one(&pool)
            .await
            .unwrap();
    let ledger: Vec<(i64, Vec<u8>)> =
        sqlx::query_as("SELECT version,checksum FROM _sqlx_migrations ORDER BY version")
            .fetch_all(&pool)
            .await
            .unwrap();
    migrator.run(&pool).await.unwrap();
    migrator.run(&pool).await.unwrap();
    let actual: Vec<(i64, Vec<u8>)> = sqlx::query_as(
        "SELECT version,checksum FROM _sqlx_migrations WHERE version<=15 ORDER BY version",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(actual, ledger);
    for m in migrator.iter() {
        let checksum: Vec<u8> = sqlx::query_scalar(
            "SELECT checksum FROM _sqlx_migrations WHERE version=$1 AND success",
        )
        .bind(m.version)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(&checksum[..], &m.checksum[..]);
    }
    let initial = head(&pool).await;
    assert_eq!(initial.sequence(), 0);
    assert_ne!(initial.epoch(), revision);
    assert!(repo.policy(a, b).await.unwrap().blocked_by_viewer);
    repo.unblock(a, b).await.unwrap();
    assert_eq!(head(&pool).await.sequence(), 1);
    pool.close().await;
}

#[tokio::test]
#[ignore = "new owned private cluster required"]
async fn native_repository_block_unblock_and_deletion() {
    let pool = support::owned_pool().await;
    let repo = PgFollowRepository::new(pool.clone());
    let initial = head(&pool).await;
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    repo.block(a, b, Utc::now()).await.unwrap();
    repo.unblock(a, b).await.unwrap();
    repo.block(b, a, Utc::now()).await.unwrap();
    let before_delete = head(&pool).await;
    assert_eq!(before_delete.sequence(), initial.sequence() + 3);
    repo.apply_user_deletion(
        Uuid::new_v4(),
        a,
        a,
        Utc::now() + chrono::Duration::days(30),
    )
    .await
    .unwrap();
    let deleted = page(&pool, before_delete, 256).await;
    assert!(deleted
        .signals()
        .iter()
        .any(|s| s.reason() == Reason::UserRemoved && s.scope() == Scope::User { user: a }));
    assert!(deleted
        .signals()
        .iter()
        .any(|s| s.reason() == Reason::RelationshipChanged));
    assert!(repo.block(a, b, Utc::now()).await.is_err());
    assert_eq!(head(&pool).await, deleted.head());
    let pair = page(&pool, initial, 3).await;
    assert_eq!(pair.signals().len(), 3);
    for s in pair.signals() {
        assert_eq!(
            s.scope(),
            Scope::Pair {
                first: a.min(b),
                second: a.max(b)
            }
        );
    }
    let same = page(&pool, initial, 3).await;
    assert_eq!(pair.signals(), same.signals());
    pool.close().await;
}

#[tokio::test]
#[ignore = "new owned private cluster required"]
async fn native_direct_sql_scopes_guards_and_rollback() {
    let pool = support::owned_pool().await;
    let initial = head(&pool).await;
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let c = Uuid::new_v4();
    block(&pool, a, b).await;
    sqlx::query("UPDATE user_blocks SET blocked_id=$3 WHERE blocker_id=$1 AND blocked_id=$2")
        .bind(a)
        .bind(b)
        .bind(c)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM user_blocks WHERE blocker_id=$1")
        .bind(a)
        .execute(&pool)
        .await
        .unwrap();
    let changed = page(&pool, initial, 256).await;
    assert_eq!(changed.signals().len(), 4);
    assert!(changed.signals().iter().any(|s| s.scope()
        == Scope::Pair {
            first: a.min(b),
            second: a.max(b)
        }));
    assert!(changed.signals().iter().any(|s| s.scope()
        == Scope::Pair {
            first: a.min(c),
            second: a.max(c)
        }));
    let u = Uuid::new_v4();
    let v = Uuid::new_v4();
    sqlx::query("INSERT INTO friend_user_deletion_tombstones(user_id,saga_id,actor_id,retention_until) VALUES($1,$2,$1,now())").bind(u).bind(Uuid::new_v4()).execute(&pool).await.unwrap();
    sqlx::query("UPDATE friend_user_deletion_tombstones SET user_id=$2 WHERE user_id=$1")
        .bind(u)
        .bind(v)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM friend_user_deletion_tombstones WHERE user_id=$1")
        .bind(v)
        .execute(&pool)
        .await
        .unwrap();
    let removed = page(&pool, changed.head(), 256).await;
    assert_eq!(removed.signals().len(), 4);
    assert!(removed
        .signals()
        .iter()
        .all(|s| s.reason() == Reason::UserRemoved));
    let before = head(&pool).await;
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO user_blocks(blocker_id,blocked_id,created_at) VALUES($1,$2,now())")
        .bind(Uuid::new_v4())
        .bind(Uuid::new_v4())
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.rollback().await.unwrap();
    assert_eq!(head(&pool).await, before);
    for query in [
        "UPDATE friend_world_invalidation_head SET sequence=sequence+1",
        "DELETE FROM friend_world_invalidation_head",
        "TRUNCATE friend_world_invalidation_journal",
        "DELETE FROM friend_world_invalidation_journal",
        "UPDATE friend_world_invalidation_journal SET signal_id=gen_random_uuid()",
        "TRUNCATE user_blocks",
        "TRUNCATE friend_user_deletion_tombstones",
    ] {
        assert!(sqlx::query(query).execute(&pool).await.is_err(), "{query}");
        assert_eq!(head(&pool).await, before);
    }
    let id: Uuid = sqlx::query_scalar(
        "SELECT signal_id FROM friend_world_invalidation_journal ORDER BY sequence LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    // BEFORE INSERT allocates, but ON CONFLICT suppresses the row; deferred guard
    // must roll the entire transaction back rather than commit a cursor gap.
    assert!(sqlx::query("INSERT INTO friend_world_invalidation_journal(signal_id,reason,user_id) VALUES($1,'FRIEND_USER_REMOVED',$2) ON CONFLICT(signal_id) DO NOTHING").bind(id).bind(Uuid::new_v4()).execute(&pool).await.is_err());
    assert_eq!(head(&pool).await, before);
    sqlx::query("UPDATE user_blocks SET deleted_at=now() WHERE false")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(head(&pool).await, before);
    pool.close().await;
}

async fn serialization_case(pool: &PgPool, rollback: bool, journal_first: bool) {
    let initial = head(pool).await;
    let mut a = pool.begin().await.unwrap();
    let apid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *a)
        .await
        .unwrap();
    if journal_first {
        sqlx::query("INSERT INTO friend_world_invalidation_journal(reason,user_id) VALUES('FRIEND_USER_REMOVED',$1)")
            .bind(Uuid::new_v4()).execute(&mut *a).await.unwrap();
    } else {
        sqlx::query(
            "INSERT INTO user_blocks(blocker_id,blocked_id,created_at) VALUES($1,$2,now())",
        )
        .bind(Uuid::new_v4())
        .bind(Uuid::new_v4())
        .execute(&mut *a)
        .await
        .unwrap();
    }
    let bp = pool.clone();
    let (send, receive) = tokio::sync::oneshot::channel();
    let b = tokio::spawn(async move {
        let mut tx = bp.begin().await.unwrap();
        let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *tx)
            .await
            .unwrap();
        send.send(pid).unwrap();
        sqlx::query(
            "INSERT INTO user_blocks(blocker_id,blocked_id,created_at) VALUES($1,$2,now())",
        )
        .bind(Uuid::new_v4())
        .bind(Uuid::new_v4())
        .execute(&mut *tx)
        .await
        .unwrap();
        tx
    });
    let bpid = receive.await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let blockers: Vec<i32> = sqlx::query_scalar("SELECT pg_blocking_pids($1)")
                .bind(bpid)
                .fetch_one(pool)
                .await
                .unwrap();
            if blockers.contains(&apid) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert!(!b.is_finished());
    assert_eq!(head(pool).await, initial);
    let empty = page(pool, initial, 256).await;
    assert!(empty.signals().is_empty());
    assert_eq!(empty.head(), initial);
    if journal_first {
        // B is waiting on A's revision lock. If direct journal allocation locked
        // only head, B would hold revision while awaiting head, and A's source
        // mutation below would form a cycle. Bound the actual second mutation.
        tokio::time::timeout(
            Duration::from_secs(2),
            sqlx::query(
                "INSERT INTO user_blocks(blocker_id,blocked_id,created_at) VALUES($1,$2,now())",
            )
            .bind(Uuid::new_v4())
            .bind(Uuid::new_v4())
            .execute(&mut *a),
        )
        .await
        .unwrap()
        .unwrap();
    }
    let a_signals = if journal_first { 2 } else { 1 };
    if rollback {
        a.rollback().await.unwrap();
    } else {
        a.commit().await.unwrap();
    }
    let btx = b.await.unwrap();
    let visible = head(pool).await;
    assert_eq!(
        visible.sequence(),
        initial.sequence() + if rollback { 0 } else { a_signals }
    );
    let partial = page(pool, initial, 256).await;
    assert_eq!(
        partial.signals().len(),
        if rollback { 0 } else { a_signals as usize }
    );
    btx.commit().await.unwrap();
    let committed = page(pool, initial, 256).await;
    assert_eq!(
        committed.signals().len(),
        if rollback { 1 } else { a_signals as usize + 1 }
    );
    assert_eq!(committed.next(), committed.head());
    assert_eq!(page(pool, committed.next(), 256).await.signals().len(), 0);
}
#[tokio::test]
#[ignore = "new owned private cluster required"]
async fn native_concurrent_commit_and_rollback_never_skip_late_commit() {
    let pool = support::owned_pool().await;
    for journal_first in [false, true] {
        Box::pin(serialization_case(&pool, false, journal_first)).await;
        Box::pin(serialization_case(&pool, true, journal_first)).await;
    }
    pool.close().await;
}

#[tokio::test]
#[ignore = "new owned private cluster required"]
async fn native_page_bounds_echo_epoch_and_future_rejected() {
    use prost::Message;
    let pool = support::owned_pool().await;
    let initial = head(&pool).await;
    sqlx::query("INSERT INTO user_blocks(blocker_id,blocked_id,created_at) SELECT gen_random_uuid(),gen_random_uuid(),now() FROM generate_series(1,260)").execute(&pool).await.unwrap();
    let first = page(&pool, initial, 256).await;
    assert_eq!(first.signals().len(), 256);
    assert_eq!(first.next().sequence(), initial.sequence() + 256);
    assert_eq!(first.head().sequence(), initial.sequence() + 260);
    let wire = first.to_wire().unwrap();
    assert_eq!(
        wire.query.unwrap(),
        proto::ReadInvalidationsRequest::from(first.query())
    );
    assert!(first.to_wire().unwrap().encoded_len() <= MAX_RESPONSE_BYTES);
    let last = page(&pool, first.next(), 256).await;
    assert_eq!(last.signals().len(), 4);
    assert_eq!(last.next(), first.head());
    let reader = PgJournal::new(pool.clone());
    for cursor in [
        Cursor::new(Uuid::new_v4(), 0).unwrap(),
        Cursor::new(first.head().epoch(), first.head().sequence() + 1).unwrap(),
    ] {
        assert_eq!(
            Box::pin(
                reader.read_after(PageQuery::new(identity(), cursor, 256).unwrap(), &budget())
            )
            .await
            .unwrap_err(),
            ReadError::FailedPrecondition
        );
    }
    let columns:Vec<String>=sqlx::query_scalar("SELECT column_name::text FROM information_schema.columns WHERE table_schema='public' AND table_name='friend_world_invalidation_journal' ORDER BY ordinal_position").fetch_all(&pool).await.unwrap();
    assert_eq!(
        columns,
        [
            "owner_epoch",
            "sequence",
            "signal_id",
            "reason",
            "user_id",
            "peer_id"
        ]
    );
    pool.close().await;
}

#[tokio::test]
#[ignore = "new owned private cluster required"]
async fn native_cancel_and_deadline_are_failures_not_empty_success() {
    let pool = support::owned_pool().await;
    let current = head(&pool).await;
    let mut lock = pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE friend_world_invalidation_head IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *lock)
        .await
        .unwrap();
    let token = ReadCancellation::default();
    let inherited =
        ReadBudget::new(Instant::now() + Duration::from_millis(900), token.clone()).unwrap();
    let reader = PgJournal::new(pool.clone());
    let task = tokio::spawn(async move {
        Box::pin(reader.read_after(
            PageQuery::new(identity(), current, 256).unwrap(),
            &inherited,
        ))
        .await
    });
    let blocked_pid=tokio::time::timeout(Duration::from_millis(400),async {loop {
        let pid:Option<i32>=sqlx::query_scalar("SELECT pid FROM pg_stat_activity WHERE datname=current_database() AND pid<>pg_backend_pid() AND state='active' AND wait_event_type='Lock' AND query LIKE 'WITH h AS MATERIALIZED%' LIMIT 1")
            .fetch_optional(&pool).await.unwrap();
        if let Some(pid)=pid {break pid;}
        tokio::time::sleep(Duration::from_millis(2)).await;
    }}).await.unwrap();
    token.cancel();
    assert_eq!(
        tokio::time::timeout(Duration::from_millis(100), task)
            .await
            .unwrap()
            .unwrap(),
        Err(ReadError::Cancelled)
    );
    // Keep ACCESS EXCLUSIVE held while proving the cancelled backend itself
    // disappears, so neither a running query nor idle-in-transaction can hide
    // behind release of the blocker. Server timeout inherits the original stop.
    tokio::time::timeout(Duration::from_millis(1100), async {
        loop {
            let exists: bool =
                sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE pid=$1)")
                    .bind(blocked_pid)
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            if !exists {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let inherited = ReadBudget::new(
        Instant::now() + Duration::from_millis(70),
        ReadCancellation::default(),
    )
    .unwrap();
    let start = Instant::now();
    assert_eq!(
        Box::pin(PgJournal::new(pool.clone()).read_head(identity(), &inherited)).await,
        Err(ReadError::DeadlineExceeded)
    );
    assert!(start.elapsed() < Duration::from_millis(250));
    // Existing ambient deadline must clip the whole operation, not restart it.
    let inherited = budget();
    assert_eq!(
        platform::call_deadline::scope(
            Instant::now() + Duration::from_millis(40),
            Box::pin(PgJournal::new(pool.clone()).read_head(identity(), &inherited))
        )
        .await,
        Err(ReadError::DeadlineExceeded)
    );
    // Deadline failures also leave no blocked journal read or idle transaction
    // while the same blocker remains held.
    tokio::time::timeout(Duration::from_millis(1100),async {loop {
        let residue:i64=sqlx::query_scalar("SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND pid<>pg_backend_pid() AND query LIKE 'SELECT owner_epoch,sequence FROM public.friend_world_invalidation_head%' AND state<>'idle'").fetch_one(&pool).await.unwrap();
        if residue==0 {break;}tokio::time::sleep(Duration::from_millis(5)).await;
    }}).await.unwrap();
    lock.rollback().await.unwrap();
    assert_eq!(head(&pool).await, current);
    pool.close().await;
}

#[tokio::test]
#[ignore = "owned restart phase only"]
async fn native_restart_replay_stability() {
    let pool = support::owned_pool().await;
    let reader = PgJournal::new(pool.clone());
    let h = Box::pin(reader.read_head(identity(), &budget()))
        .await
        .unwrap();
    let initial = Cursor::new(h.cursor().epoch(), 0).unwrap();
    let mut after = initial;
    let mut seen = 0;
    while after.sequence() < h.cursor().sequence() {
        let p = page(&pool, after, 256).await;
        assert_eq!(p.head(), h.cursor());
        seen += p.signals().len();
        after = p.next();
    }
    assert_eq!(seen as i64, h.cursor().sequence());
    pool.close().await;
}

#[tokio::test]
#[ignore = "new owned private cluster required; privileged fixture tampering only"]
async fn native_corrupt_retained_gap_fails_closed() {
    let pool = support::owned_pool().await;
    let current = head(&pool).await;
    assert!(current.sequence() > 4);
    for sequence in [current.sequence(), current.sequence() - 3] {
        Box::pin(gap_case(&pool, current, sequence)).await;
    }
    pool.close().await;
}
async fn gap_case(pool: &PgPool, current: Cursor, sequence: i64) {
    // This owner-only corruption fixture explicitly bypasses DML guards. Normal
    // application mutations cannot do this. Restore the exact row before return.
    let mut connection = pool.acquire().await.unwrap();
    sqlx::query("CREATE TEMP TABLE saved_signal AS SELECT * FROM friend_world_invalidation_journal WHERE sequence=$1").bind(sequence).execute(&mut *connection).await.unwrap();
    sqlx::query("BEGIN")
        .execute(&mut *connection)
        .await
        .unwrap();
    sqlx::query("ALTER TABLE friend_world_invalidation_journal DISABLE TRIGGER friend_world_journal_immutable").execute(&mut *connection).await.unwrap();
    sqlx::query("DELETE FROM friend_world_invalidation_journal WHERE sequence=$1")
        .bind(sequence)
        .execute(&mut *connection)
        .await
        .unwrap();
    sqlx::query("ALTER TABLE friend_world_invalidation_journal ENABLE TRIGGER friend_world_journal_immutable").execute(&mut *connection).await.unwrap();
    sqlx::query("COMMIT")
        .execute(&mut *connection)
        .await
        .unwrap();
    let after = Cursor::new(current.epoch(), sequence - 1).unwrap();
    let result = Box::pin(
        PgJournal::new(pool.clone())
            .read_after(PageQuery::new(identity(), after, 2).unwrap(), &budget()),
    )
    .await;
    sqlx::query("BEGIN")
        .execute(&mut *connection)
        .await
        .unwrap();
    sqlx::query(
        "ALTER TABLE friend_world_invalidation_journal DISABLE TRIGGER friend_world_journal_assign",
    )
    .execute(&mut *connection)
    .await
    .unwrap();
    sqlx::query("INSERT INTO friend_world_invalidation_journal SELECT * FROM saved_signal")
        .execute(&mut *connection)
        .await
        .unwrap();
    sqlx::query(
        "ALTER TABLE friend_world_invalidation_journal ENABLE TRIGGER friend_world_journal_assign",
    )
    .execute(&mut *connection)
    .await
    .unwrap();
    sqlx::query("COMMIT")
        .execute(&mut *connection)
        .await
        .unwrap();
    assert_eq!(result, Err(ReadError::Unavailable));
    // Interior removal still fills limit=2 with later rows; cardinality alone
    // would accept it. Restored rows must again replay as an exact prefix.
    assert_eq!(
        page(pool, after, 2).await.signals().len(),
        if sequence == current.sequence() { 1 } else { 2 }
    );
    sqlx::query("DROP TABLE saved_signal")
        .execute(&mut *connection)
        .await
        .unwrap();
    drop(connection);
}
