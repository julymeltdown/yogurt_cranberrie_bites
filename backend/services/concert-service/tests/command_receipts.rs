//! Test-only synthetic local policy and aggregate, never production authority.
use async_trait::async_trait;
use concert_application::commands::*;
use concert_service::adapters::out::repository::{
    outbox::{DeliveryFailure, PgOutbox},
    receipt::{CommandProducer, PgCommandRepository},
};
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgConnection, PgPool,
};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::{path::PathBuf, str::FromStr};
use uuid::Uuid;
async fn owned_pool() -> anyhow::Result<PgPool> {
    let root = PathBuf::from(
        std::env::var("CONCERT_TEST_OWNER_ROOT")
            .map_err(|_| anyhow::anyhow!("owned fixture root required"))?,
    );
    let info = std::fs::symlink_metadata(&root)?;
    anyhow::ensure!(
        info.is_dir() && !info.file_type().is_symlink() && info.permissions().mode() & 0o077 == 0,
        "private owned root required"
    );
    anyhow::ensure!(
        format!("{}:{}", info.dev(), info.ino()) == std::env::var("CONCERT_TEST_OWNER_IDENTITY")?,
        "fixture inode changed"
    );
    let marker_path = root.join("mcr-owner.json");
    let marker_info = std::fs::symlink_metadata(&marker_path)?;
    anyhow::ensure!(
        marker_info.is_file()
            && !marker_info.file_type().is_symlink()
            && marker_info.permissions().mode() & 0o077 == 0,
        "private marker required"
    );
    let marker: serde_json::Value = serde_json::from_slice(&std::fs::read(marker_path)?)?;
    anyhow::ensure!(
        marker["token"].as_str() == Some(std::env::var("CONCERT_TEST_OWNER_TOKEN")?.as_str())
            && marker["uid"].as_u64() == Some(info.uid() as u64),
        "fixture owner mismatch"
    );
    let options = PgConnectOptions::from_str(&std::env::var("CONCERT_TEST_DATABASE_URL")?)
        .map_err(|_| anyhow::anyhow!("invalid owned fixture options"))?;
    anyhow::ensure!(
        options.get_database() == Some("mcr_concert_be_commands")
            && options.get_username() == "mcr_owner"
            && options.get_socket() == Some(&root.join("socket"))
            && options.get_port() == 5432
            && options.get_options().is_none(),
        "foreign database refused"
    );
    let pool = PgPoolOptions::new()
        .max_connections(6)
        .connect_with(options)
        .await
        .map_err(|_| anyhow::anyhow!("owned fixture connection failed"))?;
    let actual: String = sqlx::query_scalar("show data_directory")
        .fetch_one(&pool)
        .await?;
    anyhow::ensure!(
        actual == root.join("data").to_string_lossy(),
        "foreign data directory refused"
    );
    Ok(pool)
}
#[derive(Clone, Copy)]
enum Mode {
    Success,
    Abort,
    Failed(SafeFailure),
    Pending,
    WrongReference,
    WrongRevision,
}
struct TestProducer {
    mode: Mode,
}
#[async_trait]
impl CommandProducer for TestProducer {
    async fn authorize(
        &self,
        c: &mut PgConnection,
        s: Scope,
        target: Uuid,
    ) -> Result<(), CommandError> {
        let allowed:Option<bool>=sqlx::query_scalar("select allowed from be15_test_policy where actor=$1 and event=$2 and operation=$3 and target=$4 for share")
   .bind(s.actor).bind(s.event).bind(s.operation.id()).bind(target).fetch_optional(c).await.map_err(|_|CommandError::Unavailable)?;
        if allowed == Some(true) {
            Ok(())
        } else {
            Err(CommandError::Denied)
        }
    }
    async fn apply(
        &self,
        c: &mut PgConnection,
        s: Scope,
        target: Uuid,
    ) -> Result<(Completion, Option<Invalidation>), CommandError> {
        let revision:i64=sqlx::query_scalar("update be15_test_aggregate set revision=revision+1 where event=$1 and target=$2 returning revision").bind(s.event).bind(target).fetch_one(c).await.map_err(|_|CommandError::Unavailable)?;
        let reference = SafeReference {
            resource: target,
            revision,
        };
        match self.mode {
            Mode::Abort => Err(CommandError::InvalidState),
            Mode::Failed(failure) => Ok((Completion::Failed(failure), None)),
            Mode::Pending => Ok((Completion::Pending, Some(Invalidation { reference }))),
            Mode::WrongRevision => Ok((
                Completion::Succeeded {
                    reference,
                    status: 200,
                },
                Some(Invalidation {
                    reference: SafeReference {
                        resource: target,
                        revision: revision + 1,
                    },
                }),
            )),
            Mode::WrongReference => Ok((
                Completion::Succeeded {
                    reference: SafeReference {
                        resource: Uuid::new_v4(),
                        revision,
                    },
                    status: 200,
                },
                None,
            )),
            Mode::Success => Ok((
                Completion::Succeeded {
                    reference,
                    status: 200,
                },
                Some(Invalidation { reference }),
            )),
        }
    }
}
async fn fixture() -> anyhow::Result<(PgPool, Scope, Uuid, CanonicalRequest)> {
    let pool = owned_pool().await?;
    // Tables created only in the harness-owned fixture. Every test has unique keys.
    sqlx::query("create table if not exists be15_test_policy(actor uuid,event uuid,operation text,target uuid,allowed boolean not null,primary key(actor,event,operation,target))").execute(&pool).await?;
    sqlx::query("create table if not exists be15_test_aggregate(event uuid,target uuid,revision bigint,primary key(event,target))").execute(&pool).await?;
    sqlx::query("create table if not exists be15_test_consumer(delivery uuid primary key,effects integer not null check(effects=1))").execute(&pool).await?;
    sqlx::query("create table if not exists be15_test_consumer_effect(event uuid primary key,applied integer not null)").execute(&pool).await?;
    let scope = Scope {
        actor: Uuid::new_v4(),
        event: Uuid::new_v4(),
        operation: RegisteredOperation::find("createConcertParty")?,
        key: Uuid::new_v4(),
    };
    let target = Uuid::new_v4();
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'BE15 synthetic test','OPEN')").bind(scope.event).bind(format!("be15-{}",scope.event.simple())).execute(&pool).await?;
    sqlx::query("insert into be15_test_policy values($1,$2,$3,$4,true)")
        .bind(scope.actor)
        .bind(scope.event)
        .bind(scope.operation.id())
        .bind(target)
        .execute(&pool)
        .await?;
    sqlx::query("insert into be15_test_aggregate values($1,$2,0)")
        .bind(scope.event)
        .bind(target)
        .execute(&pool)
        .await?;
    let hash = request(scope, 1);
    Ok((pool, scope, target, hash))
}
fn keys() -> &'static FingerprintKeys {
    static KEYS: std::sync::OnceLock<FingerprintKeys> = std::sync::OnceLock::new();
    KEYS.get_or_init(|| {
        FingerprintKeys::new(SigningKey::new("test_v1", &[0x42; 32]).unwrap(), vec![]).unwrap()
    })
}
fn request(s: Scope, value: i64) -> CanonicalRequest {
    CanonicalRequest::json(
        s,
        &[("eventId", &s.event.to_string())],
        &[],
        Some(&CanonicalValue::Integer(value)),
    )
    .unwrap()
}
async fn revision(p: &PgPool, s: Scope, t: Uuid) -> i64 {
    sqlx::query_scalar("select revision from be15_test_aggregate where event=$1 and target=$2")
        .bind(s.event)
        .bind(t)
        .fetch_one(p)
        .await
        .unwrap()
}
async fn count(p: &PgPool, table: &str, s: Scope) -> i64 {
    sqlx::query_scalar(&format!("select count(*) from {table} where event_id=$1"))
        .bind(s.event)
        .fetch_one(p)
        .await
        .unwrap()
}
/// Real local downstream transaction: dedup inbox and a separate business effect
/// commit together. Retrying after lost acknowledgement must not increment twice.
async fn consume_once(p: &PgPool, delivery: Uuid, event: Uuid) -> Result<(), sqlx::Error> {
    let mut tx = p.begin().await?;
    let inserted =
        sqlx::query("insert into be15_test_consumer values($1,1) on conflict do nothing")
            .bind(delivery)
            .execute(&mut *tx)
            .await?
            .rows_affected()
            == 1;
    if inserted {
        sqlx::query("insert into be15_test_consumer_effect values($1,1) on conflict(event) do update set applied=be15_test_consumer_effect.applied+1").bind(event).execute(&mut *tx).await?;
    }
    tx.commit().await
}
#[tokio::test]
async fn simultaneous_duplicates_apply_once_and_return_same_safe_receipt() -> anyhow::Result<()> {
    let (p, s, t, h) = fixture().await?;
    let repo = PgCommandRepository::new(p.clone());
    let producer = TestProducer {
        mode: Mode::Success,
    };
    let (a, b) = tokio::join!(
        repo.execute(&h, t, keys(), &producer),
        repo.execute(&h, t, keys(), &producer)
    );
    assert_eq!(a?, b?);
    assert_eq!(revision(&p, s, t).await, 1);
    assert_eq!(count(&p, "concert_outbox", s).await, 1);
    p.close().await;
    Ok(())
}
#[tokio::test]
async fn simultaneous_changed_body_conflicts_without_double_effect() -> anyhow::Result<()> {
    let (p, s, t, h) = fixture().await?;
    let repo = PgCommandRepository::new(p.clone());
    let producer = TestProducer {
        mode: Mode::Success,
    };
    let changed = request(s, 2);
    let (a, b) = tokio::join!(
        repo.execute(&h, t, keys(), &producer),
        repo.execute(&changed, t, keys(), &producer)
    );
    assert_eq!(
        [a.clone(), b.clone()].iter().filter(|v| v.is_ok()).count(),
        1
    );
    assert_eq!(
        [a, b]
            .iter()
            .filter(|v| **v == Err(CommandError::Conflict))
            .count(),
        1
    );
    assert_eq!(revision(&p, s, t).await, 1);
    assert_eq!(count(&p, "concert_command_receipts", s).await, 1);
    p.close().await;
    Ok(())
}
#[tokio::test]
async fn rollback_has_no_orphan_and_failed_receipt_rolls_back_effects() -> anyhow::Result<()> {
    for failure in [
        SafeFailure::VersionConflict,
        SafeFailure::PartyFull,
        SafeFailure::DuplicateApplication,
        SafeFailure::GiveawaySoldOut,
        SafeFailure::DuplicateActiveReservation,
        SafeFailure::ReservationExpired,
        SafeFailure::PickupCodeInvalid,
        SafeFailure::ResourceUnavailable,
        SafeFailure::InvalidInput,
        SafeFailure::EventReadOnly,
        SafeFailure::MapVersionMismatch,
    ] {
        let (p, s, t, h) = fixture().await?;
        let repo = PgCommandRepository::new(p.clone());
        assert_eq!(
            repo.execute(&h, t, keys(), &TestProducer { mode: Mode::Abort })
                .await,
            Err(CommandError::InvalidState)
        );
        assert_eq!(revision(&p, s, t).await, 0);
        assert_eq!(count(&p, "concert_command_receipts", s).await, 0);
        assert_eq!(count(&p, "concert_outbox", s).await, 0);
        let r = repo
            .execute(
                &h,
                t,
                keys(),
                &TestProducer {
                    mode: Mode::Failed(failure),
                },
            )
            .await?;
        assert_eq!(r.state, ReceiptState::Failed);
        assert_eq!(r.failure, Some(failure));
        assert_eq!(revision(&p, s, t).await, 0);
        assert_eq!(count(&p, "concert_outbox", s).await, 0);
        assert_eq!(
            repo.execute(
                &h,
                t,
                keys(),
                &TestProducer {
                    mode: Mode::Success
                }
            )
            .await?,
            r
        );
        p.close().await;
    }
    Ok(())
}
#[tokio::test]
async fn pending_is_not_success_or_automatic_mutation_retry() -> anyhow::Result<()> {
    let (p, s, t, h) = fixture().await?;
    let repo = PgCommandRepository::new(p.clone());
    let r = repo
        .execute(
            &h,
            t,
            keys(),
            &TestProducer {
                mode: Mode::Pending,
            },
        )
        .await?;
    assert_eq!(r.state, ReceiptState::Pending);
    assert!(r.reference.is_none() && r.result_status.is_none());
    assert_eq!(
        repo.execute(
            &h,
            t,
            keys(),
            &TestProducer {
                mode: Mode::Success
            }
        )
        .await?,
        r
    );
    assert_eq!(revision(&p, s, t).await, 1);
    p.close().await;
    Ok(())
}
#[tokio::test]
async fn current_policy_denies_existing_missing_conflict_and_wrong_scopes() -> anyhow::Result<()> {
    let (p, s, t, h) = fixture().await?;
    let repo = PgCommandRepository::new(p.clone());
    let producer = TestProducer {
        mode: Mode::Success,
    };
    repo.execute(&h, t, keys(), &producer).await?;
    for wrong in [
        Scope {
            actor: Uuid::new_v4(),
            ..s
        },
        Scope {
            event: Uuid::new_v4(),
            ..s
        },
        Scope {
            operation: RegisteredOperation::find("setConcertSettings")?,
            ..s
        },
    ] {
        assert_eq!(
            repo.execute(&request(wrong, 1), t, keys(), &producer).await,
            Err(CommandError::Denied)
        );
        assert_eq!(
            repo.lookup(wrong, t, &producer).await,
            Err(CommandError::Denied)
        );
    }
    assert_eq!(
        repo.lookup(s, Uuid::new_v4(), &producer).await,
        Err(CommandError::Denied)
    );
    sqlx::query("update be15_test_policy set allowed=false where actor=$1 and event=$2")
        .bind(s.actor)
        .bind(s.event)
        .execute(&p)
        .await?;
    assert_eq!(
        repo.lookup(s, t, &producer).await,
        Err(CommandError::Denied)
    );
    assert_eq!(
        repo.execute(&request(s, 2), t, keys(), &producer).await,
        Err(CommandError::Denied)
    );
    assert_eq!(
        repo.lookup(
            Scope {
                key: Uuid::new_v4(),
                ..s
            },
            t,
            &producer
        )
        .await,
        Err(CommandError::Denied)
    );
    assert_eq!(revision(&p, s, t).await, 1);
    p.close().await;
    Ok(())
}
#[tokio::test]
async fn expired_and_retired_tombstones_never_rerun_and_hash_conflict_persists(
) -> anyhow::Result<()> {
    let (p, s, t, h) = fixture().await?;
    let repo = PgCommandRepository::new(p.clone());
    let producer = TestProducer {
        mode: Mode::Success,
    };
    sqlx::query("insert into concert_command_receipts(receipt_id,actor_id,event_id,operation_id,idempotency_key,request_hash,target_id,state,created_at,expires_at) values($1,$2,$3,$4,$5,$6,$7,'PENDING',clock_timestamp()-interval '2 days',clock_timestamp()-interval '1 day 1 second')")
 .bind(Uuid::new_v4()).bind(s.actor).bind(s.event).bind(s.operation.id()).bind(s.key).bind(keys().sign(&h)?.as_storage()).bind(t).execute(&p).await?;
    assert_eq!(
        repo.execute(&h, t, keys(), &producer).await,
        Err(CommandError::Expired)
    );
    assert_eq!(
        repo.lookup(s, t, &producer).await,
        Err(CommandError::Expired)
    );
    assert_eq!(repo.retire_expired(1).await?, 1);
    assert_eq!(
        repo.execute(&h, t, keys(), &producer).await,
        Err(CommandError::Expired)
    );
    assert_eq!(
        repo.execute(&request(s, 2), t, keys(), &producer).await,
        Err(CommandError::Conflict)
    );
    let lost = FingerprintKeys::new(SigningKey::new("other", &[0x77; 32])?, vec![])?;
    assert_eq!(
        repo.execute(&h, t, &lost, &producer).await,
        Err(CommandError::Expired)
    );
    assert_eq!(revision(&p, s, t).await, 0);
    p.close().await;
    Ok(())
}
#[tokio::test]
async fn expired_lease_reclaim_fences_stale_workers_and_consumer_effect_is_once(
) -> anyhow::Result<()> {
    let (p, s, t, h) = fixture().await?;
    PgCommandRepository::new(p.clone())
        .execute(
            &h,
            t,
            keys(),
            &TestProducer {
                mode: Mode::Success,
            },
        )
        .await?;
    let out = PgOutbox::new(p.clone());
    // Clear only this harness's earlier test queues to isolate batch assertions.
    sqlx::query("update concert_outbox set state='DEAD',lease_token=null,lease_until=null,terminal_at=clock_timestamp(),last_error='REJECTED' where event_id<>$1 and state in ('READY','LEASED')").bind(s.event).execute(&p).await?;
    let (a, b) = tokio::join!(out.claim(1), out.claim(1));
    let mut claims = a?;
    claims.extend(b?);
    assert_eq!(claims.len(), 1);
    let first = claims.remove(0);
    // Commit consumer effect, then simulate process loss before acknowledgement.
    consume_once(&p, first.id, s.event).await?;
    sqlx::query("update concert_outbox set lease_until=clock_timestamp()-interval '1 second' where outbox_id=$1").bind(first.id).execute(&p).await?;
    assert!(!out.acknowledge(&first).await?);
    let second = out.claim(1).await?.remove(0);
    assert_eq!(first.id, second.id);
    assert_ne!(first.lease, second.lease);
    assert_eq!(second.attempt, 2);
    assert!(!out.fail(&first, DeliveryFailure::Retryable).await?);
    consume_once(&p, second.id, s.event).await?;
    assert!(out.acknowledge(&second).await?);
    assert!(!out.acknowledge(&second).await?);
    let effects: i64 =
        sqlx::query_scalar("select sum(effects)::bigint from be15_test_consumer where delivery=$1")
            .bind(first.id)
            .fetch_one(&p)
            .await?;
    assert_eq!(effects, 1);
    let applied: i32 =
        sqlx::query_scalar("select applied from be15_test_consumer_effect where event=$1")
            .bind(s.event)
            .fetch_one(&p)
            .await?;
    assert_eq!(applied, 1);
    assert!(out.claim(1).await?.is_empty());
    p.close().await;
    Ok(())
}
#[tokio::test]
async fn retries_are_bounded_backoff_and_exhausted_crash_is_dead() -> anyhow::Result<()> {
    let (p, s, t, h) = fixture().await?;
    PgCommandRepository::new(p.clone())
        .execute(
            &h,
            t,
            keys(),
            &TestProducer {
                mode: Mode::Success,
            },
        )
        .await?;
    sqlx::query("update concert_outbox set state='DEAD',lease_token=null,lease_until=null,terminal_at=clock_timestamp(),last_error='REJECTED' where event_id<>$1 and state in ('READY','LEASED')").bind(s.event).execute(&p).await?;
    let out = PgOutbox::new(p.clone());
    for attempt in 1..=8 {
        let d = out.claim(1).await?.remove(0);
        assert_eq!(d.attempt, attempt);
        if attempt == 8 {
            sqlx::query("update concert_outbox set lease_until=clock_timestamp()-interval '1 second' where outbox_id=$1").bind(d.id).execute(&p).await?;
        } else {
            assert!(out.fail(&d, DeliveryFailure::Retryable).await?);
            assert!(out.claim(1).await?.is_empty());
            let future: bool = sqlx::query_scalar(
                "select available_at>clock_timestamp() from concert_outbox where outbox_id=$1",
            )
            .bind(d.id)
            .fetch_one(&p)
            .await?;
            assert!(future);
            sqlx::query("update concert_outbox set available_at=clock_timestamp()-interval '1 second' where outbox_id=$1").bind(d.id).execute(&p).await?;
        }
    }
    assert!(out.claim(1).await?.is_empty());
    let state: String = sqlx::query_scalar("select state from concert_outbox where event_id=$1")
        .bind(s.event)
        .fetch_one(&p)
        .await?;
    assert_eq!(state, "DEAD");
    // Prune only terminal diagnostic rows, never their command tombstones.
    sqlx::query("update concert_outbox set terminal_at=clock_timestamp()-interval '8 days' where event_id=$1").bind(s.event).execute(&p).await?;
    assert_eq!(out.prune(1).await?, 1);
    assert_eq!(count(&p, "concert_command_receipts", s).await, 1);
    assert!(out.claim(101).await.is_err());
    assert!(out.prune(0).await.is_err());
    p.close().await;
    Ok(())
}
#[tokio::test]
async fn lifetime_exhaustion_rolls_back_and_storage_has_no_secret_payload_columns(
) -> anyhow::Result<()> {
    let (p, s, t, h) = fixture().await?;
    let repo = PgCommandRepository::new(p.clone());
    let used: i64 = sqlx::query_scalar("select receipts_used from concert_command_budget")
        .fetch_one(&p)
        .await?;
    sqlx::query("update concert_command_budget set receipts_used=1000000")
        .execute(&p)
        .await?;
    let result = repo
        .execute(
            &h,
            t,
            keys(),
            &TestProducer {
                mode: Mode::Success,
            },
        )
        .await;
    sqlx::query("update concert_command_budget set receipts_used=$1")
        .bind(used)
        .execute(&p)
        .await?;
    assert_eq!(result, Err(CommandError::Capacity));
    assert_eq!(revision(&p, s, t).await, 0);
    assert_eq!(count(&p, "concert_command_receipts", s).await, 0);
    let columns:Vec<String>=sqlx::query_scalar("select column_name from information_schema.columns where table_schema='public' and table_name in ('concert_outbox','concert_command_receipts')").fetch_all(&p).await?;
    for forbidden in [
        "body",
        "payload",
        "result",
        "token",
        "handle",
        "pickup_code",
        "age_proof",
    ] {
        assert!(!columns.contains(&forbidden.to_string()));
    }
    let error = sqlx::query("insert into concert_outbox(payload) values('token=do-not-store')")
        .execute(&p)
        .await
        .unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().code().as_deref(),
        Some("42703")
    );
    // Closed safe failure enum is enforced in the database as well as Rust.
    let receipt = repo
        .execute(
            &h,
            t,
            keys(),
            &TestProducer {
                mode: Mode::Failed(SafeFailure::VersionConflict),
            },
        )
        .await?;
    let error = sqlx::query(
        "update concert_command_receipts set failure_reason='private-handle' where receipt_id=$1",
    )
    .bind(receipt.id)
    .execute(&p)
    .await
    .unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().code().as_deref(),
        Some("23514")
    );
    p.close().await;
    Ok(())
}
#[tokio::test]
async fn concurrent_role_revocation_is_rechecked_under_database_lock() -> anyhow::Result<()> {
    let (p, s, t, h) = fixture().await?;
    let repo = PgCommandRepository::new(p.clone());
    repo.execute(
        &h,
        t,
        keys(),
        &TestProducer {
            mode: Mode::Success,
        },
    )
    .await?;
    let mut policy = p.begin().await?;
    sqlx::query("update be15_test_policy set allowed=false where actor=$1 and event=$2")
        .bind(s.actor)
        .bind(s.event)
        .execute(&mut *policy)
        .await?;
    let mut replay = tokio::spawn(async move {
        repo.execute(
            &h,
            t,
            keys(),
            &TestProducer {
                mode: Mode::Success,
            },
        )
        .await
    });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), &mut replay)
            .await
            .is_err()
    );
    policy.commit().await?;
    assert_eq!(replay.await?, Err(CommandError::Denied));
    assert_eq!(revision(&p, s, t).await, 1);
    p.close().await;
    Ok(())
}
struct TestSink {
    pool: PgPool,
}
#[async_trait]
impl concert_service::workers::outbox::IdempotentInvalidationSink for TestSink {
    async fn deliver(
        &self,
        d: &concert_service::adapters::out::repository::outbox::Delivery,
    ) -> Result<(), DeliveryFailure> {
        consume_once(&self.pool, d.id, d.event)
            .await
            .map_err(|_| DeliveryFailure::Retryable)?;
        Ok(())
    }
}
#[tokio::test]
async fn explicit_batch_runner_delivers_real_idempotent_consumer_effect() -> anyhow::Result<()> {
    let (p, s, t, h) = fixture().await?;
    PgCommandRepository::new(p.clone())
        .execute(
            &h,
            t,
            keys(),
            &TestProducer {
                mode: Mode::Success,
            },
        )
        .await?;
    sqlx::query("update concert_outbox set state='DEAD',lease_token=null,lease_until=null,terminal_at=clock_timestamp(),last_error='REJECTED' where event_id<>$1 and state in ('READY','LEASED')").bind(s.event).execute(&p).await?;
    let out = PgOutbox::new(p.clone());
    let sink = std::sync::Arc::new(TestSink { pool: p.clone() });
    let result = concert_service::workers::outbox::run_batch(&out, sink.clone(), 10).await?;
    assert_eq!(
        (
            result.claimed,
            result.acknowledged,
            result.failed,
            result.stale
        ),
        (1, 1, 0, 0)
    );
    let result = concert_service::workers::outbox::run_batch(&out, sink, 10).await?;
    assert_eq!(result.claimed, 0);
    let effects:i64=sqlx::query_scalar("select count(*) from be15_test_consumer c join concert_outbox o on c.delivery=o.outbox_id where o.event_id=$1").bind(s.event).fetch_one(&p).await?;
    assert_eq!(effects, 1);
    let applied: i32 =
        sqlx::query_scalar("select applied from be15_test_consumer_effect where event=$1")
            .bind(s.event)
            .fetch_one(&p)
            .await?;
    assert_eq!(applied, 1);
    p.close().await;
    Ok(())
}
#[tokio::test]
async fn actor_and_queue_admission_limits_roll_back_complete_command() -> anyhow::Result<()> {
    let (p, s, t, h) = fixture().await?;
    let repo = PgCommandRepository::new(p.clone());
    let producer = TestProducer {
        mode: Mode::Success,
    };
    sqlx::query("insert into concert_command_receipts(receipt_id,actor_id,event_id,operation_id,idempotency_key,request_hash,target_id,state) select gen_random_uuid(),$1,$2,$3,gen_random_uuid(),$4,$5,'PENDING' from generate_series(1,1000)").bind(s.actor).bind(s.event).bind(s.operation.id()).bind(keys().sign(&h)?.as_storage()).bind(t).execute(&p).await?;
    assert_eq!(
        repo.execute(&h, t, keys(), &producer).await,
        Err(CommandError::Capacity)
    );
    assert_eq!(revision(&p, s, t).await, 0);
    let (actor_scope, new_target) = (
        Scope {
            actor: Uuid::new_v4(),
            key: Uuid::new_v4(),
            ..s
        },
        Uuid::new_v4(),
    );
    sqlx::query("insert into be15_test_policy values($1,$2,$3,$4,true)")
        .bind(actor_scope.actor)
        .bind(s.event)
        .bind(s.operation.id())
        .bind(new_target)
        .execute(&p)
        .await?;
    sqlx::query("insert into be15_test_aggregate values($1,$2,0)")
        .bind(s.event)
        .bind(new_target)
        .execute(&p)
        .await?;
    sqlx::query("update concert_outbox set state='DEAD',lease_token=null,lease_until=null,terminal_at=clock_timestamp(),last_error='REJECTED' where state in ('READY','LEASED')").execute(&p).await?;
    sqlx::query("with fixtures as (insert into concert_command_receipts(receipt_id,actor_id,event_id,operation_id,idempotency_key,request_hash,target_id,state) select gen_random_uuid(),gen_random_uuid(),$1,$2,gen_random_uuid(),$3,$4,'PENDING' from generate_series(1,10000) returning receipt_id,event_id,target_id) insert into concert_outbox(outbox_id,receipt_id,event_id,resource_id,resource_revision) select gen_random_uuid(),receipt_id,event_id,target_id,1 from fixtures").bind(s.event).bind(s.operation.id()).bind(keys().sign(&h)?.as_storage()).bind(t).execute(&p).await?;
    assert_eq!(
        repo.execute(&request(actor_scope, 1), new_target, keys(), &producer)
            .await,
        Err(CommandError::Capacity)
    );
    assert_eq!(revision(&p, actor_scope, new_target).await, 0);
    assert!(repo
        .lookup(actor_scope, new_target, &producer)
        .await?
        .is_none());
    // This is test-only fixture administration; no production cleanup API exists.
    sqlx::query("update concert_outbox set state='DEAD',lease_token=null,lease_until=null,terminal_at=clock_timestamp(),last_error='REJECTED' where event_id=$1").bind(s.event).execute(&p).await?;
    p.close().await;
    Ok(())
}

#[tokio::test]
async fn safe_reference_must_bind_authorized_target_and_roll_back_other_effects(
) -> anyhow::Result<()> {
    let (p, s, t, h) = fixture().await?;
    let repo = PgCommandRepository::new(p.clone());
    assert_eq!(
        repo.execute(
            &h,
            t,
            keys(),
            &TestProducer {
                mode: Mode::WrongReference
            }
        )
        .await,
        Err(CommandError::InvalidState)
    );
    assert_eq!(revision(&p, s, t).await, 0);
    assert_eq!(count(&p, "concert_command_receipts", s).await, 0);
    assert_eq!(count(&p, "concert_outbox", s).await, 0);
    p.close().await;
    Ok(())
}
#[tokio::test]
async fn rotated_active_key_verifies_original_receipt_and_lost_key_never_reruns(
) -> anyhow::Result<()> {
    let (p, s, t, input) = fixture().await?;
    let repo = PgCommandRepository::new(p.clone());
    let producer = TestProducer {
        mode: Mode::Success,
    };
    let receipt = repo.execute(&input, t, keys(), &producer).await?;
    let before: String =
        sqlx::query_scalar("select request_hash from concert_command_receipts where receipt_id=$1")
            .bind(receipt.id)
            .fetch_one(&p)
            .await?;
    let rotated = FingerprintKeys::new(
        SigningKey::new("test_v2", &[0x77; 32])?,
        vec![SigningKey::new("test_v1", &[0x42; 32])?],
    )?;
    assert_eq!(repo.execute(&input, t, &rotated, &producer).await?, receipt);
    let after: String =
        sqlx::query_scalar("select request_hash from concert_command_receipts where receipt_id=$1")
            .bind(receipt.id)
            .fetch_one(&p)
            .await?;
    assert!(before == after);
    let lost = FingerprintKeys::new(SigningKey::new("test_v2", &[0x77; 32])?, vec![])?;
    assert_eq!(
        repo.execute(&input, t, &lost, &producer).await,
        Err(CommandError::Unavailable)
    );
    assert_eq!(revision(&p, s, t).await, 1);
    assert_eq!(count(&p, "concert_outbox", s).await, 1);
    let next = Scope {
        key: Uuid::new_v4(),
        ..s
    };
    let new_receipt = repo
        .execute(&request(next, 1), t, &rotated, &producer)
        .await?;
    let stored: String =
        sqlx::query_scalar("select request_hash from concert_command_receipts where receipt_id=$1")
            .bind(new_receipt.id)
            .fetch_one(&p)
            .await?;
    assert!(stored.starts_with("hmac-sha256:v1:test_v2:"));
    p.close().await;
    Ok(())
}
#[tokio::test]
async fn low_entropy_secret_changes_conflict_without_plaintext_unkeyed_or_error_leaks(
) -> anyhow::Result<()> {
    let (p, s, t, _) = fixture().await?;
    let s = Scope {
        operation: RegisteredOperation::find("claimConcertReservation")?,
        ..s
    };
    sqlx::query("insert into be15_test_policy values($1,$2,$3,$4,true)")
        .bind(s.actor)
        .bind(s.event)
        .bind(s.operation.id())
        .bind(t)
        .execute(&p)
        .await?;
    let build = |secret: &str| {
        CanonicalRequest::json(
            s,
            &[
                ("eventId", &s.event.to_string()),
                ("reservationId", &t.to_string()),
            ],
            &[],
            Some(&CanonicalValue::Object(vec![
                ("expected_revision".into(), CanonicalValue::Integer(1)),
                (
                    "confirmation_code".into(),
                    CanonicalValue::String(secret.into()),
                ),
            ])),
        )
        .unwrap()
    };
    let input = build("123456");
    let changed = build("654321");
    let repo = PgCommandRepository::new(p.clone());
    let producer = TestProducer {
        mode: Mode::Success,
    };
    let receipt = repo.execute(&input, t, keys(), &producer).await?;
    let error = repo
        .execute(&changed, t, keys(), &producer)
        .await
        .unwrap_err();
    assert_eq!(error, CommandError::Conflict);
    let stored: String =
        sqlx::query_scalar("select request_hash from concert_command_receipts where receipt_id=$1")
            .bind(receipt.id)
            .fetch_one(&p)
            .await?;
    assert!(stored.starts_with("hmac-sha256:v1:test_v1:"));
    // Public/debug projection and errors have no fingerprint, raw code or injected key.
    for output in [
        format!("{receipt:?}"),
        format!("{error:?}"),
        error.to_string(),
    ] {
        for private in [
            &stored,
            "123456",
            "654321",
            "confirmation_code",
            "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
        ] {
            assert!(!output.contains(private));
        }
    }
    let row: String = sqlx::query_scalar(
        "select row_to_json(r)::text from concert_command_receipts r where receipt_id=$1",
    )
    .bind(receipt.id)
    .fetch_one(&p)
    .await?;
    assert!(!row.contains("confirmation_code"));
    assert_eq!(revision(&p, s, t).await, 1);
    p.close().await;
    Ok(())
}
struct DelayedAuthorization {
    calls: std::sync::atomic::AtomicUsize,
}
#[async_trait]
impl CommandProducer for DelayedAuthorization {
    async fn authorize(&self, c: &mut PgConnection, s: Scope, t: Uuid) -> Result<(), CommandError> {
        TestProducer {
            mode: Mode::Success,
        }
        .authorize(c, s, t)
        .await?;
        if self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 1 {
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
        Ok(())
    }
    async fn apply(
        &self,
        _: &mut PgConnection,
        _: Scope,
        _: Uuid,
    ) -> Result<(Completion, Option<Invalidation>), CommandError> {
        panic!("expired receipt cannot execute")
    }
}
#[tokio::test]
async fn expiry_during_stored_target_authorization_is_rechecked_for_execute_and_lookup(
) -> anyhow::Result<()> {
    for lookup in [true, false] {
        let (p, s, t, input) = fixture().await?;
        let repo = PgCommandRepository::new(p.clone());
        sqlx::query("insert into concert_command_receipts(receipt_id,actor_id,event_id,operation_id,idempotency_key,request_hash,target_id,state,resource_id,resource_revision,result_status,created_at,expires_at) values($1,$2,$3,$4,$5,$6,$7,'SUCCEEDED',$7,1,200,clock_timestamp()-interval '1 hour',clock_timestamp()+interval '150 milliseconds')").bind(Uuid::new_v4()).bind(s.actor).bind(s.event).bind(s.operation.id()).bind(s.key).bind(keys().sign(&input)?.as_storage()).bind(t).execute(&p).await?;
        let delayed = DelayedAuthorization {
            calls: std::sync::atomic::AtomicUsize::new(0),
        };
        if lookup {
            assert_eq!(
                repo.lookup(s, t, &delayed).await,
                Err(CommandError::Expired)
            );
        } else {
            assert_eq!(
                repo.execute(&input, t, keys(), &delayed).await,
                Err(CommandError::Expired)
            );
        }
        assert_eq!(revision(&p, s, t).await, 0);
        assert_eq!(count(&p, "concert_outbox", s).await, 0);
        p.close().await;
    }
    Ok(())
}
#[tokio::test]
async fn succeeded_outbox_revision_mismatch_rolls_back_receipt_aggregate_and_enqueue(
) -> anyhow::Result<()> {
    let (p, s, t, input) = fixture().await?;
    let repo = PgCommandRepository::new(p.clone());
    assert_eq!(
        repo.execute(
            &input,
            t,
            keys(),
            &TestProducer {
                mode: Mode::WrongRevision
            }
        )
        .await,
        Err(CommandError::InvalidState)
    );
    assert_eq!(revision(&p, s, t).await, 0);
    assert_eq!(count(&p, "concert_command_receipts", s).await, 0);
    assert_eq!(count(&p, "concert_outbox", s).await, 0);
    p.close().await;
    Ok(())
}
