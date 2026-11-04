//! Real owned PostgreSQL only. No ambient DB, default skip, listener or fake Auth.
//! The two ignored phases REQUIRE the external owned runner's actual PG restart.
mod concert_inbox_support;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use notification_service::{
    adapters::out::persistence::notification_repository::PgNotificationRepository,
    application::{
        dto::repository_dto::NotificationRecordDto,
        errors::{NotificationError, NotificationInvalidInput},
        notification_contracts::NotificationCreateOutcome,
        ports::NotificationRepositoryPort,
    },
    concert::{deletion_owner::*, PgConcertInbox, UnavailableAuthority},
};
use sqlx::{migrate::Migrator, PgPool, Row};
use std::{borrow::Cow, os::unix::fs::OpenOptionsExt, time::Duration};
use uuid::Uuid;

async fn pool() -> anyhow::Result<PgPool> {
    let pool = concert_inbox_support::owned_pool("mcr_concert_notification").await?;
    Migrator::new(std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations"
    )))
    .await?
    .run(&pool)
    .await?;
    Ok(pool)
}
fn command() -> DeletionCommand {
    DeletionCommand::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        DateTime::from_timestamp(1_800_000_000, 123_456_789).unwrap(),
    )
    .unwrap()
}
fn notification(user: Uuid, actor: Uuid) -> NotificationRecordDto {
    NotificationRecordDto {
        id: Uuid::new_v4(),
        user_id: user,
        notification_type: "SYSTEM".into(),
        payload_json: "retained-private-fixture".into(),
        created_at: Utc::now(),
        read_at: None,
        event_id: Uuid::new_v4(),
        actor_id: Some(actor),
        occurred_at: Utc::now(),
    }
}
async fn live_count(pool: &PgPool, user: Uuid) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar("select count(*) from notifications where (user_id=$1 or actor_id=$1) and deleted_at is null")
        .bind(user).fetch_one(pool).await?)
}
async fn proof_count(pool: &PgPool, saga: Uuid) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar(
        "select count(*) from notification_deletion_receipts_v1 where saga_id=$1",
    )
    .bind(saga)
    .fetch_one(pool)
    .await?)
}
async fn probe(pool: &PgPool) -> anyhow::Result<bool> {
    Ok(
        sqlx::query_scalar(include_str!("../src/config/schema-probe.sql"))
            .fetch_one(pool)
            .await?,
    )
}
async fn wait_for_waiters(pool: &PgPool, count: i64) -> anyhow::Result<()> {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let waiting: i64 = sqlx::query_scalar("select count(*) from pg_stat_activity where datname=current_database() and pid<>pg_backend_pid() and wait_event_type='Lock'")
                .fetch_one(pool).await?;
            if waiting >= count { return Ok::<(), sqlx::Error>(()); }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await??;
    Ok(())
}

#[test]
fn command_rejects_nil_and_redacts_private_values() -> anyhow::Result<()> {
    let c = command();
    for (s, u, a) in [
        (Uuid::nil(), c.user_id(), c.actor_id()),
        (c.saga_id(), Uuid::nil(), c.actor_id()),
        (c.saga_id(), c.user_id(), Uuid::nil()),
    ] {
        assert!(DeletionCommand::new(s, u, a, c.retention_until()).is_err());
    }
    let debug = format!("{c:?}");
    for value in [
        c.saga_id().to_string(),
        c.user_id().to_string(),
        c.actor_id().to_string(),
        c.retention_until().to_rfc3339(),
    ] {
        assert!(!debug.contains(&value));
    }
    assert_eq!(c.retention_until().timestamp_subsec_nanos(), 123_456_789);
    Ok(())
}
#[tokio::test]
async fn committed_exact_receipt_retry_full_command_rebound_and_wire_compatibility(
) -> anyhow::Result<()> {
    let pool = pool().await?;
    let repo = PgNotificationRepository::new(pool.clone());
    let c = command();
    let n = notification(c.user_id(), Uuid::new_v4());
    repo.create_consistent(&n).await?;
    let first = repo.apply_user_deletion_receipt(c.clone()).await?;
    assert_eq!(first.command(), &c);
    assert!(first.fence_revision() > 0);
    let xid: String = sqlx::query_scalar(
        "select effect_xid::text from notification_deletion_receipts_v1 where saga_id=$1",
    )
    .bind(c.saga_id())
    .fetch_one(&pool)
    .await?;
    assert_eq!(first.effect_xid(), xid);
    assert_eq!(repo.apply_user_deletion_receipt(c.clone()).await?, first);
    repo.apply_user_deletion(c.saga_id(), c.user_id(), c.actor_id(), c.retention_until())
        .await?;
    for changed in [
        DeletionCommand::new(
            c.saga_id(),
            Uuid::new_v4(),
            c.actor_id(),
            c.retention_until(),
        )?,
        DeletionCommand::new(
            c.saga_id(),
            c.user_id(),
            Uuid::new_v4(),
            c.retention_until(),
        )?,
        DeletionCommand::new(
            c.saga_id(),
            c.user_id(),
            c.actor_id(),
            c.retention_until() + ChronoDuration::nanoseconds(1),
        )?,
    ] {
        assert_eq!(
            repo.apply_user_deletion_receipt(changed).await.unwrap_err(),
            NotificationError::InvalidInput(NotificationInvalidInput::DeletionSagaInputMismatch)
        );
    }
    assert_eq!(proof_count(&pool, c.saga_id()).await?, 1);
    assert_eq!(live_count(&pool, c.user_id()).await?, 0);
    let row = sqlx::query("select payload_json,retention_until from notifications where id=$1")
        .bind(n.id)
        .fetch_one(&pool)
        .await?;
    assert_eq!(row.get::<String, _>("payload_json"), n.payload_json);
    assert_eq!(
        row.get::<DateTime<Utc>, _>("retention_until")
            .timestamp_micros(),
        c.retention_until().timestamp_micros()
    );
    assert!(!format!("{first:?}").contains(&c.user_id().to_string()));
    pool.close().await;
    Ok(())
}
#[tokio::test]
async fn failure_after_effects_rolls_back_receipt_fence_audit_and_subject() -> anyhow::Result<()> {
    let pool = pool().await?;
    let repo = PgNotificationRepository::new(pool.clone());
    let c = command();
    let n = notification(c.user_id(), Uuid::new_v4());
    repo.create_consistent(&n).await?;
    // A deferred test-only constraint fails at the REAL commit, after every
    // production statement. Cleanup is in an unconditional result branch.
    sqlx::raw_sql("create function be33_test_commit_failure() returns trigger language plpgsql as $$ begin raise exception 'owned commit failure'; end $$; create constraint trigger be33_test_commit_failure after insert on notification_deletion_receipts_v1 deferrable initially deferred for each row execute function be33_test_commit_failure();").execute(&pool).await?;
    let result = repo.apply_user_deletion_receipt(c.clone()).await;
    sqlx::raw_sql("drop trigger be33_test_commit_failure on notification_deletion_receipts_v1; drop function be33_test_commit_failure();").execute(&pool).await?;
    assert!(result.is_err());
    assert_eq!(proof_count(&pool, c.saga_id()).await?, 0);
    assert_eq!(live_count(&pool, c.user_id()).await?, 1);
    let leftovers:i64=sqlx::query_scalar("select (select count(*) from notification_deletion_fences_v1 where subject_id=$1)+(select count(*) from notification_user_deletion_receipts where saga_id=$2)+(select count(*) from notification_audit_events where id=$2)").bind(c.user_id()).bind(c.saga_id()).fetch_one(&pool).await?;
    assert_eq!(leftovers, 0);
    assert!(!repo.is_actor_suppressed(c.user_id()).await?);
    repo.apply_user_deletion_receipt(c).await?;
    pool.close().await;
    Ok(())
}
#[tokio::test]
async fn immutable_evidence_permanent_deny_and_bounded_purge_remain_real() -> anyhow::Result<()> {
    let pool = pool().await?;
    let repo = PgNotificationRepository::new(pool.clone());
    let c = command();
    let c = DeletionCommand::new(
        c.saga_id(),
        c.user_id(),
        c.actor_id(),
        c.retention_until() - ChronoDuration::seconds(100),
    )?;
    let recipient = notification(c.user_id(), Uuid::new_v4());
    let actor = notification(Uuid::new_v4(), c.user_id());
    repo.create_consistent(&recipient).await?;
    repo.create_consistent(&actor).await?;
    let proof = repo.apply_user_deletion_receipt(c.clone()).await?;
    for sql in [
        "update notification_deletion_receipts_v1 set actor_id=actor_id where saga_id=$1",
        "delete from notification_deletion_receipts_v1 where saga_id=$1",
        "update notification_deletion_fences_v1 set recorded_at=recorded_at where subject_id=$1",
        "delete from notification_deletion_fences_v1 where subject_id=$1",
        "update notification_user_deletion_receipts set actor_id=actor_id where saga_id=$1",
        "delete from notification_user_deletion_receipts where saga_id=$1",
        "update notification_audit_events set metadata_json='{}' where id=$1",
        "delete from notification_audit_events where id=$1",
        "update notification_subject_states set suppress_as_actor=false where subject_id=$1",
        "update notification_subject_states set suppress_as_recipient=false where subject_id=$1",
        "delete from notification_subject_states where subject_id=$1",
        "update notifications set deleted_at=null where user_id=$1 or actor_id=$1",
    ] {
        let id=if sql.contains("subject_id") || sql.contains("update notifications") {c.user_id()}else{c.saga_id()};
        let error=sqlx::query(sql).bind(id).execute(&pool).await.unwrap_err();
        assert!(error.as_database_error().is_some_and(|e|e.code().is_some_and(|c|c=="23514")),"{sql}");
    }
    for table in [
        "notification_deletion_receipts_v1",
        "notification_deletion_fences_v1",
        "notification_subject_states",
        "notification_audit_events",
        "notification_user_deletion_receipts",
    ] {
        assert!(sqlx::raw_sql(&format!("truncate {table} cascade"))
            .execute(&pool)
            .await
            .is_err());
    }
    assert!(matches!(
        repo.create_consistent(&notification(c.user_id(), Uuid::new_v4()))
            .await?,
        NotificationCreateOutcome::Suppressed(_)
    ));
    assert!(matches!(
        repo.create_consistent(&notification(Uuid::new_v4(), c.user_id()))
            .await?,
        NotificationCreateOutcome::Suppressed(_)
    ));
    // Existing bounded retention purge is unchanged. It retains the immutable
    // command/audit/fence even when all recipient/actor notification rows go.
    let purged = repo
        .purge_expired_batch_consistent(
            c.retention_until() + ChronoDuration::seconds(1),
            c.actor_id(),
            None,
            None,
            None,
            false,
            1,
        )
        .await?;
    assert_eq!(purged.len(), 1);
    let second = repo
        .purge_expired_batch_consistent(
            c.retention_until() + ChronoDuration::seconds(1),
            c.actor_id(),
            None,
            None,
            None,
            false,
            1,
        )
        .await?;
    assert_eq!(second.len(), 1);
    let retained: i64 =
        sqlx::query_scalar("select count(*) from notifications where user_id=$1 or actor_id=$1")
            .bind(c.user_id())
            .fetch_one(&pool)
            .await?;
    assert_eq!(retained, 0);
    assert_eq!(repo.apply_user_deletion_receipt(c.clone()).await?, proof);
    assert_eq!(proof_count(&pool, c.saga_id()).await?, 1);
    pool.close().await;
    Ok(())
}
#[tokio::test]
async fn concurrent_create_delete_retry_and_concert_replay_never_resurrect() -> anyhow::Result<()> {
    use concert_domain::notification::{Envelope, Kind, Recipient};
    let pool = pool().await?;
    let repo = PgNotificationRepository::new(pool.clone());
    let c = command();
    sqlx::query("insert into notification_subject_states(subject_id) values($1)")
        .bind(c.user_id())
        .execute(&pool)
        .await?;
    let mut blocker = pool.begin().await?;
    sqlx::query(
        "select subject_id from notification_subject_states where subject_id=$1 for update",
    )
    .bind(c.user_id())
    .execute(&mut *blocker)
    .await?;
    let creation = {
        let repo = repo.clone();
        let n = notification(c.user_id(), Uuid::new_v4());
        tokio::spawn(async move { repo.create_consistent(&n).await })
    };
    let deletion = {
        let repo = repo.clone();
        let c = c.clone();
        tokio::spawn(async move { repo.apply_user_deletion_receipt(c).await })
    };
    wait_for_waiters(&pool, 2).await?;
    blocker.commit().await?;
    tokio::time::timeout(Duration::from_secs(5), creation).await???;
    let original = tokio::time::timeout(Duration::from_secs(5), deletion).await???;
    let (a, b) = tokio::join!(
        repo.apply_user_deletion_receipt(c.clone()),
        repo.apply_user_deletion_receipt(c.clone())
    );
    assert_eq!(a?, original);
    assert_eq!(b?, original);
    assert_eq!(live_count(&pool, c.user_id()).await?, 0);
    let envelope = Envelope {
        outbox_id: Uuid::new_v4(),
        receipt_id: Uuid::new_v4(),
        event_id: Uuid::new_v4(),
        resource_id: Uuid::new_v4(),
        resource_revision: 1,
    };
    let recipients = vec![
        Recipient {
            user_id: c.user_id(),
            source_actor_id: Some(Uuid::new_v4()),
            kind: Kind::Notice,
            target: None,
        },
        Recipient {
            user_id: Uuid::new_v4(),
            source_actor_id: Some(c.user_id()),
            kind: Kind::Notice,
            target: None,
        },
    ];
    let inbox = PgConcertInbox::new(pool.clone(), UnavailableAuthority);
    inbox.ingest(envelope, recipients.clone()).await?;
    inbox.ingest(envelope, recipients).await?;
    assert_eq!(live_count(&pool, c.user_id()).await?, 0);
    assert_eq!(proof_count(&pool, c.saga_id()).await?, 1);
    pool.close().await;
    Ok(())
}
#[tokio::test]
async fn upgrade_does_not_certify_historical_mutable_receipts_or_audits() -> anyhow::Result<()> {
    let pool = concert_inbox_support::owned_pool("mcr_concert_notification_source").await?;
    let path = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations"));
    let mut initial = Migrator::new(path).await?;
    initial.migrations = Cow::Owned(initial.iter().filter(|m| m.version <= 6).cloned().collect());
    initial.run(&pool).await?;
    let c = command();
    sqlx::query("insert into notification_user_deletion_receipts(saga_id,user_id,actor_id,retention_until) values($1,$2,$3,$4)").bind(c.saga_id()).bind(c.user_id()).bind(c.actor_id()).bind(c.retention_until()).execute(&pool).await?;
    let audit_only = command();
    sqlx::query("insert into notification_audit_events(id,user_id,actor_id,action,outcome) values($1,$2,$3,'USER_DELETION_APPLIED','SUCCESS')").bind(audit_only.saga_id()).bind(audit_only.user_id()).bind(audit_only.actor_id()).execute(&pool).await?;
    Migrator::new(path).await?.run(&pool).await?;
    assert!(probe(&pool).await?);
    assert_eq!(proof_count(&pool, c.saga_id()).await?, 0);
    let repo = PgNotificationRepository::new(pool.clone());
    assert_eq!(
        repo.apply_user_deletion_receipt(c.clone())
            .await
            .unwrap_err(),
        NotificationError::DependencyUnavailable
    );
    assert_eq!(
        repo.apply_user_deletion_receipt(audit_only.clone())
            .await
            .unwrap_err(),
        NotificationError::DependencyUnavailable
    );
    assert_eq!(proof_count(&pool, audit_only.saga_id()).await?, 0);
    // Unlinked historical compatibility rows were not silently made immutable.
    sqlx::query(
        "update notification_user_deletion_receipts set actor_id=actor_id where saga_id=$1",
    )
    .bind(c.saga_id())
    .execute(&pool)
    .await?;
    let ledger: i64 = sqlx::query_scalar("select count(*) from _sqlx_migrations where success")
        .fetch_one(&pool)
        .await?;
    assert_eq!(ledger, 7);
    pool.close().await;
    Ok(())
}
#[tokio::test]
async fn schema_probe_refuses_disabled_immutability_or_commit_proof() -> anyhow::Result<()> {
    let pool = pool().await?;
    assert!(probe(&pool).await?);
    for (table, trigger) in [
        (
            "notification_deletion_receipts_v1",
            "notification_deletion_receipt_immutable_v1",
        ),
        (
            "notification_deletion_fences_v1",
            "notification_deletion_fence_immutable_v1",
        ),
        (
            "notification_audit_events",
            "notification_deletion_audit_guard_v1",
        ),
        (
            "notification_user_deletion_receipts",
            "notification_deletion_legacy_guard_v1",
        ),
        (
            "notification_subject_states",
            "notification_deletion_subject_guard_v1",
        ),
        (
            "notification_deletion_receipts_v1",
            "notification_deletion_receipt_commit_v1",
        ),
    ] {
        let mut tx = pool.begin().await?;
        sqlx::raw_sql(&format!("alter table {table} disable trigger {trigger}"))
            .execute(&mut *tx)
            .await?;
        let valid: bool = sqlx::query_scalar(include_str!("../src/config/schema-probe.sql"))
            .fetch_one(&mut *tx)
            .await?;
        tx.rollback().await?;
        assert!(!valid, "{trigger}");
    }
    let mut tx = pool.begin().await?;
    let c = command();
    sqlx::query("insert into notification_subject_states(subject_id,suppress_as_actor,suppress_as_recipient) values($1,true,true)").bind(c.user_id()).execute(&mut *tx).await?;
    assert!(sqlx::query("insert into notification_deletion_fences_v1(subject_id,established_xid) values($1,'1'::xid8)").bind(c.user_id()).execute(&mut *tx).await.is_err());
    tx.rollback().await?;
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn minimal_runtime_role_commits_without_proof_rewrite_privileges() -> anyhow::Result<()> {
    let owner = pool().await?;
    let role = format!("be33_deletion_{}", Uuid::new_v4().simple());
    sqlx::raw_sql(&format!("create role {role} nologin"))
        .execute(&owner)
        .await?;
    let grants =
        include_str!("../runtime-grants.sql").replace(":\"runtime_role\"", &format!("\"{role}\""));
    let role_for_pool = role.clone();
    let runtime = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .after_connect(move |connection, _| {
            let role = role_for_pool.clone();
            Box::pin(async move {
                sqlx::query(&format!("set role {role}"))
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .connect_lazy_with((*owner.connect_options()).clone());
    let result:anyhow::Result<()>=async {
        sqlx::raw_sql(&grants).execute(&owner).await?;
        // Exact future Root grant dependency, not a worker edit to its file.
        sqlx::raw_sql(&format!("grant select,insert on notification_deletion_fences_v1,notification_deletion_receipts_v1 to {role}; grant usage on sequence notification_deletion_fences_v1_fence_revision_seq to {role}")).execute(&owner).await?;
        let repo=PgNotificationRepository::new(runtime.clone());let c=command();
        let proof=repo.apply_user_deletion_receipt(c.clone()).await?;
        assert!(proof.fence_revision()>0);
        assert_eq!(repo.apply_user_deletion_receipt(c.clone()).await?,proof);
        for sql in ["delete from notification_deletion_receipts_v1 where saga_id=$1","update notification_deletion_receipts_v1 set actor_id=actor_id where saga_id=$1","update notification_audit_events set metadata_json='{}' where id=$1"] {
            assert!(sqlx::query(sql).bind(c.saga_id()).execute(&runtime).await.is_err());
        }
        Ok(())
    }.await;
    runtime.close().await;
    sqlx::raw_sql(&format!("drop owned by {role}; drop role {role}"))
        .execute(&owner)
        .await?;
    owner.close().await;
    result
}

fn checkpoint() -> anyhow::Result<std::path::PathBuf> {
    // Reuse the existing ownership guard before calling this. Never accept a
    // caller-chosen checkpoint outside the private root verified by owned_pool.
    Ok(
        std::path::PathBuf::from(std::env::var("CONCERT_TEST_OWNER_ROOT")?)
            .join("deletion-restart.json"),
    )
}
#[tokio::test]
#[ignore = "owned runner seed, then actual PostgreSQL restart before recovery"]
async fn restart_seed_committed_receipt_with_lost_reply() -> anyhow::Result<()> {
    use std::io::Write;
    let pool = pool().await?;
    let repo = PgNotificationRepository::new(pool.clone());
    let c = command();
    let proof = repo.apply_user_deletion_receipt(c.clone()).await?;
    let value = serde_json::json!({"saga":c.saga_id(),"user":c.user_id(),"actor":c.actor_id(),"retention":c.retention_until(),"revision":proof.fence_revision(),"xid":proof.effect_xid(),"recorded":proof.recorded_at(),"postmaster":sqlx::query_scalar::<_,DateTime<Utc>>("select pg_postmaster_start_time()").fetch_one(&pool).await?});
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(checkpoint()?)?;
    file.write_all(serde_json::to_string(&value)?.as_bytes())?;
    file.sync_all()?;
    // Discard the returned response: this proves durable reply recovery, not an
    // invented transport-level dropped packet or actual Auth saga completion.
    drop(proof);
    pool.close().await;
    Ok(())
}
#[tokio::test]
#[ignore = "owned runner must actually restart PostgreSQL after seed"]
async fn restart_recovers_exact_receipt_and_permanent_deny() -> anyhow::Result<()> {
    let pool = pool().await?;
    let path = checkpoint()?;
    let meta = std::fs::symlink_metadata(&path)?;
    use std::os::unix::fs::PermissionsExt;
    anyhow::ensure!(
        meta.is_file() && !meta.file_type().is_symlink() && meta.permissions().mode() & 0o077 == 0,
        "private checkpoint required"
    );
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?)?;
    let c = DeletionCommand::new(
        serde_json::from_value(v["saga"].clone())?,
        serde_json::from_value(v["user"].clone())?,
        serde_json::from_value(v["actor"].clone())?,
        serde_json::from_value(v["retention"].clone())?,
    )?;
    let before: DateTime<Utc> = serde_json::from_value(v["postmaster"].clone())?;
    let after: DateTime<Utc> = sqlx::query_scalar("select pg_postmaster_start_time()")
        .fetch_one(&pool)
        .await?;
    assert_ne!(before, after, "no actual PostgreSQL restart");
    let repo = PgNotificationRepository::new(pool.clone());
    let proof = repo.apply_user_deletion_receipt(c.clone()).await?;
    assert_eq!(proof.fence_revision(), v["revision"].as_i64().unwrap());
    assert_eq!(proof.effect_xid(), v["xid"].as_str().unwrap());
    assert_eq!(
        proof.recorded_at(),
        serde_json::from_value::<DateTime<Utc>>(v["recorded"].clone())?
    );
    assert!(matches!(
        repo.create_consistent(&notification(c.user_id(), Uuid::new_v4()))
            .await?,
        NotificationCreateOutcome::Suppressed(_)
    ));
    assert_eq!(proof_count(&pool, c.saga_id()).await?, 1);
    assert_eq!(live_count(&pool, c.user_id()).await?, 0);
    pool.close().await;
    Ok(())
}
