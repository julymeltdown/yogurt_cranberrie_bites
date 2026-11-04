//! Dedicated public RPC through actual signature/caller/deadline middleware.
#[path = "support/public_profile/mod.rs"]
mod fixture;
use fixture::*;
use prost::Message;
use std::{sync::atomic::Ordering, time::Duration};
use tonic::Code;
use uuid::Uuid;

#[tokio::test]
async fn arbitrary_subject_is_not_self_and_only_public_owner_fields_cross_rpc() -> anyhow::Result<()>
{
    let (mut s, owner) = controlled(Mode::Success).await?;
    let m = message(Uuid::new_v4());
    assert_ne!(m.actor_id, m.subject_id);
    let r = call(&mut s, m.clone()).await?;
    assert_eq!(r.metadata().get("cache-control").unwrap(), "no-store");
    let b = r.into_inner();
    assert_eq!(b.subject_id, m.subject_id);
    assert_eq!(b.actor_id, m.actor_id);
    assert_eq!(b.event_id, m.event_id);
    assert_eq!(b.session_family_id, m.session_family_id);
    assert_eq!(b.request_id, m.request_id);
    assert_eq!(b.display_name, "Mira");
    assert_eq!(b.bio.as_deref(), Some("Public bio 音楽"));
    assert!(b.observed_at_unix_micros > 0);
    assert_eq!(owner.calls.load(Ordering::SeqCst), 1);
    s.stop().await
}
#[tokio::test]
async fn signed_scope_mutation_and_wrong_purpose_are_denied_before_owner() -> anyhow::Result<()> {
    let (mut s, owner) = controlled(Mode::Success).await?;
    let m = message(Uuid::new_v4());
    for field in 0..7 {
        let proof = token(&m);
        let mut changed = m.clone();
        match field {
            0 => changed.actor_id = Uuid::new_v4().to_string(),
            1 => changed.event_id = Uuid::new_v4().to_string(),
            2 => changed.subject_id = Uuid::new_v4().to_string(),
            3 => changed.session_family_id = Uuid::new_v4().to_string(),
            4 => changed.request_id = Uuid::new_v4().to_string(),
            5 => changed.purpose = 2,
            _ => changed.expires_unix_micros += 1,
        }
        let e = s
            .client
            .read_current(request(changed, Some(&proof), Some("2S")))
            .await
            .unwrap_err();
        assert!(matches!(
            e.code(),
            Code::PermissionDenied | Code::Unauthenticated
        ));
    }
    for field in 0..5 {
        let mut changed = m.clone();
        match field {
            0 => changed.purpose = 0,
            1 => changed.purpose = 2,
            2 => changed.event_id = Uuid::nil().to_string(),
            3 => changed.subject_id = Uuid::new_v4().simple().to_string(),
            _ => changed.request_id = "invalid".into(),
        }
        let e = call(&mut s, changed).await.unwrap_err();
        assert!(matches!(
            e.code(),
            Code::PermissionDenied | Code::InvalidArgument
        ));
    }
    assert_eq!(owner.calls.load(Ordering::SeqCst), 0);
    s.stop().await
}
#[tokio::test]
async fn proof_caller_audience_method_replay_and_constructed_extensions_do_not_bypass(
) -> anyhow::Result<()> {
    use platform::{actor_delegation::ActorDelegationSigner, grpc_auth::Workload};
    use proto::concert_public_profile_service_server::ConcertPublicProfileService;
    let (mut s, owner) = controlled(Mode::Success).await?;
    let m = message(Uuid::new_v4());
    let actor = Uuid::parse_str(&m.actor_id)?;
    let now = chrono::Utc::now().timestamp();
    for p in [
        None,
        Some("forged".to_string()),
        Some(signer().issue(Workload::Auth, METHOD, actor, None, &frame(&m), now)),
        Some(signer().issue(
            Workload::Profile,
            "/profile.v1.ConcertWorldProfileService/ReadCurrent",
            actor,
            None,
            &frame(&m),
            now,
        )),
        Some(signer().issue(
            Workload::Profile,
            METHOD,
            Uuid::new_v4(),
            None,
            &frame(&m),
            now,
        )),
        Some(signer().issue(
            Workload::Profile,
            METHOD,
            actor,
            None,
            &frame(&m),
            now - 120,
        )),
        Some(
            ActorDelegationSigner::from_seed(Workload::Gateway, &[41; 32])?.issue(
                Workload::Profile,
                METHOD,
                actor,
                None,
                &frame(&m),
                now,
            ),
        ),
    ] {
        let e = s
            .client
            .read_current(request(m.clone(), p.as_deref(), Some("2S")))
            .await
            .unwrap_err();
        assert!(matches!(
            e.code(),
            Code::Unauthenticated | Code::PermissionDenied
        ));
    }
    let p = token(&m);
    let mut wrong = request(m.clone(), Some(&p), Some("2S"));
    wrong
        .metadata_mut()
        .insert("x-service-caller", "gateway".parse()?);
    wrong
        .metadata_mut()
        .insert("x-service-token", "public-profile-gateway".parse()?);
    assert_eq!(
        s.client.read_current(wrong).await.unwrap_err().code(),
        Code::PermissionDenied
    );
    let p = token(&m);
    s.client
        .read_current(request(m.clone(), Some(&p), Some("2S")))
        .await?;
    assert!(s
        .client
        .read_current(request(m.clone(), Some(&p), Some("2S")))
        .await
        .is_err());
    let mut unguarded = tonic::Request::new(m);
    unguarded
        .extensions_mut()
        .insert(platform::grpc_auth::VerifiedCaller(Workload::Concert));
    unguarded
        .extensions_mut()
        .insert(platform::actor_delegation::VerifiedActor(actor));
    assert_eq!(
        PublicProfileServer::new(owner.clone())
            .read_current(unguarded)
            .await
            .unwrap_err()
            .code(),
        Code::Unauthenticated
    );
    assert_eq!(owner.calls.load(Ordering::SeqCst), 1);
    s.stop().await
}
#[tokio::test]
async fn malformed_missing_misbound_stale_future_and_owner_failures_remain_closed(
) -> anyhow::Result<()> {
    let (mut s, owner) = controlled(Mode::Success).await?;
    for (mode, code) in [
        (Mode::Missing, Code::NotFound),
        (Mode::Malformed, Code::Unavailable),
        (Mode::Misbound, Code::Unavailable),
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
async fn original_deadline_and_transport_cancel_drop_owner_future() -> anyhow::Result<()> {
    let (mut s, owner) = controlled(Mode::Hold).await?;
    let mut m = message(Uuid::new_v4());
    m.expires_unix_micros = chrono::Utc::now().timestamp_micros() + 70_000;
    let start = std::time::Instant::now();
    assert_eq!(
        call(&mut s, m).await.unwrap_err().code(),
        Code::DeadlineExceeded
    );
    assert!(start.elapsed() < Duration::from_millis(500));
    tokio::time::timeout(Duration::from_millis(500), async {
        while owner.dropped.load(Ordering::SeqCst) < 1 {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    let mut client = s.client.clone();
    let m = message(Uuid::new_v4());
    let p = token(&m);
    let task =
        tokio::spawn(async move { client.read_current(request(m, Some(&p), Some("2S"))).await });
    tokio::time::timeout(Duration::from_millis(500), async {
        while owner.calls.load(Ordering::SeqCst) < 2 {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    task.abort();
    let _ = task.await;
    tokio::time::timeout(Duration::from_millis(500), async {
        while owner.dropped.load(Ordering::SeqCst) < 2 {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    assert_eq!(owner.completed.load(Ordering::SeqCst), 0);
    *owner.mode.lock().unwrap() = Mode::Success;
    let m = message(Uuid::new_v4());
    let p = token(&m);
    assert_eq!(
        s.client
            .read_current(request(m, Some(&p), None))
            .await
            .unwrap_err()
            .code(),
        Code::InvalidArgument
    );
    s.stop().await
}
#[tokio::test]
async fn child_check_is_read_only_and_only_exact_completion_secret_can_release(
) -> anyhow::Result<()> {
    let (mut s, owner) = controlled(Mode::Success).await?;
    let m = message(Uuid::new_v4());
    let b = call(&mut s, m.clone()).await?.into_inner();
    let check = "/profile.concert_public.v1.ConcertPublicProfileService/Check";
    let complete = "/profile.concert_public.v1.ConcertPublicProfileService/Complete";
    assert!(
        s.client
            .check(control_request(control(&m, &b, false), check))
            .await?
            .into_inner()
            .held
    );
    let mut wrong = control(&m, &b, true);
    wrong.completion_secret = Uuid::new_v4().simple().to_string();
    assert!(s
        .client
        .complete(control_request(wrong, complete))
        .await
        .is_err());
    let mut wrong = control(&m, &b, false);
    wrong.event_id = Uuid::new_v4().to_string();
    assert!(s.client.check(control_request(wrong, check)).await.is_err());
    assert_eq!(owner.held.lock().unwrap().len(), 1);
    finish(&mut s, &m, &b).await?;
    assert!(s
        .client
        .check(control_request(control(&m, &b, false), check))
        .await
        .is_err());
    assert!(owner.held.lock().unwrap().is_empty());
    s.stop().await
}
#[tokio::test]
#[ignore = "requires explicitly owned native PostgreSQL controller"]
async fn native_current_public_privacy_tombstones_and_minimal_wire() -> anyhow::Result<()> {
    let pool = owner_pool().await?;
    let subject = seed(&pool, "Canonical 音楽", "Public biography").await?;
    let mut s = Running::start(PublicProfileServer::from_pool(pool.clone())).await?;
    let make = || {
        let mut m = message(Uuid::new_v4());
        m.subject_id = subject.to_string();
        m
    };
    assert_eq!(
        call(&mut s, make()).await.unwrap_err().code(),
        Code::NotFound
    );
    sqlx::query("update profiles set account_visibility='PUBLIC' where user_id=$1")
        .bind(subject)
        .execute(&pool)
        .await?;
    let m = make();
    let b = call(&mut s, m.clone()).await?.into_inner();
    assert_eq!(b.display_name, "Canonical 音楽");
    assert_eq!(b.bio.as_deref(), Some("Public biography"));
    let wire = String::from_utf8_lossy(&b.encode_to_vec()).to_string();
    for private in [
        "private-handle-",
        "legacy-token-canary",
        "private-delete-canary",
    ] {
        assert!(!wire.contains(private));
    }
    finish(&mut s, &m, &b).await?;
    sqlx::query("update profiles set nickname='Changed',bio='' where user_id=$1")
        .bind(subject)
        .execute(&pool)
        .await?;
    let m = make();
    let b = call(&mut s, m.clone()).await?.into_inner();
    assert_eq!(b.display_name, "Changed");
    assert!(b.bio.is_none());
    finish(&mut s, &m, &b).await?;
    sqlx::query("update profiles set account_visibility='PRIVATE' where user_id=$1")
        .bind(subject)
        .execute(&pool)
        .await?;
    assert_eq!(
        call(&mut s, make()).await.unwrap_err().code(),
        Code::NotFound
    );
    sqlx::query(
        "update profiles set account_visibility='PUBLIC',deleted_at=now() where user_id=$1",
    )
    .bind(subject)
    .execute(&pool)
    .await?;
    assert_eq!(
        call(&mut s, make()).await.unwrap_err().code(),
        Code::NotFound
    );
    let tomb = seed(&pool, "Tombstoned", "").await?;
    sqlx::query("update profiles set account_visibility='PUBLIC' where user_id=$1")
        .bind(tomb)
        .execute(&pool)
        .await?;
    sqlx::query("insert into profile_user_deletion_tombstones(user_id,saga_id,actor_id,retention_until) values($1,$2,$3,now()+interval '1 day')").bind(tomb).bind(Uuid::new_v4()).bind(Uuid::new_v4()).execute(&pool).await?;
    let mut m = make();
    m.subject_id = tomb.to_string();
    assert_eq!(call(&mut s, m).await.unwrap_err().code(), Code::NotFound);
    s.stop().await?;
    pool.close().await;
    Ok(())
}
#[tokio::test]
#[ignore = "requires explicitly owned native PostgreSQL controller"]
async fn native_prepared_child_excludes_privacy_and_direct_tombstone_until_settled(
) -> anyhow::Result<()> {
    let pool = owner_pool().await?;
    let subject = seed(&pool, "Held owner", "").await?;
    sqlx::query("update profiles set account_visibility='PUBLIC' where user_id=$1")
        .bind(subject)
        .execute(&pool)
        .await?;
    let mut s = Running::start(PublicProfileServer::from_pool(pool.clone())).await?;
    let mut m = message(Uuid::new_v4());
    m.subject_id = subject.to_string();
    let b = call(&mut s, m.clone()).await?.into_inner();
    for direct_tombstone in [false, true] {
        let mut tx = pool.begin().await?;
        sqlx::query("set local statement_timeout='100ms'")
            .execute(&mut *tx)
            .await?;
        let blocked = if direct_tombstone {
            sqlx::query("insert into profile_user_deletion_tombstones(user_id,saga_id,actor_id,retention_until) values($1,$2,$3,now()+interval '1 day')").bind(subject).bind(Uuid::new_v4()).bind(Uuid::new_v4()).execute(&mut *tx).await.is_err()
        } else {
            sqlx::query("update profiles set account_visibility='PRIVATE' where user_id=$1")
                .bind(subject)
                .execute(&mut *tx)
                .await
                .is_err()
        };
        assert!(blocked);
        tx.rollback().await?;
    }
    // Owner service shutdown does not release a prepared child.
    s.stop().await?;
    let mut s = Running::start(PublicProfileServer::from_pool(pool.clone())).await?;
    let check = "/profile.concert_public.v1.ConcertPublicProfileService/Check";
    assert!(
        s.client
            .check(control_request(control(&m, &b, false), check))
            .await?
            .into_inner()
            .held
    );
    finish(&mut s, &m, &b).await?;
    sqlx::query("update profiles set account_visibility='PRIVATE' where user_id=$1")
        .bind(subject)
        .execute(&pool)
        .await?;
    let mut denied = m.clone();
    denied.expires_unix_micros = chrono::Utc::now().timestamp_micros() + 1_900_000;
    assert_eq!(
        call(&mut s, denied).await.unwrap_err().code(),
        Code::NotFound
    );
    s.stop().await?;
    pool.close().await;
    Ok(())
}
#[tokio::test]
#[ignore = "requires explicitly owned native PostgreSQL controller"]
async fn native_prepare_restart_quarantine() -> anyhow::Result<()> {
    let pool = owner_pool().await?;
    let subject = seed(&pool, "Restart held", "").await?;
    sqlx::query("update profiles set account_visibility='PUBLIC' where user_id=$1")
        .bind(subject)
        .execute(&pool)
        .await?;
    let mut s = Running::start(PublicProfileServer::from_pool(pool.clone())).await?;
    let mut m = message(Uuid::new_v4());
    m.subject_id = subject.to_string();
    let b = call(&mut s, m.clone()).await?.into_inner();
    let path = std::path::PathBuf::from(std::env::var("MCR_PUBLIC_PROFILE_RESTART_FILE")?);
    let content = serde_json::json!({"actor":m.actor_id,"event":m.event_id,"subject":m.subject_id,"family":m.session_family_id,"request":m.request_id,"child":b.child_id,"secret":b.completion_secret});
    use std::{io::Write, os::unix::fs::OpenOptionsExt};
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(serde_json::to_string(&content)?.as_bytes())?;
    s.stop().await?;
    pool.close().await;
    Ok(())
}
#[tokio::test]
#[ignore = "requires preceding restart phase in owned native controller"]
async fn native_settle_after_database_restart_and_original_expiry() -> anyhow::Result<()> {
    let pool = owner_pool().await?;
    let bytes = std::fs::read(std::env::var("MCR_PUBLIC_PROFILE_RESTART_FILE")?)?;
    let v: serde_json::Value = serde_json::from_slice(&bytes)?;
    let mut m = message(Uuid::parse_str(v["actor"].as_str().unwrap())?);
    m.event_id = v["event"].as_str().unwrap().into();
    m.subject_id = v["subject"].as_str().unwrap().into();
    m.session_family_id = v["family"].as_str().unwrap().into();
    m.request_id = v["request"].as_str().unwrap().into();
    m.expires_unix_micros = chrono::Utc::now().timestamp_micros() - 1;
    let b = Reply {
        child_id: v["child"].as_str().unwrap().into(),
        completion_secret: v["secret"].as_str().unwrap().into(),
        ..Default::default()
    };
    let mut s = Running::start(PublicProfileServer::from_pool(pool.clone())).await?;
    assert!(s
        .client
        .check(control_request(
            control(&m, &b, false),
            "/profile.concert_public.v1.ConcertPublicProfileService/Check"
        ))
        .await
        .is_err());
    // Expired contexts cannot read data; separate terminal settlement still works.
    finish(&mut s, &m, &b).await?;
    let subject = Uuid::parse_str(&m.subject_id)?;
    sqlx::query("update profiles set account_visibility='PRIVATE' where user_id=$1")
        .bind(subject)
        .execute(&pool)
        .await?;
    let count: i64 = sqlx::query_scalar(
        "select count(*) from pg_prepared_xacts where database=current_database()",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(count, 0);
    s.stop().await?;
    pool.close().await;
    Ok(())
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn signed_expiry_and_noncooperative_completion_never_refresh_budget() -> anyhow::Result<()> {
    let (mut s, owner) = controlled(Mode::Success).await?;
    let mut m = message(Uuid::new_v4());
    m.expires_unix_micros = chrono::Utc::now().timestamp_micros() - 1;
    assert_eq!(
        call(&mut s, m).await.unwrap_err().code(),
        Code::DeadlineExceeded
    );
    assert_eq!(owner.calls.load(Ordering::SeqCst), 0);
    *owner.mode.lock().unwrap() = Mode::Block(Duration::from_millis(100));
    let mut m = message(Uuid::new_v4());
    m.expires_unix_micros = chrono::Utc::now().timestamp_micros() + 30_000;
    assert_eq!(
        call(&mut s, m).await.unwrap_err().code(),
        Code::DeadlineExceeded
    );
    s.stop().await
}
#[tokio::test]
#[ignore = "requires explicitly owned native PostgreSQL controller"]
async fn native_original_budget_bounds_owner_lock_wait_and_recovers() -> anyhow::Result<()> {
    let pool = owner_pool().await?;
    let subject = seed(&pool, "Budget owner", "").await?;
    sqlx::query("update profiles set account_visibility='PUBLIC' where user_id=$1")
        .bind(subject)
        .execute(&pool)
        .await?;
    let mut s = Running::start(PublicProfileServer::from_pool(pool.clone())).await?;
    let mut blocked = pool.begin().await?;
    sqlx::query("lock table profiles in access exclusive mode")
        .execute(&mut *blocked)
        .await?;
    let mut m = message(Uuid::new_v4());
    m.subject_id = subject.to_string();
    m.expires_unix_micros = chrono::Utc::now().timestamp_micros() + 100_000;
    let start = std::time::Instant::now();
    let e = call(&mut s, m).await.unwrap_err();
    assert!(matches!(
        e.code(),
        Code::Unavailable | Code::DeadlineExceeded
    ));
    assert!(start.elapsed() < Duration::from_millis(800));
    blocked.rollback().await?;
    let mut m = message(Uuid::new_v4());
    m.subject_id = subject.to_string();
    let b = call(&mut s, m.clone()).await?.into_inner();
    finish(&mut s, &m, &b).await?;
    s.stop().await?;
    pool.close().await;
    Ok(())
}
#[tokio::test]
#[ignore = "requires explicitly owned native PostgreSQL controller"]
async fn native_full_hundred_child_capacity_and_fail_closed_overflow() -> anyhow::Result<()> {
    let pool = owner_pool().await?;
    let subject = seed(&pool, "Capacity owner", "").await?;
    sqlx::query("update profiles set account_visibility='PUBLIC' where user_id=$1")
        .bind(subject)
        .execute(&pool)
        .await?;
    let mut s = Running::start(PublicProfileServer::from_pool(pool.clone())).await?;
    let mut held = Vec::new();
    for _ in 0..=100 {
        let mut m = message(Uuid::new_v4());
        m.subject_id = subject.to_string();
        let b = call(&mut s, m.clone()).await?.into_inner();
        held.push((m, b));
    }
    let mut overflow = message(Uuid::new_v4());
    overflow.subject_id = subject.to_string();
    assert_eq!(
        call(&mut s, overflow).await.unwrap_err().code(),
        Code::Unavailable
    );
    for (m, b) in held {
        finish(&mut s, &m, &b).await?;
    }
    let mut recovered = message(Uuid::new_v4());
    recovered.subject_id = subject.to_string();
    let b = call(&mut s, recovered.clone()).await?.into_inner();
    finish(&mut s, &recovered, &b).await?;
    s.stop().await?;
    pool.close().await;
    Ok(())
}
#[test]
fn opaque_children_bind_all_dimensions_and_unique_nonce_with_separate_completion_key() {
    use profile_service::concert_public_profile::ReadBinding;
    let binding = ReadBinding {
        actor: Uuid::new_v4(),
        event: Uuid::new_v4(),
        subject: Uuid::new_v4(),
        family: Uuid::new_v4(),
        request: Uuid::new_v4(),
    };
    let child = binding.new_child_id().unwrap();
    let next = binding.new_child_id().unwrap();
    assert_ne!(child, next);
    let secret = Uuid::new_v4().simple().to_string();
    binding.validate_completion(&child, &secret).unwrap();
    let gid = binding.prepared_id(&child, &"A".repeat(43)).unwrap();
    assert_eq!(gid.len(), 187);
    assert!(!gid.contains(&secret));
    for field in 0..5 {
        let mut changed = binding;
        match field {
            0 => changed.actor = Uuid::new_v4(),
            1 => changed.event = Uuid::new_v4(),
            2 => changed.subject = Uuid::new_v4(),
            3 => changed.family = Uuid::new_v4(),
            _ => changed.request = Uuid::new_v4(),
        };
        assert!(changed.validate_child(&child).is_err());
    }
    for malformed in [
        "",
        "' OR true; --",
        "00000000000000000000000000000000",
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
    ] {
        assert!(binding.validate_completion(&child, malformed).is_err());
    }
}
