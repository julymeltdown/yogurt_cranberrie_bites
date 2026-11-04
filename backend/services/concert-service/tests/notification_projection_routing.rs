//! SQL routing component checks reuse the original owned NB03 fixture. Protected
//! setup stays uncommitted and rolls back; this is not public IN03 acceptance.
#[path = "notification_target_policy_support/fixture.rs"]
mod fixture;
use chrono::Utc;
use concert_domain::notification::*;
use concert_service::adapters::out::notification::{
    routing::SocialProjectionRouting,
    source::{ProjectionRouting, Source},
};
use uuid::Uuid;
#[path = "../../notification-service/tests/concert_inbox_support/mod.rs"]
mod delivery_support;

fn source(f: &fixture::Fixture, operation: &str, actor: Uuid, revision: i64) -> Source {
    Source {
        actor_id: actor,
        operation: operation.into(),
        envelope: Envelope {
            outbox_id: Uuid::new_v4(),
            receipt_id: Uuid::new_v4(),
            event_id: f.event,
            resource_id: f.social,
            resource_revision: revision,
        },
    }
}
#[tokio::test]
async fn four_social_operations_route_only_the_actual_peer() -> anyhow::Result<()> {
    let f = fixture::Fixture::new().await?;
    let mut tx = f.pool.begin().await?;
    f.prepare(&mut tx, false).await?;
    let created = SocialProjectionRouting
        .route_in(
            &mut tx,
            &source(&f, "createConcertSocialRequest", f.actor, 1),
            Utc::now(),
        )
        .await?;
    assert_eq!(created.len(), 1);
    assert_eq!(created[0].user_id, f.owner);
    assert_eq!(created[0].source_actor_id, Some(f.actor));
    assert_eq!(created[0].kind, Kind::Social);
    assert_eq!(
        created[0].target,
        Some(Target {
            target_type: TargetType::SocialRequest,
            target_id: f.social
        })
    );
    sqlx::query("update public.concert_social_requests set state='APPROVED',revision=2 where event_id=$1 and request_id=$2").bind(f.event).bind(f.social).execute(&mut *tx).await?;
    for (op, actor, expected) in [
        ("decideConcertSocialRequest", f.owner, f.actor),
        ("cancelConcertSocialRequest", f.actor, f.owner),
        ("revokeConcertSocialRequest", f.owner, f.actor),
        ("revokeConcertSocialRequest", f.actor, f.owner),
    ] {
        let routed = SocialProjectionRouting
            .route_in(&mut tx, &source(&f, op, actor, 2), Utc::now())
            .await?;
        assert_eq!(routed.len(), 1);
        assert_eq!(routed[0].user_id, expected);
    }
    // Older immutable create delivery remains meaningful after later transitions.
    assert_eq!(
        SocialProjectionRouting
            .route_in(
                &mut tx,
                &source(&f, "createConcertSocialRequest", f.actor, 1),
                Utc::now()
            )
            .await?,
        created
    );
    tx.rollback().await?;
    Ok(())
}
#[tokio::test]
async fn missing_or_inconsistent_sources_never_become_empty_success() -> anyhow::Result<()> {
    let f = fixture::Fixture::new().await?;
    let mut tx = f.pool.begin().await?;
    f.prepare(&mut tx, false).await?;
    for s in [
        source(&f, "createConcertPost", f.actor, 1),
        source(&f, "createConcertSocialRequest", f.owner, 1),
        source(&f, "createConcertSocialRequest", f.actor, 2),
    ] {
        assert_eq!(
            SocialProjectionRouting
                .route_in(&mut tx, &s, Utc::now())
                .await,
            Err(NotificationError::Unavailable)
        );
    }
    let mut missing = source(&f, "createConcertSocialRequest", f.actor, 1);
    missing.envelope.resource_id = Uuid::new_v4();
    assert_eq!(
        SocialProjectionRouting
            .route_in(&mut tx, &missing, Utc::now())
            .await,
        Err(NotificationError::Unavailable)
    );
    tx.rollback().await?;
    Ok(())
}
#[tokio::test]
async fn explicit_local_suppression_and_missing_policy_are_distinct() -> anyhow::Result<()> {
    let f = fixture::Fixture::new().await?;
    let mut tx = f.pool.begin().await?;
    f.prepare(&mut tx, true).await?;
    let s = source(&f, "createConcertSocialRequest", f.actor, 1);
    // Expired metadata is safe; no sharing grant is selected or prolonged.
    assert_eq!(
        SocialProjectionRouting
            .route_in(&mut tx, &s, Utc::now())
            .await?
            .len(),
        1
    );
    sqlx::query("update public.concert_event_settings set social=false where event_id=$1")
        .bind(f.event)
        .execute(&mut *tx)
        .await?;
    assert!(SocialProjectionRouting
        .route_in(&mut tx, &s, Utc::now())
        .await?
        .is_empty());
    sqlx::query("update public.concert_event_settings set social=true where event_id=$1")
        .bind(f.event)
        .execute(&mut *tx)
        .await?;
    sqlx::query("update public.concert_relationship_pairs set low_blocks_high=true,revision=revision+1 where event_id=$1 and low_user=$2 and high_user=$3").bind(f.event).bind(f.actor.min(f.owner)).bind(f.actor.max(f.owner)).execute(&mut *tx).await?;
    assert!(SocialProjectionRouting
        .route_in(&mut tx, &s, Utc::now())
        .await?
        .is_empty());
    sqlx::query("delete from public.concert_relationship_pairs where event_id=$1")
        .bind(f.event)
        .execute(&mut *tx)
        .await?;
    assert_eq!(
        SocialProjectionRouting
            .route_in(&mut tx, &s, Utc::now())
            .await,
        Err(NotificationError::Unavailable)
    );
    tx.rollback().await?;
    Ok(())
}

#[tokio::test]
async fn actual_concert_client_repeats_one_notification_through_live_ingress() -> anyhow::Result<()>
{
    use concert_application::notification::DurableNotificationOwner;
    use concert_service::adapters::out::notification::delivery::NotificationDeliveryClient;
    // The finite qualifier launches the copied production Notification main.
    // Reuse the original owned DB guard; never accept an ambient service URL.
    let pool = delivery_support::owned_pool("mcr_concert_notification").await?;
    let root = std::path::PathBuf::from(std::env::var("CONCERT_TEST_OWNER_ROOT")?);
    let endpoint = std::env::var("MCR_NOTIFICATION_DELIVERY_ENDPOINT")?;
    let uri: axum::http::Uri = endpoint.parse()?;
    anyhow::ensure!(
        uri.scheme_str() == Some("https")
            && uri.host() == Some("127.0.0.1")
            && uri.port_u16().is_some(),
        "owned loopback ingress required"
    );
    let pki = root.join("pki");
    let tls = platform::grpc_tls::TlsMaterial::load(
        pki.join("concert.crt"),
        pki.join("concert.key"),
        pki.join("ca.crt"),
    )?;
    let client = NotificationDeliveryClient::connect(
        &endpoint,
        &std::env::var("MCR_NOTIFICATION_CONCERT_CREDENTIAL")?,
        &tls,
    )?;
    let envelope = Envelope {
        outbox_id: Uuid::new_v4(),
        receipt_id: Uuid::new_v4(),
        event_id: Uuid::new_v4(),
        resource_id: Uuid::new_v4(),
        resource_revision: 1,
    };
    let recipient = Recipient {
        user_id: Uuid::new_v4(),
        source_actor_id: Some(Uuid::new_v4()),
        kind: Kind::Social,
        target: Some(Target {
            target_type: TargetType::SocialRequest,
            target_id: envelope.resource_id,
        }),
    };
    let (left, right) = tokio::join!(
        client.deliver(envelope, vec![recipient]),
        client.deliver(envelope, vec![recipient])
    );
    left?;
    right?;
    client.deliver(envelope, vec![recipient]).await?;
    let count = sqlx::query_scalar::<_, i64>(
        "select count(*) from notifications where event_id=$1 and user_id=$2 and payload_json=''",
    )
    .bind(envelope.outbox_id)
    .bind(recipient.user_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(count, 1);
    drop(client);
    pool.close().await;
    Ok(())
}
