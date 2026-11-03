//! Real-PostgreSQL acceptance for notification ordering, exact cursors, and
//! bounded retention claims. Testcontainers remains the default; an explicit
//! loopback-only local PostgreSQL fixture route is supported without silent skip.

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use notification_service::adapters::out::persistence::notification_repository::PgNotificationRepository;
use notification_service::application::dto::notification_dto::{
    ApplyUserDeletionDto, NotificationCreateDto, PurgeExpiredNotificationsDto,
};
use notification_service::application::notification_purge::NotificationPurgePort;
use std::collections::HashSet;
use std::time::Duration;
use tokio::time::{sleep, timeout};
use uuid::Uuid;

const OPERATOR: &str = "99999999-9999-9999-9999-999999999999";

#[derive(Clone, Copy)]
enum DeletedRole {
    Recipient,
    Actor,
}

fn uuid(value: u128) -> Uuid {
    Uuid::from_u128(value)
}

fn advisory_key() -> i64 {
    let bytes: [u8; 8] = Uuid::new_v4().as_bytes()[..8]
        .try_into()
        .expect("UUID prefix");
    i64::from_be_bytes(bytes)
}

fn create_request(
    recipient_id: Uuid,
    actor_id: Uuid,
    event_id: Uuid,
    payload: &str,
) -> NotificationCreateDto {
    NotificationCreateDto {
        user_id: recipient_id.to_string(),
        r#type: "POST_LIKED".into(),
        payload_json: payload.into(),
        event_id: event_id.to_string(),
        actor_id: actor_id.to_string(),
        occurred_at: Utc::now().to_rfc3339(),
    }
}

fn deletion_request(
    saga_id: Uuid,
    user_id: Uuid,
    retention_until: DateTime<Utc>,
) -> ApplyUserDeletionDto {
    deletion_request_with_actor(
        saga_id,
        user_id,
        Uuid::parse_str(OPERATOR).expect("operator UUID"),
        retention_until,
    )
}

fn deletion_request_with_actor(
    saga_id: Uuid,
    user_id: Uuid,
    actor_id: Uuid,
    retention_until: DateTime<Utc>,
) -> ApplyUserDeletionDto {
    ApplyUserDeletionDto {
        saga_id: saga_id.to_string(),
        user_id: user_id.to_string(),
        actor_id: actor_id.to_string(),
        retention_until: retention_until.to_rfc3339(),
    }
}

async fn hold_advisory_lock(
    pool: &sqlx::PgPool,
    key: i64,
) -> anyhow::Result<sqlx::pool::PoolConnection<sqlx::Postgres>> {
    let mut connection = pool.acquire().await?;
    sqlx::query("select pg_advisory_lock($1)")
        .bind(key)
        .execute(&mut *connection)
        .await?;
    Ok(connection)
}

async fn release_advisory_lock(
    connection: &mut sqlx::pool::PoolConnection<sqlx::Postgres>,
    key: i64,
) -> anyhow::Result<()> {
    let released: bool = sqlx::query_scalar("select pg_advisory_unlock($1)")
        .bind(key)
        .fetch_one(&mut **connection)
        .await?;
    anyhow::ensure!(released, "test advisory lock was not held");
    Ok(())
}

async fn wait_for_lock_waiters(pool: &sqlx::PgPool, minimum: i64) -> anyhow::Result<()> {
    timeout(Duration::from_secs(10), async {
        loop {
            let waiting: i64 = sqlx::query_scalar(
                r#"
                select count(*)
                from pg_stat_activity
                where datname = current_database()
                  and pid <> pg_backend_pid()
                  and application_name = current_setting('application_name')
                  and wait_event_type = 'Lock'
                "#,
            )
            .fetch_one(pool)
            .await?;
            if waiting >= minimum {
                return Ok::<(), anyhow::Error>(());
            }
            sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .map_err(|_| anyhow::anyhow!("timed out observing {minimum} PostgreSQL lock waiters"))??;
    Ok(())
}

async fn install_blocking_trigger(
    pool: &sqlx::PgPool,
    timing: &str,
    table: &str,
    operation: &str,
    trigger: &str,
    function: &str,
    key: i64,
) -> anyhow::Result<()> {
    let sql = format!(
        r#"
        create function {function}() returns trigger language plpgsql as $$
        begin
            perform pg_advisory_xact_lock({key});
            return new;
        end $$;
        create trigger {trigger}
        {timing} {operation} on {table}
        for each row execute function {function}();
        "#,
    );
    sqlx::raw_sql(&sql).execute(pool).await?;
    Ok(())
}

async fn assert_creation_deletion_order(
    role: DeletedRole,
    creation_first: bool,
) -> anyhow::Result<()> {
    let app = crate::support::start_postgres_app().await?;
    let recipient_id = uuid(0x100);
    let actor_id = uuid(0x200);
    let deleted_id = match role {
        DeletedRole::Recipient => recipient_id,
        DeletedRole::Actor => actor_id,
    };
    let key = advisory_key();
    let mut blocker = hold_advisory_lock(&app.pool, key).await?;
    if creation_first {
        // The migration guard must acquire the ordered subject locks before
        // this barrier pauses the insert. PostgreSQL orders same-kind triggers
        // by name, so the `zz_...` test trigger follows `notification_...`.
        install_blocking_trigger(
            &app.pool,
            "before",
            "notifications",
            "insert",
            "zz_block_notification_create",
            "zz_block_notification_create_fn",
            key,
        )
        .await?;
    } else {
        install_blocking_trigger(
            &app.pool,
            "before",
            "notification_subject_states",
            "update",
            "block_notification_suppression",
            "block_notification_suppression_fn",
            key,
        )
        .await?;
    }

    let create = app.state.create_notification.clone();
    let delete = app.state.apply_user_deletion.clone();
    let request = create_request(recipient_id, actor_id, uuid(0x300), "{\"version\":1}");
    let deletion = deletion_request(
        uuid(0x400),
        deleted_id,
        Utc::now() + ChronoDuration::days(30),
    );

    let (first, second) = if creation_first {
        let create_task = tokio::spawn(async move { create.execute(request).await });
        wait_for_lock_waiters(&app.pool, 1).await?;
        let delete_task = tokio::spawn(async move { delete.execute(deletion).await });
        wait_for_lock_waiters(&app.pool, 2).await?;
        release_advisory_lock(&mut blocker, key).await?;
        let created = create_task.await??;
        delete_task.await??;
        (Some(created), None)
    } else {
        let delete_task = tokio::spawn(async move { delete.execute(deletion).await });
        wait_for_lock_waiters(&app.pool, 1).await?;
        let create_task = tokio::spawn(async move { create.execute(request).await });
        wait_for_lock_waiters(&app.pool, 2).await?;
        release_advisory_lock(&mut blocker, key).await?;
        delete_task.await??;
        let created = create_task.await??;
        (None, Some(created))
    };

    if let Some(created) = first {
        assert!(!created.suppressed, "creation held the subject guard first");
        assert!(created.notification.is_some());
    }
    if let Some(created) = second {
        assert!(created.suppressed, "deletion held the subject guard first");
        assert!(created.notification.is_none());
    }

    let live: i64 = sqlx::query_scalar(
        "select count(*) from notifications where deleted_at is null and (user_id = $1 or actor_id = $1)",
    )
    .bind(deleted_id)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(
        live, 0,
        "deleted identities cannot retain or create live notifications"
    );
    // Release the held lease before removing this owned fixture database.
    blocker.close().await?;
    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn recipient_create_before_delete_is_serialized_and_delete_wins_final_visibility(
) -> anyhow::Result<()> {
    assert_creation_deletion_order(DeletedRole::Recipient, true).await
}

#[tokio::test]
async fn recipient_delete_before_create_is_serialized_and_creation_is_suppressed(
) -> anyhow::Result<()> {
    assert_creation_deletion_order(DeletedRole::Recipient, false).await
}

#[tokio::test]
async fn actor_create_before_delete_is_serialized_and_actor_rows_are_hidden() -> anyhow::Result<()>
{
    assert_creation_deletion_order(DeletedRole::Actor, true).await
}

#[tokio::test]
async fn actor_delete_before_create_is_serialized_and_actor_creation_is_suppressed(
) -> anyhow::Result<()> {
    assert_creation_deletion_order(DeletedRole::Actor, false).await
}

async fn legacy_save_notification(
    pool: &sqlx::PgPool,
    recipient_id: Uuid,
    actor_id: Uuid,
    event_id: Uuid,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        insert into notifications (
            id, user_id, type, payload_json, created_at, read_at,
            event_id, actor_id, occurred_at
        ) values ($1, $2, 'POST_LIKED', '{}', now(), null, $3, $4, now())
        on conflict (user_id, event_id, type) do update
        set id = notifications.id
        returning id
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(recipient_id)
    .bind(event_id)
    .bind(actor_id)
    .fetch_one(pool)
    .await
}

async fn legacy_apply_user_deletion(
    pool: &sqlx::PgPool,
    saga_id: Uuid,
    user_id: Uuid,
    actor_id: Uuid,
    retention_until: DateTime<Utc>,
) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        r#"
        insert into notification_recipient_suppressions (
            user_id, reason, suppressed_at, retention_until
        ) values ($1, 'account_deletion', now(), $2)
        on conflict (user_id) do update
        set reason = excluded.reason,
            suppressed_at = least(
                notification_recipient_suppressions.suppressed_at,
                excluded.suppressed_at
            ),
            retention_until = excluded.retention_until
        "#,
    )
    .bind(user_id)
    .bind(retention_until)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        r#"
        update notifications
        set deleted_at = coalesce(deleted_at, now()),
            deleted_by = coalesce(deleted_by, $2),
            delete_reason = coalesce(delete_reason, 'account_deletion'),
            retention_until = $3
        where user_id = $1 or actor_id = $1
        "#,
    )
    .bind(user_id)
    .bind(actor_id)
    .bind(retention_until)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        r#"
        insert into notification_audit_events (
            id, actor_id, notification_id, user_id, action, outcome,
            request_id, ip_address, user_agent, metadata_json, occurred_at
        ) values (
            $1, $2, null, $3, 'USER_DELETION_APPLIED', 'SUCCESS',
            null, null, null, $4, now()
        ) on conflict (id) do nothing
        "#,
    )
    .bind(saga_id)
    .bind(actor_id)
    .bind(user_id)
    .bind(format!(r#"{{"sagaId":"{saga_id}"}}"#))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

async fn assert_legacy_writer_deletion_order(
    role: DeletedRole,
    creation_first: bool,
) -> anyhow::Result<()> {
    let app = crate::support::start_postgres_app().await?;
    let recipient_id = uuid(0x4100);
    let actor_id = uuid(0x4200);
    let deleted_id = match role {
        DeletedRole::Recipient => recipient_id,
        DeletedRole::Actor => actor_id,
    };
    let stale_precheck: bool = sqlx::query_scalar(
        "select exists(select 1 from notification_recipient_suppressions where user_id = $1)",
    )
    .bind(recipient_id)
    .fetch_one(&app.pool)
    .await?;
    assert!(!stale_precheck, "legacy check must precede the forced race");

    let key = advisory_key();
    let mut blocker = hold_advisory_lock(&app.pool, key).await?;
    if creation_first {
        // PostgreSQL fires same-kind triggers by name. The migration guard's
        // `notification_...` trigger therefore locks both identities before this
        // `zz_...` barrier stops the legacy insert.
        install_blocking_trigger(
            &app.pool,
            "before",
            "notifications",
            "insert",
            "zz_block_legacy_notification_insert",
            "zz_block_legacy_notification_insert_fn",
            key,
        )
        .await?;
    } else {
        // The legacy mirror's `notification_...` AFTER trigger obtains the state
        // lock before this `zz_...` barrier pauses the old deletion transaction.
        install_blocking_trigger(
            &app.pool,
            "after",
            "notification_recipient_suppressions",
            "insert",
            "zz_block_legacy_suppression",
            "zz_block_legacy_suppression_fn",
            key,
        )
        .await?;
    }

    let event_id = uuid(0x4300);
    let saga_id = uuid(0x4400);
    let operator_id = Uuid::parse_str(OPERATOR)?;
    let retention_until = Utc::now() + ChronoDuration::days(30);
    if creation_first {
        let create_pool = app.pool.clone();
        let create_task = tokio::spawn(async move {
            legacy_save_notification(&create_pool, recipient_id, actor_id, event_id).await
        });
        wait_for_lock_waiters(&app.pool, 1).await?;
        let delete_pool = app.pool.clone();
        let delete_task = tokio::spawn(async move {
            legacy_apply_user_deletion(
                &delete_pool,
                saga_id,
                deleted_id,
                operator_id,
                retention_until,
            )
            .await
        });
        wait_for_lock_waiters(&app.pool, 2).await?;
        release_advisory_lock(&mut blocker, key).await?;
        create_task.await??;
        delete_task.await??;
    } else {
        let delete_pool = app.pool.clone();
        let delete_task = tokio::spawn(async move {
            legacy_apply_user_deletion(
                &delete_pool,
                saga_id,
                deleted_id,
                operator_id,
                retention_until,
            )
            .await
        });
        wait_for_lock_waiters(&app.pool, 1).await?;
        let create_pool = app.pool.clone();
        let create_task = tokio::spawn(async move {
            legacy_save_notification(&create_pool, recipient_id, actor_id, event_id).await
        });
        wait_for_lock_waiters(&app.pool, 2).await?;
        release_advisory_lock(&mut blocker, key).await?;
        delete_task.await??;
        let error = create_task
            .await?
            .expect_err("database guard must reject a stale legacy save after deletion");
        let database_error = error
            .as_database_error()
            .expect("suppression rejection is a PostgreSQL error");
        assert_eq!(database_error.code().as_deref(), Some("23514"));
        let expected_constraint = match role {
            DeletedRole::Recipient => "notifications_recipient_not_suppressed",
            DeletedRole::Actor => "notifications_actor_not_suppressed",
        };
        assert_eq!(database_error.constraint(), Some(expected_constraint));
    }

    let state: (i64, i64, bool, bool) = sqlx::query_as(
        r#"
        select
          (select count(*) from notifications
             where deleted_at is null and (user_id = $1 or actor_id = $1)),
          (select count(*) from notification_recipient_suppressions where user_id = $1),
          (select suppress_as_recipient from notification_subject_states where subject_id = $1),
          (select suppress_as_actor from notification_subject_states where subject_id = $1)
        "#,
    )
    .bind(deleted_id)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(state, (0, 1, true, true));
    let rows: i64 = sqlx::query_scalar(
        "select count(*) from notifications where user_id = $1 and event_id = $2",
    )
    .bind(recipient_id)
    .bind(event_id)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(rows, if creation_first { 1 } else { 0 });
    // Release the held lease before removing this owned fixture database.
    blocker.close().await?;
    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn legacy_recipient_save_before_legacy_delete_is_serialized_and_then_hidden(
) -> anyhow::Result<()> {
    assert_legacy_writer_deletion_order(DeletedRole::Recipient, true).await
}

#[tokio::test]
async fn legacy_recipient_delete_before_stale_save_is_rejected_at_database_boundary(
) -> anyhow::Result<()> {
    assert_legacy_writer_deletion_order(DeletedRole::Recipient, false).await
}

#[tokio::test]
async fn legacy_actor_save_before_legacy_delete_is_serialized_and_then_hidden() -> anyhow::Result<()>
{
    assert_legacy_writer_deletion_order(DeletedRole::Actor, true).await
}

#[tokio::test]
async fn legacy_actor_delete_before_stale_save_is_rejected_at_database_boundary(
) -> anyhow::Result<()> {
    assert_legacy_writer_deletion_order(DeletedRole::Actor, false).await
}

#[tokio::test]
async fn creation_and_deletion_failures_roll_back_guards_effects_receipts_and_audit(
) -> anyhow::Result<()> {
    let app = crate::support::start_postgres_app().await?;
    let recipient_id = uuid(0x510);
    let actor_id = uuid(0x511);

    sqlx::raw_sql(
        r#"
        create function reject_notification_insert() returns trigger language plpgsql as $$
        begin raise exception 'injected notification insert failure'; end $$;
        create trigger reject_notification_insert_trigger
        before insert on notifications
        for each row execute function reject_notification_insert();
        "#,
    )
    .execute(&app.pool)
    .await?;
    assert!(app
        .state
        .create_notification
        .execute(create_request(recipient_id, actor_id, uuid(0x512), "{}"))
        .await
        .is_err());
    let subjects = vec![recipient_id, actor_id];
    let guards: i64 = sqlx::query_scalar(
        "select count(*) from notification_subject_states where subject_id = any($1)",
    )
    .bind(&subjects)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(
        guards, 0,
        "guard insertion is part of the failed creation transaction"
    );
    sqlx::raw_sql(
        "drop trigger reject_notification_insert_trigger on notifications; drop function reject_notification_insert();",
    )
    .execute(&app.pool)
    .await?;

    let created = app
        .state
        .create_notification
        .execute(create_request(recipient_id, actor_id, uuid(0x513), "{}"))
        .await?
        .notification
        .expect("created notification");
    sqlx::raw_sql(
        r#"
        create function reject_deletion_audit() returns trigger language plpgsql as $$
        begin
            if new.action = 'USER_DELETION_APPLIED' then
                raise exception 'injected deletion audit failure';
            end if;
            return new;
        end $$;
        create trigger reject_deletion_audit_trigger
        before insert on notification_audit_events
        for each row execute function reject_deletion_audit();
        "#,
    )
    .execute(&app.pool)
    .await?;
    let saga_id = uuid(0x514);
    assert!(app
        .state
        .apply_user_deletion
        .execute(deletion_request(
            saga_id,
            recipient_id,
            Utc::now() + ChronoDuration::days(30),
        ))
        .await
        .is_err());
    let state: (i64, i64, i64, i64, i64) = sqlx::query_as(
        r#"
        select
          (select count(*) from notifications where id = $1 and deleted_at is null),
          (select count(*) from notification_recipient_suppressions where user_id = $2),
          (select count(*) from notification_subject_states
             where subject_id = $2 and (suppress_as_recipient or suppress_as_actor)),
          (select count(*) from notification_user_deletion_receipts where saga_id = $3),
          (select count(*) from notification_audit_events where id = $3)
        "#,
    )
    .bind(Uuid::parse_str(&created.id)?)
    .bind(recipient_id)
    .bind(saga_id)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(state, (1, 0, 0, 0, 0));
    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn legacy_suppression_writes_are_mirrored_for_rolling_binary_compatibility(
) -> anyhow::Result<()> {
    let app = crate::support::start_postgres_app().await?;
    let deleted_id = uuid(0x580);
    sqlx::query(
        r#"
        insert into notification_recipient_suppressions (
            user_id, reason, suppressed_at, retention_until
        ) values ($1, 'account_deletion', now(), now() + interval '30 days')
        "#,
    )
    .bind(deleted_id)
    .execute(&app.pool)
    .await?;

    let recipient_result = app
        .state
        .create_notification
        .execute(create_request(deleted_id, uuid(0x581), uuid(0x582), "{}"))
        .await?;
    assert!(recipient_result.suppressed);

    let actor_result = app
        .state
        .create_notification
        .execute(create_request(uuid(0x583), deleted_id, uuid(0x584), "{}"))
        .await?;
    assert!(actor_result.suppressed);

    let flags: (bool, bool) = sqlx::query_as(
        "select suppress_as_recipient, suppress_as_actor from notification_subject_states where subject_id = $1",
    )
    .bind(deleted_id)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(flags, (true, true));
    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn migration_upgrade_reconciles_preexisting_stale_rows_and_installs_raw_save_guard(
) -> anyhow::Result<()> {
    let app = crate::support::start_postgres_app().await?;
    let schema = format!("notification_upgrade_{}", Uuid::new_v4().simple());
    let quoted_schema = format!("\"{}\"", schema.replace('"', "\"\""));
    let mut tx = app.pool.begin().await?;
    sqlx::query(&format!("create schema {quoted_schema}"))
        .execute(&mut *tx)
        .await?;
    sqlx::query(&format!(
        "set local search_path to {quoted_schema}, pg_catalog"
    ))
    .execute(&mut *tx)
    .await?;
    for migration in [
        include_str!("../../migrations/0001_init_notification.sql"),
        include_str!("../../migrations/0002_governance.sql"),
        include_str!("../../migrations/0003_recipient_suppression.sql"),
    ] {
        sqlx::raw_sql(migration).execute(&mut *tx).await?;
    }

    let recipient_id = uuid(0x590);
    let actor_id = uuid(0x591);
    let other_recipient = uuid(0x592);
    sqlx::query(
        r#"
        insert into notification_recipient_suppressions (
            user_id, reason, suppressed_at, retention_until
        ) values
          ($1, 'account_deletion', '2026-01-01T00:00:00Z', '2026-02-01T00:00:00Z'),
          ($2, 'account_deletion', '2026-01-02T00:00:00Z', '2026-03-01T00:00:00Z')
        "#,
    )
    .bind(recipient_id)
    .bind(actor_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        r#"
        insert into notifications (
            id, user_id, type, payload_json, created_at, read_at,
            event_id, actor_id, occurred_at
        ) values
          ($1, $2, 'POST_LIKED', '{}', now(), null, $3, $4, now()),
          ($5, $6, 'POST_LIKED', '{}', now(), null, $7, $8, now()),
          ($9, $2, 'POST_LIKED', '{}', now(), null, $10, $8, now())
        "#,
    )
    .bind(uuid(0x593))
    .bind(recipient_id)
    .bind(uuid(0x594))
    .bind(uuid(0x595))
    .bind(uuid(0x596))
    .bind(other_recipient)
    .bind(uuid(0x597))
    .bind(actor_id)
    .bind(uuid(0x599))
    .bind(uuid(0x59a))
    .execute(&mut *tx)
    .await?;

    sqlx::raw_sql(include_str!(
        "../../migrations/0004_notification_consistency.sql"
    ))
    .execute(&mut *tx)
    .await?;

    let repaired: (i64, i64, i64) = sqlx::query_as(
        r#"
        select
          (select count(*) from notification_subject_states
             where subject_id in ($1, $2)
               and suppress_as_recipient and suppress_as_actor),
          (select count(*) from notifications
             where id in ($3, $4, $5) and deleted_at is not null
               and deleted_by is null and delete_reason = 'account_deletion'),
          (select count(*) from notifications
             where id in ($4, $5) and retention_until = '2026-03-01T00:00:00Z')
        "#,
    )
    .bind(recipient_id)
    .bind(actor_id)
    .bind(uuid(0x593))
    .bind(uuid(0x596))
    .bind(uuid(0x599))
    .fetch_one(&mut *tx)
    .await?;
    assert_eq!(repaired, (2, 3, 2));

    for (savepoint, constrained_id, recipient, actor, expected_constraint) in [
        (
            "rejected_recipient_save",
            recipient_id,
            recipient_id,
            uuid(0x598),
            "notifications_recipient_not_suppressed",
        ),
        (
            "rejected_actor_save",
            actor_id,
            other_recipient,
            actor_id,
            "notifications_actor_not_suppressed",
        ),
    ] {
        sqlx::query(&format!("savepoint {savepoint}"))
            .execute(&mut *tx)
            .await?;
        let error = sqlx::query(
            r#"
            insert into notifications (
                id, user_id, type, payload_json, created_at, read_at,
                event_id, actor_id, occurred_at
            ) values ($1, $2, 'POST_LIKED', '{}', now(), null, $3, $4, now())
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(recipient)
        .bind(constrained_id)
        .bind(actor)
        .execute(&mut *tx)
        .await
        .expect_err("migration guard must reject a suppressed legacy raw save");
        let database_error = error
            .as_database_error()
            .expect("suppression rejection is a PostgreSQL error");
        assert_eq!(database_error.code().as_deref(), Some("23514"));
        assert_eq!(database_error.constraint(), Some(expected_constraint));
        sqlx::query(&format!("rollback to savepoint {savepoint}"))
            .execute(&mut *tx)
            .await?;
    }

    tx.rollback().await?;
    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn duplicate_delivery_is_immutable_and_deletion_saga_retries_are_idempotent(
) -> anyhow::Result<()> {
    let app = crate::support::start_postgres_app().await?;
    let recipient_id = uuid(0x610);
    let actor_id = uuid(0x611);
    let event_id = uuid(0x612);
    let first = app
        .state
        .create_notification
        .execute(create_request(
            recipient_id,
            actor_id,
            event_id,
            "{\"version\":1}",
        ))
        .await?
        .notification
        .expect("first notification");
    let duplicate = app
        .state
        .create_notification
        .execute(create_request(
            recipient_id,
            actor_id,
            event_id,
            "{\"version\":2}",
        ))
        .await?
        .notification
        .expect("existing notification");
    assert_eq!(duplicate.id, first.id);
    assert_eq!(duplicate.payload_json, "{\"version\":1}");
    let rows: i64 = sqlx::query_scalar(
        "select count(*) from notifications where user_id = $1 and event_id = $2 and type = 'POST_LIKED'",
    )
    .bind(recipient_id)
    .bind(event_id)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(rows, 1);

    let alternate_user = uuid(0x614);
    let alternate_actor = uuid(0x615);
    let alternate = app
        .state
        .create_notification
        .execute(create_request(
            alternate_user,
            alternate_actor,
            uuid(0x616),
            "{\"alternate\":true}",
        ))
        .await?
        .notification
        .expect("alternate notification");

    let saga_id = uuid(0x613);
    let coordinator_id = Uuid::parse_str(OPERATOR)?;
    let retention = Utc::now() + ChronoDuration::days(30);
    let request = deletion_request_with_actor(saga_id, recipient_id, coordinator_id, retention);
    app.state
        .apply_user_deletion
        .execute(request.clone())
        .await?;
    app.state.apply_user_deletion.execute(request).await?;
    let counts: (i64, i64, i64) = sqlx::query_as(
        r#"
        select
          (select count(*) from notification_recipient_suppressions where user_id = $1),
          (select count(*) from notification_user_deletion_receipts where saga_id = $2),
          (select count(*) from notification_audit_events where id = $2)
        "#,
    )
    .bind(recipient_id)
    .bind(saga_id)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(counts, (1, 1, 1));

    let conflicts = [
        (
            "retention",
            deletion_request_with_actor(
                saga_id,
                recipient_id,
                coordinator_id,
                retention + ChronoDuration::seconds(1),
            ),
        ),
        (
            "subject",
            deletion_request_with_actor(saga_id, alternate_user, coordinator_id, retention),
        ),
        (
            "actor",
            deletion_request_with_actor(saga_id, recipient_id, uuid(0x617), retention),
        ),
    ];
    for (dimension, conflict) in conflicts {
        let error = match app.state.apply_user_deletion.execute(conflict).await {
            Ok(_) => panic!("{dimension} saga conflict must fail"),
            Err(error) => error,
        };
        assert!(
            error.to_string().contains("reused with different inputs"),
            "unexpected {dimension} conflict error: {error:#}"
        );
    }

    let post_conflict: (i64, i64, i64, i64, bool, bool, i64) = sqlx::query_as(
        r#"
        select
          (select count(*) from notification_recipient_suppressions where user_id = $1),
          (select count(*) from notification_user_deletion_receipts where saga_id = $2),
          (select count(*) from notification_audit_events where id = $2),
          (select count(*) from notification_recipient_suppressions where user_id = $3),
          (select suppress_as_recipient from notification_subject_states where subject_id = $3),
          (select suppress_as_actor from notification_subject_states where subject_id = $3),
          (select count(*) from notifications where id = $4 and deleted_at is null)
        "#,
    )
    .bind(recipient_id)
    .bind(saga_id)
    .bind(alternate_user)
    .bind(Uuid::parse_str(&alternate.id)?)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(post_conflict, (1, 1, 1, 0, false, false, 1));
    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn account_deletion_tombstone_survives_notification_purge_and_suppresses_late_events(
) -> anyhow::Result<()> {
    let app = crate::support::start_postgres_app().await?;
    let deleted_user = uuid(0x1710);
    let original_actor = uuid(0x1711);
    let original_event = uuid(0x1712);
    let original = app
        .state
        .create_notification
        .execute(create_request(
            deleted_user,
            original_actor,
            original_event,
            "{\"body\":\"content removed after retention\"}",
        ))
        .await?
        .notification
        .expect("original notification");

    let saga_id = uuid(0x1713);
    let retention_until = Utc::now() - ChronoDuration::days(1);
    app.state
        .apply_user_deletion
        .execute(deletion_request_with_actor(
            saga_id,
            deleted_user,
            Uuid::parse_str(OPERATOR)?,
            retention_until,
        ))
        .await?;

    let purge = app
        .state
        .purge_expired_notifications
        .execute(PurgeExpiredNotificationsDto {
            actor_id: OPERATOR.into(),
            request_id: Some("account-deletion-retention-purge".into()),
            ip_address: None,
            user_agent: Some("notification-consistency-test".into()),
            dry_run: false,
        })
        .await?;
    assert_eq!(purge.purged_count, 1);

    let original_rows: i64 = sqlx::query_scalar("select count(*) from notifications where id = $1")
        .bind(Uuid::parse_str(&original.id)?)
        .fetch_one(&app.pool)
        .await?;
    assert_eq!(original_rows, 0, "expired notification content is purged");

    let tombstone: (bool, bool, Option<String>) = sqlx::query_as(
        r#"
        select suppress_as_recipient, suppress_as_actor, reason
        from notification_subject_states
        where subject_id = $1
        "#,
    )
    .bind(deleted_user)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(tombstone, (true, true, Some("account_deletion".into())));

    let late_recipient_event = uuid(0x1714);
    let late_actor_event = uuid(0x1715);
    for request in [
        create_request(
            deleted_user,
            uuid(0x1716),
            late_recipient_event,
            "{\"late\":\"recipient\"}",
        ),
        create_request(
            uuid(0x1717),
            deleted_user,
            late_actor_event,
            "{\"late\":\"actor\"}",
        ),
    ] {
        let outcome = app.state.create_notification.execute(request).await?;
        assert!(
            outcome.suppressed,
            "late event must honor deletion tombstone"
        );
        assert!(outcome.notification.is_none());
    }

    let late_rows: i64 =
        sqlx::query_scalar("select count(*) from notifications where event_id = any($1::uuid[])")
            .bind(vec![late_recipient_event, late_actor_event])
            .fetch_one(&app.pool)
            .await?;
    assert_eq!(
        late_rows, 0,
        "suppressed late events leave no notification data"
    );

    app.shutdown().await?;
    Ok(())
}

async fn insert_notification(
    pool: &sqlx::PgPool,
    id: Uuid,
    recipient_id: Uuid,
    actor_id: Uuid,
    created_at: DateTime<Utc>,
    deleted: bool,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        insert into notifications (
            id, user_id, type, payload_json, created_at, read_at,
            event_id, actor_id, occurred_at, deleted_at, retention_until
        ) values ($1, $2, 'SYSTEM', $3, $4, null, $5, $6, $4,
                  case when $7 then $4 else null end,
                  case when $7 then $4 + interval '30 days' else null end)
        "#,
    )
    .bind(id)
    .bind(recipient_id)
    .bind(format!("{{\"id\":\"{id}\"}}"))
    .bind(created_at)
    .bind(Uuid::new_v4())
    .bind(actor_id)
    .bind(deleted)
    .execute(pool)
    .await?;
    Ok(())
}

async fn collect_pages(
    app: &crate::support::TestApp,
    recipient_id: Uuid,
    limit: i32,
) -> anyhow::Result<Vec<Uuid>> {
    let mut cursor = String::new();
    let mut values = Vec::new();
    loop {
        let page = app
            .state
            .list_notifications
            .execute(&recipient_id.to_string(), &cursor, limit)
            .await?;
        values.extend(
            page.items
                .iter()
                .map(|item| Uuid::parse_str(&item.id))
                .collect::<Result<Vec<_>, _>>()?,
        );
        match page.next_cursor {
            Some(next) => cursor = next,
            None => return Ok(values),
        }
        anyhow::ensure!(values.len() < 10_000, "cursor traversal did not terminate");
    }
}

#[tokio::test]
async fn v2_pagination_is_lossless_recipient_bound_and_matches_reference_order(
) -> anyhow::Result<()> {
    let app = crate::support::start_postgres_app().await?;
    let recipient_id = uuid(0x710);
    let other_recipient = uuid(0x711);
    let actor_id = uuid(0x712);
    let base = DateTime::<Utc>::from_timestamp_micros(1_800_000_000_123_000).unwrap();
    for index in 0..205_u128 {
        let created_at = base + ChronoDuration::microseconds((index % 23) as i64);
        insert_notification(
            &app.pool,
            uuid(0x10_000 + index),
            recipient_id,
            actor_id,
            created_at,
            index == 17,
        )
        .await?;
    }
    insert_notification(
        &app.pool,
        uuid(0x20_000),
        other_recipient,
        actor_id,
        base + ChronoDuration::microseconds(22),
        false,
    )
    .await?;

    let expected: Vec<Uuid> = sqlx::query_scalar(
        "select id from notifications where user_id = $1 and deleted_at is null order by created_at desc, id desc",
    )
    .bind(recipient_id)
    .fetch_all(&app.pool)
    .await?;
    for limit in [1, 2, 100] {
        assert_eq!(collect_pages(&app, recipient_id, limit).await?, expected);
    }

    let first = app
        .state
        .list_notifications
        .execute(&recipient_id.to_string(), "", 1)
        .await?;
    let token = first.next_cursor.expect("v2 continuation");
    assert!(token.starts_with(&format!("v2:{recipient_id}:")));
    let error = app
        .state
        .list_notifications
        .execute(&other_recipient.to_string(), &token, 1)
        .await
        .expect_err("cursor cannot cross recipients");
    assert_eq!(error.to_string(), "Cursor recipient mismatch");
    for malformed in ["bad", "v2:bad:1:bad", "1:not-a-uuid"] {
        assert!(app
            .state
            .list_notifications
            .execute(&recipient_id.to_string(), malformed, 1)
            .await
            .is_err());
    }
    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn legacy_cursor_resolves_exact_anchor_precision_and_has_explicit_expiry(
) -> anyhow::Result<()> {
    let app = crate::support::start_postgres_app().await?;
    let recipient_id = uuid(0x810);
    let other_recipient = uuid(0x811);
    let actor_id = uuid(0x812);
    let base = DateTime::<Utc>::from_timestamp_micros(1_800_000_000_123_000).unwrap();
    let anchor = uuid(0x813);
    let unseen = uuid(0x814);
    insert_notification(
        &app.pool,
        anchor,
        recipient_id,
        actor_id,
        base + ChronoDuration::microseconds(900),
        false,
    )
    .await?;
    insert_notification(
        &app.pool,
        unseen,
        recipient_id,
        actor_id,
        base + ChronoDuration::microseconds(800),
        false,
    )
    .await?;
    let token = format!(
        "{}:{anchor}",
        (base + ChronoDuration::microseconds(900)).timestamp_millis()
    );
    let page = app
        .state
        .list_notifications
        .execute(&recipient_id.to_string(), &token, 10)
        .await?;
    assert_eq!(
        page.items
            .iter()
            .map(|item| item.id.clone())
            .collect::<Vec<_>>(),
        vec![unseen.to_string()]
    );
    let mismatched_token = format!(
        "{}:{anchor}",
        (base + ChronoDuration::microseconds(900)).timestamp_millis() + 1
    );
    let mismatch = app
        .state
        .list_notifications
        .execute(&recipient_id.to_string(), &mismatched_token, 10)
        .await
        .expect_err("legacy cursor timestamp must match the stored anchor");
    assert_eq!(
        mismatch.to_string(),
        "Legacy cursor anchor does not match its timestamp"
    );

    sqlx::query("update notifications set deleted_at = now() where id = $1")
        .bind(anchor)
        .execute(&app.pool)
        .await?;
    let page = app
        .state
        .list_notifications
        .execute(&recipient_id.to_string(), &token, 10)
        .await?;
    assert_eq!(
        page.items.len(),
        1,
        "soft-deleted legacy anchors remain resolvable"
    );
    let cross_recipient = app
        .state
        .list_notifications
        .execute(&other_recipient.to_string(), &token, 10)
        .await
        .expect_err("legacy cursor cannot cross recipients");
    assert_eq!(
        cross_recipient.to_string(),
        "Legacy cursor anchor is unavailable"
    );

    sqlx::query("delete from notifications where id = $1")
        .bind(anchor)
        .execute(&app.pool)
        .await?;
    let error = app
        .state
        .list_notifications
        .execute(&recipient_id.to_string(), &token, 10)
        .await
        .expect_err("purged legacy anchor has explicit expiry");
    assert_eq!(error.to_string(), "Legacy cursor anchor is unavailable");
    app.shutdown().await?;
    Ok(())
}

async fn seed_expired(pool: &sqlx::PgPool, ids: &[Uuid]) -> anyhow::Result<()> {
    let recipient_id = uuid(0x900);
    let actor_id = uuid(0x901);
    let base = DateTime::<Utc>::from_timestamp_micros(1_700_000_000_000_000).unwrap();
    for (offset, id) in ids.iter().enumerate() {
        let at = base + ChronoDuration::microseconds(offset as i64);
        insert_notification(pool, *id, recipient_id, actor_id, at, true).await?;
        sqlx::query("update notifications set retention_until = $2 where id = $1")
            .bind(id)
            .bind(at)
            .execute(pool)
            .await?;
    }
    Ok(())
}

#[tokio::test]
async fn purge_claims_are_bounded_skip_locked_and_audit_is_atomic_with_delete() -> anyhow::Result<()>
{
    let app = crate::support::start_postgres_app().await?;
    let ids: Vec<Uuid> = (0..5).map(|index| uuid(0xa00 + index)).collect();
    seed_expired(&app.pool, &ids).await?;
    let key = advisory_key();
    let mut blocker = hold_advisory_lock(&app.pool, key).await?;
    sqlx::raw_sql(&format!(
        r#"
        create function block_first_purge() returns trigger language plpgsql as $$
        begin
            if new.request_id = 'purge-a' then
                perform pg_advisory_xact_lock({key});
            end if;
            return new;
        end $$;
        create trigger block_first_purge_trigger
        before insert on notification_audit_events
        for each row execute function block_first_purge();
        "#,
    ))
    .execute(&app.pool)
    .await?;

    let first_repository = PgNotificationRepository::new(app.pool.clone());
    let second_repository = first_repository.clone();
    let first = tokio::spawn(async move {
        first_repository
            .purge_expired_batch(
                Utc::now(),
                uuid(0x999),
                Some("purge-a".into()),
                None,
                None,
                false,
                2,
            )
            .await
    });
    wait_for_lock_waiters(&app.pool, 1).await?;
    let second = tokio::spawn(async move {
        second_repository
            .purge_expired_batch(
                Utc::now(),
                uuid(0x999),
                Some("purge-b".into()),
                None,
                None,
                false,
                2,
            )
            .await
    });
    let second_join = timeout(Duration::from_secs(10), second)
        .await
        .map_err(|_| anyhow::anyhow!("second purge did not skip locked rows"))?;
    let second_ids = second_join??;
    assert_eq!(second_ids.len(), 2);
    release_advisory_lock(&mut blocker, key).await?;
    let first_ids = first.await??;
    assert_eq!(first_ids.len(), 2);
    assert!(first_ids.iter().all(|id| !second_ids.contains(id)));
    let claimed: HashSet<Uuid> = first_ids.iter().chain(second_ids.iter()).copied().collect();
    assert_eq!(claimed, ids[..4].iter().copied().collect::<HashSet<_>>());
    let remaining: i64 = sqlx::query_scalar(
        "select count(*) from notifications where deleted_at is not null and retention_until <= now()",
    )
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(remaining, 1);
    let audited: i64 = sqlx::query_scalar(
        "select count(*) from notification_audit_events where request_id in ('purge-a','purge-b') and action = 'NOTIFICATION_PURGED'",
    )
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(audited, 4);
    // Release the held lease before removing this owned fixture database.
    blocker.close().await?;
    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn purge_audit_failure_rolls_back_claimed_deletion_for_retry() -> anyhow::Result<()> {
    let app = crate::support::start_postgres_app().await?;
    let id = uuid(0xb00);
    seed_expired(&app.pool, &[id]).await?;
    sqlx::raw_sql(
        r#"
        create function reject_purge_audit() returns trigger language plpgsql as $$
        begin
            if new.request_id = 'purge-fail' then
                raise exception 'injected purge audit failure';
            end if;
            return new;
        end $$;
        create trigger reject_purge_audit_trigger
        before insert on notification_audit_events
        for each row execute function reject_purge_audit();
        "#,
    )
    .execute(&app.pool)
    .await?;
    let repository = PgNotificationRepository::new(app.pool.clone());
    assert!(repository
        .purge_expired_batch(
            Utc::now(),
            uuid(0x999),
            Some("purge-fail".into()),
            None,
            None,
            false,
            1,
        )
        .await
        .is_err());
    let row: i64 = sqlx::query_scalar("select count(*) from notifications where id = $1")
        .bind(id)
        .fetch_one(&app.pool)
        .await?;
    let audit: i64 = sqlx::query_scalar(
        "select count(*) from notification_audit_events where request_id = 'purge-fail'",
    )
    .fetch_one(&app.pool)
    .await?;
    assert_eq!((row, audit), (1, 0));

    sqlx::raw_sql(
        "drop trigger reject_purge_audit_trigger on notification_audit_events; \
         drop function reject_purge_audit();",
    )
    .execute(&app.pool)
    .await?;
    let retried = repository
        .purge_expired_batch(
            Utc::now(),
            uuid(0x999),
            Some("purge-retry".into()),
            None,
            None,
            false,
            1,
        )
        .await?;
    assert_eq!(retried, vec![id]);
    let final_state: (i64, i64, i64) = sqlx::query_as(
        r#"
        select
          (select count(*) from notifications where id = $1),
          (select count(*) from notification_audit_events where request_id = 'purge-fail'),
          (select count(*) from notification_audit_events
             where request_id = 'purge-retry' and notification_id = $1
               and action = 'NOTIFICATION_PURGED' and outcome = 'SUCCESS')
        "#,
    )
    .bind(id)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(final_state, (0, 0, 1));
    app.shutdown().await?;
    Ok(())
}
