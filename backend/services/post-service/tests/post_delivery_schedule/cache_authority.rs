//! Dedicated CACHE lookup acceptance against the actual scheduler/PG owner.
use super::*;
use post_service::adapters::{
    out::persistence::{
        cache_authority_repository::PgCacheAuthorityRepository,
        delivery_authority_repository::PgDeliveryAuthorityRepository,
    },
    r#in::grpc::cache_authority::reference,
};
use post_service::application::{
    cache_authority::*,
    delivery_authority::{
        DeliveryAuthorityErrorDto as Error, DeliveryAuthorityRepositoryPort, DeliveryClaimLookupDto,
    },
};

async fn seed(f: &Fixture, kind: &str) -> Result<TargetDeliveryClaimDto> {
    let post = Uuid::new_v4();
    let actor = Uuid::new_v4();
    let author = Uuid::new_v4();
    let now = Utc::now() - Duration::seconds(1);
    let payload = match kind {
        "POST_COMMENTED" => {
            serde_json::json!({"postId":post,"authorId":actor,"actorId":actor,"notifyUserId":author})
        }
        "POST_COMMENT_DELETED" => {
            serde_json::json!({"postId":post,"authorId":actor,"actorId":actor,"postAuthorId":author})
        }
        _ => {
            serde_json::json!({"postId":post,"authorId":actor,"actorId":actor,"notifyUserId":actor})
        }
    };
    let post_author = if matches!(kind, "POST_COMMENTED" | "POST_COMMENT_DELETED") {
        author
    } else {
        actor
    };
    sqlx::query("insert into posts(id,author_id,content,visibility,created_at) values($1,$2,'cache source fixture','PUBLIC',clock_timestamp())")
        .bind(post).bind(post_author).execute(&f.pool).await?;
    PgEventOutbox::new(f.pool.clone())
        .save(&StoredEvent {
            event_id: Uuid::new_v4(),
            event_type: kind.into(),
            aggregate_id: post,
            aggregate_version: Some(1),
            origin_traceparent: Some(
                "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01".into(),
            ),
            actor_id: Some(actor),
            attributes: vec!["cache".into()],
            payload: payload.to_string(),
            occurred_at: now,
            created_at: now,
            published: false,
            published_at: None,
            replay_resolution_id: None,
        })
        .await?;
    // Capture engagement provenance while its owning Post exists, then model
    // a retained event after hard purge. Do not bypass the source trigger.
    sqlx::query("delete from posts where id=$1")
        .bind(post)
        .execute(&f.pool)
        .await?;
    f.claim(DeliveryTarget::Cache, &Default::default()).await
}
async fn reference_for(f: &Fixture, c: &TargetDeliveryClaimDto) -> Result<CachePageReferenceDto> {
    let owner = PgDeliveryAuthorityRepository::new(f.pool.clone())
        .resolve_claimed_delivery(&DeliveryClaimLookupDto::new(c.clone())?)
        .await?;
    let author = CacheInvalidationPlanDto::from_event(&c.event)?
        .feed_author_id()
        .unwrap();
    reference(&CachePageLookupDto::new(owner, author, None, 2)?)
}
async fn snapshot(f: &Fixture) -> Result<String> {
    Ok(sqlx::query_scalar("select jsonb_build_object('outbox',(select jsonb_agg(to_jsonb(x) order by event_id) from post_event_outbox x),'jobs',(select jsonb_agg(to_jsonb(x) order by event_id,target) from post_event_delivery_jobs x),'attempts',(select jsonb_agg(to_jsonb(x) order by id) from post_event_delivery_attempts x),'publications',(select jsonb_agg(to_jsonb(x) order by event_id,target) from event_publications x))::text").fetch_one(&f.pool).await?)
}
#[tokio::test]
async fn stored_event_derived_cache_authors_survive_missing_post_without_durable_mutation(
) -> Result<()> {
    let f = Fixture::new(29).await?;
    let reader = PgCacheAuthorityRepository::new(f.pool.clone(), 2)?;
    for kind in [
        "POST_CREATED",
        "POST_DELETED",
        "POST_COMMENTED",
        "POST_COMMENT_DELETED",
    ] {
        let c = seed(&f, kind).await?;
        let r = reference_for(&f, &c).await?;
        let before = snapshot(&f).await?;
        assert_eq!(
            sqlx::query_scalar::<_, i64>("select count(*) from posts where id=$1")
                .bind(c.event.aggregate_id)
                .fetch_one(&f.pool)
                .await?,
            0
        );
        let o = reader.resolve_cache_page(&r).await?;
        assert_eq!(o.reference().author_id, r.author_id);
        assert!(o.checked_at() < c.lease_expires_at);
        assert_eq!(snapshot(&f).await?, before);
        f.schedule().complete_target(&c).await?;
    }
    f.shutdown().await
}
#[tokio::test]
async fn forged_cache_source_attempt_generation_author_and_limit_are_read_only_denials(
) -> Result<()> {
    let f = Fixture::new(29).await?;
    let c = seed(&f, "POST_COMMENTED").await?;
    let r = reference_for(&f, &c).await?;
    let reader = PgCacheAuthorityRepository::new(f.pool.clone(), 2)?;
    for case in 0..10 {
        let mut wrong = r.clone();
        match case {
            0 => wrong.event_id = Uuid::new_v4(),
            1 => wrong.post_id = Uuid::new_v4(),
            2 => wrong.author_id = c.event.actor_id.unwrap(),
            3 => wrong.event_digest[0] ^= 1,
            4 => wrong.attempt_digest[0] ^= 1,
            5 => wrong.lease_token = Uuid::new_v4(),
            6 => wrong.attempt_id = Uuid::new_v4(),
            7 => wrong.attempt_number += 1,
            8 => wrong.lease_expires_at += Duration::microseconds(1),
            _ => wrong.limit = 3,
        }
        let before = snapshot(&f).await?;
        assert_eq!(
            reader.resolve_cache_page(&wrong).await.err(),
            Some(Error::Denied),
            "case {case}"
        );
        assert_eq!(snapshot(&f).await?, before);
    }
    f.shutdown().await
}
#[tokio::test]
async fn cache_authority_rejects_local_only_event_and_freshly_changed_attempt_budget() -> Result<()>
{
    let f = Fixture::new(29).await?;
    let c = seed(&f, "POST_CREATED").await?;
    let r = reference_for(&f, &c).await?;
    let reader = PgCacheAuthorityRepository::new(f.pool.clone(), 2)?;
    sqlx::query("update post_event_delivery_jobs set max_attempts=max_attempts+1 where event_id=$1 and target='CACHE'")
        .bind(c.event.event_id).execute(&f.pool).await?;
    let before = snapshot(&f).await?;
    assert_eq!(
        reader.resolve_cache_page(&r).await.err(),
        Some(Error::Denied)
    );
    assert_eq!(snapshot(&f).await?, before);
    f.schedule().complete_target(&c).await?;
    let local = seed(&f, "POST_LIKED").await?;
    let owned = PgDeliveryAuthorityRepository::new(f.pool.clone())
        .resolve_claimed_delivery(&DeliveryClaimLookupDto::new(local.clone())?)
        .await?;
    assert!(CachePageLookupDto::new(owned, r.author_id, None, 2).is_err());
    let mut wrong = r;
    wrong.event_id = local.event.event_id;
    wrong.post_id = local.event.aggregate_id;
    assert_eq!(
        reader.resolve_cache_page(&wrong).await.err(),
        Some(Error::Denied)
    );
    f.shutdown().await
}
#[tokio::test]
async fn cache_lookup_preserves_retained_trace_and_denies_closed_owner_storage() -> Result<()> {
    let f = Fixture::new(29).await?;
    let c = seed(&f, "POST_DELETED").await?;
    let r = reference_for(&f, &c).await?;
    let reader = PgCacheAuthorityRepository::new(f.pool.clone(), 2)?;
    sqlx::query(
        "update post_event_outbox set origin_traceparent='00-4bf92f3577b34da6a3ce929d0e0e4736-11f067aa0ba902b7-01' where event_id=$1",
    )
    .bind(c.event.event_id)
    .execute(&f.pool)
    .await?;
    assert_eq!(
        reader.resolve_cache_page(&r).await.err(),
        Some(Error::Denied)
    );
    let independent = PgPool::connect_with(f.pool.connect_options().as_ref().clone()).await?;
    let closed = PgCacheAuthorityRepository::new(independent.clone(), 2)?;
    independent.close().await;
    assert_eq!(
        closed.resolve_cache_page(&r).await.err(),
        Some(Error::Unavailable)
    );
    f.shutdown().await
}
