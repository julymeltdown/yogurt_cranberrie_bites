//! Owner-local SQL component tests, not a RootTargetSnapshotOwner qualification.
#[path = "../src/notification_target_policy/mod.rs"]
mod notification_target_policy;
mod notification_target_policy_support;

use chrono::{Duration, Utc};
use concert_application::notification::Candidate;
use concert_domain::notification::{NotificationError, Target, TargetType, Visibility};
use notification_target_policy::observe_local_target;
use notification_target_policy_support::fixture::Fixture;
use sqlx::{Connection, Row};
use uuid::Uuid;

const KINDS: [TargetType; 6] = [
    TargetType::Party,
    TargetType::Application,
    TargetType::Giveaway,
    TargetType::Reservation,
    TargetType::SocialRequest,
    TargetType::Notice,
];

#[tokio::test]
#[ignore = "named owned PG16 allocation required; local SQL policy only"]
async fn local_targets_use_actual_current_owner_rows() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let mut tx = f.pool.begin().await?;
    f.prepare(&mut tx, false).await?;
    let before: (i32, String) =
        sqlx::query_as("select pg_backend_pid(),pg_current_xact_id()::text")
            .fetch_one(&mut *tx)
            .await?;
    for kind in KINDS {
        let context = f.context();
        let candidate = f.candidate(kind);
        let original = candidate;
        assert_eq!(
            observe_local_target(&mut tx, context, candidate, Utc::now()).await?,
            Visibility::Visible,
            "{kind:?}"
        );
        assert_eq!(candidate, original, "identity must not be rebound");
        assert!(tx.is_in_transaction(), "component released caller TX");
    }
    let after: (i32, String) = sqlx::query_as("select pg_backend_pid(),pg_current_xact_id()::text")
        .fetch_one(&mut *tx)
        .await?;
    assert_eq!(before, after, "component must use same native transaction");
    tx.rollback().await?;
    let settings: bool = sqlx::query_scalar(
        "select exists(select 1 from public.concert_event_settings where event_id=$1)",
    )
    .bind(f.event)
    .fetch_one(&f.pool)
    .await?;
    assert!(!settings, "protected fixture effects must never commit");
    Ok(())
}

#[tokio::test]
#[ignore = "named owned PG16 allocation required; local SQL policy only"]
async fn missing_foreign_and_unsupported_targets_fail_unavailable() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let mut tx = f.pool.begin().await?;
    f.prepare(&mut tx, false).await?;
    for kind in KINDS {
        let mut candidate = f.candidate(kind);
        candidate.target.as_mut().unwrap().target_id = Uuid::new_v4();
        assert_eq!(
            observe_local_target(&mut tx, f.context(), candidate, Utc::now()).await,
            Err(NotificationError::Unavailable),
            "missing {kind:?}"
        );
    }
    for kind in [TargetType::Post, TargetType::Report] {
        assert_eq!(
            observe_local_target(&mut tx, f.context(), f.candidate(kind), Utc::now()).await,
            Err(NotificationError::Unavailable)
        );
    }
    let no_target = Candidate {
        target: None,
        ..f.candidate(TargetType::Party)
    };
    assert_eq!(
        observe_local_target(&mut tx, f.context(), no_target, Utc::now()).await,
        Err(NotificationError::Unavailable)
    );
    let mut wrong = f.context();
    wrong.event_id = Uuid::new_v4();
    assert_eq!(
        observe_local_target(&mut tx, wrong, f.candidate(TargetType::Party), Utc::now()).await,
        Err(NotificationError::Unavailable)
    );
    let candidate = Candidate {
        source_actor_id: Some(Uuid::new_v4()),
        ..f.candidate(TargetType::Party)
    };
    assert_eq!(
        observe_local_target(&mut tx, f.context(), candidate, Utc::now()).await,
        Err(NotificationError::Unavailable)
    );
    // A physically existing foreign-event ID remains unavailable in this event.
    let foreign = Fixture::new().await?;
    let candidate = Candidate {
        target: Some(Target {
            target_type: TargetType::Party,
            target_id: foreign.party,
        }),
        ..f.candidate(TargetType::Party)
    };
    assert_eq!(
        observe_local_target(&mut tx, f.context(), candidate, Utc::now()).await,
        Err(NotificationError::Unavailable)
    );
    sqlx::query("delete from public.concert_relationship_pairs where event_id=$1")
        .bind(f.event)
        .execute(&mut *tx)
        .await?;
    assert_eq!(
        observe_local_target(
            &mut tx,
            f.context(),
            f.candidate(TargetType::Party),
            Utc::now()
        )
        .await,
        Err(NotificationError::Unavailable),
        "unfenced missing block policy"
    );
    tx.rollback().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "named owned PG16 allocation required; local SQL policy only"]
async fn explicit_local_restrictions_hide_without_dependency_downgrade() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    for kind in [
        TargetType::Application,
        TargetType::Reservation,
        TargetType::SocialRequest,
    ] {
        let mut tx = f.pool.begin().await?;
        f.prepare(&mut tx, false).await?;
        let mut context = f.context();
        context.actor_id = f.outsider;
        assert_eq!(
            observe_local_target(&mut tx, context, f.candidate(kind), Utc::now()).await?,
            Visibility::Hidden,
            "wrong audience {kind:?}"
        );
        tx.rollback().await?;
    }
    for (sql,kind) in [
        ("update public.concert_event_admissions set state='SUSPENDED',restriction_revision=restriction_revision+1 where event_id=$1 and user_id=$2",TargetType::Party),
        ("update public.concert_relationship_pairs set low_blocks_high=true,revision=revision+1 where event_id=$1 and (low_user=$2 or high_user=$2)",TargetType::Party),
        ("update public.concert_event_settings set giveaways=false where event_id=$1 and $2::uuid is not null",TargetType::Giveaway),
        ("update public.concert_operation_notices set state='WITHDRAWN',revision=revision+1 where event_id=$1 and $2::uuid is not null",TargetType::Notice),
        ("update public.concert_events set phase='ARCHIVED',revision=revision+1 where event_id=$1 and $2::uuid is not null",TargetType::Party),
        ("update public.concert_community_giveaways set state='DRAFT',aggregate=jsonb_set(aggregate,'{state}','\"DRAFT\"'::jsonb) where event_id=$1 and $2::uuid is not null",TargetType::Giveaway),
    ] {
        let mut tx=f.pool.begin().await?;f.prepare(&mut tx,false).await?;sqlx::query(sql).bind(f.event).bind(f.owner).execute(&mut *tx).await?;
        assert_eq!(observe_local_target(&mut tx,f.context(),f.candidate(kind),Utc::now()).await?,Visibility::Hidden,"{kind:?}");tx.rollback().await?;
    }
    // Missing participation is not a locked negative state or proof of absence.
    let mut tx = f.pool.begin().await?;
    f.prepare(&mut tx, false).await?;
    sqlx::query("delete from public.concert_participants where event_id=$1 and user_id=$2")
        .bind(f.event)
        .bind(f.owner)
        .execute(&mut *tx)
        .await?;
    assert_eq!(
        observe_local_target(
            &mut tx,
            f.context(),
            f.candidate(TargetType::Party),
            Utc::now()
        )
        .await,
        Err(NotificationError::Unavailable)
    );
    tx.rollback().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "named owned PG16 allocation required; local SQL policy only"]
async fn terminal_metadata_never_creates_a_private_action_grant() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let mut tx = f.pool.begin().await?;
    f.prepare(&mut tx, true).await?;
    // Current owner endpoint explicitly retains expired metadata. Visible here
    // means the same metadata target remains readable, never a reveal grant.
    assert_eq!(
        observe_local_target(
            &mut tx,
            f.context(),
            f.candidate(TargetType::SocialRequest),
            Utc::now()
        )
        .await?,
        Visibility::Visible
    );
    sqlx::query("update public.concert_social_requests set state='REVOKED',revision=revision+1 where event_id=$1").bind(f.event).execute(&mut *tx).await?;
    assert_eq!(
        observe_local_target(
            &mut tx,
            f.context(),
            f.candidate(TargetType::SocialRequest),
            Utc::now()
        )
        .await?,
        Visibility::Visible
    );
    let row=sqlx::query("select aggregate from public.concert_community_giveaways where event_id=$1 and giveaway_id=$2").bind(f.event).bind(f.giveaway).fetch_one(&mut *tx).await?;
    let mut gift: concert_domain::community::giveaway::Giveaway =
        serde_json::from_value(row.try_get("aggregate")?)?;
    let expiry: chrono::DateTime<Utc> =
        sqlx::query_scalar("select clock_timestamp()-interval '1 hour'")
            .fetch_one(&mut *tx)
            .await?;
    gift.reservations[0].state = concert_domain::community::giveaway::ReservationState::Expired;
    gift.reservations[0].created_at = expiry - Duration::hours(1);
    gift.reservations[0].expires_at = expiry;
    gift.reservations[0].revision += 1;
    gift.reserved_unclaimed = 0;
    gift.revision += 1;
    gift.validate()?;
    sqlx::query("update public.concert_community_giveaways set aggregate=$3,revision=revision+1,reserved_unclaimed=0 where event_id=$1 and giveaway_id=$2").bind(f.event).bind(f.giveaway).bind(serde_json::to_value(gift)?).execute(&mut *tx).await?;
    sqlx::query("update public.concert_community_reservations set state='EXPIRED',revision=revision+1,expires_at=$3 where event_id=$1 and reservation_id=$2").bind(f.event).bind(f.reservation).bind(expiry).execute(&mut *tx).await?;
    assert_eq!(
        observe_local_target(
            &mut tx,
            f.context(),
            f.candidate(TargetType::Reservation),
            Utc::now()
        )
        .await?,
        Visibility::Visible
    );
    tx.rollback().await?;
    Ok(())
}

async fn writer_probe(
    f: &Fixture,
    kind: TargetType,
    sql: &'static str,
    target: Uuid,
) -> anyhow::Result<()> {
    let mut observed = f.pool.begin().await?;
    f.prepare(&mut observed, false).await?;
    assert_eq!(
        observe_local_target(&mut observed, f.context(), f.candidate(kind), Utc::now()).await?,
        Visibility::Visible
    );
    let observer_pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *observed)
        .await?;
    let pool = f.pool.clone();
    let event = f.event;
    let (send, receive) = tokio::sync::oneshot::channel();
    let writer = tokio::spawn(async move {
        let mut tx = pool.begin().await?;
        let pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
            .fetch_one(&mut *tx)
            .await?;
        let _ = send.send(pid);
        let result = sqlx::query(sql)
            .bind(event)
            .bind(target)
            .execute(&mut *tx)
            .await;
        tx.rollback().await?;
        result?;
        Ok::<(), anyhow::Error>(())
    });
    let writer_pid = receive.await?;
    let until = tokio::time::Instant::now() + std::time::Duration::from_secs(1);
    let mut blocked = false;
    while tokio::time::Instant::now() < until {
        blocked = sqlx::query_scalar("select $1=any(pg_blocking_pids($2))")
            .bind(observer_pid)
            .bind(writer_pid)
            .fetch_one(&f.pool)
            .await?;
        if blocked {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    assert!(
        blocked,
        "actual writer did not wait on retained observation"
    );
    assert!(!writer.is_finished());
    observed.rollback().await?;
    tokio::time::timeout(std::time::Duration::from_secs(2), writer).await???;
    Ok(())
}
#[tokio::test]
#[ignore = "named owned PG16 allocation required; committed row fences only"]
async fn retained_row_exclusions_block_actual_writers() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    for (kind,sql,target) in [
        (TargetType::Party,"update public.concert_events set title=title,revision=revision+1 where event_id=$1 and $2::uuid is not null",f.event),
        (TargetType::Party,"update public.concert_community_parties set revision=revision+1,aggregate=jsonb_set(aggregate,'{revision}',to_jsonb(revision+1)) where event_id=$1 and party_id=$2",f.party),
        (TargetType::Giveaway,"update public.concert_community_giveaways set revision=revision+1,aggregate=jsonb_set(aggregate,'{revision}',to_jsonb(revision+1)) where event_id=$1 and giveaway_id=$2",f.giveaway),
        (TargetType::Reservation,"update public.concert_community_reservations set revision=revision+1 where event_id=$1 and reservation_id=$2",f.reservation),
        (TargetType::Application,"update public.concert_community_applications set revision=revision+1 where event_id=$1 and application_id=$2",f.application),
        (TargetType::Party,"update public.concert_event_admissions set state='SUSPENDED',restriction_revision=restriction_revision+1 where event_id=$1 and user_id=$2",f.owner),
        (TargetType::Party,"update public.concert_participants set low_power=true,revision=revision+1 where event_id=$1 and user_id=$2",f.owner),
    ] {writer_probe(&f,kind,sql,target).await?;}
    // Notice/Social/settings/pairs are uncommitted protected test metadata;
    // interleaving fences for their actual owner commits remain NOT_RUN.
    Ok(())
}

#[tokio::test]
#[ignore = "named owned PG16 allocation required; deadline and real SQL failures"]
async fn original_deadline_and_dependency_failures_remain_errors() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let mut raw = f.pool.acquire().await?;
    assert_eq!(
        observe_local_target(
            &mut raw,
            f.context(),
            f.candidate(TargetType::Party),
            Utc::now()
        )
        .await,
        Err(NotificationError::Unavailable),
        "autocommit cannot retain exclusions"
    );
    drop(raw);
    let mut tx = f.pool.begin().await?;
    f.prepare(&mut tx, false).await?;
    let mut invalid = f.context();
    invalid.expires_at = invalid.observed_at + Duration::seconds(6);
    assert_eq!(
        observe_local_target(&mut tx, invalid, f.candidate(TargetType::Party), Utc::now()).await,
        Err(NotificationError::PermissionDenied)
    );
    invalid = f.context();
    invalid.observed_at = Utc::now() - Duration::seconds(2);
    invalid.expires_at = Utc::now() - Duration::seconds(1);
    assert_eq!(
        observe_local_target(&mut tx, invalid, f.candidate(TargetType::Party), Utc::now()).await,
        Err(NotificationError::PermissionDenied)
    );
    tx.rollback().await?;
    let mut blocker = f.pool.begin().await?;
    sqlx::query("select event_id from public.concert_events where event_id=$1 for update")
        .bind(f.event)
        .execute(&mut *blocker)
        .await?;
    let mut tx = f.pool.begin().await?;
    let mut context = f.context();
    context.expires_at = context.observed_at + Duration::milliseconds(150);
    let started = tokio::time::Instant::now();
    assert_eq!(
        observe_local_target(&mut tx, context, f.candidate(TargetType::Party), Utc::now()).await,
        Err(NotificationError::Unavailable)
    );
    assert!(
        started.elapsed() < std::time::Duration::from_secs(1),
        "original deadline was widened"
    );
    tx.rollback().await?;
    blocker.rollback().await?;
    // An actual disconnected owned SQL dependency must not become Hidden/Stale.
    let mut tx = f.pool.begin().await?;
    let pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *tx)
        .await?;
    let killed: bool = sqlx::query_scalar("select pg_terminate_backend($1)")
        .bind(pid)
        .fetch_one(&f.pool)
        .await?;
    assert!(killed);
    assert_eq!(
        observe_local_target(
            &mut tx,
            f.context(),
            f.candidate(TargetType::Party),
            Utc::now()
        )
        .await,
        Err(NotificationError::Unavailable)
    );
    drop(tx);
    Ok(())
}
