//! Real PG owner acceptance. No network, recipient-membership or delivery proof.
use super::*;
use post_service::adapters::out::persistence::{
    delivery_authority_repository::PgDeliveryAuthorityRepository,
    notification_authority_repository::{
        notification_reference, PgNotificationAuthorityRepository,
    },
};
use post_service::application::{
    delivery_authority::{
        DeliveryAuthorityErrorDto as Error, DeliveryAuthorityRepositoryPort,
        DeliveryClaimLookupDto, DeliveryLeaseKindDto, DeliveryReplayRestoreDto,
    },
    notification_authority::*,
    ports::ReplaySchedulePort,
};
use sha2::{Digest, Sha256};

struct Source {
    event: Uuid,
    post: Uuid,
    actor: Uuid,
    recipient: Uuid,
}
async fn seed(f: &Fixture, kind: &str) -> Result<Source> {
    let source = Source {
        event: Uuid::new_v4(),
        post: Uuid::new_v4(),
        actor: Uuid::new_v4(),
        recipient: Uuid::new_v4(),
    };
    let author = if kind == "POST_CREATED" {
        source.actor
    } else {
        source.recipient
    };
    sqlx::query("insert into posts(id,author_id,content,visibility,created_at) values($1,$2,'retained source text','PUBLIC',clock_timestamp())")
        .bind(source.post).bind(author).execute(&f.pool).await?;
    let payload = match kind {
        "POST_CREATED" => {
            serde_json::json!({"postId":source.post,"authorId":source.actor,"content":"must not copy"})
        }
        "POST_LIKED" => {
            serde_json::json!({"postId":source.post,"actorId":source.actor,"authorId":source.recipient,"notifyUserId":source.recipient})
        }
        "POST_COMMENTED" => {
            serde_json::json!({"postId":source.post,"authorId":source.actor,"notifyUserId":source.recipient,"content":"must not copy"})
        }
        _ => serde_json::json!({"postId":source.post,"notifyUserId":source.recipient}),
    };
    let now = Utc::now() - Duration::seconds(1);
    PgEventOutbox::new(f.pool.clone())
        .save(&StoredEvent {
            event_id: source.event,
            event_type: kind.into(),
            aggregate_id: if kind == "COMMENT" {
                Uuid::new_v4()
            } else {
                source.post
            },
            aggregate_version: Some(1),
            actor_id: Some(source.actor),
            attributes: vec!["notification".into()],
            payload: payload.to_string(),
            occurred_at: now,
            created_at: now,
            published: false,
            published_at: None,
            replay_resolution_id: None,
            origin_traceparent: Some(
                "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01".into(),
            ),
        })
        .await?;
    Ok(source)
}
fn reader(f: &Fixture) -> PgNotificationAuthorityRepository {
    PgNotificationAuthorityRepository::new(f.pool.clone(), 2).unwrap()
}
fn policy(s: &Source) -> NotificationScopeDto {
    NotificationScopeDto::RecipientPolicy {
        recipient_id: s.recipient,
    }
}
async fn reference(
    f: &Fixture,
    c: &TargetDeliveryClaimDto,
    scope: NotificationScopeDto,
) -> Result<NotificationOwnerReferenceDto> {
    let o = PgDeliveryAuthorityRepository::new(f.pool.clone())
        .resolve_claimed_delivery(&DeliveryClaimLookupDto::new(c.clone())?)
        .await?;
    Ok(notification_reference(&o, scope)?)
}
async fn snapshot(f: &Fixture) -> Result<String> {
    Ok(sqlx::query_scalar("select jsonb_build_object('outbox',(select jsonb_agg(to_jsonb(x) order by event_id) from post_event_outbox x),'jobs',(select jsonb_agg(to_jsonb(x) order by event_id,target) from post_event_delivery_jobs x),'attempts',(select jsonb_agg(to_jsonb(x) order by id) from post_event_delivery_attempts x),'publications',(select jsonb_agg(to_jsonb(x) order by event_id,target) from event_publications x),'posts',(select jsonb_agg(to_jsonb(x) order by id) from posts x),'resolutions',(select jsonb_agg(to_jsonb(x) order by resolution_id) from post_event_dead_letter_resolutions x))::text").fetch_one(&f.pool).await?)
}
async fn denied(f: &Fixture, r: &NotificationOwnerReferenceDto) -> Result<()> {
    let before = snapshot(f).await?;
    assert_eq!(
        reader(f).resolve_notification_scope(r).await.err(),
        Some(Error::Denied)
    );
    assert_eq!(snapshot(f).await?, before);
    Ok(())
}

#[tokio::test]
async fn automatic_direct_intent_survives_purge_and_preserves_exact_original_identity() -> Result<()>
{
    let f = Fixture::new(29).await?;
    for kind in ["POST_LIKED", "POST_COMMENTED", "COMMENT"] {
        let s = seed(&f, kind).await?;
        let c = f
            .claim(DeliveryTarget::Internal, &Default::default())
            .await?;
        assert_eq!(c.event.event_id, s.event);
        let r = reference(&f, &c, policy(&s)).await?;
        sqlx::query("delete from posts where id=$1")
            .bind(s.post)
            .execute(&f.pool)
            .await?;
        let before = snapshot(&f).await?;
        let o = reader(&f).resolve_notification_scope(&r).await?.unwrap();
        assert!(o.visibility().is_none());
        assert!(!o.requires_following());
        let n = o.intent().notification_for(s.recipient)?;
        assert_eq!(n.event_id, s.event);
        assert_eq!(n.notification_type, kind);
        assert_eq!(n.actor_id, Some(s.actor));
        assert_eq!(n.occurred_at, c.event.occurred_at.to_rfc3339());
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&n.payload_json)?,
            serde_json::json!({"payloadVersion":1,"postId":s.post})
        );
        assert_eq!(snapshot(&f).await?, before);
        f.schedule().complete_target(&c).await?;
    }
    f.shutdown().await
}

#[tokio::test]
async fn current_created_author_visibility_and_lifecycle_are_fresh_read_only_observations(
) -> Result<()> {
    let f = Fixture::new(29).await?;
    let s = seed(&f, "POST_CREATED").await?;
    let c = f
        .claim(DeliveryTarget::Internal, &Default::default())
        .await?;
    let r = reference(&f, &c, policy(&s)).await?;
    let page = reference(
        &f,
        &c,
        NotificationScopeDto::FollowerPage {
            author_id: s.actor,
            cursor: None,
            limit: 2,
        },
    )
    .await?;
    assert!(reader(&f)
        .resolve_notification_scope(&page)
        .await?
        .is_some());
    for (visibility, present, following) in [
        ("PUBLIC", true, false),
        ("FRIENDS", true, true),
        ("PRIVATE", false, false),
        ("UNKNOWN", false, false),
    ] {
        sqlx::query("update posts set visibility=$2 where id=$1")
            .bind(s.post)
            .bind(visibility)
            .execute(&f.pool)
            .await?;
        let before = snapshot(&f).await?;
        let o = reader(&f).resolve_notification_scope(&r).await?;
        assert_eq!(o.is_some(), present);
        if let Some(o) = o {
            assert_eq!(o.requires_following(), following);
            assert_eq!(
                o.visibility(),
                Some(if following {
                    NotificationVisibilityDto::Friends
                } else {
                    NotificationVisibilityDto::Public
                })
            );
        }
        assert_eq!(snapshot(&f).await?, before);
    }
    sqlx::query("update posts set visibility='PUBLIC',author_id=$2 where id=$1")
        .bind(s.post)
        .bind(Uuid::new_v4())
        .execute(&f.pool)
        .await?;
    denied(&f, &r).await?;
    sqlx::query("update posts set author_id=$2,moderation_hidden_at=clock_timestamp() where id=$1")
        .bind(s.post)
        .bind(s.actor)
        .execute(&f.pool)
        .await?;
    assert!(reader(&f).resolve_notification_scope(&r).await?.is_none());
    sqlx::query(
        "update posts set moderation_hidden_at=null,deleted_at=clock_timestamp() where id=$1",
    )
    .bind(s.post)
    .execute(&f.pool)
    .await?;
    assert!(reader(&f).resolve_notification_scope(&r).await?.is_none());
    sqlx::query("delete from posts where id=$1")
        .bind(s.post)
        .execute(&f.pool)
        .await?;
    assert!(reader(&f).resolve_notification_scope(&r).await?.is_none());
    f.shutdown().await
}

#[tokio::test]
async fn ordinary_pending_retry_and_completed_replay_keep_actual_frozen_budget() -> Result<()> {
    let f = Fixture::new(29).await?;
    let s = seed(&f, "POST_COMMENTED").await?;
    let cfg = DeliveryScheduleConfigDto {
        max_attempts: 2,
        ..Default::default()
    };
    let first = f
        .schedule()
        .claim_replay_target(s.event, DeliveryTarget::Internal, &cfg)
        .await?;
    let r = reference(&f, &first, policy(&s)).await?;
    assert!(reader(&f).resolve_notification_scope(&r).await?.is_some());
    let o = PgDeliveryAuthorityRepository::new(f.pool.clone())
        .resolve_claimed_delivery(&DeliveryClaimLookupDto::new(first.clone())?)
        .await?;
    assert_eq!(
        o.provenance().lease_kind,
        DeliveryLeaseKindDto::Replay(DeliveryReplayRestoreDto::Pending)
    );
    assert!(o.provenance().resolution_id.is_none());
    f.schedule().fail_target(&first).await?;
    denied(&f, &r).await?;
    f.due(s.event, DeliveryTarget::Internal).await?;
    let second = f
        .schedule()
        .claim_replay_target(s.event, DeliveryTarget::Internal, &cfg)
        .await?;
    assert_eq!(second.attempt_number, 2);
    let r = reference(&f, &second, policy(&s)).await?;
    assert!(reader(&f).resolve_notification_scope(&r).await?.is_some());
    let o = PgDeliveryAuthorityRepository::new(f.pool.clone())
        .resolve_claimed_delivery(&DeliveryClaimLookupDto::new(second.clone())?)
        .await?;
    assert_eq!(
        o.provenance().lease_kind,
        DeliveryLeaseKindDto::Replay(DeliveryReplayRestoreDto::Retry)
    );
    f.schedule().complete_target(&second).await?;
    let completed = f
        .schedule()
        .claim_replay_target(s.event, DeliveryTarget::Internal, &cfg)
        .await?;
    assert_eq!(completed.attempt_number, 2);
    let r = reference(&f, &completed, policy(&s)).await?;
    let before = snapshot(&f).await?;
    assert!(reader(&f).resolve_notification_scope(&r).await?.is_some());
    assert_eq!(snapshot(&f).await?, before);
    let o = PgDeliveryAuthorityRepository::new(f.pool.clone())
        .resolve_claimed_delivery(&DeliveryClaimLookupDto::new(completed)?)
        .await?;
    assert_eq!(
        o.provenance().lease_kind,
        DeliveryLeaseKindDto::Replay(DeliveryReplayRestoreDto::Succeeded)
    );
    f.shutdown().await
}

#[tokio::test]
async fn actual_legacy_migration_preserves_published_skipped_and_succeeded_zero_count_replay(
) -> Result<()> {
    for checkpoint in [false, true] {
        let f = Fixture::new(16).await?;
        let s = seed(&f, "COMMENT").await?;
        if checkpoint {
            f.checkpoint(s.event, DeliveryTarget::Internal).await?;
        }
        sqlx::query("update post_event_outbox set published=true,published_at=clock_timestamp() where event_id=$1").bind(s.event).execute(&f.pool).await?;
        f.upgrade().await?;
        let c = f
            .schedule()
            .claim_replay_target(s.event, DeliveryTarget::Internal, &Default::default())
            .await?;
        assert!(c.event.published);
        assert_eq!(c.attempt_number, 0);
        let r = reference(&f, &c, policy(&s)).await?;
        let before = snapshot(&f).await?;
        let o = reader(&f).resolve_notification_scope(&r).await?.unwrap();
        assert_eq!(o.intent().post_id(), s.post);
        assert_eq!(snapshot(&f).await?, before);
        let owned = PgDeliveryAuthorityRepository::new(f.pool.clone())
            .resolve_claimed_delivery(&DeliveryClaimLookupDto::new(c)?)
            .await?;
        assert_eq!(
            owned.provenance().lease_kind,
            DeliveryLeaseKindDto::Replay(if checkpoint {
                DeliveryReplayRestoreDto::Succeeded
            } else {
                DeliveryReplayRestoreDto::Skipped
            })
        );
        assert_eq!(owned.provenance().target_required, checkpoint);
        f.shutdown().await?;
    }
    Ok(())
}

#[tokio::test]
async fn audited_auto_recovery_is_distinct_and_provenance_changes_deny_old_reference() -> Result<()>
{
    let f = Fixture::new(29).await?;
    let s = seed(&f, "POST_LIKED").await?;
    let first = f
        .claim(
            DeliveryTarget::Internal,
            &DeliveryScheduleConfigDto {
                max_attempts: 1,
                ..Default::default()
            },
        )
        .await?;
    f.schedule().fail_target(&first).await?;
    let resolution = Uuid::new_v4();
    f.resolve(s.event, resolution).await?;
    let c = f
        .claim(DeliveryTarget::Internal, &Default::default())
        .await?;
    let r = reference(&f, &c, policy(&s)).await?;
    assert!(reader(&f).resolve_notification_scope(&r).await?.is_some());
    let o = PgDeliveryAuthorityRepository::new(f.pool.clone())
        .resolve_claimed_delivery(&DeliveryClaimLookupDto::new(c.clone())?)
        .await?;
    assert_eq!(o.provenance().lease_kind, DeliveryLeaseKindDto::Automatic);
    assert_eq!(o.provenance().resolution_id, Some(resolution));
    assert_eq!(
        o.provenance().source,
        post_service::domain::post::DeliveryAttemptSource::AdminReplay
    );
    sqlx::query("update post_event_delivery_jobs set max_attempts=max_attempts+1 where event_id=$1 and target='INTERNAL'").bind(s.event).execute(&f.pool).await?;
    denied(&f, &r).await?;
    f.shutdown().await
}

#[tokio::test]
async fn forged_source_claim_purpose_recipient_page_and_sibling_target_deny_without_mutation(
) -> Result<()> {
    let f = Fixture::new(29).await?;
    let s = seed(&f, "POST_COMMENTED").await?;
    let c = f
        .claim(DeliveryTarget::Internal, &Default::default())
        .await?;
    let r = reference(&f, &c, policy(&s)).await?;
    for case in 0..11 {
        let mut wrong = r.clone();
        match case {
            0 => wrong.event_id = Uuid::new_v4(),
            1 => wrong.aggregate_id = Uuid::new_v4(),
            2 => wrong.event_digest[0] ^= 1,
            3 => wrong.attempt_digest[0] ^= 1,
            4 => wrong.lease_token = Uuid::new_v4(),
            5 => wrong.attempt_id = Uuid::new_v4(),
            6 => wrong.attempt_number += 1,
            7 => wrong.lease_expires_at += Duration::microseconds(1),
            8 => {
                wrong.scope = NotificationScopeDto::RecipientPolicy {
                    recipient_id: Uuid::new_v4(),
                }
            }
            9 => {
                wrong.scope = NotificationScopeDto::FollowerPage {
                    author_id: s.actor,
                    cursor: None,
                    limit: 2,
                }
            }
            _ => {
                let sibling = f.claim(DeliveryTarget::Cache, &Default::default()).await?;
                wrong.lease_token = sibling.lease_token;
                wrong.attempt_id = sibling.attempt_id;
                wrong.lease_expires_at = sibling.lease_expires_at;
            }
        }
        denied(&f, &wrong).await?;
    }
    let created = seed(&f, "POST_CREATED").await?;
    // Finish the older INTERNAL head before claiming another aggregate.
    f.schedule().complete_target(&c).await?;
    let c = f
        .claim(DeliveryTarget::Internal, &Default::default())
        .await?;
    let mut page = reference(
        &f,
        &c,
        NotificationScopeDto::FollowerPage {
            author_id: created.actor,
            cursor: None,
            limit: 2,
        },
    )
    .await?;
    page.scope = NotificationScopeDto::FollowerPage {
        author_id: created.actor,
        cursor: None,
        limit: 3,
    };
    denied(&f, &page).await?;
    page.scope = NotificationScopeDto::FollowerPage {
        author_id: Uuid::new_v4(),
        cursor: None,
        limit: 2,
    };
    denied(&f, &page).await?;
    // Trace is part of the exact event digest, never caller metadata.
    let mut trace_changed = c.event.clone();
    trace_changed.origin_traceparent = None;
    let mut forged = page;
    forged.scope = NotificationScopeDto::RecipientPolicy {
        recipient_id: created.recipient,
    };
    forged.event_digest = Sha256::digest(notification_event_identity(&trace_changed)?).into();
    denied(&f, &forged).await?;
    f.shutdown().await
}

#[tokio::test]
async fn source_read_wait_cannot_outlive_original_lease_or_renew_owner() -> Result<()> {
    let f = Fixture::new(29).await?;
    let s = seed(&f, "POST_CREATED").await?;
    let c = f
        .claim(
            DeliveryTarget::Internal,
            &DeliveryScheduleConfigDto {
                lease_duration_ms: 2000,
                ..Default::default()
            },
        )
        .await?;
    let r = reference(&f, &c, policy(&s)).await?;
    let mut blocker = f.pool.begin().await?;
    sqlx::query("lock table posts in access exclusive mode")
        .execute(&mut *blocker)
        .await?;
    let repo = reader(&f);
    let task = tokio::spawn(async move { repo.resolve_notification_scope(&r).await });
    let mut waited = false;
    for _ in 0..100 {
        waited=sqlx::query_scalar::<_,bool>("select exists(select 1 from pg_stat_activity where datname=current_database() and wait_event_type='Lock' and query like 'select id,author_id,visibility from posts%')").fetch_one(&f.pool).await?;
        if waited {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    ensure!(waited, "owner must actually wait on the current Post read");
    sqlx::query("select pg_sleep(2.1)")
        .execute(&mut *blocker)
        .await?;
    blocker.commit().await?;
    assert_eq!(task.await?.err(), Some(Error::Denied));
    let row=sqlx::query("select state,attempt_count,lease_expires_at from post_event_delivery_jobs where event_id=$1 and target='INTERNAL'").bind(s.event).fetch_one(&f.pool).await?;
    assert_eq!(row.get::<String, _>("state"), "LEASED");
    assert_eq!(row.get::<i32, _>("attempt_count"), 1);
    assert_eq!(
        row.get::<chrono::DateTime<Utc>, _>("lease_expires_at"),
        c.lease_expires_at
    );
    f.shutdown().await
}

#[tokio::test]
async fn unavailable_owner_storage_is_an_error_instead_of_source_suppression() -> Result<()> {
    let f = Fixture::new(29).await?;
    let s = seed(&f, "POST_CREATED").await?;
    let c = f
        .claim(DeliveryTarget::Internal, &Default::default())
        .await?;
    let r = reference(&f, &c, policy(&s)).await?;
    let independent = PgPool::connect_with(f.pool.connect_options().as_ref().clone()).await?;
    let repo = PgNotificationAuthorityRepository::new(independent.clone(), 2)?;
    independent.close().await;
    assert_eq!(
        repo.resolve_notification_scope(&r).await.err(),
        Some(Error::Unavailable)
    );
    f.shutdown().await
}

#[tokio::test]
async fn inactive_created_source_suppresses_before_malformed_payload_parsing() -> Result<()> {
    // The legacy listener checks current Post state before parsing a created
    // payload. Preserve that order even when constructing the reference.
    let f = Fixture::new(29).await?;
    let s = seed(&f, "POST_CREATED").await?;
    sqlx::query("update post_event_outbox set payload='malformed' where event_id=$1")
        .bind(s.event)
        .execute(&f.pool)
        .await?;
    let c = f
        .claim(DeliveryTarget::Internal, &Default::default())
        .await?;
    let r = reference(&f, &c, policy(&s)).await?;
    denied(&f, &r).await?;
    sqlx::query("delete from posts where id=$1")
        .bind(s.post)
        .execute(&f.pool)
        .await?;
    let before = snapshot(&f).await?;
    assert!(reader(&f).resolve_notification_scope(&r).await?.is_none());
    assert_eq!(snapshot(&f).await?, before);
    f.shutdown().await
}

fn network(r: &NotificationOwnerReferenceDto) -> NotificationNetworkReferenceDto {
    NotificationNetworkReferenceDto {
        event_id: r.event_id,
        aggregate_id: r.aggregate_id,
        event_digest: r.event_digest,
        attempt_digest: r.attempt_digest,
        lease_digest: Sha256::digest(r.lease_token.as_bytes()).into(),
        attempt_id: r.attempt_id,
        attempt_number: r.attempt_number,
        lease_expires_at: r.lease_expires_at,
        scope: r.scope.clone(),
    }
}

#[tokio::test]
async fn digest_only_lookup_owns_direct_source_and_rejects_forged_or_completed_claims() -> Result<()>
{
    let f = Fixture::new(29).await?;
    let s = seed(&f, "POST_COMMENTED").await?;
    let c = f
        .claim(DeliveryTarget::Internal, &Default::default())
        .await?;
    let r = network(&reference(&f, &c, policy(&s)).await?);
    sqlx::query("delete from posts where id=$1")
        .bind(s.post)
        .execute(&f.pool)
        .await?;
    let before = snapshot(&f).await?;
    let observed = reader(&f).resolve_notification_source(&r).await?;
    assert_eq!(observed.scope().unwrap().intent().post_id(), s.post);
    assert!(observed.scope().unwrap().visibility().is_none());
    assert!(observed.checked_at() < c.lease_expires_at);
    assert_eq!(snapshot(&f).await?, before);
    for case in 0..9 {
        let mut wrong = r.clone();
        match case {
            0 => wrong.lease_digest[0] ^= 1,
            1 => wrong.event_digest[0] ^= 1,
            2 => wrong.attempt_digest[0] ^= 1,
            3 => wrong.attempt_id = Uuid::new_v4(),
            4 => wrong.attempt_number += 1,
            5 => wrong.lease_expires_at += Duration::microseconds(1),
            6 => wrong.aggregate_id = Uuid::new_v4(),
            7 => wrong.event_id = Uuid::new_v4(),
            _ => {
                wrong.scope = NotificationScopeDto::RecipientPolicy {
                    recipient_id: Uuid::new_v4(),
                }
            }
        }
        assert_eq!(
            reader(&f).resolve_notification_source(&wrong).await.err(),
            Some(Error::Denied)
        );
        assert_eq!(snapshot(&f).await?, before);
    }
    // A valid token digest cannot hide changed frozen provenance.
    sqlx::query("update post_event_delivery_jobs set max_attempts=max_attempts+1 where event_id=$1 and target='INTERNAL'")
        .bind(s.event).execute(&f.pool).await?;
    assert_eq!(
        reader(&f).resolve_notification_source(&r).await.err(),
        Some(Error::Denied)
    );
    f.schedule().complete_target(&c).await?;
    assert_eq!(
        reader(&f).resolve_notification_source(&r).await.err(),
        Some(Error::Denied)
    );
    f.shutdown().await
}

#[tokio::test]
async fn digest_only_suppression_preserves_owned_clock_and_unavailable_is_not_suppression(
) -> Result<()> {
    let f = Fixture::new(29).await?;
    let s = seed(&f, "POST_CREATED").await?;
    sqlx::query("update post_event_outbox set payload='malformed' where event_id=$1")
        .bind(s.event)
        .execute(&f.pool)
        .await?;
    let c = f
        .claim(DeliveryTarget::Internal, &Default::default())
        .await?;
    let r = network(&reference(&f, &c, policy(&s)).await?);
    assert_eq!(
        reader(&f).resolve_notification_source(&r).await.err(),
        Some(Error::Denied)
    );
    sqlx::query("delete from posts where id=$1")
        .bind(s.post)
        .execute(&f.pool)
        .await?;
    let before = snapshot(&f).await?;
    // Both the observation and lease use PostgreSQL's clock. The host and
    // Docker VM may have different wall clocks, so bracket with this owner DB.
    let start: chrono::DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
        .fetch_one(&f.pool)
        .await?;
    let observed = reader(&f).resolve_notification_source(&r).await?;
    let end: chrono::DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
        .fetch_one(&f.pool)
        .await?;
    assert!(observed.scope().is_none());
    assert!(
        observed.checked_at() >= start && observed.checked_at() <= end,
        "owner observation {} must be within database interval [{start}, {end}]",
        observed.checked_at(),
    );
    assert!(
        observed.checked_at() < c.lease_expires_at,
        "owner observation {} must precede lease expiry {}",
        observed.checked_at(),
        c.lease_expires_at,
    );
    assert_eq!(snapshot(&f).await?, before);
    let independent = PgPool::connect_with(f.pool.connect_options().as_ref().clone()).await?;
    let repo = PgNotificationAuthorityRepository::new(independent.clone(), 2)?;
    independent.close().await;
    assert_eq!(
        repo.resolve_notification_source(&r).await.err(),
        Some(Error::Unavailable)
    );
    f.shutdown().await
}

#[tokio::test]
async fn digest_only_legacy_zero_count_replay_resolves_actual_internal_lease() -> Result<()> {
    let f = Fixture::new(16).await?;
    let s = seed(&f, "COMMENT").await?;
    sqlx::query("update post_event_outbox set published=true,published_at=clock_timestamp() where event_id=$1")
        .bind(s.event).execute(&f.pool).await?;
    f.upgrade().await?;
    let c = f
        .schedule()
        .claim_replay_target(s.event, DeliveryTarget::Internal, &Default::default())
        .await?;
    assert_eq!(c.attempt_number, 0);
    let r = network(&reference(&f, &c, policy(&s)).await?);
    let before = snapshot(&f).await?;
    let observed = reader(&f).resolve_notification_source(&r).await?;
    assert_eq!(observed.scope().unwrap().intent().post_id(), s.post);
    assert_eq!(snapshot(&f).await?, before);
    f.shutdown().await
}

#[tokio::test]
async fn owning_notification_rpc_uses_mtls_real_claim_and_exact_stored_identity() -> Result<()> {
    use platform::actor_delegation::notification::*;
    use platform::grpc_auth::{CallerRegistry, Workload};
    use platform::grpc_method_policy::guard_without_delegation;
    use platform::grpc_tls::TlsMaterial;
    use post_service::adapters::r#in::grpc::{
        notification_authority::{NotificationOwnerGrpcServer, NOTIFICATION_OWNER_POLICY},
        resource_authority::proto,
    };
    use std::sync::Arc;
    use tokio_stream::wrappers::TcpListenerStream;
    use tonic::{transport::Server, Request};
    fn tls(workload: Workload) -> Result<TlsMaterial> {
        let leaf = test_support::tls::issue(workload.as_str())?;
        TlsMaterial::load(&leaf.cert, &leaf.key, &leaf.ca)
    }
    fn identified(
        message: proto::ResolveNotificationSourceRequest,
    ) -> Request<proto::ResolveNotificationSourceRequest> {
        let mut r = Request::new(message);
        r.metadata_mut()
            .insert("x-service-caller", "gateway".parse().unwrap());
        r.metadata_mut()
            .insert("x-service-token", "gateway-fixture".parse().unwrap());
        r
    }
    fn message(p: &PreparedNotification) -> proto::ResolveNotificationSourceRequest {
        proto::ResolveNotificationSourceRequest {
            binding: p.binding().encode().unwrap(),
            framed_body: p.body().to_vec(),
            challenge: p.challenge().to_string(),
        }
    }
    struct Task(tokio::task::JoinHandle<Result<(), tonic::transport::Error>>);
    impl Drop for Task {
        fn drop(&mut self) {
            self.0.abort();
        }
    }
    let f = Fixture::new(29).await?;
    let signer = NotificationOwnerSigner::new(&[71; 32], LifetimePolicy::new(10_000_000).unwrap())?;
    let issuer = NotificationIssuer::new(
        &[72; 32],
        &[signer.public_key()],
        &[signer.public_key()],
        LifetimePolicy::new(10_000_000).unwrap(),
    )?;
    let server = NotificationOwnerGrpcServer::new(Arc::new(reader(&f)), signer);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("https://{}", listener.local_addr()?);
    let registry = CallerRegistry::new()
        .register(Workload::Gateway, "gateway-fixture")?
        .register(Workload::Post, "post-fixture")?;
    let mut task = Task(tokio::spawn(Server::builder().tls_config(tls(Workload::Post)?.server_config()?)?
        .add_service(guard_without_delegation(&NOTIFICATION_OWNER_POLICY,
            proto::notification_owner_authority_service_server::NotificationOwnerAuthorityServiceServer::new(server), registry)?)
        .serve_with_incoming(TcpListenerStream::new(listener))));
    let channel = tls(Workload::Gateway)?
        .endpoint(&endpoint, Workload::Post)?
        .connect()
        .await?;
    let mut client = proto::notification_owner_authority_service_client::NotificationOwnerAuthorityServiceClient::new(channel);
    let s = seed(&f, "POST_COMMENTED").await?;
    let c = f
        .claim(DeliveryTarget::Internal, &Default::default())
        .await?;
    let r = reference(&f, &c, policy(&s)).await?;
    let claim = NotificationClaim {
        event_id: r.event_id,
        aggregate_id: r.aggregate_id,
        event_digest: NotificationClaim::encode_digest(&r.event_digest),
        attempt_digest: NotificationClaim::encode_digest(&r.attempt_digest),
        lease_digest: NotificationClaim::digest(r.lease_token.as_bytes()),
        attempt_id: r.attempt_id,
        attempt: r.attempt_number,
        lease_expires_micros: r.lease_expires_at.timestamp_micros(),
    };
    let d = NotificationDelivery {
        claim,
        recipient_id: s.recipient,
        actor_id: s.actor,
        post_id: s.post,
        kind: NotificationKind::Commented,
        occurred_at: c.event.occurred_at.to_rfc3339(),
        membership: None,
    };
    let b = NotificationBinding::Delivery(d.clone());
    let p = issuer
        .prepare(
            Workload::Post,
            b.clone(),
            b.frame().unwrap(),
            Utc::now().timestamp_micros(),
        )
        .map_err(status)?;
    assert!(client
        .resolve_notification_source(message(&p))
        .await
        .is_err());
    let bad_channel = tls(Workload::Post)?
        .endpoint(&endpoint, Workload::Post)?
        .connect()
        .await?;
    let mut stolen = proto::notification_owner_authority_service_client::NotificationOwnerAuthorityServiceClient::new(bad_channel);
    assert!(stolen
        .resolve_notification_source(identified(message(&p)))
        .await
        .is_err());
    sqlx::query("delete from posts where id=$1")
        .bind(s.post)
        .execute(&f.pool)
        .await?;
    let before = snapshot(&f).await?;
    let evidence = client
        .resolve_notification_source(identified(message(&p)))
        .await?
        .into_inner()
        .evidence;
    assert!(issuer
        .source_suppression(&p, &evidence, Utc::now().timestamp_micros())
        .map_err(status)?
        .is_none());
    issuer
        .prepare_policy(p, &evidence, Utc::now().timestamp_micros())
        .map_err(status)?;
    for case in 0..5 {
        let mut wrong = d.clone();
        match case {
            0 => wrong.actor_id = Uuid::new_v4(),
            1 => wrong.recipient_id = Uuid::new_v4(),
            2 => wrong.occurred_at = Utc::now().to_rfc3339(),
            3 => wrong.kind = NotificationKind::Liked,
            _ => wrong.claim.lease_digest = NotificationClaim::digest(Uuid::new_v4().as_bytes()),
        }
        let b = NotificationBinding::Delivery(wrong);
        let p = issuer
            .prepare(
                Workload::Post,
                b.clone(),
                b.frame().unwrap(),
                Utc::now().timestamp_micros(),
            )
            .map_err(status)?;
        assert_eq!(
            client
                .resolve_notification_source(identified(message(&p)))
                .await
                .unwrap_err()
                .code(),
            tonic::Code::PermissionDenied
        );
    }
    assert_eq!(snapshot(&f).await?, before);
    f.schedule().complete_target(&c).await?;
    let s = seed(&f, "POST_CREATED").await?;
    sqlx::query("update post_event_outbox set payload='malformed' where event_id=$1")
        .bind(s.event)
        .execute(&f.pool)
        .await?;
    let c = f
        .claim(DeliveryTarget::Internal, &Default::default())
        .await?;
    let r = reference(&f, &c, policy(&s)).await?;
    sqlx::query("delete from posts where id=$1")
        .bind(s.post)
        .execute(&f.pool)
        .await?;
    let claim = NotificationClaim {
        event_id: r.event_id,
        aggregate_id: r.aggregate_id,
        event_digest: NotificationClaim::encode_digest(&r.event_digest),
        attempt_digest: NotificationClaim::encode_digest(&r.attempt_digest),
        lease_digest: NotificationClaim::digest(r.lease_token.as_bytes()),
        attempt_id: r.attempt_id,
        attempt: r.attempt_number,
        lease_expires_micros: r.lease_expires_at.timestamp_micros(),
    };
    let page = NotificationPage {
        claim: claim.clone(),
        author_id: s.actor,
        cursor: None,
        limit: 2,
    };
    let b = NotificationBinding::FollowerPage(page.clone());
    let p = issuer
        .prepare(
            Workload::Post,
            b.clone(),
            b.frame().unwrap(),
            Utc::now().timestamp_micros(),
        )
        .map_err(status)?;
    let evidence = client
        .resolve_notification_source(identified(message(&p)))
        .await?
        .into_inner()
        .evidence;
    assert!(issuer
        .source_suppression(&p, &evidence, Utc::now().timestamp_micros())
        .map_err(status)?
        .is_some());
    let d = NotificationDelivery {
        claim,
        recipient_id: s.recipient,
        actor_id: s.actor,
        post_id: s.post,
        kind: NotificationKind::Created,
        occurred_at: c.event.occurred_at.to_rfc3339(),
        membership: Some("unverified-selected-membership".into()),
    };
    for case in 0..4 {
        let mut wrong = d.clone();
        match case {
            1 => wrong.actor_id = Uuid::new_v4(),
            2 => wrong.occurred_at = Utc::now().to_rfc3339(),
            3 => {
                wrong.kind = NotificationKind::Liked;
                wrong.membership = None;
            }
            _ => {}
        }
        let b = NotificationBinding::Delivery(wrong);
        let p = issuer
            .prepare(
                Workload::Post,
                b.clone(),
                b.frame().unwrap(),
                Utc::now().timestamp_micros(),
            )
            .map_err(status)?;
        let result = client
            .resolve_notification_source(identified(message(&p)))
            .await;
        if case == 0 {
            assert!(issuer
                .source_suppression(
                    &p,
                    &result?.into_inner().evidence,
                    Utc::now().timestamp_micros()
                )
                .map_err(status)?
                .is_some());
        } else {
            assert_eq!(result.unwrap_err().code(), tonic::Code::PermissionDenied);
        }
    }
    // The dummy membership above is never promoted to delivery authority.
    task.0.abort();
    let _ = (&mut task.0).await;
    f.shutdown().await
}

#[tokio::test]
async fn final_owner_wait_does_not_redate_suppressed_post_observation() -> Result<()> {
    use platform::actor_delegation::notification::*;
    let f = Fixture::new(29).await?;
    let s = seed(&f, "POST_CREATED").await?;
    let c = f
        .claim(DeliveryTarget::Internal, &Default::default())
        .await?;
    let local = reference(&f, &c, policy(&s)).await?;
    let r = network(&local);
    sqlx::query("delete from posts where id=$1")
        .bind(s.post)
        .execute(&f.pool)
        .await?;
    let mut post_blocker = f.pool.begin().await?;
    sqlx::query("lock table posts in access exclusive mode")
        .execute(&mut *post_blocker)
        .await?;
    let repo = reader(&f);
    let task = tokio::spawn(async move { repo.resolve_notification_source(&r).await });
    let mut waiting = false;
    for _ in 0..100 {
        waiting = sqlx::query_scalar::<_, bool>("select exists(select 1 from pg_stat_activity where datname=current_database() and wait_event_type='Lock' and query like 'select id,author_id,visibility from posts%')").fetch_one(&f.pool).await?;
        if waiting {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    ensure!(
        waiting,
        "source read must wait before the final owner check"
    );
    let mut owner_blocker = f.pool.begin().await?;
    sqlx::query("select event_id from post_event_delivery_jobs where event_id=$1 and target='INTERNAL' for update")
        .bind(s.event).fetch_one(&mut *owner_blocker).await?;
    let source_released: chrono::DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
        .fetch_one(&mut *post_blocker)
        .await?;
    post_blocker.commit().await?;
    waiting = false;
    for _ in 0..100 {
        waiting = sqlx::query_scalar::<_, bool>("select exists(select 1 from pg_stat_activity where datname=current_database() and wait_event_type='Lock' and query like 'select * from post_event_delivery_jobs%for share')").fetch_one(&f.pool).await?;
        if waiting {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    ensure!(
        waiting,
        "final owner must actually wait after the source read"
    );
    sqlx::query("select pg_sleep(0.2)")
        .execute(&mut *owner_blocker)
        .await?;
    owner_blocker.commit().await?;
    let observed = task.await??;
    assert!(observed.scope().is_none());
    assert!(observed.checked_at() <= source_released);
    assert_eq!(observed.reference().attempt_id, c.attempt_id);
    let binding = NotificationBinding::FollowerPage(NotificationPage {
        claim: NotificationClaim {
            event_id: local.event_id,
            aggregate_id: local.aggregate_id,
            event_digest: NotificationClaim::encode_digest(&local.event_digest),
            attempt_digest: NotificationClaim::encode_digest(&local.attempt_digest),
            lease_digest: NotificationClaim::digest(local.lease_token.as_bytes()),
            attempt_id: local.attempt_id,
            attempt: local.attempt_number,
            lease_expires_micros: local.lease_expires_at.timestamp_micros(),
        },
        author_id: s.actor,
        cursor: None,
        limit: 2,
    });
    let now = Utc::now().timestamp_micros();
    assert!(now < c.lease_expires_at.timestamp_micros());
    let signer = NotificationOwnerSigner::new(&[71; 32], LifetimePolicy::new(100_000).unwrap())?;
    assert!(signer
        .attest_suppressed(
            &binding,
            &binding.frame().unwrap(),
            Uuid::new_v4(),
            observed.checked_at().timestamp_micros(),
            now
        )
        .is_err());
    f.shutdown().await
}

#[tokio::test]
async fn digest_hint_does_not_authorize_after_real_lease_rotation_during_owner_wait() -> Result<()>
{
    let f = Fixture::new(29).await?;
    let s = seed(&f, "POST_LIKED").await?;
    let c = f
        .claim(DeliveryTarget::Internal, &Default::default())
        .await?;
    let r = network(&reference(&f, &c, policy(&s)).await?);
    let mut blocker = f.pool.begin().await?;
    sqlx::query("select event_id from post_event_outbox where event_id=$1 for update")
        .bind(s.event)
        .fetch_one(&mut *blocker)
        .await?;
    let repo = reader(&f);
    let task = tokio::spawn(async move { repo.resolve_notification_source(&r).await });
    let mut waiting = false;
    for _ in 0..100 {
        waiting = sqlx::query_scalar::<_, bool>("select exists(select 1 from pg_stat_activity where datname=current_database() and wait_event_type='Lock' and query like 'select event_id,event_type%from post_event_outbox%for share')").fetch_one(&f.pool).await?;
        if waiting {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    ensure!(
        waiting,
        "owner must wait after reading the digest-matching token hint"
    );
    sqlx::query("update post_event_delivery_jobs set lease_token=$2 where event_id=$1 and target='INTERNAL'")
        .bind(s.event).bind(Uuid::new_v4()).execute(&f.pool).await?;
    blocker.commit().await?;
    let rotated = snapshot(&f).await?;
    assert_eq!(task.await?.err(), Some(Error::Denied));
    assert_eq!(snapshot(&f).await?, rotated);
    f.shutdown().await
}
