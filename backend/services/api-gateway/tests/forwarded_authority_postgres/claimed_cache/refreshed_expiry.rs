//! Queued commands execute against actual Redis after the authority expired.
use super::*;
use post_service::application::delivery_effect::DeliveryEffectDeadlineDto;
#[tokio::test]
async fn queued_post_and_feed_mutations_deny_at_redis_after_local_timeout() -> anyhow::Result<()> {
    let app = post_support::start_postgres_redis_app(false).await?;
    let effects = app.claimed_cache_effects().await?;
    let (feeds, posts) = app.claimed_cache_effect_ports();
    let client = app.claimed_cache_test_client();
    let mut admin = client.get_multiplexed_async_connection().await?;
    for post_mode in [true, false] {
        let id = Uuid::new_v4();
        let author = Uuid::new_v4();
        let cached = post_service::domain::post::Post {
            id,
            author_id: author,
            content: "expiry seed".into(),
            visibility: "PUBLIC".into(),
            created_at: chrono::Utc::now(),
            comment_count: 0,
            like_count: 0,
        };
        posts.set_post(&cached).await?;
        let key = post_service::adapters::out::cache::feed_cache::feed_cache_generation_key(author);
        assert_eq!(feeds.current_generation(author).await?, 0);
        let _: () = redis::cmd("CLIENT")
            .arg("PAUSE")
            .arg(600)
            .arg("WRITE")
            .query_async(&mut admin)
            .await?;
        let bound = DeliveryEffectDeadlineDto::new(
            chrono::Utc::now() + chrono::Duration::milliseconds(200),
            std::time::Instant::now() + Duration::from_millis(200),
        )?;
        let effect = effects.clone();
        let task = tokio::spawn(async move {
            if post_mode {
                effect.evict_post(id, bound).await
            } else {
                effect.invalidate_feeds(&[author], bound).await
            }
        });
        tokio::time::timeout(Duration::from_millis(120), async {
            loop {
                let clients: String = redis::cmd("CLIENT")
                    .arg("LIST")
                    .query_async(&mut admin)
                    .await
                    .unwrap();
                if clients.lines().any(|line| line.contains("cmd=eval")) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
        })
        .await?;
        let error = tokio::time::timeout(Duration::from_secs(1), task)
            .await??
            .unwrap_err();
        assert_eq!(
            DeliveryEffectFailure::classify(&error),
            DeliveryEffectFailure::Uncertain,
            "dispatched command has no reliable acknowledgement"
        );
        tokio::time::sleep(Duration::from_millis(650)).await;
        // Fresh connection observes server state after queue release. A local
        // timeout alone is deliberately not used as no-effect evidence.
        let mut independent = client.get_multiplexed_async_connection().await?;
        let exists: i64 = redis::cmd("EXISTS")
            .arg(format!("post:{id}"))
            .query_async(&mut independent)
            .await?;
        assert_eq!(exists, 1);
        let generation: Option<String> = redis::cmd("GET")
            .arg(&key)
            .query_async(&mut independent)
            .await?;
        assert_eq!(generation, None);
    }
    // A successful acknowledged mutation retains the existing epoch/600s TTL.
    let author = Uuid::new_v4();
    effects.invalidate_feeds(&[author], test_deadline()).await?;
    assert_eq!(feeds.current_generation(author).await?, 1);
    let ttl: i64 = redis::cmd("TTL")
        .arg(post_service::adapters::out::cache::feed_cache::feed_cache_generation_key(author))
        .query_async(&mut admin)
        .await?;
    assert!((590..=600).contains(&ttl));
    println!("REFRESHED_CACHE_REDIS: actual queued DEL/INCR scripts reject after expiry; no post eviction/author generation write; dispatch uncertainty retained; epoch/TTL preserved");
    Ok(())
}

#[derive(Clone)]
pub(super) struct RefreshingOwner {
    pub pool: PgPool,
    pub lifetimes: Arc<std::sync::Mutex<std::collections::VecDeque<i64>>>,
}
#[tonic::async_trait]
impl owner::cache_owner_authority_service_server::CacheOwnerAuthorityService for RefreshingOwner {
    async fn resolve_cache_page(
        &self,
        request: Request<owner::ResolveCachePageRequest>,
    ) -> Result<Response<owner::ResolveCachePageResponse>, Status> {
        let lifetime = self
            .lifetimes
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(30_000_000);
        let signer =
            CacheOwnerSigner::new(&[51; 32], LifetimePolicy::new(lifetime).unwrap()).unwrap();
        CacheOwnerGrpcServer::new(
            Arc::new(PgCacheAuthorityRepository::new(self.pool.clone(), 2).unwrap()),
            signer,
        )
        .resolve_cache_page(request)
        .await
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn exercise_refresh(
    app: &post_support::TestApp,
    friend_pool: &PgPool,
    post_tls: &TlsMaterial,
    gateway: &str,
    friend: &str,
    key: [u8; 32],
    lifetimes: Arc<std::sync::Mutex<std::collections::VecDeque<i64>>>,
    entries: Arc<AtomicUsize>,
    reply_expiry: Arc<std::sync::atomic::AtomicI64>,
) -> anyhow::Result<()> {
    use post_service::application::cache_authority::ClaimedCachePageReadPort;
    let c = seed(&app.pool, Uuid::new_v4(), "POST_CREATED", 60_000).await?;
    let message = wire(&app.pool, &c).await?;
    let owned = PgDeliveryAuthorityRepository::new(app.pool.clone())
        .resolve_claimed_delivery(&DeliveryClaimLookupDto::new(c.clone())?)
        .await?;
    let lookup = CachePageLookupDto::new(owned, c.event.actor_id.unwrap(), None, 2)?;

    let mut issuer = IssuerClient::new(post_tls.connect(gateway, Workload::Gateway).await?);
    let grant = issue(&mut issuer, &message).await?;
    let friend_tls = tls(Workload::Friend)?;
    let mut validator = IssuerClient::new(friend_tls.connect(gateway, Workload::Gateway).await?);
    lifetimes.lock().unwrap().push_back(800_000);
    let refreshed = validator
        .validate_cache_grant(identified(
            authority::ValidateCacheGrantRequest {
                token: grant.token.clone(),
                framed_body: frame(&message),
            },
            Workload::Friend,
            "friend-fixture",
            None,
        ))
        .await?
        .into_inner();
    assert!(refreshed.expires_micros < grant.expires_micros - 10_000_000);
    let mut projection =
        authority::cache_projection_service_client::CacheProjectionServiceClient::new(
            post_tls.connect(friend, Workload::Friend).await?,
        );
    lifetimes.lock().unwrap().push_back(800_000);
    let response = projection
        .list_cache_followers(presented(message.clone(), Some(&grant.token)))
        .await?
        .into_inner();
    assert!(response.expires_micros < grant.expires_micros - 10_000_000);
    assert!(response.expires_micros > chrono::Utc::now().timestamp_micros());
    // The actual Post presenter must retain the independently refreshed expiry,
    // including an empty page, rather than returning its original issuance cap.
    let reader = GrpcClaimedCacheClient::connect(
        gateway,
        friend,
        "post-fixture",
        Some(post_tls),
        [budget("refresh issue"), budget("refresh projection")],
        CacheVerifier::new(&[key], 128)?,
    )?;
    lifetimes.lock().unwrap().extend([30_000_000, 800_000]);
    let page = reader.fetch_cache_page(&lookup).await?;
    assert!(page.expires_at() <= chrono::Utc::now() + chrono::Duration::milliseconds(800));
    assert!(page.expires_at() < c.lease_expires_at - chrono::Duration::seconds(10));
    // Proto3 zero expiry serializes as an omitted scalar. Corrupt only the
    // response of the actual owning handler, after all real admission/query work.
    for mode in 1..=3 {
        reply_expiry.store(mode, Ordering::SeqCst);
        assert!(
            reader.fetch_cache_page(&lookup).await.is_err(),
            "malformed projection expiry {mode}"
        );
        let (feeds, _) = app.claimed_cache_effect_ports();
        assert_eq!(
            feeds.current_generation(c.event.actor_id.unwrap()).await?,
            0
        );
    }
    reply_expiry.store(0, Ordering::SeqCst);
    // A real Friend query blocks after admission; expiry must deny its result
    // while the owner lease and original grant still have substantial lifetime.
    let before = entries.load(Ordering::SeqCst);
    let mut blocked = friend_pool.begin().await?;
    sqlx::query("lock table follows in access exclusive mode")
        .execute(&mut *blocked)
        .await?;
    lifetimes.lock().unwrap().extend([30_000_000, 800_000]);
    let copy = lookup.clone();
    let task = tokio::spawn(async move { reader.fetch_cache_page(&copy).await });
    tokio::time::timeout(Duration::from_millis(600),async {
        loop {
            let waiting:i64=sqlx::query_scalar("select count(*) from pg_stat_activity where datname=current_database() and wait_event_type='Lock' and query like '%follows%'").fetch_one(friend_pool).await.unwrap();
            if waiting>0 {break;}
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }).await?;
    let result = tokio::time::timeout(Duration::from_secs(2), task).await??;
    blocked.rollback().await?;
    assert!(result.is_err());
    assert_eq!(entries.load(Ordering::SeqCst), before + 1);
    assert!(chrono::Utc::now().timestamp_micros() < grant.expires_micros);
    assert!(chrono::Utc::now() < c.lease_expires_at);
    retire(&app.pool, &c).await?;
    println!("REFRESHED_Cache_MTLS: actual owner shorter signing policy, Gateway validation, Friend seal/response, Post DTO including empty page; actual blocked Friend query denied while original grant/lease live");
    Ok(())
}
