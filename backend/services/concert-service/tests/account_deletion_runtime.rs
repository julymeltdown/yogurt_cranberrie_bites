//! Production Auth worker -> authenticated source -> factory Apply -> actual
//! Ticket/Notification commits. Missing owners cannot settle the eight-step saga.
mod account_deletion_runtime_support;
#[path = "../../auth-service/tests/support/auth_fixture_config.rs"]
mod auth_fixture;
#[path = "canonical_owner_composition_support/environment.rs"]
mod environment;
use account_deletion_runtime_support::*;
use auth_service::{
    adapters::out::{
        grpc::concert_deletion_client::ConcertDeletionClient,
        persistence::user_deletion_saga_repository::PgUserDeletionSagaRepository,
    },
    application::{
        dto::{auth_dto::DeleteUserDto, deletion_dto::AuthUserDeletionTargetDto as Target},
        errors::AuthError,
        ports::{UserDeletionDispatcherPort, UserDeletionSagaPort},
    },
    config::{
        build_state_with_deletion_runtime, concert_deletion::ConcertDeletionSettings, AppState,
    },
    domain::user_deletion::{UserDeletionSaga, UserDeletionStepStatus, UserDeletionTarget},
};
use concert_service::{
    adapters::r#in::deletion_rpc::{self, proto},
    config::account_deletion_runtime::{
        AccountDeletionRuntime, AccountDeletionSettings, AuthDeletionCallers, LiveDeletionOwners,
    },
    world::account_deletion::{self as common},
};
use platform::{
    grpc_auth::{CallerRegistry, Workload},
    grpc_tls::TlsMaterial,
};
use sqlx::PgPool;
use std::{sync::Arc, time::Duration};
use uuid::Uuid;

fn settings(endpoint: &str, ms: u64) -> anyhow::Result<ConcertDeletionSettings> {
    ConcertDeletionSettings::from_values(Some("managed"), Some(endpoint), Some(&ms.to_string()))
}
async fn state(
    pool: PgPool,
    concert: &str,
    notification: &str,
    ms: u64,
) -> anyhow::Result<AppState> {
    let mut c = auth_fixture::test_config(std::env::var("MCR_CANONICAL_AUTH_URL")?)?;
    c.outbound_identity = AUTH.into();
    c.tls_material = Some(environment::tls(Workload::Auth)?);
    c.profile_grpc_endpoint = "https://127.0.0.1:1".into();
    c.friend_grpc_endpoint = c.profile_grpc_endpoint.clone();
    c.post_grpc_endpoint = c.profile_grpc_endpoint.clone();
    c.notification_grpc_endpoint = notification.into();
    c.user_deletion_worker_enabled = true;
    c.user_deletion_worker_batch_size = 100;
    c.user_deletion_worker_max_attempts = 8;
    c.user_deletion_worker_lease_timeout_ms = 1000;
    build_state_with_deletion_runtime(&c, pool, settings(concert, ms)?).await
}
async fn identity(s: &AppState, p: &PgPool) -> anyhow::Result<Uuid> {
    let email = format!("runtime-{}@example.invalid", Uuid::new_v4().simple());
    let created = s.signup.signup_email(&email, "Owned-runtime-42!").await?;
    assert_eq!(created.status, "PENDING_VERIFICATION");
    let code: String = sqlx::query_scalar("select code from email_verifications where user_id=$1")
        .bind(created.user_id)
        .fetch_one(p)
        .await?;
    assert_eq!(
        s.signup.verify_email(&email, &code).await?.status,
        "VERIFIED"
    );
    let login = s
        .login
        .login(&email, "Owned-runtime-42!", None, None)
        .await?;
    assert!(!login.access_token.is_empty() && !login.refresh_token.is_empty());
    Ok(created.user_id)
}
async fn begin(s: &AppState, p: &PgPool) -> anyhow::Result<UserDeletionSaga> {
    let user = identity(s, p).await?;
    let deleted = s
        .delete_user
        .execute(DeleteUserDto {
            user_id: user.to_string(),
            actor_id: user.to_string(),
            request_id: None,
            ip_address: None,
            user_agent: None,
        })
        .await?;
    saga(p, deleted.saga_id.parse()?).await
}
async fn saga(p: &PgPool, id: Uuid) -> anyhow::Result<UserDeletionSaga> {
    Ok(PgUserDeletionSagaRepository::new(p.clone())
        .find_by_saga_id(id)
        .await?
        .unwrap()
        .into())
}
async fn park(p: &PgPool, id: Uuid) -> anyhow::Result<()> {
    // Scheduling only: preserve every required target/receipt/status; never
    // manufacture a positive owner response or remove a required step.
    sqlx::query("update user_deletion_steps set next_attempt_at=clock_timestamp()+interval '1 day', status=case when status='IN_PROGRESS' then 'RETRY' else status end, locked_at=case when status='IN_PROGRESS' then null else locked_at end where saga_id=$1").bind(id).execute(p).await?;
    Ok(())
}
async fn due(p: &PgPool, id: Uuid, target: &str) -> anyhow::Result<()> {
    sqlx::query("update user_deletion_steps set next_attempt_at=clock_timestamp()-interval '1 second' where saga_id=$1 and target=$2").bind(id).bind(target).execute(p).await?;
    Ok(())
}
fn assert_pending(s: &UserDeletionSaga, successes: &[UserDeletionTarget]) {
    assert_eq!(s.steps.len(), 8);
    assert!(s.completed_at.is_none());
    for step in &s.steps {
        assert_eq!(
            step.status == UserDeletionStepStatus::Succeeded,
            successes.contains(&step.target),
            "target {:?}",
            step.target
        );
    }
}
async fn apply(
    d: &auth_service::config::concert_deletion::RuntimeDispatcher,
    s: &UserDeletionSaga,
    target: Target,
) -> Result<(), AuthError> {
    d.apply(target, s.id, s.user_id, s.actor_id, s.retention_until)
        .await
}
async fn ticket_receipt(p: &PgPool, id: Uuid) -> anyhow::Result<serde_json::Value> {
    Ok(sqlx::query_scalar(
        "select to_jsonb(p) from concert_world_ticket_deletion_progress p where saga_id=$1",
    )
    .bind(id)
    .fetch_one(p)
    .await?)
}
async fn notification_receipt(p: &PgPool, id: Uuid) -> anyhow::Result<serde_json::Value> {
    Ok(sqlx::query_scalar(
        "select to_jsonb(r) from notification_deletion_receipts_v1 r where saga_id=$1",
    )
    .bind(id)
    .fetch_one(p)
    .await?)
}
#[test]
fn factory_modes_and_auth_registry_are_explicit() {
    assert_eq!(
        AccountDeletionSettings::from_value(None).unwrap(),
        AccountDeletionSettings::Off
    );
    assert_eq!(
        AccountDeletionSettings::from_value(Some("managed")).unwrap(),
        AccountDeletionSettings::Managed
    );
    for mode in ["", "true", "auto"] {
        assert!(AccountDeletionSettings::from_value(Some(mode)).is_err());
    }
    assert!(AuthDeletionCallers::from_credentials("", None).is_err());
    assert!(AuthDeletionCallers::from_credentials(AUTH, Some(AUTH)).is_err());
    assert!(AuthDeletionCallers::from_credentials(AUTH, Some("previous-auth")).is_ok());
}
#[tokio::test]
async fn factory_off_has_no_service_and_managed_has_no_plaintext_or_missing_tls(
) -> anyhow::Result<()> {
    let p = sqlx::postgres::PgPoolOptions::new().connect_lazy("postgres://unused/mcr_concert")?;
    assert!(AccountDeletionRuntime::compose(
        AccountDeletionSettings::Off,
        p.clone(),
        "",
        None,
        "",
        AuthDeletionCallers::from_credentials(AUTH, None)?,
        LiveDeletionOwners::default()
    )?
    .is_none());
    assert!(AccountDeletionRuntime::compose(
        AccountDeletionSettings::Managed,
        p.clone(),
        "https://auth.example",
        None,
        "concert-fixture",
        AuthDeletionCallers::from_credentials(AUTH, None)?,
        LiveDeletionOwners::default()
    )
    .is_err());
    for endpoint in [
        "http://auth.example",
        "https://user@auth.example",
        "https://auth.example/path",
        "https://auth.example?query=secret",
    ] {
        let error = AccountDeletionRuntime::compose(
            AccountDeletionSettings::Managed,
            p.clone(),
            endpoint,
            None,
            CONCERT,
            AuthDeletionCallers::from_credentials(AUTH, None)?,
            LiveDeletionOwners::default(),
        )
        .err()
        .unwrap();
        assert_eq!(
            error.to_string(),
            "Canonical Auth deletion endpoint must be an explicit HTTPS origin"
        );
    }
    let empty = TlsMaterial::from_pem(Vec::new(), Vec::new(), Vec::new());
    assert!(AccountDeletionRuntime::compose(
        AccountDeletionSettings::Managed,
        p,
        "http://auth.example",
        Some(&empty),
        "concert-fixture",
        AuthDeletionCallers::from_credentials(AUTH, None)?,
        LiveDeletionOwners::default()
    )
    .is_err());
    Ok(())
}
#[tokio::test]
#[ignore = "fresh PG16/PKI allocation; SOURCE then physical PG and RPC restart in fresh PID"]
async fn actual_production_worker_factory_commit_restart_and_non_ack() -> anyhow::Result<()> {
    let auth = environment::pool("auth").await?;
    let concert = environment::pool("concert").await?;
    let notification = environment::pool("notification").await?;
    let source = source_server(auth.clone()).await?;
    let rpc = factory_server(
        concert.clone(),
        &source.url,
        Workload::Concert,
        LiveDeletionOwners::default(),
    )
    .await?;
    let notif = notification_server(notification.clone()).await?;
    let s = state(auth.clone(), &rpc.url, &notif.url, 5000).await?;
    let d = runtime_dispatcher(&rpc.url, &notif.url, 5000)?;
    let phase = std::env::var("MCR_BE42_RUNTIME_PHASE")?;
    if phase == "RESTARTED" {
        let saved = load()?;
        assert_ne!(saved.pid, std::process::id());
        let started: String = sqlx::query_scalar("select pg_postmaster_start_time()::text")
            .fetch_one(&auth)
            .await?;
        assert_ne!(saved.pg_started, started);
        let g = saga(&auth, saved.saga).await?;
        assert_pending(
            &g,
            &[
                UserDeletionTarget::ConcertTickets,
                UserDeletionTarget::Notification,
            ],
        );
        // Drive the same durable worker after actual PG and all RPC restarts;
        // already succeeded rows are never claimed and all other owners fail.
        for target in [
            "PROFILE",
            "FRIEND",
            "POST",
            "CONCERT_SESSIONS",
            "CONCERT_SHARING",
            "CONCERT_PRIVATE_DATA",
        ] {
            due(&auth, g.id, target).await?;
        }
        let run = s.user_deletion_worker.run_once(chrono::Utc::now()).await?;
        assert_eq!((run.claimed, run.succeeded, run.retried), (6, 0, 6));
        apply(&d, &g, Target::ConcertTickets).await?;
        apply(&d, &g, Target::Notification).await?;
        assert_eq!(ticket_receipt(&concert, g.id).await?, saved.ticket);
        assert_eq!(
            notification_receipt(&notification, g.id).await?,
            saved.notification
        );
        assert_pending(
            &saga(&auth, g.id).await?,
            &[
                UserDeletionTarget::ConcertTickets,
                UserDeletionTarget::Notification,
            ],
        );
        println!("BE42 RUNTIME RESTART PASS: durable worker, exact committed receipts and six unavailable steps retained");
    } else {
        assert_eq!(phase, "SOURCE");
        let g = begin(&s, &auth).await?;
        let run = s.user_deletion_worker.run_once(chrono::Utc::now()).await?;
        assert_eq!((run.claimed, run.succeeded, run.retried), (8, 2, 6));
        assert_pending(
            &saga(&auth, g.id).await?,
            &[
                UserDeletionTarget::ConcertTickets,
                UserDeletionTarget::Notification,
            ],
        );
        assert_eq!(run.dead_lettered, 0);
        let ticket = ticket_receipt(&concert, g.id).await?;
        let notification_receipt = notification_receipt(&notification, g.id).await?;
        // A new dispatcher/factory connection must replay the same committed proof.
        apply(&d, &g, Target::ConcertTickets).await?;
        apply(&d, &g, Target::Notification).await?;
        assert_eq!(ticket_receipt(&concert, g.id).await?, ticket);
        let client =
            ConcertDeletionClient::connect(&rpc.url, &environment::tls(Workload::Auth)?, AUTH)?;
        let c = auth_service::application::concert_deletion::ConcertDeletionCommand {
            key: auth_service::application::concert_deletion::ConcertDeletionKey {
                saga_id: g.id,
                user_id: g.user_id,
                target: auth_service::application::concert_deletion::ConcertDeletionTarget::Tickets,
            },
            actor_id: g.actor_id,
            retention_until: g.retention_until,
        };
        let mut rebound = c.clone();
        rebound.retention_until += chrono::Duration::nanoseconds(1);
        assert!(client
            .apply_with_deadline(
                rebound,
                tokio::time::Instant::now() + Duration::from_secs(1)
            )
            .await
            .is_err());
        let expired = std::time::Instant::now();
        assert!(
            platform::call_deadline::scope(expired, apply(&d, &g, Target::ConcertTickets))
                .await
                .is_err()
        );
        for target in [
            Target::ConcertSessions,
            Target::ConcertSharing,
            Target::ConcertPrivateData,
        ] {
            assert!(apply(&d, &g, target).await.is_err());
        }
        park(&auth, g.id).await?;
        save(&Saved {
            saga: g.id,
            ticket,
            notification: notification_receipt,
            pid: std::process::id(),
            pg_started: sqlx::query_scalar("select pg_postmaster_start_time()::text")
                .fetch_one(&auth)
                .await?,
        })?;
        // mTLS peer identity, caller role and credential are still required by
        // the actual production factory, not inferred from a command or header.
        let bad = ConcertDeletionClient::connect(
            &rpc.url,
            &environment::tls(Workload::Auth)?,
            "wrong-auth",
        )?;
        assert!(bad
            .apply_with_deadline(
                c.clone(),
                tokio::time::Instant::now() + Duration::from_secs(1)
            )
            .await
            .is_err());
        let wrong = factory_server(
            concert.clone(),
            &source.url,
            Workload::Friend,
            LiveDeletionOwners::default(),
        )
        .await?;
        let bad =
            ConcertDeletionClient::connect(&wrong.url, &environment::tls(Workload::Auth)?, AUTH)?;
        assert!(bad
            .apply_with_deadline(c, tokio::time::Instant::now() + Duration::from_secs(1))
            .await
            .is_err());
        wrong.finish().await?;
        // No empty parallel World registry may qualify Sessions even at startup.
        assert!(AccountDeletionRuntime::compose(
            AccountDeletionSettings::Managed,
            concert.clone(),
            &source.url,
            Some(&environment::tls(Workload::Concert)?),
            CONCERT,
            AuthDeletionCallers::from_credentials(AUTH, None)?,
            LiveDeletionOwners {
                worlds: Some(Vec::new()),
                sharing_publisher: None
            }
        )
        .is_err());
        hostile_and_clock(&auth, &notif.url, &s).await?;
        cancellation(&auth, &concert, &rpc.url, &notif.url, &s).await?;
        println!("BE42 RUNTIME SOURCE PASS: real production worker/factory, Ticket/Notification commits, absent owners, hostile reply, original clock and cancellation non-ACK");
    }
    notif.finish().await?;
    rpc.finish().await?;
    source.finish().await?;
    auth.close().await;
    concert.close().await;
    notification.close().await;
    Ok(())
}
async fn hostile_and_clock(auth: &PgPool, notification: &str, s: &AppState) -> anyhow::Result<()> {
    let hostile = hostile_server().await?;
    let h = state(auth.clone(), &hostile.server.url, notification, 120).await?;
    let g = begin(s, auth).await?;
    park(auth, g.id).await?;
    due(auth, g.id, "CONCERT_TICKETS").await?;
    let run = h.user_deletion_worker.run_once(chrono::Utc::now()).await?;
    assert_eq!((run.succeeded, run.retried), (0, 1));
    assert_pending(&saga(auth, g.id).await?, &[]);
    park(auth, g.id).await?;
    hostile
        .delay
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let d = runtime_dispatcher(&hostile.server.url, notification, 5000)?;
    let original = std::time::Instant::now() + Duration::from_millis(35);
    let began = std::time::Instant::now();
    assert!(
        platform::call_deadline::scope(original, apply(&d, &g, Target::ConcertTickets))
            .await
            .is_err()
    );
    assert!(began.elapsed() < Duration::from_millis(500));
    let calls = hostile.calls.load(std::sync::atomic::Ordering::SeqCst);
    assert!(
        platform::call_deadline::scope(original, apply(&d, &g, Target::ConcertTickets))
            .await
            .is_err()
    );
    assert_eq!(
        hostile.calls.load(std::sync::atomic::Ordering::SeqCst),
        calls
    );
    // Cancellation after the authenticated peer observes actual dispatch leaves
    // its durable claimed step unresolved; it cannot be turned into success.
    let g = begin(s, auth).await?;
    park(auth, g.id).await?;
    due(auth, g.id, "CONCERT_TICKETS").await?;
    let before = hostile.calls.load(std::sync::atomic::Ordering::SeqCst);
    let w = h.user_deletion_worker.clone();
    let task = tokio::spawn(async move { w.run_once(chrono::Utc::now()).await });
    tokio::time::timeout(Duration::from_secs(2), async {
        while hostile.calls.load(std::sync::atomic::Ordering::SeqCst) == before {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await?;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_pending(&saga(auth, g.id).await?, &[]);
    park(auth, g.id).await?;
    hostile.server.finish().await?;
    Ok(())
}
async fn cancellation(
    auth: &PgPool,
    concert: &PgPool,
    rpc: &str,
    notification: &str,
    s: &AppState,
) -> anyhow::Result<()> {
    let g = begin(s, auth).await?;
    park(auth, g.id).await?;
    due(auth, g.id, "CONCERT_TICKETS").await?;
    let mut blocker = concert.begin().await?;
    common::lock_user(&mut blocker, g.user_id).await?;
    let bound = state(auth.clone(), rpc, notification, 180).await?;
    let w = bound.user_deletion_worker.clone();
    let task = tokio::spawn(async move { w.run_once(chrono::Utc::now()).await });
    // Observe actual SQL reached from factory RPC before cancelling; a sleeping
    // client or a sent request alone is insufficient transaction evidence.
    tokio::time::timeout(Duration::from_secs(2),async{loop{let wait:bool=sqlx::query_scalar("select exists(select 1 from pg_stat_activity where datname=current_database() and pid<>pg_backend_pid() and wait_event='advisory' and query like '%mcr-world-account-deletion:%')").fetch_one(concert).await.unwrap();if wait{break;}tokio::time::sleep(Duration::from_millis(2)).await;}}).await?;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    tokio::time::sleep(Duration::from_millis(220)).await;
    blocker.rollback().await?;
    tokio::time::timeout(Duration::from_secs(2),async{loop{let live:bool=sqlx::query_scalar("select exists(select 1 from pg_stat_activity where datname=current_database() and pid<>pg_backend_pid() and xact_start is not null)").fetch_one(concert).await.unwrap();if !live{break;}tokio::time::sleep(Duration::from_millis(2)).await;}}).await?;
    let count:i64=sqlx::query_scalar("select (select count(*) from concert_world_account_deletion_fences where user_id=$1)+(select count(*) from concert_world_ticket_deletion_progress where saga_id=$2)").bind(g.user_id).bind(g.id).fetch_one(concert).await?;
    assert_eq!(count, 0);
    assert_pending(&saga(auth, g.id).await?, &[]);
    // Expired dispatch never settles it; a new bounded dispatch can commit and
    // the actual durable worker recovers its lease without duplicating effects.
    let d = runtime_dispatcher(rpc, notification, 5000)?;
    assert!(platform::call_deadline::scope(
        std::time::Instant::now(),
        apply(&d, &g, Target::ConcertTickets)
    )
    .await
    .is_err());
    let run = bound
        .user_deletion_worker
        .run_once(chrono::Utc::now() + chrono::Duration::seconds(2))
        .await?;
    assert_eq!(run.succeeded, 1);
    assert_pending(
        &saga(auth, g.id).await?,
        &[UserDeletionTarget::ConcertTickets],
    );
    park(auth, g.id).await?;
    Ok(())
}

/// Real Auth source -> mounted factory Apply over mTLS HTTP/2 -> production
/// PrivateData coordinator. Proves the composed serving service seals the whole
/// acknowledgement, refuses a caller before the deleter, and replays after a
/// service restart without duplicating effects.
async fn seed_private_rows(p: &PgPool, user: Uuid, control: Uuid) -> anyhow::Result<()> {
    let event = Uuid::new_v4();
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,$3,'OPEN')")
        .bind(event)
        .bind(format!("mount-{event}"))
        .bind("Deletion mount")
        .execute(p)
        .await?;
    for id in [user, control] {
        sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'DANGER',true,true,true)")
            .bind(event)
            .bind(id)
            .execute(p)
            .await?;
    }
    sqlx::query("insert into concert_avatar_settings(event_id,user_id,contract_version,representation,preset_id,manifest_id,manifest_version) values($1,$2,'1.1.0','whole_character','photo01_front_01','mcr_selected_static_20261103',1)")
        .bind(event)
        .bind(user)
        .execute(p)
        .await?;
    sqlx::query("insert into concert_age_owners(user_id) values($1)")
        .bind(user)
        .execute(p)
        .await?;
    Ok(())
}

async fn private_rows_remaining(p: &PgPool, user: Uuid) -> anyhow::Result<i64> {
    let core: i64 = sqlx::query_scalar(
        "select count(*) from concert_participants where user_id=$1 and (theme<>'PARADE' or low_power or reduced_motion or sound_enabled)",
    )
    .bind(user)
    .fetch_one(p)
    .await?;
    let avatars: i64 =
        sqlx::query_scalar("select count(*) from concert_avatar_settings where user_id=$1")
            .bind(user)
            .fetch_one(p)
            .await?;
    let age: i64 = sqlx::query_scalar(
        "select (select count(*) from concert_age_verifications where user_id=$1)+(select count(*) from concert_age_claims where user_id=$1)+(select count(*) from concert_age_owners where user_id=$1)",
    )
    .bind(user)
    .fetch_one(p)
    .await?;
    let media: i64 = sqlx::query_scalar(
        "select count(*) from concert_media where owner_id=$1 and state<>'DELETED'",
    )
    .bind(user)
    .fetch_one(p)
    .await?;
    let intro: i64 = sqlx::query_scalar("select count(*) from concert_intros where owner_id=$1")
        .bind(user)
        .fetch_one(p)
        .await?;
    let party: i64 = sqlx::query_scalar(
        "select case when concert_party_has_private_input($1) then 1::bigint else 0::bigint end",
    )
    .bind(user)
    .fetch_one(p)
    .await?;
    let meeting: i64 = sqlx::query_scalar(
        "select case when concert_meeting_has_private_input($1) then 1::bigint else 0::bigint end",
    )
    .bind(user)
    .fetch_one(p)
    .await?;
    Ok(core + avatars + age + media + intro + party + meeting)
}

/// Machine-readable before/after evidence for the QA09 row: per-table row
/// counts plus the canonical Auth saga steps. The mounted composer proves the
/// PrivateData owner only; these steps stay PENDING because no worker drives
/// the other seven targets in this test.
async fn deletion_probe(
    phase: &str,
    concert: &PgPool,
    auth: &PgPool,
    deleted: Uuid,
    control: Uuid,
) -> anyhow::Result<()> {
    let deleted_rows: (i64, i64, i64) = sqlx::query_as(
        "select (select count(*) from concert_participants where user_id=$1),\
                (select count(*) from concert_avatar_settings where user_id=$1),\
                (select count(*) from concert_age_owners where user_id=$1)",
    )
    .bind(deleted)
    .fetch_one(concert)
    .await?;
    let control_rows: (i64, i64, i64) = sqlx::query_as(
        "select (select count(*) from concert_participants where user_id=$1),\
                (select count(*) from concert_avatar_settings where user_id=$1),\
                (select count(*) from concert_age_owners where user_id=$1)",
    )
    .bind(control)
    .fetch_one(concert)
    .await?;
    let (saga_status, saga_completed): (String, Option<chrono::DateTime<chrono::Utc>>) =
        sqlx::query_as("select status, completed_at from user_deletion_sagas where user_id=$1")
            .bind(deleted)
            .fetch_one(auth)
            .await?;
    let steps: Vec<(String, String, i32)> = sqlx::query_as(
        "select s.target, s.status, s.attempt_count from user_deletion_steps s \
         join user_deletion_sagas g on g.id=s.saga_id where g.user_id=$1 order by s.target",
    )
    .bind(deleted)
    .fetch_all(auth)
    .await?;
    let progress: i64 = sqlx::query_scalar(
        "select count(*) from concert_private_data_owner_progress \
         where user_id=$1 and completed_at is not null",
    )
    .bind(deleted)
    .fetch_one(concert)
    .await?;
    let remaining = private_rows_remaining(concert, deleted).await?;
    println!(
        "QA09_DELETION_PROBE {}",
        serde_json::json!({
            "phase": phase,
            "deleted_user": deleted.to_string(),
            "control_user": control.to_string(),
            "deleted": {
                "participants": deleted_rows.0,
                "avatar_settings": deleted_rows.1,
                "age_owners": deleted_rows.2,
            },
            "control": {
                "participants": control_rows.0,
                "avatar_settings": control_rows.1,
                "age_owners": control_rows.2,
            },
            "private_rows_remaining": remaining,
            "saga_status": saga_status,
            "saga_completed_at": saga_completed.map(|value| value.to_rfc3339()),
            "private_data_progress_completed": progress,
            "steps": steps.iter().map(|(target, status, attempts)| serde_json::json!({
                "target": target,
                "status": status,
                "attempt_count": attempts,
            })).collect::<Vec<_>>(),
        })
    );
    Ok(())
}

/// Sharing runs before PrivateData in the canonical saga. Its durable terminal
/// row for this saga is the only evidence the aggregate accepts; this fixture
/// records that preceding step, never a fabricated PrivateData receipt.
async fn record_sharing_terminal(
    p: &PgPool,
    c: &common::WorldDeletionCommand,
) -> anyhow::Result<()> {
    let mut sharing = c.clone();
    sharing.target = common::WorldDeletionTarget::Sharing;
    let source = [0x42u8; 32];
    sqlx::query("insert into concert_social_account_deletions(saga_id,user_id,actor_id,retention_seconds,retention_nanos,identity_binding,command_binding,source_binding,fence_revision) values($1,$2,$3,$4,$5,$6,$7,$8,1)")
        .bind(c.saga_id)
        .bind(c.user_id)
        .bind(c.actor_id)
        .bind(c.retention_until.timestamp())
        .bind(c.retention_until.timestamp_subsec_nanos() as i32)
        .bind(sharing.identity_binding().map_err(|e| anyhow::anyhow!("{e:?}"))?.as_slice())
        .bind(sharing.binding().map_err(|e| anyhow::anyhow!("{e:?}"))?.as_slice())
        .bind(source.as_slice())
        .execute(p)
        .await?;
    let hex: String = source.iter().map(|x| format!("{x:02x}")).collect();
    let mut tx = p.begin().await?;
    sqlx::query("select set_config('mcr.social_deletion_saga',$1,true),set_config('mcr.social_deletion_source',$2,true),set_config('mcr.social_deletion_until',(clock_timestamp()+interval '4 seconds')::text,true)")
        .bind(c.saga_id.to_string())
        .bind(hex)
        .execute(&mut *tx)
        .await?;
    sqlx::query("update concert_social_account_deletions set completed_at=clock_timestamp(),progress_revision=progress_revision+1 where saga_id=$1 and completed_at is null")
        .bind(c.saga_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

fn private_command(
    g: &UserDeletionSaga,
) -> auth_service::application::concert_deletion::ConcertDeletionCommand {
    auth_service::application::concert_deletion::ConcertDeletionCommand {
        key: auth_service::application::concert_deletion::ConcertDeletionKey {
            saga_id: g.id,
            user_id: g.user_id,
            target: auth_service::application::concert_deletion::ConcertDeletionTarget::PrivateData,
        },
        actor_id: g.actor_id,
        retention_until: g.retention_until,
    }
}

async fn private_rounds(
    client: &ConcertDeletionClient,
    command: &auth_service::application::concert_deletion::ConcertDeletionCommand,
) -> anyhow::Result<Option<auth_service::application::concert_deletion::ConcertDeletionAcknowledgement>>
{
    use auth_service::application::concert_deletion::ConcertDeletionFailure;
    let mut ack = None;
    for _ in 0..=8 {
        match client
            .apply_with_deadline(
                command.clone(),
                tokio::time::Instant::now() + Duration::from_secs(4),
            )
            .await
        {
            Ok(whole) => {
                ack = Some(whole);
                break;
            }
            Err(ConcertDeletionFailure::Incomplete) => continue,
            Err(other) => anyhow::bail!("PrivateData apply failed before completion: {other:?}"),
        }
    }
    Ok(ack)
}

#[tokio::test]
#[ignore = "fresh PG16/PKI allocation; mounted mTLS factory, PrivateData ACK, restart and caller refusal"]
async fn mounted_private_data_ack_restart_and_session_revocation_boundary() -> anyhow::Result<()> {
    let auth = environment::pool("auth").await?;
    let concert = environment::pool("concert").await?;
    let source = source_server(auth.clone()).await?;
    let rpc = factory_server(
        concert.clone(),
        &source.url,
        Workload::Concert,
        LiveDeletionOwners::default(),
    )
    .await?;
    let s = state(auth.clone(), &rpc.url, "https://127.0.0.1:1", 5000).await?;
    // Auth workload material authenticates the caller; the endpoint check still
    // requires the Concert server identity.
    let tls = environment::tls(Workload::Auth)?;
    let client = ConcertDeletionClient::connect(&rpc.url, &tls, AUTH)?;

    let g = begin(&s, &auth).await?;
    let control = Uuid::new_v4();
    seed_private_rows(&concert, g.user_id, control).await?;
    assert!(private_rows_remaining(&concert, g.user_id).await? >= 3);
    deletion_probe("before", &concert, &auth, g.user_id, control).await?;
    let command = private_command(&g);
    // A bounded round either seals the whole acknowledgement already or
    // establishes the common fence and stops incomplete. Only the latter needs
    // the preceding Sharing terminal, which the canonical saga records first.
    let mut whole = match client
        .apply_with_deadline(
            command.clone(),
            tokio::time::Instant::now() + Duration::from_secs(4),
        )
        .await
    {
        Ok(whole) => Some(whole),
        Err(auth_service::application::concert_deletion::ConcertDeletionFailure::Incomplete) => {
            None
        }
        Err(other) => anyhow::bail!("first bounded PrivateData round failed: {other:?}"),
    };
    if whole.is_none() {
        record_sharing_terminal(
            &concert,
            &common::WorldDeletionCommand {
                saga_id: g.id,
                user_id: g.user_id,
                actor_id: g.actor_id,
                target: common::WorldDeletionTarget::PrivateData,
                retention_until: g.retention_until,
            },
        )
        .await?;
        whole = private_rounds(&client, &command).await?;
    }
    let ack = whole.ok_or_else(|| anyhow::anyhow!("mounted PrivateData Apply never sealed an ACK"))?;
    assert_eq!(ack.command, command);
    assert!(ack.fence_revision > 0);
    assert_eq!(private_rows_remaining(&concert, g.user_id).await?, 0);
    let control_theme: String =
        sqlx::query_scalar("select theme from concert_participants where user_id=$1")
            .bind(control)
            .fetch_one(&concert)
            .await?;
    assert_eq!(control_theme, "DANGER");

    // A caller whose credential is not the configured Auth workload is rejected
    // before the source or any deleter is reached.
    let revoked = begin(&s, &auth).await?;
    let revoked_control = Uuid::new_v4();
    seed_private_rows(&concert, revoked.user_id, revoked_control).await?;
    let rejected = ConcertDeletionClient::connect(&rpc.url, &tls, "revoked-auth")?;
    assert!(rejected
        .apply_with_deadline(
            private_command(&revoked),
            tokio::time::Instant::now() + Duration::from_secs(4)
        )
        .await
        .is_err());
    let reached: i64 = sqlx::query_scalar(
        "select (select count(*) from concert_private_data_owner_progress where user_id=$1)+(select count(*) from concert_world_account_deletion_fences where user_id=$1)",
    )
    .bind(revoked.user_id)
    .fetch_one(&concert)
    .await?;
    assert_eq!(reached, 0, "rejected caller reached the deleter");
    assert!(private_rows_remaining(&concert, revoked.user_id).await? >= 3);

    // The schema and the canonical Auth source require every session to be
    // revoked before a deletion source is granted; no live token may exist.
    let live_tokens: i64 = sqlx::query_scalar(
        "select count(*) from refresh_tokens where user_id=$1 and revoked_at is null",
    )
    .bind(revoked.user_id)
    .fetch_one(&auth)
    .await?;
    assert_eq!(live_tokens, 0, "deleted user retains a live session");
    // A tampered retention is refused by the canonical Auth source before any
    // deleter runs.
    let mut tampered = private_command(&revoked);
    tampered.retention_until += chrono::Duration::microseconds(1);
    assert!(client
        .apply_with_deadline(
            tampered,
            tokio::time::Instant::now() + Duration::from_secs(4)
        )
        .await
        .is_err());
    let reached: i64 = sqlx::query_scalar(
        "select (select count(*) from concert_private_data_owner_progress where user_id=$1)+(select count(*) from concert_world_account_deletion_fences where user_id=$1)",
    )
    .bind(revoked.user_id)
    .fetch_one(&concert)
    .await?;
    assert_eq!(reached, 0, "source-refused request reached the deleter");

    // Service restart: a fresh composed factory replays the committed receipt.
    let receipt_before: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
        "select completed_at from concert_private_data_owner_progress where user_id=$1",
    )
    .bind(g.user_id)
    .fetch_one(&concert)
    .await?;
    rpc.finish().await?;
    let restarted = factory_server(
        concert.clone(),
        &source.url,
        Workload::Concert,
        LiveDeletionOwners::default(),
    )
    .await?;
    let replay = ConcertDeletionClient::connect(&restarted.url, &tls, AUTH)?;
    let again = replay
        .apply_with_deadline(
            command.clone(),
            tokio::time::Instant::now() + Duration::from_secs(4),
        )
        .await?;
    assert_eq!(again, ack);
    let receipt_after: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
        "select completed_at from concert_private_data_owner_progress where user_id=$1",
    )
    .bind(g.user_id)
    .fetch_one(&concert)
    .await?;
    assert_eq!(
        receipt_before, receipt_after,
        "restart rewrote the terminal receipt"
    );
    assert_eq!(private_rows_remaining(&concert, g.user_id).await?, 0);
    deletion_probe("after", &concert, &auth, g.user_id, control).await?;
    restarted.finish().await?;
    source.finish().await?;
    auth.close().await;
    concert.close().await;
    println!("MOUNTED DELETION PASS: sealed PrivateData ACK over mTLS, caller refusal before deleter, zero unrevoked sessions, tampered source refusal, restart replay without duplicate effects, zero private rows, control intact");
    Ok(())
}
