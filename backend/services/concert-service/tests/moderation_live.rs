//! BE39 ledger qualification. Synthetic owner/role semantics, actual private PG.
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::{
    admin::{moderation::*, *},
    commands::*,
};
use concert_service::{
    adapters::out::{
        relationship_authority::guarded::{ProducerKind, ProducerTransaction},
        repository::audit::PgModeration,
    },
    moderation_live::ports::*,
};
use prost::Message;
use sqlx::{PgConnection, PgPool, Row};
use std::sync::{atomic::Ordering, Arc};
use uuid::Uuid;
mod moderation_live_support;
use moderation_live_support::*;

#[tokio::test]
#[ignore = "private harness explicit published checkpoint"]
async fn published_checkpoint() -> anyhow::Result<()> {
    use std::{os::unix::fs::PermissionsExt, path::PathBuf};
    let pool = owned_pool().await?;
    let root = PathBuf::from(std::env::var("CONCERT_TEST_OWNER_ROOT")?);
    let baseline = root.join("published-migrations-before-0043");
    std::fs::create_dir(&baseline)?;
    std::fs::set_permissions(&baseline, std::fs::Permissions::from_mode(0o700))?;
    for entry in std::fs::read_dir(migration_directory()?)? {
        let path = entry?.path();
        let name = path.file_name().unwrap().to_str().unwrap();
        if path.extension().and_then(|v| v.to_str()) == Some("sql") && !name.starts_with("0043_") {
            std::fs::copy(&path, baseline.join(name))?;
        }
    }
    let m = migration_runtime::load_migrations(&baseline).await?;
    m.run(&pool).await?;
    migration_runtime::verify_receipts(&pool, &m).await?;
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values('00000000-0000-4000-8000-000000000039','be39-published-state','Synthetic checkpoint','OPEN')").execute(&pool).await?;
    pool.close().await;
    Ok(())
}
#[tokio::test]
#[ignore = "private harness explicit migration application"]
async fn apply_migrations() -> anyhow::Result<()> {
    let pool = owned_pool().await?;
    let m = migration_runtime::load_migrations(&migration_directory()?).await?;
    m.run(&pool).await?;
    migration_runtime::verify_receipts(&pool, &m).await?;
    pool.close().await;
    Ok(())
}
#[tokio::test]
async fn success_replay_hmac_conflict_key_rotation_and_redacted_audit() -> anyhow::Result<()> {
    let (pool, actor) = fixture().await?;
    let owner = Owner::new(&pool, 0);
    let auth = Authority::operator(actor);
    let store = PgModeration::new(pool.clone(), owners(&owner), keys());
    let key = Uuid::new_v4();
    let c = user(Uuid::new_v4(), UserAction::Suspend, 1);
    let a = store.submit(actor, key, &c, &auth).await?;
    assert_eq!(a.state, ReceiptState::Succeeded);
    assert_eq!(owner.calls.load(Ordering::SeqCst), 3);
    assert_eq!(outcomes(&pool, a.id).await, vec!["PENDING", "SUCCEEDED"]);
    let b = store.submit(actor, key, &c, &auth).await?;
    assert_eq!(a.id, b.id);
    assert_eq!(owner.calls.load(Ordering::SeqCst), 3);
    let mut different = c.clone();
    if let Change::User { reason, .. } = &mut different {
        *reason = "Different".into();
    }
    assert!(matches!(
        store.submit(actor, key, &different, &auth).await,
        Err(CommandError::Conflict)
    ));
    let rotated = Arc::new(FingerprintKeys::new(
        SigningKey::new("be39-next", &[40; 32])?,
        vec![SigningKey::new("be39-fixture", &[39; 32])?],
    )?);
    let rotated = PgModeration::new(pool.clone(), owners(&owner), rotated);
    assert_eq!(rotated.submit(actor, key, &c, &auth).await?.id, a.id);
    let query = concert_service::moderation_live::wire::Query {
        limit: 20,
        ..Default::default()
    };
    let rows = store
        .audit_page(actor, &auth, &query, None, Utc::now())
        .await?;
    let value = serde_json::json!({"items":rows,"next_cursor":null});
    concert_service::moderation_live::wire::validate_value("Domain_admin_AuditList", &value)?;
    let output = value.to_string();
    for secret in [
        "Private synthetic",
        "reason",
        "native_identity",
        "request_hash",
    ] {
        assert!(!output.contains(secret));
    }
    let stale = store
        .submit(
            actor,
            Uuid::new_v4(),
            &user(c.target(), UserAction::Unsuspend, 1),
            &auth,
        )
        .await?;
    assert_eq!(stale.state, ReceiptState::Failed);
    assert_eq!(stale.failure, Some(SafeFailure::VersionConflict));
    let lifted = store
        .submit(
            actor,
            Uuid::new_v4(),
            &user(c.target(), UserAction::Unsuspend, 2),
            &auth,
        )
        .await?;
    assert_eq!(lifted.state, ReceiptState::Succeeded);
    let state=sqlx::query("select state,revision,epoch,sessions_live,grants_live from be39_fixture_targets where event_id=$1 and target_id=$2").bind(actor.event).bind(c.target()).fetch_one(&pool).await?;
    assert_eq!(state.try_get::<String, _>("state")?, "ACTIVE");
    assert_eq!(state.try_get::<i64, _>("revision")?, 3);
    assert_eq!(state.try_get::<i64, _>("epoch")?, 2);
    assert!(!state.try_get::<bool, _>("sessions_live")?);
    assert!(!state.try_get::<bool, _>("grants_live")?);
    pool.close().await;
    Ok(())
}
#[tokio::test]
async fn roles_event_stale_lease_operator_peer_and_revoked_replay_fail_closed() -> anyhow::Result<()>
{
    let (pool, actor) = fixture().await?;
    let owner = Owner::new(&pool, 0);
    let store = PgModeration::new(pool.clone(), owners(&owner), keys());
    let c = user(Uuid::new_v4(), UserAction::Unsuspend, 1);
    for mode in [1, 2, 3] {
        let mut a = Authority::operator(actor);
        a.mode = mode;
        assert!(store.submit(actor, Uuid::new_v4(), &c, &a).await.is_err());
    }
    let mut auth = Authority::operator(actor);
    auth.role = Role::Member;
    assert!(matches!(
        store.submit(actor, Uuid::new_v4(), &c, &auth).await,
        Err(CommandError::Denied)
    ));
    auth.role = Role::Moderator;
    auth.target_role = Role::Operator;
    assert!(matches!(
        store.submit(actor, Uuid::new_v4(), &c, &auth).await,
        Err(CommandError::Denied)
    ));
    let q = concert_service::moderation_live::wire::Query {
        limit: 20,
        ..Default::default()
    };
    assert!(matches!(
        store.audit_page(actor, &auth, &q, None, Utc::now()).await,
        Err(CommandError::Denied)
    ));
    let other = Actor {
        event: Uuid::new_v4(),
        ..actor
    };
    assert!(store
        .submit(other, Uuid::new_v4(), &c, &auth)
        .await
        .is_err());
    assert_eq!(owner.calls.load(Ordering::SeqCst), 0);
    let auth = Authority::operator(actor);
    let key = Uuid::new_v4();
    let r = store.submit(actor, key, &c, &auth).await?;
    sqlx::query("update be39_fixture_roles set allowed=false where actor=$1 and event=$2")
        .bind(actor.actor)
        .bind(actor.event)
        .execute(&pool)
        .await?;
    assert!(matches!(
        store.submit(actor, key, &c, &auth).await,
        Err(CommandError::Denied)
    ));
    assert_eq!(outcomes(&pool, r.id).await, vec!["PENDING", "SUCCEEDED"]);
    pool.close().await;
    Ok(())
}
#[tokio::test]
async fn lost_ack_is_pending_replay_does_not_apply_and_explicit_recovery_uses_same_identity(
) -> anyhow::Result<()> {
    let (pool, actor) = fixture().await?;
    let owner = Owner::new(&pool, 1);
    let auth = Authority::operator(actor);
    let store = PgModeration::new(pool.clone(), owners(&owner), keys());
    let key = Uuid::new_v4();
    let c = user(Uuid::new_v4(), UserAction::Unsuspend, 1);
    let r = store.submit(actor, key, &c, &auth).await?;
    assert_eq!(r.state, ReceiptState::Pending);
    assert_eq!(outcomes(&pool, r.id).await, vec!["PENDING"]);
    assert_eq!(
        store.submit(actor, key, &c, &auth).await?.state,
        ReceiptState::Pending
    );
    assert_eq!(owner.calls.load(Ordering::SeqCst), 1);
    assert_eq!(owner.recoveries.load(Ordering::SeqCst), 0);
    let original: Vec<u8> =
        sqlx::query_scalar("select native from be39_fixture_owner where receipt=$1")
            .bind(r.id)
            .fetch_one(&pool)
            .await?;
    let r2 = store.resume(actor, key, &c, &auth).await?;
    assert_eq!(r2.id, r.id);
    assert_eq!(r2.state, ReceiptState::Succeeded);
    assert_eq!(owner.calls.load(Ordering::SeqCst), 1);
    assert_eq!(owner.recoveries.load(Ordering::SeqCst), 1);
    let settled:Vec<u8>=sqlx::query_scalar("select native_identity from concert_moderation_owner_results where receipt_id=$1 and outcome='APPLIED'").bind(r.id).fetch_one(&pool).await?;
    assert_eq!(original, settled);
    assert!(matches!(
        store.resume(actor, Uuid::new_v4(), &c, &auth).await,
        Err(CommandError::Denied)
    ));
    pool.close().await;
    Ok(())
}
#[tokio::test]
async fn partial_failure_records_applied_and_failed_without_success_audit() -> anyhow::Result<()> {
    let (pool, actor) = fixture().await?;
    let first = Owner::new(&pool, 0);
    let fail = Owner::new(&pool, 2);
    let mut ports = owners(&first);
    ports.event_world_termination = Some(Arc::new(fail.clone()));
    let store = PgModeration::new(pool.clone(), ports, keys());
    let r = store
        .submit(
            actor,
            Uuid::new_v4(),
            &user(Uuid::new_v4(), UserAction::Suspend, 1),
            &Authority::operator(actor),
        )
        .await?;
    assert_eq!(r.state, ReceiptState::Failed);
    assert_eq!(r.failure, Some(SafeFailure::InvalidTransition));
    assert_eq!(outcomes(&pool, r.id).await, vec!["PENDING", "FAILED"]);
    assert_eq!(first.calls.load(Ordering::SeqCst), 1);
    assert_eq!(fail.calls.load(Ordering::SeqCst), 1);
    let applied:i64=sqlx::query_scalar("select count(*) from concert_moderation_owner_results where receipt_id=$1 and outcome='APPLIED'").bind(r.id).fetch_one(&pool).await?;
    assert_eq!(applied, 1);
    pool.close().await;
    Ok(())
}
#[tokio::test]
async fn unavailable_owner_unknown_result_and_mismatched_ack_never_succeed() -> anyhow::Result<()> {
    let (pool, actor) = fixture().await?;
    let auth = Authority::operator(actor);
    let c = user(Uuid::new_v4(), UserAction::Suspend, 1);
    let missing = PgModeration::new(pool.clone(), Default::default(), keys());
    assert!(matches!(
        missing.submit(actor, Uuid::new_v4(), &c, &auth).await,
        Err(CommandError::Unavailable)
    ));
    let count: i64 =
        sqlx::query_scalar("select count(*) from concert_command_receipts where event_id=$1")
            .bind(actor.event)
            .fetch_one(&pool)
            .await?;
    assert_eq!(count, 0);
    for mode in [4, 5] {
        let owner = Owner::new(&pool, mode);
        let store = PgModeration::new(pool.clone(), owners(&owner), keys());
        let _ = store.submit(actor, Uuid::new_v4(), &c, &auth).await;
    }
    let count: i64 = sqlx::query_scalar(
        "select count(*) from concert_command_receipts where event_id=$1 and state='PENDING'",
    )
    .bind(actor.event)
    .fetch_one(&pool)
    .await?;
    assert_eq!(count, 2);
    let success: i64 = sqlx::query_scalar(
        "select count(*) from concert_moderation_audit where event_id=$1 and outcome='SUCCEEDED'",
    )
    .bind(actor.event)
    .fetch_one(&pool)
    .await?;
    assert_eq!(success, 0);
    pool.close().await;
    Ok(())
}
#[tokio::test]
async fn content_deleted_restore_denial_and_unsuspend_only_call_restriction() -> anyhow::Result<()>
{
    let (pool, actor) = fixture().await?;
    let owner = Owner::new(&pool, 3);
    let auth = Authority::operator(actor);
    let store = PgModeration::new(pool.clone(), owners(&owner), keys());
    let deleted = Uuid::new_v4();
    sqlx::query("insert into be39_fixture_targets values($1,$2,'DELETED',1,1,false,false)")
        .bind(actor.event)
        .bind(deleted)
        .execute(&pool)
        .await?;
    let r = store
        .submit(
            actor,
            Uuid::new_v4(),
            &content(deleted, ContentAction::Restore),
            &auth,
        )
        .await?;
    assert_eq!(r.state, ReceiptState::Failed);
    assert_eq!(outcomes(&pool, r.id).await, vec!["PENDING", "FAILED"]);
    let target = Uuid::new_v4();
    let r = store
        .submit(
            actor,
            Uuid::new_v4(),
            &user(target, UserAction::Unsuspend, 1),
            &auth,
        )
        .await?;
    assert_eq!(r.state, ReceiptState::Succeeded);
    let steps: Vec<String> =
        sqlx::query_scalar("select step from be39_fixture_owner where receipt=$1")
            .bind(r.id)
            .fetch_all(&pool)
            .await?;
    assert_eq!(steps, vec!["EVENT_RESTRICTION"]);
    pool.close().await;
    Ok(())
}
#[tokio::test]
async fn expiry_tombstone_no_new_execution_and_immutable_ledger() -> anyhow::Result<()> {
    let (pool, actor) = fixture().await?;
    let owner = Owner::new(&pool, 0);
    let auth = Authority::operator(actor);
    let store = PgModeration::new(pool.clone(), owners(&owner), keys());
    let key = Uuid::new_v4();
    let c = user(Uuid::new_v4(), UserAction::Unsuspend, 1);
    let r = store.submit(actor, key, &c, &auth).await?;
    for table in [
        "concert_moderation_commands",
        "concert_moderation_owner_results",
        "concert_moderation_audit",
    ] {
        for verb in ["delete from", "update"] {
            let sql = if verb == "update" {
                format!("update {table} set receipt_id=receipt_id where receipt_id=$1")
            } else {
                format!("delete from {table} where receipt_id=$1")
            };
            assert!(sqlx::query(&sql).bind(r.id).execute(&pool).await.is_err());
        }
    }
    // Insert a historically expired tombstone with a genuine server HMAC. Never
    // alter immutable receipt correlation or disable a production trigger.
    let expired_key = Uuid::new_v4();
    let scope = Scope {
        actor: actor.actor,
        event: actor.event,
        operation: RegisteredOperation::find(c.action().operation())?,
        key: expired_key,
    };
    let event = actor.event.to_string();
    let target = c.target().to_string();
    let input = CanonicalRequest::json(
        scope,
        &[("eventId", &event), ("userId", &target)],
        &[],
        Some(&c.canonical()),
    )?;
    sqlx::query("insert into concert_command_receipts(receipt_id,actor_id,event_id,operation_id,idempotency_key,request_hash,target_id,state,created_at,expires_at) values($1,$2,$3,$4,$5,$6,$7,'RETIRED',statement_timestamp()-interval '2 days',statement_timestamp()-interval '1 day')").bind(Uuid::new_v4()).bind(actor.actor).bind(actor.event).bind(scope.operation.id()).bind(expired_key).bind(keys().sign(&input)?.as_storage()).bind(c.target()).execute(&pool).await?;
    assert!(matches!(
        store.submit(actor, expired_key, &c, &auth).await,
        Err(CommandError::Expired)
    ));
    assert_eq!(owner.calls.load(Ordering::SeqCst), 1);
    pool.close().await;
    Ok(())
}
#[tokio::test]
async fn deferred_authority_deadline_rolls_back_owner_outcome() -> anyhow::Result<()> {
    let (pool, actor) = fixture().await?;
    let owner = Owner::new(&pool, 4);
    let auth = Authority::operator(actor);
    let store = PgModeration::new(pool.clone(), owners(&owner), keys());
    let r = store
        .submit(
            actor,
            Uuid::new_v4(),
            &user(Uuid::new_v4(), UserAction::Unsuspend, 1),
            &auth,
        )
        .await?;
    let mut tx = pool.begin().await?;
    sqlx::query("select set_config('mcr.moderation_authority_until',(clock_timestamp()-interval '1 second')::text,true)").execute(&mut *tx).await?;
    sqlx::query("insert into concert_moderation_owner_results(result_id,receipt_id,step,intent_digest,outcome,failure_reason) select $1,receipt_id,step,intent_digest,'FAILED','DEPENDENCY_UNAVAILABLE' from concert_moderation_owner_results where receipt_id=$2 and outcome='PENDING'").bind(Uuid::new_v4()).bind(r.id).execute(&mut *tx).await?;
    assert!(tx.commit().await.is_err());
    assert_eq!(outcomes(&pool, r.id).await, vec!["PENDING"]);
    let count:i64=sqlx::query_scalar("select count(*) from concert_moderation_owner_results where receipt_id=$1 and outcome='FAILED'").bind(r.id).fetch_one(&pool).await?;
    assert_eq!(count, 0);
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn canonical_parent_frame_cannot_be_rebound() -> anyhow::Result<()> {
    use concert_service::{
        moderation_live::wire::{warm_contracts, ModerationRequest},
        product_rpc::proto::ProductRequest,
    };
    warm_contracts()?;
    let event = Uuid::new_v4();
    let now = Utc::now().timestamp_micros();
    let message = ProductRequest {
        user_id: Uuid::new_v4().to_string(),
        event_id: event.to_string(),
        session_family_id: Uuid::new_v4().to_string(),
        request_id: Uuid::new_v4().to_string(),
        http_method: "GET".into(),
        http_target: format!("/api/concert/events/{event}/admin/users"),
        authority_expires_unix_micros: now + 4_800_000,
        access_expires_unix_micros: now + 60_000_000,
        auth_observed_at_unix_micros: now,
        profile_observed_at_unix_micros: now,
        ..Default::default()
    };
    let binding = moderation_live_support::capture::binding(&message).await?;
    assert_eq!(
        ModerationRequest::from_owner_scope(&binding, &message)?.action,
        Action::AdminUsers
    );
    let mut altered = message.clone();
    altered.idempotency_key = Uuid::new_v4().to_string();
    assert!(ModerationRequest::from_owner_scope(&binding, &altered).is_err());
    let mut altered = message.clone();
    altered.event_id = Uuid::new_v4().to_string();
    assert!(ModerationRequest::from_owner_scope(&binding, &altered).is_err());
    let mut altered = message.clone();
    altered.session_family_id = Uuid::new_v4().to_string();
    assert!(ModerationRequest::from_owner_scope(&binding, &altered).is_err());
    let mut altered = message.clone();
    altered.http_body = b"{}".to_vec();
    assert!(ModerationRequest::from_owner_scope(&binding, &altered).is_err());
    let mut altered = message.clone();
    altered.http_target = format!("/api/concert/events/{event}/admin/audit");
    assert!(ModerationRequest::from_owner_scope(&binding, &altered).is_err());
    let mut altered = message;
    altered.authority_expires_unix_micros += 1;
    assert!(ModerationRequest::from_owner_scope(&binding, &altered).is_err());
    Ok(())
}
