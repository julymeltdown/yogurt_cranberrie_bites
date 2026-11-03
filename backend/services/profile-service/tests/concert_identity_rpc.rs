//! Actual generated Profile service, production caller/actor guard, controlled plaintext loopback.
#[path = "support/concert_identity_fixture.rs"]
mod fixture;
use fixture::*;
use platform::actor_delegation::{ActorDelegationSigner, VerifiedActor, LIFETIME_SECONDS};
use platform::grpc_auth::{VerifiedCaller, Workload};
use profile_service::adapters::r#in::grpc::server::{proto, ProfileGrpcServer};
use prost::Message;
use proto::profile_service_server::ProfileService;
use std::{
    sync::{atomic::Ordering, Arc},
    time::{Duration, Instant},
};
use tonic::{Code, Request};
use uuid::Uuid;

#[tokio::test]
async fn signed_gateway_self_returns_only_required_profile_and_no_store() -> anyhow::Result<()> {
    let (mut server, owner) = controlled(Mode::Success).await?;
    let user = Uuid::new_v4();
    let response = call(&mut server, message(user)).await?;
    assert_eq!(
        response.metadata().get("cache-control").unwrap(),
        "no-store"
    );
    let profile = response.into_inner().profile.expect("required on success");
    assert_eq!(profile.user_id, user.to_string());
    assert_eq!(profile.display_name, "Mira");
    assert_eq!(profile.bio.as_deref(), Some("Self content"));
    assert!(profile.observed_at_unix_micros > 0);
    assert_eq!(owner.calls.load(Ordering::SeqCst), 1);
    server.stop().await
}
#[tokio::test]
async fn missing_wrong_and_registered_sibling_callers_never_dispatch() -> anyhow::Result<()> {
    let (mut server, owner) = controlled(Mode::Success).await?;
    for (caller, credential, code) in [
        (None, None, Code::Unauthenticated),
        (Some("gateway"), Some("wrong-secret"), Code::Unauthenticated),
        (
            Some("gateway"),
            Some("profile-rpc-post-fixture"),
            Code::Unauthenticated,
        ),
        (
            Some("post"),
            Some("profile-rpc-post-fixture"),
            Code::PermissionDenied,
        ),
        (
            Some("auth"),
            Some("profile-rpc-auth-fixture"),
            Code::PermissionDenied,
        ),
        (
            Some("friend"),
            Some("profile-rpc-friend-fixture"),
            Code::PermissionDenied,
        ),
    ] {
        let user = Uuid::new_v4();
        let msg = message(user);
        let proof = token(&msg, user);
        let mut r = request(msg, Some(&proof), Some("2S"));
        r.metadata_mut().remove("x-service-caller");
        r.metadata_mut().remove("x-service-token");
        if let Some(c) = caller {
            r.metadata_mut().insert("x-service-caller", c.parse()?);
        }
        if let Some(c) = credential {
            r.metadata_mut().insert("x-service-token", c.parse()?);
        }
        assert_eq!(
            server
                .client
                .read_concert_self_profile(r)
                .await
                .unwrap_err()
                .code(),
            code
        );
    }
    assert_eq!(owner.calls.load(Ordering::SeqCst), 0);
    server.stop().await
}
#[tokio::test]
async fn signature_audience_method_body_actor_and_missing_proof_never_dispatch(
) -> anyhow::Result<()> {
    let (mut server, owner) = controlled(Mode::Success).await?;
    let actor = Uuid::new_v4();
    let msg = message(actor);
    let bytes = frame(&msg);
    let now = chrono::Utc::now().timestamp();
    let signer = ActorDelegationSigner::local_parity(Workload::Gateway);
    let forged = ActorDelegationSigner::from_seed(Workload::Gateway, &[42; 32])?;
    for proof in [
        None,
        Some("malformed-private-proof".into()),
        Some(forged.issue(Workload::Profile, METHOD, actor, None, &bytes, now)),
        Some(signer.issue(Workload::Auth, METHOD, actor, None, &bytes, now)),
        Some(signer.issue(
            Workload::Profile,
            "/profile.v1.ProfileService/GetProfile",
            actor,
            None,
            &bytes,
            now,
        )),
        Some(signer.issue(Workload::Profile, METHOD, Uuid::new_v4(), None, &bytes, now)),
        Some(signer.issue(
            Workload::Profile,
            METHOD,
            actor,
            None,
            &frame(&message(Uuid::new_v4())),
            now,
        )),
    ] {
        let mut r = request(msg.clone(), proof.as_deref(), Some("2S"));
        r.metadata_mut()
            .insert("x-verified-actor", actor.to_string().parse()?);
        let e = server
            .client
            .read_concert_self_profile(r)
            .await
            .unwrap_err();
        assert!(matches!(
            e.code(),
            Code::Unauthenticated | Code::PermissionDenied
        ));
        assert!(!e.message().contains("private"));
    }
    assert_eq!(owner.calls.load(Ordering::SeqCst), 0);
    server.stop().await
}
#[tokio::test]
async fn bare_or_constructed_verified_extensions_cannot_replace_private_authority(
) -> anyhow::Result<()> {
    let owner = Controlled::new(Mode::Success);
    let handler = ProfileGrpcServer::new(Arc::new(state(lazy_pool(), Some(owner.clone())).await?));
    let actor = Uuid::new_v4();
    for (caller, verified_actor, code) in [
        (None, None, Code::Unauthenticated),
        (Some(Workload::Post), Some(actor), Code::PermissionDenied),
        (Some(Workload::Gateway), None, Code::Unauthenticated),
        (Some(Workload::Gateway), Some(actor), Code::Unauthenticated),
    ] {
        let mut r = Request::new(message(actor));
        if let Some(c) = caller {
            r.extensions_mut().insert(VerifiedCaller(c));
        }
        if let Some(a) = verified_actor {
            r.extensions_mut().insert(VerifiedActor(a));
        }
        assert_eq!(
            handler
                .read_concert_self_profile(r)
                .await
                .unwrap_err()
                .code(),
            code
        );
    }
    assert_eq!(owner.calls.load(Ordering::SeqCst), 0);
    Ok(())
}
#[tokio::test]
async fn exact_signed_proof_replay_never_redispatches() -> anyhow::Result<()> {
    let (mut server, owner) = controlled(Mode::Success).await?;
    let user = Uuid::new_v4();
    let msg = message(user);
    let proof = token(&msg, user);
    server
        .client
        .read_concert_self_profile(request(msg.clone(), Some(&proof), Some("2S")))
        .await?;
    assert_eq!(
        server
            .client
            .read_concert_self_profile(request(msg, Some(&proof), Some("2S")))
            .await
            .unwrap_err()
            .code(),
        Code::Unauthenticated
    );
    assert_eq!(owner.calls.load(Ordering::SeqCst), 1);
    server.stop().await
}
#[tokio::test]
async fn malformed_and_noncanonical_subjects_never_dispatch() -> anyhow::Result<()> {
    let (mut server, owner) = controlled(Mode::Success).await?;
    let user = Uuid::parse_str("abcdef01-2345-6789-abcd-ef0123456789")?;
    for value in [
        "".into(),
        "private-subject".into(),
        Uuid::nil().to_string(),
        user.simple().to_string(),
        user.to_string().to_uppercase(),
        format!(" {user}"),
    ] {
        let msg = Msg { user_id: value };
        let proof = token(
            &msg,
            if msg.user_id == Uuid::nil().to_string() {
                Uuid::nil()
            } else {
                user
            },
        );
        let e = server
            .client
            .read_concert_self_profile(request(msg, Some(&proof), Some("2S")))
            .await
            .unwrap_err();
        assert!(matches!(
            e.code(),
            Code::InvalidArgument | Code::PermissionDenied | Code::Unauthenticated
        ));
        assert!(!e.message().contains("private"));
    }
    assert_eq!(owner.calls.load(Ordering::SeqCst), 0);
    server.stop().await
}
#[tokio::test]
async fn original_budget_is_required_and_never_restarted() -> anyhow::Result<()> {
    let (mut server, owner) = controlled(Mode::Success).await?;
    for (remaining, code) in [
        (None, Code::InvalidArgument),
        (Some("garbage"), Code::InvalidArgument),
        (Some("0n"), Code::DeadlineExceeded),
    ] {
        let user = Uuid::new_v4();
        let msg = message(user);
        let proof = token(&msg, user);
        assert_eq!(
            server
                .client
                .read_concert_self_profile(request(msg, Some(&proof), remaining))
                .await
                .unwrap_err()
                .code(),
            code
        );
    }
    assert_eq!(owner.calls.load(Ordering::SeqCst), 0);
    server.stop().await
}
#[tokio::test]
async fn expired_proof_has_no_strict_authority_even_inside_legacy_skew() -> anyhow::Result<()> {
    let (mut server, owner) = controlled(Mode::Success).await?;
    let user = Uuid::new_v4();
    let msg = message(user);
    let proof = ActorDelegationSigner::local_parity(Workload::Gateway).issue(
        Workload::Profile,
        METHOD,
        user,
        None,
        &frame(&msg),
        chrono::Utc::now().timestamp() - LIFETIME_SECONDS - 1,
    );
    assert_eq!(
        server
            .client
            .read_concert_self_profile(request(msg, Some(&proof), Some("2S")))
            .await
            .unwrap_err()
            .code(),
        Code::Unauthenticated
    );
    assert_eq!(owner.calls.load(Ordering::SeqCst), 0);
    server.stop().await
}
#[tokio::test]
async fn missing_malformed_misbound_and_dependency_results_are_fixed_and_uncached(
) -> anyhow::Result<()> {
    let (mut server, owner) = controlled(Mode::Success).await?;
    let msg = message(Uuid::new_v4());
    call(&mut server, msg.clone()).await?;
    for (mode, code, text) in [
        (Mode::Missing, Code::NotFound, "Profile content unavailable"),
        (
            Mode::Malformed,
            Code::NotFound,
            "Profile content unavailable",
        ),
        (
            Mode::Misbound,
            Code::NotFound,
            "Profile content unavailable",
        ),
        (Mode::Error, Code::Unavailable, "Profile owner unavailable"),
    ] {
        *owner.mode.lock().unwrap() = mode;
        let e = call(&mut server, msg.clone()).await.unwrap_err();
        assert_eq!(e.code(), code);
        assert_eq!(e.message(), text);
    }
    *owner.mode.lock().unwrap() = Mode::Success;
    call(&mut server, msg).await?;
    assert_eq!(owner.calls.load(Ordering::SeqCst), 6);
    server.stop().await
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn whole_owner_await_and_late_noncooperative_completion_are_deadline_fenced(
) -> anyhow::Result<()> {
    for mode in [
        Mode::Delay(Duration::from_millis(180)),
        Mode::Block(Duration::from_millis(180)),
    ] {
        let blocking = matches!(mode, Mode::Block(_));
        let (mut server, owner) = controlled(mode).await?;
        let user = Uuid::new_v4();
        let msg = message(user);
        let proof = token(&msg, user);
        let start = Instant::now();
        assert_eq!(
            server
                .client
                .read_concert_self_profile(request(msg, Some(&proof), Some("50m")))
                .await
                .unwrap_err()
                .code(),
            Code::DeadlineExceeded
        );
        assert!(start.elapsed() < Duration::from_secs(1));
        assert_eq!(owner.calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            owner.completed.load(Ordering::SeqCst),
            usize::from(blocking)
        );
        assert_eq!(owner.dropped.load(Ordering::SeqCst), 1);
        server.stop().await?;
    }
    Ok(())
}
#[tokio::test]
async fn retained_gateway_profile_local_ceiling_bounds_a_longer_original_budget(
) -> anyhow::Result<()> {
    let (mut server, owner) = controlled(Mode::Hold).await?;
    let user = Uuid::new_v4();
    let msg = message(user);
    let proof = token(&msg, user);
    let sent = Instant::now();
    assert_eq!(
        server
            .client
            .read_concert_self_profile(request(msg, Some(&proof), Some("5S")))
            .await
            .unwrap_err()
            .code(),
        Code::DeadlineExceeded
    );
    let ceiling = platform::grpc_deadline::local_deadline(platform::edge::Edge::GatewayProfile);
    assert!(sent.elapsed() >= ceiling.saturating_sub(Duration::from_millis(50)));
    assert!(sent.elapsed() < Duration::from_secs(2));
    assert_eq!(owner.calls.load(Ordering::SeqCst), 1);
    assert_eq!(owner.dropped.load(Ordering::SeqCst), 1);
    server.stop().await
}
#[tokio::test]
async fn signed_expiry_shortens_original_and_resource_budgets() -> anyhow::Result<()> {
    let (mut server, owner) = controlled(Mode::Hold).await?;
    let sampled = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let now = chrono::Utc::now();
            let us = now.timestamp_subsec_micros();
            if (600000..=750000).contains(&us) {
                break now;
            }
            let wait = if us < 600000 {
                600000 - us
            } else {
                1600000 - us
            };
            tokio::time::sleep(Duration::from_micros(u64::from(wait) + 1000)).await;
        }
    })
    .await?;
    let user = Uuid::new_v4();
    let msg = message(user);
    let exp = (sampled.timestamp() + 1) * 1000000;
    let proof = ActorDelegationSigner::local_parity(Workload::Gateway).issue(
        Workload::Profile,
        METHOD,
        user,
        None,
        &frame(&msg),
        sampled.timestamp() + 1 - LIFETIME_SECONDS,
    );
    let sent = Instant::now();
    assert_eq!(
        server
            .client
            .read_concert_self_profile(request(msg, Some(&proof), Some("5S")))
            .await
            .unwrap_err()
            .code(),
        Code::DeadlineExceeded
    );
    assert!(chrono::Utc::now().timestamp_micros() >= exp);
    assert!(sent.elapsed() < Duration::from_millis(650));
    assert_eq!(owner.calls.load(Ordering::SeqCst), 1);
    assert_eq!(owner.dropped.load(Ordering::SeqCst), 1);
    server.stop().await
}

#[tokio::test]
async fn native_rpc_minimal_private_projection_unicode_and_fresh_updates() -> anyhow::Result<()> {
    let owner = owner_pool().await?;
    let reader = reader_pool().await?;
    let user = seed(&owner, &"🎸".repeat(40), &"🎵".repeat(160)).await?;
    let other = seed(&owner, "Other", "other self").await?;
    let mut server = Running::start(state(reader.clone(), None).await?).await?;
    let before = chrono::Utc::now().timestamp_micros();
    let response = call(&mut server, message(user)).await?;
    let after = chrono::Utc::now().timestamp_micros();
    assert_eq!(
        response.metadata().get("cache-control").unwrap(),
        "no-store"
    );
    let wire = response.get_ref().encode_to_vec();
    assert!(wire.len() < 1024);
    for private in [
        "private-handle-",
        "legacy-token-canary",
        "private-delete-canary",
        "https://private.invalid",
    ] {
        assert!(!wire.windows(private.len()).any(|w| w == private.as_bytes()));
    }
    let p = response.into_inner().profile.unwrap();
    assert_eq!(p.user_id, user.to_string());
    assert_eq!(p.display_name, "🎸".repeat(40));
    assert_eq!(p.bio, Some("🎵".repeat(160)));
    assert!(before <= p.observed_at_unix_micros && p.observed_at_unix_micros <= after);
    assert_eq!(
        call(&mut server, message(other))
            .await?
            .get_ref()
            .profile
            .as_ref()
            .unwrap()
            .display_name,
        "Other"
    );
    assert!(
        sqlx::query("select username,avatar_url,delete_reason from profiles where user_id=$1")
            .bind(user)
            .fetch_optional(&reader)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("update profiles set nickname='forbidden' where user_id=$1")
            .bind(user)
            .execute(&reader)
            .await
            .is_err()
    );
    sqlx::query("update profiles set nickname='  Changed  ',bio='' where user_id=$1")
        .bind(user)
        .execute(&owner)
        .await?;
    let p = call(&mut server, message(user))
        .await?
        .into_inner()
        .profile
        .unwrap();
    assert_eq!(p.display_name, "  Changed  ");
    assert_eq!(p.bio, None);
    server.stop().await?;
    reader.close().await;
    owner.close().await;
    Ok(())
}
#[tokio::test]
async fn native_rpc_missing_deleted_fenced_and_invalid_content_share_not_found(
) -> anyhow::Result<()> {
    let owner = owner_pool().await?;
    let reader = reader_pool().await?;
    let mut server = Running::start(state(reader.clone(), None).await?).await?;
    let deleted = seed(&owner, "Deleted", "private").await?;
    let fenced = seed(&owner, "Fenced", "private").await?;
    sqlx::query("update profiles set deleted_at=clock_timestamp() where user_id=$1")
        .bind(deleted)
        .execute(&owner)
        .await?;
    sqlx::query("insert into profile_user_deletion_tombstones(user_id,saga_id,actor_id,retention_until) values($1,$2,$3,now()+interval '1 day')").bind(fenced).bind(Uuid::new_v4()).bind(Uuid::new_v4()).execute(&owner).await?;
    let mut subjects = vec![Uuid::new_v4(), deleted, fenced];
    for (name, bio) in [
        (" ".into(), "bio".into()),
        ("bad\nname".into(), "bio".into()),
        ("Name".into(), "bad\nbio".into()),
        ("a".repeat(41), "bio".into()),
        ("🎸".repeat(41), "bio".into()),
        ("Name".into(), "b".repeat(161)),
        ("x".repeat(2 * 1024 * 1024), "bio".into()),
        ("Name".into(), "b".repeat(2 * 1024 * 1024)),
    ] {
        subjects.push(seed(&owner, &name, &bio).await?);
    }
    for subject in subjects {
        let e = call(&mut server, message(subject)).await.unwrap_err();
        assert_eq!(e.code(), Code::NotFound);
        assert_eq!(e.message(), "Profile content unavailable");
    }
    server.stop().await?;
    reader.close().await;
    owner.close().await;
    Ok(())
}
#[tokio::test]
async fn native_rpc_statement_visibility_includes_only_committed_edits_and_fences(
) -> anyhow::Result<()> {
    let owner = owner_pool().await?;
    let reader = reader_pool().await?;
    let user = seed(&owner, "Before", "before").await?;
    let mut server = Running::start(state(reader.clone(), None).await?).await?;
    let mut tx = owner.begin().await?;
    sqlx::query("update profiles set nickname='After',bio='after' where user_id=$1")
        .bind(user)
        .execute(&mut *tx)
        .await?;
    let p = call(&mut server, message(user))
        .await?
        .into_inner()
        .profile
        .unwrap();
    assert_eq!(
        (p.display_name, p.bio),
        ("Before".into(), Some("before".into()))
    );
    tx.commit().await?;
    let p = call(&mut server, message(user))
        .await?
        .into_inner()
        .profile
        .unwrap();
    assert_eq!(
        (p.display_name, p.bio),
        ("After".into(), Some("after".into()))
    );
    let mut tx = owner.begin().await?;
    sqlx::query("select profile_lock_subject_ids($1::uuid[])")
        .bind(vec![user])
        .execute(&mut *tx)
        .await?;
    sqlx::query("insert into profile_user_deletion_tombstones(user_id,saga_id,actor_id,retention_until) values($1,$2,$3,now()+interval '1 day')").bind(user).bind(Uuid::new_v4()).bind(Uuid::new_v4()).execute(&mut *tx).await?;
    call(&mut server, message(user)).await?;
    tx.commit().await?;
    let e = call(&mut server, message(user)).await.unwrap_err();
    assert_eq!(e.code(), Code::NotFound);
    assert_eq!(e.message(), "Profile content unavailable");
    server.stop().await?;
    reader.close().await;
    owner.close().await;
    Ok(())
}
#[tokio::test]
async fn native_rpc_locked_owner_times_out_and_same_single_connection_recovers(
) -> anyhow::Result<()> {
    let owner = owner_pool().await?;
    let reader = reader_pool().await?;
    let user = seed(&owner, "Live", "bio").await?;
    let mut server = Running::start(state(reader.clone(), None).await?).await?;
    let mut tx = owner.begin().await?;
    sqlx::query("lock table profiles in access exclusive mode")
        .execute(&mut *tx)
        .await?;
    let msg = message(user);
    let proof = token(&msg, user);
    let e = server
        .client
        .read_concert_self_profile(request(msg, Some(&proof), Some("80m")))
        .await
        .unwrap_err();
    assert_eq!(e.code(), Code::DeadlineExceeded);
    tx.rollback().await?;
    tokio::time::timeout(
        Duration::from_secs(3),
        sqlx::query("select 1").execute(&reader),
    )
    .await??;
    assert_eq!(
        call(&mut server, message(user))
            .await?
            .get_ref()
            .profile
            .as_ref()
            .unwrap()
            .display_name,
        "Live"
    );
    server.stop().await?;
    reader.close().await;
    owner.close().await;
    Ok(())
}
#[tokio::test]
async fn native_rpc_real_permission_failure_is_fixed_unavailable() -> anyhow::Result<()> {
    let owner = owner_pool().await?;
    let pool = sqlx::postgres::PgPoolOptions::new()
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
    let mut server = Running::start(state(pool.clone(), None).await?).await?;
    let e = call(&mut server, message(Uuid::new_v4()))
        .await
        .unwrap_err();
    assert_eq!(e.code(), Code::Unavailable);
    assert_eq!(e.message(), "Profile owner unavailable");
    server.stop().await?;
    pool.close().await;
    owner.close().await;
    Ok(())
}
#[tokio::test]
async fn native_rpc_stopped_owned_database_is_fixed_unavailable() -> anyhow::Result<()> {
    // Controller invokes this case only while its exact owned PG server is stopped.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .acquire_timeout(Duration::from_millis(150))
        .connect_lazy_with(owned_options()?);
    let mut server = Running::start(state(pool.clone(), None).await?).await?;
    let e = call(&mut server, message(Uuid::new_v4()))
        .await
        .unwrap_err();
    assert_eq!(e.code(), Code::Unavailable);
    assert_eq!(e.message(), "Profile owner unavailable");
    server.stop().await?;
    pool.close().await;
    Ok(())
}
