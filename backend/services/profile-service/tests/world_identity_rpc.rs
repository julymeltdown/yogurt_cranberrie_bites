//! Generated RPC through real caller/delegation middleware; native DB is opt-in.
#[path = "support/world_identity/mod.rs"]
mod fixture;
use fixture::*;
use platform::{actor_delegation::ActorDelegationSigner, grpc_auth::Workload};
use prost::Message;
use std::{sync::atomic::Ordering, time::Duration};
use tonic::Code;
use uuid::Uuid;

#[tokio::test]
async fn signed_concert_self_exposes_exact_minimal_current_content() -> anyhow::Result<()> {
    let (mut s, owner) = controlled(Mode::Success).await?;
    let actor = Uuid::new_v4();
    let response = call(&mut s, message(actor)).await?;
    assert_eq!(
        response.metadata().get("cache-control").unwrap(),
        "no-store"
    );
    let b = response.into_inner();
    assert_eq!(b.subject_id, actor.to_string());
    assert_eq!(b.display_name, "Mira");
    assert!(b.observed_at_unix_micros > 0);
    assert!(!String::from_utf8_lossy(&b.encode_to_vec()).contains("private-bio-canary"));
    assert_eq!(owner.calls.load(Ordering::SeqCst), 1);
    s.stop().await
}
#[tokio::test]
async fn caller_proof_domains_and_all_signed_scope_fields_are_enforced() -> anyhow::Result<()> {
    let (mut s, owner) = controlled(Mode::Success).await?;
    let actor = Uuid::new_v4();
    let msg = message(actor);
    for (caller, credential, code) in [
        ("concert", "wrong", Code::Unauthenticated),
        ("gateway", "world-profile-gateway", Code::PermissionDenied),
        ("auth", "world-profile-auth", Code::PermissionDenied),
        ("friend", "world-profile-friend", Code::PermissionDenied),
        ("post", "world-profile-post", Code::PermissionDenied),
    ] {
        let p = token(&msg);
        let mut r = request(msg.clone(), Some(&p), Some("2S"));
        r.metadata_mut().insert("x-service-caller", caller.parse()?);
        r.metadata_mut()
            .insert("x-service-token", credential.parse()?);
        assert_eq!(s.client.read_current(r).await.unwrap_err().code(), code);
    }
    let now = chrono::Utc::now().timestamp();
    let bytes = frame(&msg);
    for proof in [
        None,
        Some("forged-private-proof".into()),
        Some(
            ActorDelegationSigner::from_seed(Workload::Concert, &[99; 32])?.issue(
                Workload::Profile,
                METHOD,
                actor,
                None,
                &bytes,
                now,
            ),
        ),
        Some(
            ActorDelegationSigner::from_seed(Workload::Gateway, &[41; 32])?.issue(
                Workload::Profile,
                METHOD,
                actor,
                None,
                &bytes,
                now,
            ),
        ),
        Some(signer().issue(Workload::Auth, METHOD, actor, None, &bytes, now)),
        Some(signer().issue(
            Workload::Profile,
            "/profile.v1.ProfileService/ReadConcertSelfProfile",
            actor,
            None,
            &bytes,
            now,
        )),
        Some(signer().issue(Workload::Profile, METHOD, Uuid::new_v4(), None, &bytes, now)),
        Some(signer().issue(Workload::Profile, METHOD, actor, None, &bytes, now - 120)),
    ] {
        let e = s
            .client
            .read_current(request(msg.clone(), proof.as_deref(), Some("2S")))
            .await
            .unwrap_err();
        assert!(matches!(
            e.code(),
            Code::Unauthenticated | Code::PermissionDenied
        ));
        assert!(!e.message().contains("private"));
    }
    // Every scope dimension belongs to the signed body, including time and intent.
    for field in 0..7 {
        let p = token(&msg);
        let mut changed = msg.clone();
        match field {
            0 => changed.actor_id = Uuid::new_v4().to_string(),
            1 => changed.event_id = Uuid::new_v4().to_string(),
            2 => changed.subject_id = Uuid::new_v4().to_string(),
            3 => changed.session_family_id = Uuid::new_v4().to_string(),
            4 => changed.request_id = Uuid::new_v4().to_string(),
            5 => changed.purpose = 0,
            _ => changed.expires_unix_micros += 1,
        }
        let code = s
            .client
            .read_current(request(changed, Some(&p), Some("2S")))
            .await
            .unwrap_err()
            .code();
        assert!(matches!(
            code,
            Code::Unauthenticated | Code::PermissionDenied
        ));
    }
    assert_eq!(owner.calls.load(Ordering::SeqCst), 0);
    s.stop().await
}
#[tokio::test]
async fn signed_wrong_scope_and_expired_or_missing_deadline_do_not_read_owner() -> anyhow::Result<()>
{
    let (mut s, owner) = controlled(Mode::Success).await?;
    let actor = Uuid::new_v4();
    for field in 0..7 {
        let mut m = message(actor);
        match field {
            0 => m.subject_id = Uuid::new_v4().to_string(),
            1 => m.event_id = Uuid::nil().to_string(),
            2 => m.session_family_id = Uuid::new_v4().simple().to_string(),
            3 => m.request_id = "invalid".into(),
            4 => m.purpose = 0,
            5 => m.purpose = 2,
            _ => m.expires_unix_micros = chrono::Utc::now().timestamp_micros() - 1,
        }
        let e = call(&mut s, m).await.unwrap_err();
        assert!(matches!(
            e.code(),
            Code::PermissionDenied | Code::InvalidArgument | Code::DeadlineExceeded
        ));
    }
    let m = message(actor);
    let p = token(&m);
    assert_eq!(
        s.client
            .read_current(request(m, Some(&p), None))
            .await
            .unwrap_err()
            .code(),
        Code::InvalidArgument
    );
    assert_eq!(owner.calls.load(Ordering::SeqCst), 0);
    s.stop().await
}
#[tokio::test]
async fn proof_replay_and_constructed_extensions_cannot_replace_guard() -> anyhow::Result<()> {
    use proto::concert_world_profile_service_server::ConcertWorldProfileService;
    let (mut s, owner) = controlled(Mode::Success).await?;
    let m = message(Uuid::new_v4());
    let p = token(&m);
    s.client
        .read_current(request(m.clone(), Some(&p), Some("2S")))
        .await?;
    assert_eq!(
        s.client
            .read_current(request(m.clone(), Some(&p), Some("2S")))
            .await
            .unwrap_err()
            .code(),
        Code::Unauthenticated
    );
    let mut r = tonic::Request::new(m.clone());
    r.extensions_mut()
        .insert(platform::grpc_auth::VerifiedCaller(Workload::Concert));
    r.extensions_mut()
        .insert(platform::actor_delegation::VerifiedActor(Uuid::parse_str(
            &m.actor_id,
        )?));
    assert_eq!(
        WorldProfileServer::new(owner.clone())
            .read_current(r)
            .await
            .unwrap_err()
            .code(),
        Code::Unauthenticated
    );
    assert_eq!(owner.calls.load(Ordering::SeqCst), 1);
    s.stop().await
}
#[tokio::test]
async fn missing_invalid_misbound_stale_and_owner_errors_fail_closed() -> anyhow::Result<()> {
    let (mut s, owner) = controlled(Mode::Success).await?;
    for (mode, code) in [
        (Mode::Missing, Code::NotFound),
        (Mode::Malformed, Code::NotFound),
        (Mode::Misbound, Code::NotFound),
        (Mode::Stale, Code::Unavailable),
        (Mode::Future, Code::Unavailable),
        (Mode::Error, Code::Unavailable),
    ] {
        *owner.mode.lock().unwrap() = mode;
        let e = call(&mut s, message(Uuid::new_v4())).await.unwrap_err();
        assert_eq!(e.code(), code);
        assert!(!e.message().contains("canary"));
    }
    s.stop().await
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn inherited_deadline_drops_pending_owner_and_rejects_noncooperative_completion(
) -> anyhow::Result<()> {
    let (mut s, owner) = controlled(Mode::Hold).await?;
    let mut m = message(Uuid::new_v4());
    m.expires_unix_micros = chrono::Utc::now().timestamp_micros() + 70_000;
    let start = std::time::Instant::now();
    assert_eq!(
        call(&mut s, m).await.unwrap_err().code(),
        Code::DeadlineExceeded
    );
    assert!(start.elapsed() < Duration::from_millis(500));
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert_eq!(owner.dropped.load(Ordering::SeqCst), 1);
    assert_eq!(owner.completed.load(Ordering::SeqCst), 0);
    *owner.mode.lock().unwrap() = Mode::Block(Duration::from_millis(100));
    let mut m = message(Uuid::new_v4());
    m.expires_unix_micros = chrono::Utc::now().timestamp_micros() + 30_000;
    assert_eq!(
        call(&mut s, m).await.unwrap_err().code(),
        Code::DeadlineExceeded
    );
    s.stop().await
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn transport_cancellation_drops_inflight_owner() -> anyhow::Result<()> {
    let (s, owner) = controlled(Mode::Hold).await?;
    let mut client = s.client.clone();
    let m = message(Uuid::new_v4());
    let p = token(&m);
    let task =
        tokio::spawn(async move { client.read_current(request(m, Some(&p), Some("2S"))).await });
    tokio::time::timeout(Duration::from_millis(500), async {
        while owner.calls.load(Ordering::SeqCst) == 0 {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await?;
    task.abort();
    let _ = task.await;
    tokio::time::timeout(Duration::from_millis(500), async {
        while owner.dropped.load(Ordering::SeqCst) == 0 {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await?;
    assert_eq!(owner.completed.load(Ordering::SeqCst), 0);
    s.stop().await
}

#[tokio::test]
#[ignore = "requires explicitly owned native PostgreSQL controller"]
async fn native_world_profile_current_private_content_and_fences() -> anyhow::Result<()> {
    let pool = owner_pool().await?;
    let reader = reader_pool().await?;
    let mut s = Running::start(WorldProfileServer::from_pool(reader.clone())).await?;
    let actor = seed(&pool, "Mira 音楽 🫖", "private-bio-canary").await?;
    let b = call(&mut s, message(actor)).await?.into_inner();
    assert_eq!(b.display_name, "Mira 音楽 🫖");
    assert_eq!(b.subject_id, actor.to_string());
    let wire = String::from_utf8_lossy(&b.encode_to_vec()).to_string();
    for private in [
        "private-bio-canary",
        "private-handle-",
        "legacy-token-canary",
        "private-delete-canary",
    ] {
        assert!(!wire.contains(private));
    }
    sqlx::query("update profiles set nickname='Renamed' where user_id=$1")
        .bind(actor)
        .execute(&pool)
        .await?;
    assert_eq!(
        call(&mut s, message(actor))
            .await?
            .into_inner()
            .display_name,
        "Renamed"
    );
    let mut tx = pool.begin().await?;
    sqlx::query("update profiles set nickname='Uncommitted' where user_id=$1")
        .bind(actor)
        .execute(&mut *tx)
        .await?;
    assert_eq!(
        call(&mut s, message(actor))
            .await?
            .into_inner()
            .display_name,
        "Renamed"
    );
    tx.commit().await?;
    assert_eq!(
        call(&mut s, message(actor))
            .await?
            .into_inner()
            .display_name,
        "Uncommitted"
    );
    sqlx::query("update profiles set deleted_at=now() where user_id=$1")
        .bind(actor)
        .execute(&pool)
        .await?;
    assert_eq!(
        call(&mut s, message(actor)).await.unwrap_err().code(),
        Code::NotFound
    );
    let other = seed(&pool, "Other", "").await?;
    sqlx::query("insert into profile_user_deletion_tombstones(user_id,saga_id,actor_id,retention_until) values($1,$2,$3,now()+interval '1 day')").bind(other).bind(Uuid::new_v4()).bind(Uuid::new_v4()).execute(&pool).await?;
    assert_eq!(
        call(&mut s, message(other)).await.unwrap_err().code(),
        Code::NotFound
    );
    assert_eq!(
        call(&mut s, message(Uuid::new_v4()))
            .await
            .unwrap_err()
            .code(),
        Code::NotFound
    );
    s.stop().await?;
    reader.close().await;
    pool.close().await;
    Ok(())
}
#[tokio::test]
#[ignore = "requires explicitly owned native PostgreSQL controller"]
async fn native_world_profile_locked_owner_preserves_deadline_and_recovers() -> anyhow::Result<()> {
    let pool = owner_pool().await?;
    let reader = reader_pool().await?;
    let mut s = Running::start(WorldProfileServer::from_pool(reader.clone())).await?;
    let actor = seed(&pool, "Locked", "").await?;
    let mut tx = pool.begin().await?;
    sqlx::query("lock table profiles in access exclusive mode")
        .execute(&mut *tx)
        .await?;
    let mut m = message(actor);
    m.expires_unix_micros = chrono::Utc::now().timestamp_micros() + 100_000;
    let start = std::time::Instant::now();
    let e = call(&mut s, m).await.unwrap_err();
    assert!(matches!(
        e.code(),
        Code::DeadlineExceeded | Code::Unavailable
    ));
    assert!(start.elapsed() < Duration::from_millis(800));
    tx.rollback().await?;
    assert_eq!(
        call(&mut s, message(actor))
            .await?
            .into_inner()
            .display_name,
        "Locked"
    );
    s.stop().await?;
    reader.close().await;
    pool.close().await;
    Ok(())
}
#[tokio::test]
#[ignore = "requires explicitly owned native PostgreSQL controller"]
async fn native_world_profile_permission_denied_is_sanitized() -> anyhow::Result<()> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .after_connect(|conn, _| {
            Box::pin(async move {
                sqlx::query("set role mcr_profile_rpc_noaccess")
                    .execute(conn)
                    .await?;
                Ok(())
            })
        })
        .connect_with(owned_options()?)
        .await?;
    let mut s = Running::start(WorldProfileServer::from_pool(pool.clone())).await?;
    let e = call(&mut s, message(Uuid::new_v4())).await.unwrap_err();
    assert_eq!(e.code(), Code::Unavailable);
    assert_eq!(e.message(), "World Profile owner unavailable");
    s.stop().await?;
    pool.close().await;
    Ok(())
}
#[tokio::test]
#[ignore = "requires explicitly owned native PostgreSQL controller"]
async fn native_world_profile_stopped_database_is_sanitized() -> anyhow::Result<()> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_millis(100))
        .connect_lazy_with(owned_options()?);
    let mut s = Running::start(WorldProfileServer::from_pool(pool.clone())).await?;
    let e = call(&mut s, message(Uuid::new_v4())).await.unwrap_err();
    assert_eq!(e.code(), Code::Unavailable);
    assert_eq!(e.message(), "World Profile owner unavailable");
    s.stop().await?;
    pool.close().await;
    Ok(())
}
