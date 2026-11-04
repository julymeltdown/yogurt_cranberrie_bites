//! Actual Concert resource-lock tests. No Auth/Friend authority is simulated or certified.
#[path = "gift_live_support/owned.rs"]
mod owned;
use async_trait::async_trait;
use chrono::Utc;
use concert_application::commands::CommandError;
use concert_service::{
    community::{policy::CommunityResources, CommunityAccess},
    gift_live::{protocol::*, resources::*},
};
use platform::actor_delegation::authority_deadline::AuthorityRpcDeadline;
use sqlx::{PgConnection, PgPool};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use uuid::Uuid;

struct DenyMedia(Arc<AtomicUsize>);
#[async_trait]
impl GiftMediaLink for DenyMedia {
    async fn link_in(
        &self,
        _: &mut PgConnection,
        _: GiftIdentity,
        _: Uuid,
        _: Uuid,
    ) -> Result<(), CommandError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(CommandError::Unavailable)
    }
}
fn request(event: Uuid, cancel: bool) -> GiftRequest {
    let identity = GiftIdentity {
        actor: Uuid::new_v4(),
        event,
        session_family: Uuid::new_v4(),
        request_id: Uuid::new_v4(),
        deadline: AuthorityRpcDeadline::new(
            Utc::now().timestamp_micros() + 2_000_000,
            Duration::from_secs(2),
        )
        .unwrap(),
    };
    let (operation, method, target, key) = if cancel {
        (
            GiftOperation::Cancel,
            "DELETE",
            format!(
                "/api/concert/events/{event}/reservations/{}?expected_revision=1",
                Uuid::new_v4()
            ),
            Uuid::new_v4().to_string(),
        )
    } else {
        (
            GiftOperation::List,
            "GET",
            format!("/api/concert/events/{event}/giveaways"),
            String::new(),
        )
    };
    GiftRequest::from_verified(identity, operation.id(), method, &target, "", &key, vec![])
        .ok()
        .expect("valid fixture request")
}
async fn fixture(pool: &PgPool, phase: &str, enabled: bool) -> anyhow::Result<Uuid> {
    let event = Uuid::new_v4();
    let mut tx = pool.begin().await?;
    sqlx::query("select set_config('mcr.operations_authority_until',(clock_timestamp()+interval '30 seconds')::text,true)").execute(&mut *tx).await?;
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Gift resource fixture',$3)").bind(event).bind(format!("gift-{event}")).bind(phase).execute(&mut *tx).await?;
    sqlx::query("insert into concert_event_settings(event_id,parties,giveaways,intros,social,mural,beacon_ttl_seconds,pickup_grace_seconds,social_grant_ttl_seconds,mural_cooldown_ms) values($1,false,$2,false,false,false,60,37,60,0)").bind(event).bind(enabled).execute(&mut *tx).await?;
    sqlx::query("insert into concert_meeting_settings(event_id,event_cutoff) values($1,clock_timestamp()+interval '1 day')").bind(event).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(event)
}
#[tokio::test]
#[ignore = "requires explicitly allocated native target and private Gift PG fixture"]
async fn rejects_unbound_connection_and_cross_actor_or_event() -> anyhow::Result<()> {
    let pool = owned::owned_pool().await?;
    let event = fixture(&pool, "OPEN", true).await?;
    let r = request(event, false);
    let actor = r.identity().actor;
    let a = PgGiftResources::new(&r, DenyMedia(Arc::new(AtomicUsize::new(0))))
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let mut c = pool.acquire().await?;
    assert_eq!(
        a.feature_in(&mut c, event, actor, CommunityAccess::ReadGifts)
            .await,
        Err(CommandError::Denied)
    );
    drop(c);
    let mut tx = pool.begin().await?;
    assert_eq!(
        a.feature_in(&mut tx, event, Uuid::new_v4(), CommunityAccess::ReadGifts)
            .await,
        Err(CommandError::Denied)
    );
    assert_eq!(
        a.feature_in(&mut tx, Uuid::new_v4(), actor, CommunityAccess::ReadGifts)
            .await,
        Err(CommandError::Denied)
    );
    assert_eq!(
        a.feature_in(&mut tx, event, actor, CommunityAccess::ReadParties)
            .await,
        Err(CommandError::Denied)
    );
    tx.rollback().await?;
    pool.close().await;
    Ok(())
}
#[tokio::test]
#[ignore = "requires explicitly allocated native target and private Gift PG fixture"]
async fn shutdown_preserves_only_validated_cancel_and_read_only_denies_writes() -> anyhow::Result<()>
{
    let pool = owned::owned_pool().await?;
    let event = fixture(&pool, "READ_ONLY", false).await?;
    let r = request(event, false);
    let actor = r.identity().actor;
    let a = PgGiftResources::new(&r, DenyMedia(Arc::new(AtomicUsize::new(0))))
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let mut tx = pool.begin().await?;
    assert_eq!(
        a.feature_in(&mut tx, event, actor, CommunityAccess::ReadGifts)
            .await,
        Err(CommandError::Denied)
    );
    tx.rollback().await?;
    let r = request(event, true);
    let actor = r.identity().actor;
    let a = PgGiftResources::new(&r, DenyMedia(Arc::new(AtomicUsize::new(0))))
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let mut tx = pool.begin().await?;
    // Retained receipt authorization calls ReadGifts even for safety cancellation.
    assert_eq!(
        a.feature_in(&mut tx, event, actor, CommunityAccess::ReadGifts)
            .await,
        Ok(())
    );
    assert_eq!(
        a.feature_in(&mut tx, event, actor, CommunityAccess::SafetyGifts)
            .await,
        Ok(())
    );
    assert_eq!(
        a.feature_in(&mut tx, event, actor, CommunityAccess::WriteGifts)
            .await,
        Err(CommandError::Denied)
    );
    tx.commit().await?;
    pool.close().await;
    Ok(())
}
#[tokio::test]
#[ignore = "requires explicitly allocated native target and private Gift PG fixture"]
async fn settings_capture_real_grace_cutoff_and_hold_lock_until_outer_terminal(
) -> anyhow::Result<()> {
    let pool = owned::owned_pool().await?;
    let event = fixture(&pool, "OPEN", true).await?;
    let r = request(event, false);
    let a = PgGiftResources::new(&r, DenyMedia(Arc::new(AtomicUsize::new(0))))
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let mut tx = pool.begin().await?;
    let settings = a
        .gift_settings_in(&mut tx, event)
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    assert_eq!(settings.pickup_grace_seconds, 37);
    let expected: chrono::DateTime<Utc> =
        sqlx::query_scalar("select event_cutoff from concert_meeting_settings where event_id=$1")
            .bind(event)
            .fetch_one(&mut *tx)
            .await?;
    assert_eq!(settings.event_cutoff, expected);
    let mut blocked = pool.begin().await?;
    sqlx::query("set local lock_timeout='80ms'")
        .execute(&mut *blocked)
        .await?;
    let e =
        sqlx::query("update concert_event_settings set pickup_grace_seconds=38 where event_id=$1")
            .bind(event)
            .execute(&mut *blocked)
            .await
            .unwrap_err();
    assert_eq!(
        e.as_database_error().and_then(|e| e.code()).as_deref(),
        Some("55P03")
    );
    blocked.rollback().await?;
    tx.commit().await?;
    let mut unblocked = pool.begin().await?;
    sqlx::query("select set_config('mcr.operations_authority_until',(clock_timestamp()+interval '2 seconds')::text,true)").execute(&mut *unblocked).await?;
    sqlx::query("update concert_event_settings set pickup_grace_seconds=38 where event_id=$1")
        .bind(event)
        .execute(&mut *unblocked)
        .await?;
    unblocked.commit().await?;
    pool.close().await;
    Ok(())
}
#[tokio::test]
#[ignore = "requires explicitly allocated native target and private Gift PG fixture"]
async fn optional_media_does_not_create_a_permissive_link_fallback() -> anyhow::Result<()> {
    let pool = owned::owned_pool().await?;
    let event = fixture(&pool, "OPEN", true).await?;
    let r = request(event, false);
    let actor = r.identity().actor;
    let calls = Arc::new(AtomicUsize::new(0));
    let a =
        PgGiftResources::new(&r, DenyMedia(calls.clone())).map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let mut tx = pool.begin().await?;
    let gift = Uuid::new_v4();
    assert_eq!(
        a.gift_media_in(&mut tx, event, actor, gift, None).await,
        Ok(())
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        a.gift_media_in(&mut tx, event, actor, gift, Some(Uuid::new_v4()))
            .await,
        Err(CommandError::Unavailable)
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        a.gift_media_in(&mut tx, event, actor, gift, Some(Uuid::nil()))
            .await,
        Err(CommandError::InvalidInput)
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    tx.rollback().await?;
    pool.close().await;
    Ok(())
}
