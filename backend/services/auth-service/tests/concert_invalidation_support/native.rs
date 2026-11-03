use super::*;
use auth_service::adapters::out::cache::session_store::PgRefreshTokenStore;
use auth_service::adapters::out::persistence::user_deletion_saga_repository::PgUserDeletionSagaRepository;
use auth_service::adapters::out::persistence::user_repository::{
    PgAuthAuditRepository, PgUserRepository,
};
use auth_service::adapters::out::runtime::SystemAuthRuntime;
use auth_service::application::auth_transactions::AuthTransactionsPort;
use auth_service::application::ports::{RefreshTokenStorePort, UserRepositoryPort};
use auth_service::domain::session::RefreshTokenRecord;
use auth_service::domain::user::{User, UserStatus};
use chrono::{Duration, Utc};
use sqlx::{PgPool, Row};
use std::sync::Arc;

async fn pool() -> anyhow::Result<PgPool> {
    let p = PgPool::connect(&std::env::var("MCR_INVALIDATION_DATABASE_URL")?).await?;
    let identity: String =
        sqlx::query_scalar("select system_identifier::text from pg_control_system()")
            .fetch_one(&p)
            .await?;
    anyhow::ensure!(
        identity == std::env::var("MCR_INVALIDATION_SYSTEM_IDENTIFIER")?,
        "foreign cluster"
    );
    let row=sqlx::query("select current_database() db,current_setting('server_version_num')::int version,current_setting('listen_addresses') listeners").fetch_one(&p).await?;
    anyhow::ensure!(
        row.get::<String, _>("db") == "mcr_auth_invalidation"
            && row.get::<i32, _>("version") / 10000 == 16
            && row.get::<String, _>("listeners").is_empty(),
        "unsafe fixture"
    );
    Ok(p)
}
fn journal(p: &PgPool) -> PgInvalidationJournal {
    PgInvalidationJournal::new(p.clone(), std::time::Duration::from_secs(2)).unwrap()
}
fn users(p: &PgPool) -> PgUserRepository {
    PgUserRepository::new(p.clone(), Arc::new(SystemAuthRuntime))
}
fn sessions(p: &PgPool) -> PgRefreshTokenStore {
    PgRefreshTokenStore::new(p.clone(), Arc::new(SystemAuthRuntime))
}
async fn user(p: &PgPool) -> anyhow::Result<User> {
    let u = User {
        id: Uuid::new_v4(),
        primary_email: format!("{}@example.invalid", Uuid::new_v4()),
        password_hash: "synthetic-original-secret".into(),
        status: UserStatus::Active,
        last_login_at: None,
    };
    users(p).save(&(&u).into()).await?;
    Ok(u)
}
async fn session(p: &PgPool, u: Uuid, f: Uuid) -> anyhow::Result<RefreshTokenRecord> {
    let s = RefreshTokenRecord {
        user_id: u,
        family_id: f,
        jti: Uuid::new_v4().to_string(),
        token_hash: "synthetic-refresh-secret".into(),
        expires_at: Utc::now() + Duration::hours(1),
    };
    sessions(p).store(&(&s).into()).await?;
    Ok(s)
}
async fn migrate(p: &PgPool, legacy: bool) -> anyhow::Result<()> {
    let mut m = sqlx::migrate::Migrator::new(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("migrations")
            .as_path(),
    )
    .await?;
    if legacy {
        m.migrations = std::borrow::Cow::Owned(
            m.migrations
                .into_owned()
                .into_iter()
                .filter(|m| m.version < 16)
                .collect(),
        );
    }
    let mut c = p.acquire().await?.detach();
    m.run(&mut c).await?;
    Ok(())
}
async fn hints(p: &PgPool, after: ReplayCursor) -> anyhow::Result<Vec<InvalidationSignal>> {
    let b = journal(p).replay(after, 256).await?;
    assert!(!b.has_more);
    Ok(b.signals)
}

#[tokio::test]
#[ignore = "requires auth_invalidation_smoke.py fresh owned PG16"]
async fn migration_and_real_mutators_capture_atomically() -> anyhow::Result<()> {
    let p = pool().await?;
    migrate(&p, true).await?;
    assert!(
        sqlx::query_scalar::<_, bool>("select success from _sqlx_migrations where version=15")
            .fetch_one(&p)
            .await?
    );
    let u = user(&p).await?;
    let family = Uuid::new_v4();
    let s = session(&p, u.id, family).await?;
    // Force a late 0016 DDL collision; earlier DDL/head/receipt must roll back.
    sqlx::raw_sql("create function public.auth_capture_concert_user_invalidation() returns trigger language plpgsql as $$begin return null; end;$$;").execute(&p).await?;
    assert!(migrate(&p, false).await.is_err());
    assert!(
        sqlx::query_scalar::<_, bool>(
            "select to_regclass('public.auth_concert_invalidation_head') is null"
        )
        .fetch_one(&p)
        .await?
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from _sqlx_migrations where version=16")
            .fetch_one(&p)
            .await?,
        0
    );
    sqlx::query("drop function public.auth_capture_concert_user_invalidation()")
        .execute(&p)
        .await?;
    migrate(&p, false).await?;
    migrate(&p, false).await?;
    let j = journal(&p);
    let initial = j.checkpoint().await?;
    assert_eq!(initial.after, 0);
    let empty = j.replay(initial, 1).await?;
    assert!(empty.signals.is_empty());
    assert_eq!(empty.next, initial);
    assert!(!empty.has_more);
    // Actual logout owning store, not a journal insert fixture.
    sessions(&p).revoke(&s.jti).await?;
    let out = hints(&p, initial).await?;
    assert!(out
        .iter()
        .any(|h| h.affects(u.id, family) && h.family_id == Some(family)));
    assert!(
        !sessions(&p)
            .session_is_live(u.id, family, Utc::now())
            .await?
    );
    let before = j.checkpoint().await?;
    sessions(&p).revoke(&s.jti).await?;
    assert_eq!(j.checkpoint().await?, before);
    let u2 = user(&p).await?;
    let f2 = Uuid::new_v4();
    session(&p, u2.id, f2).await?;
    let before = j.checkpoint().await?;
    sessions(&p).revoke_family(f2).await?;
    assert!(hints(&p, before)
        .await?
        .iter()
        .any(|h| h.affects(u2.id, f2) && !h.affects(u.id, f2)));
    // User-wide revocation emits exactly bound hints for both actual families.
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    session(&p, u.id, a).await?;
    session(&p, u.id, b).await?;
    let before = j.checkpoint().await?;
    sessions(&p).revoke_all_for_user(u.id).await?;
    let out = hints(&p, before).await?;
    assert!(out.iter().any(|h| h.family_id == Some(a)));
    assert!(out.iter().any(|h| h.family_id == Some(b)));
    assert!(out.iter().all(|h| h.user_id == u.id));
    // Journal insert failure must roll back the real session store and head.
    let f = Uuid::new_v4();
    let active = session(&p, u.id, f).await?;
    let pending_delete = user(&p).await?;
    let delete_family = Uuid::new_v4();
    session(&p, pending_delete.id, delete_family).await?;
    sqlx::query("insert into password_reset_tokens(id,user_id,token_hash,expires_at) values($1,$2,'synthetic-failing-reset',now()+interval '1 hour')").bind(Uuid::new_v4()).bind(u2.id).execute(&p).await?;
    let before = j.checkpoint().await?;
    sqlx::query("alter table auth_concert_invalidation_journal add constraint synthetic_journal_failure check(false) not valid").execute(&p).await?;
    assert!(sessions(&p).revoke(&active.jti).await.is_err());
    assert_eq!(j.checkpoint().await?, before);
    assert!(sessions(&p).session_is_live(u.id, f, Utc::now()).await?);
    assert!(users(&p)
        .update_password_hash(u.id, "synthetic-new-secret")
        .await
        .is_err());
    assert_eq!(j.checkpoint().await?, before);
    assert_eq!(
        sqlx::query_scalar::<_, String>("select password_hash from users where id=$1")
            .bind(u.id)
            .fetch_one(&p)
            .await?,
        "synthetic-original-secret"
    );
    assert!(users(&p)
        .reset_password(
            "synthetic-failing-reset",
            "synthetic-failing-change",
            Utc::now()
        )
        .await
        .is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from password_reset_tokens where token_hash='synthetic-failing-reset'"
        )
        .fetch_one(&p)
        .await?,
        1
    );
    assert!(
        sqlx::query("update users set status='SUSPENDED' where id=$1")
            .bind(u.id)
            .execute(&p)
            .await
            .is_err()
    );
    let deletion = auth_service::application::use_cases::delete_user::DeleteUserUseCase::new(
        Arc::new(users(&p)),
        Arc::new(PgAuthAuditRepository::new(p.clone())),
        PgUserDeletionSagaRepository::new(p.clone()),
        Arc::new(SystemAuthRuntime),
    );
    assert!(deletion
        .execute(auth_service::application::dto::auth_dto::DeleteUserDto {
            user_id: pending_delete.id.to_string(),
            actor_id: pending_delete.id.to_string(),
            request_id: None,
            ip_address: None,
            user_agent: None
        })
        .await
        .is_err());
    assert_eq!(
        sqlx::query_scalar::<_, String>("select status from users where id=$1")
            .bind(pending_delete.id)
            .fetch_one(&p)
            .await?,
        "ACTIVE"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from user_deletion_sagas where user_id=$1")
            .bind(pending_delete.id)
            .fetch_one(&p)
            .await?,
        0
    );
    assert!(
        sessions(&p)
            .session_is_live(pending_delete.id, delete_family, Utc::now())
            .await?
    );
    assert_eq!(j.checkpoint().await?, before);
    sqlx::query(
        "alter table auth_concert_invalidation_journal drop constraint synthetic_journal_failure",
    )
    .execute(&p)
    .await?;
    // Real credential repository + inherited trigger effects share the record.
    let before = j.checkpoint().await?;
    users(&p)
        .update_password_hash(u.id, "synthetic-new-secret")
        .await?;
    let out = hints(&p, before).await?;
    assert!(out
        .iter()
        .any(|h| h.kind == InvalidationKind::CredentialsChanged
            && h.user_id == u.id
            && h.family_id.is_none()));
    assert!(!sessions(&p).session_is_live(u.id, f, Utc::now()).await?);
    // Actual atomic reset-password path (including token consumption/audit).
    let reset = user(&p).await?;
    let rf = Uuid::new_v4();
    session(&p, reset.id, rf).await?;
    sqlx::query("insert into password_reset_tokens(id,user_id,token_hash,expires_at) values($1,$2,'synthetic-reset-secret',now()+interval '1 hour')").bind(Uuid::new_v4()).bind(reset.id).execute(&p).await?;
    let before = j.checkpoint().await?;
    assert_eq!(
        users(&p)
            .reset_password(
                "synthetic-reset-secret",
                "synthetic-reset-new-secret",
                Utc::now()
            )
            .await?,
        Some(reset.id)
    );
    assert!(hints(&p, before)
        .await?
        .iter()
        .any(|h| h.kind == InvalidationKind::CredentialsChanged && h.user_id == reset.id));
    assert!(
        !sessions(&p)
            .session_is_live(reset.id, rf, Utc::now())
            .await?
    );
    // Suspension has no typed Auth-domain mutator today. Exercise its real
    // owning users statement; the live authority check rejects non-ACTIVE.
    let suspended = user(&p).await?;
    session(&p, suspended.id, Uuid::new_v4()).await?;
    let before = j.checkpoint().await?;
    sqlx::query("update users set status='SUSPENDED' where id=$1")
        .bind(suspended.id)
        .execute(&p)
        .await?;
    assert!(hints(&p, before)
        .await?
        .iter()
        .any(|h| h.user_id == suspended.id
            && h.family_id.is_none()
            && h.kind == InvalidationKind::UserAuthorityChanged));
    // Canonical delete use case directly mutates user/refresh SQL; capture both.
    let deleted = user(&p).await?;
    let df = Uuid::new_v4();
    session(&p, deleted.id, df).await?;
    let before = j.checkpoint().await?;
    let delete = auth_service::application::use_cases::delete_user::DeleteUserUseCase::new(
        Arc::new(users(&p)),
        Arc::new(PgAuthAuditRepository::new(p.clone())),
        PgUserDeletionSagaRepository::new(p.clone()),
        Arc::new(SystemAuthRuntime),
    );
    let response = delete
        .execute(auth_service::application::dto::auth_dto::DeleteUserDto {
            user_id: deleted.id.to_string(),
            actor_id: deleted.id.to_string(),
            request_id: None,
            ip_address: None,
            user_agent: None,
        })
        .await?;
    assert!(response.deleted);
    assert_eq!(response.saga.steps.len(), 8);
    let out = hints(&p, before).await?;
    assert!(out
        .iter()
        .any(|h| h.user_id == deleted.id && h.family_id.is_none()));
    assert!(out.iter().any(|h| h.family_id == Some(df)));
    // Secret allowlist and immutable owner epoch/records.
    let serialized = serde_json::to_string(&out)?;
    for secret in [
        "synthetic-",
        "password",
        "token_hash",
        "jti",
        "email",
        "roles",
    ] {
        assert!(!serialized.contains(secret));
    }
    for sql in [
        "update auth_concert_invalidation_head set owner_epoch=gen_random_uuid()",
        "delete from auth_concert_invalidation_head",
        "truncate auth_concert_invalidation_head",
        "delete from auth_concert_invalidation_journal",
        "update auth_concert_invalidation_journal set signal_id=gen_random_uuid()",
        "truncate auth_concert_invalidation_journal",
    ] {
        assert!(sqlx::query(sql).execute(&p).await.is_err());
    }
    p.close().await;
    Ok(())
}

#[tokio::test]
#[ignore = "requires owned PG16 after migration/mutator phase"]
async fn commit_order_rollback_binding_and_bounded_replay() -> anyhow::Result<()> {
    let p = pool().await?;
    let j = journal(&p);
    let a = user(&p).await?;
    let b = user(&p).await?;
    let sa = session(&p, a.id, Uuid::new_v4()).await?;
    let sb = session(&p, b.id, Uuid::new_v4()).await?;
    let before = j.checkpoint().await?;
    let mut first = p.begin().await?;
    sqlx::query("update refresh_tokens set revoked_at=now() where jti=$1")
        .bind(&sa.jti)
        .execute(&mut *first)
        .await?;
    // The first transaction has allocated a cursor but has not committed.
    // A reader sees the old committed high-water, never its uncommitted slot.
    assert_eq!(j.checkpoint().await?, before);
    let waiter = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .after_connect(|c, _| {
            Box::pin(async move {
                sqlx::query("set application_name='mcr_invalidation_waiter'")
                    .execute(c)
                    .await?;
                Ok(())
            })
        })
        .connect(&std::env::var("MCR_INVALIDATION_DATABASE_URL")?)
        .await?;
    let waiter_store = sessions(&waiter);
    let jti = sb.jti.clone();
    let second = tokio::spawn(async move { waiter_store.revoke(&jti).await });
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(1);
    loop {
        let waiting:bool=sqlx::query_scalar("select exists(select 1 from pg_stat_activity where application_name='mcr_invalidation_waiter' and wait_event_type='Lock')").fetch_one(&p).await?;
        if waiting {
            break;
        }
        anyhow::ensure!(
            tokio::time::Instant::now() < deadline,
            "second owning mutation did not wait on transactional serialization"
        );
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    assert!(!second.is_finished());
    assert_eq!(j.checkpoint().await?, before);
    first.commit().await?;
    second.await??;
    let batch = j.replay(before, 256).await?;
    assert_eq!(batch.signals.len(), 2);
    assert_eq!(batch.signals[0].user_id, a.id);
    assert_eq!(batch.signals[1].user_id, b.id);
    assert_eq!(batch.signals[0].cursor.after, before.after + 1);
    assert_eq!(batch.next.after, before.after + 2);
    waiter.close().await;
    // An aborted transaction returns its cursor allocation and user mutation.
    let before = j.checkpoint().await?;
    let mut tx = p.begin().await?;
    sqlx::query("update users set status='SUSPENDED' where id=$1")
        .bind(a.id)
        .execute(&mut *tx)
        .await?;
    tx.rollback().await?;
    assert_eq!(j.checkpoint().await?, before);
    assert_eq!(
        sqlx::query_scalar::<_, String>("select status from users where id=$1")
            .bind(a.id)
            .fetch_one(&p)
            .await?,
        "ACTIVE"
    );
    // A failed real rotation must roll back predecessor hint and revoke, even
    // though its trigger ran before replacement-insert uniqueness failed.
    let f = Uuid::new_v4();
    let old = session(&p, a.id, f).await?;
    let collision = session(&p, a.id, f).await?;
    let before = j.checkpoint().await?;
    assert!(sessions(&p)
        .rotate(&old.jti, &old.token_hash, &(&collision).into())
        .await
        .is_err());
    assert_eq!(j.checkpoint().await?, before);
    assert!(sessions(&p).find(&old.jti).await?.is_some());
    // Real rotation remains usable: its hint is not an instruction to revoke.
    let next = RefreshTokenRecord {
        jti: Uuid::new_v4().to_string(),
        ..old.clone()
    };
    sessions(&p)
        .rotate(&old.jti, &old.token_hash, &(&next).into())
        .await?;
    assert!(sessions(&p).session_is_live(a.id, f, Utc::now()).await?);
    assert!(hints(&p, before)
        .await?
        .iter()
        .all(|h| h.user_id == a.id && h.family_id == Some(f)));
    // Same family UUID on different owning rows never changes user binding.
    let shared = Uuid::new_v4();
    session(&p, a.id, shared).await?;
    session(&p, b.id, shared).await?;
    let before = j.checkpoint().await?;
    sessions(&p).revoke_family(shared).await?;
    let out = hints(&p, before).await?;
    assert!(out
        .iter()
        .any(|h| h.user_id == a.id && h.family_id == Some(shared)));
    assert!(out
        .iter()
        .any(|h| h.user_id == b.id && h.family_id == Some(shared)));
    assert!(!out[0].affects(Uuid::new_v4(), shared));
    // Retained journal survives source hard deletion without a user FK.
    let removed = user(&p).await?;
    let before = j.checkpoint().await?;
    sqlx::query("delete from users where id=$1")
        .bind(removed.id)
        .execute(&p)
        .await?;
    assert!(hints(&p, before)
        .await?
        .iter()
        .any(|h| h.user_id == removed.id && h.kind == InvalidationKind::UserRemoved));
    // 300 separate safe signals force bounded pagination; no naked MAX cursor
    // may advance the first page past its own last delivered record.
    sqlx::query("insert into refresh_tokens(jti,token_hash,user_id,family_id,expires_at) select gen_random_uuid()::text,'synthetic-bulk-secret',$1,gen_random_uuid(),now()+interval '1 hour' from generate_series(1,300)").bind(a.id).execute(&p).await?;
    let head = j.checkpoint().await?;
    let start = ReplayCursor::new(head.owner_epoch, 0)?;
    let first_page = j.replay(start, 256).await?;
    assert_eq!(first_page.signals.len(), 256);
    assert!(first_page.has_more);
    assert_eq!(first_page.next.after, 256);
    assert!(first_page.high_water.after > 256);
    assert_eq!(
        j.replay(start, 256).await?,
        first_page,
        "stable replay identity"
    );
    let mut cursor = start;
    let mut all = Vec::new();
    loop {
        let page = j.replay(cursor, 17).await?;
        assert!(page.signals.len() <= 17);
        all.extend(page.signals);
        cursor = page.next;
        if !page.has_more {
            break;
        }
    }
    assert_eq!(cursor, head);
    assert_eq!(all.len() as i64, head.after);
    let ids: std::collections::HashSet<_> = all.iter().map(|h| h.signal_id).collect();
    assert_eq!(ids.len(), all.len());
    let caught = j.replay(head, 256).await?;
    assert!(caught.signals.is_empty());
    assert_eq!(caught.next, head);
    assert!(!caught.has_more);
    assert_eq!(
        j.replay(ReplayCursor::new(Uuid::new_v4(), 0)?, 1).await,
        Err(ReplayError::OwnerEpochMismatch)
    );
    assert_eq!(
        j.replay(ReplayCursor::new(head.owner_epoch, head.after + 1)?, 1)
            .await,
        Err(ReplayError::InvalidCursor)
    );
    sqlx::query("create table public.mcr_invalidation_restart_receipt(payload jsonb not null)")
        .execute(&p)
        .await?;
    sqlx::query("insert into public.mcr_invalidation_restart_receipt(payload) values($1)")
        .bind(serde_json::json!({"epoch":head.owner_epoch,"cursor":head.after,"signals":all}))
        .execute(&p)
        .await?;
    p.close().await;
    Ok(())
}

#[tokio::test]
#[ignore = "runner executes after actual postmaster restart"]
async fn restart_replays_stable_identity_and_refuses_changed_checksum() -> anyhow::Result<()> {
    let p = pool().await?;
    migrate(&p, false).await?;
    let j = journal(&p);
    let saved: serde_json::Value =
        sqlx::query_scalar("select payload from public.mcr_invalidation_restart_receipt")
            .fetch_one(&p)
            .await?;
    let head = j.checkpoint().await?;
    assert_eq!(serde_json::to_value(head.owner_epoch)?, saved["epoch"]);
    assert_eq!(head.after, saved["cursor"].as_i64().unwrap());
    let mut cursor = ReplayCursor::new(head.owner_epoch, 0)?;
    let mut all = Vec::new();
    loop {
        let b = j.replay(cursor, 256).await?;
        all.extend(b.signals);
        cursor = b.next;
        if !b.has_more {
            break;
        }
    }
    assert_eq!(serde_json::to_value(all)?, saved["signals"]);
    let checksum: Vec<u8> =
        sqlx::query_scalar("select checksum from _sqlx_migrations where version=16")
            .fetch_one(&p)
            .await?;
    sqlx::query("update _sqlx_migrations set checksum=$1 where version=16")
        .bind(vec![0u8])
        .execute(&p)
        .await?;
    assert!(migrate(&p, false).await.is_err());
    sqlx::query("update _sqlx_migrations set checksum=$1 where version=16")
        .bind(checksum)
        .execute(&p)
        .await?;
    migrate(&p, false).await?;
    p.close().await;
    Ok(())
}
