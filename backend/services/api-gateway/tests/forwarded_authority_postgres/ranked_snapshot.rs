//! Actual production Post GetFeed handler/composition with real candidate,
//! snapshot and impression persistence. Gateway root signing remains fixture-side.
use super::*;

const FEED: &str = "/post.v1.PostService/GetFeed";

fn root(request: &post::FeedRequest) -> String {
    ActorDelegationSigner::local_parity(Workload::Gateway).issue(
        Workload::Post,
        FEED,
        Uuid::parse_str(&request.user_id).unwrap(),
        None,
        &frame(request),
        chrono::Utc::now().timestamp(),
    )
}
async fn feed(
    grpc: &mut tonic::client::Grpc<tonic::transport::Channel>,
    request: post::FeedRequest,
    token: Option<&str>,
) -> Result<Response<post::FeedResponse>, Status> {
    grpc.ready()
        .await
        .map_err(|_| Status::unavailable("fixture channel"))?;
    grpc.unary(
        identified(request, Workload::Gateway, "gateway-fixture", token),
        http::uri::PathAndQuery::from_static(FEED),
        tonic_prost::ProstCodec::default(),
    )
    .await
}
async fn counts(pool: &PgPool) -> anyhow::Result<(i64, i64, i64, i64)> {
    Ok(sqlx::query_as("select (select count(*) from post_authority_resource_witnesses), (select count(*) from feed_recommendation_snapshots), (select count(*) from feed_recommendation_impressions), (select count(*) from feed_recommendation_events)").fetch_one(pool).await?)
}

#[tokio::test]
async fn production_ranked_snapshot_pages_keep_current_visibility_and_only_delivered_impressions(
) -> anyhow::Result<()> {
    let app = post_support::start_postgres_redis_app(false).await?;
    let friend_app = friend_support::start_postgres_app(false).await?;
    let viewer = Uuid::new_v4();
    for n in 0u128..7 {
        let author = Uuid::from_u128(20_000 + n);
        sqlx::query("insert into follows(follower_id,followee_id) values($1,$2)")
            .bind(viewer)
            .bind(author)
            .execute(&friend_app.pool)
            .await?;
        sqlx::query("insert into posts(id,author_id,content,visibility,created_at) values($1,$2,'ranked snapshot native','PUBLIC',clock_timestamp()-make_interval(secs=>$3))")
            .bind(Uuid::from_u128(10_000+n)).bind(author).bind(n as f64).execute(&app.pool).await?;
    }
    sqlx::query(
        "insert into user_blocks(blocker_id,blocked_id,created_at) values($1,$2,clock_timestamp())",
    )
    .bind(Uuid::from_u128(20_005))
    .bind(viewer)
    .execute(&friend_app.pool)
    .await?;
    sqlx::query(
        "insert into user_mutes(muter_id,muted_id,created_at) values($1,$2,clock_timestamp())",
    )
    .bind(viewer)
    .bind(Uuid::from_u128(20_006))
    .execute(&friend_app.pool)
    .await?;
    let post_tls = tls(Workload::Post)?;
    let gateway_tls = tls(Workload::Gateway)?;
    let friend_tls = tls(Workload::Friend)?;
    let effects = Arc::new(AtomicUsize::new(0));
    let friend_listener = TcpListener::bind("127.0.0.1:0").await?;
    let friend_url = format!("https://{}", friend_listener.local_addr()?);
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let friend_server = ServerTask::spawn(
        Server::builder()
            .tls_config(friend_tls.server_config()?)?
            .add_service(guard_with_forwarding(
                &friend_service::adapters::r#in::grpc::method_policy::FRIEND_METHOD_POLICY,
                FRIEND_PROOF,
                FriendOwner {
                    entries: effects.clone(),
                    server: friend_support::grpc_server(friend_app.state.clone()),
                    mute_after_list: None,
                },
                registry()?,
                ActorDelegationVerifier::local_parity(Workload::Friend),
            )?)
            .serve_with_incoming_shutdown(TcpListenerStream::new(friend_listener), async {
                let _ = stopped.await;
            }),
        stop,
    );
    let lookup_pool = PgPool::connect_with(app.pool.connect_options().as_ref().clone()).await?;
    let owner_key = ResourceEvidenceSigner::from_seed(&[11; 32])?;
    let resource = PostResourceAuthorityGrpcServer::new(
        Arc::new(PgAuthorityWitnessRepository::new(lookup_pool.clone())),
        ActorDelegationVerifier::local_parity(Workload::Post),
        owner_key.clone(),
    )?;
    let post_listener = TcpListener::bind("127.0.0.1:0").await?;
    let post_url = format!("https://{}", post_listener.local_addr()?);
    let gateway_listener = TcpListener::bind("127.0.0.1:0").await?;
    let gateway_url = format!("https://{}", gateway_listener.local_addr()?);
    let owner_client = GrpcPostResourceAuthorityClient::connect(
        &post_url,
        "gateway-fixture",
        Some(&gateway_tls),
        Default::default(),
    )?;
    let issuer = DelegationExchange::new(
        ActorDelegationSigner::local_parity(Workload::Gateway),
        ActorDelegationVerifier::local_parity(Workload::Post),
        ResourceEvidenceVerifier::new(&[owner_key.public_key()])?,
    )?;
    let broker = GatewayAuthorizationExchangeGrpcServer::new(Arc::new(
        DelegationExchangeRuntime::new(issuer, Arc::new(owner_client)),
    ));
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let gateway_server = ServerTask::spawn(
Server::builder()
    .tls_config(gateway_tls.server_config()?)?
    .add_service(guard_without_delegation(
        &AUTHORIZATION_EXCHANGE_POLICY,
        authority::authorization_exchange_service_server::AuthorizationExchangeServiceServer::new(broker),
        registry()?,
    )?)
    .serve_with_incoming_shutdown(TcpListenerStream::new(gateway_listener), async {
        let _ = stopped.await;
    }),
stop,
    );
    // Fixture-only injection. No operational edge default or SLO is introduced.
    let budget = EdgeBudget {
        edge: "fixture-post-exchange",
        expected_latency: Duration::from_millis(20),
        deadline: Duration::from_secs(3),
        connect_deadline: Duration::from_secs(1),
        retry: RetryClass::Never,
        failure_policy: FailurePolicy::FailRequest,
    };
    let exchange = GrpcAuthorizationExchangeClient::connect(
        &gateway_url,
        "post-fixture",
        Some(&post_tls),
        budget,
    )?;
    let friend_client = GrpcFriendClient::connect(
        friend_url.clone(),
        Workload::Post,
        "post-fixture",
        Some(&post_tls),
    )
    .await?
    .with_exchange(Arc::new(exchange));
    let state = app
        .state_with_query_witnesses(
            Arc::new(AdmittedQueryWitnessProducer::new(Arc::new(
                PgAuthorityWitnessRepository::new(app.pool.clone()),
            ))),
            Arc::new(friend_client),
        )
        .await?;
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let post_server = ServerTask::spawn(
Server::builder()
    .tls_config(post_tls.server_config()?)?
    .add_service(guard(
        &POST_METHOD_POLICY,
        post_service::adapters::r#in::grpc::server::proto::post_service_server::PostServiceServer::new(
                post_service::adapters::r#in::grpc::server::PostGrpcServer::new(state),
            ),
        registry()?,
        ActorDelegationVerifier::local_parity(Workload::Post),
    )?)
    .add_service(guard_without_delegation(
        &POST_RESOURCE_AUTHORITY_POLICY,
        owner::post_resource_authority_service_server::PostResourceAuthorityServiceServer::new(resource),
        registry()?,
    )?)
    .serve_with_incoming_shutdown(TcpListenerStream::new(post_listener), async {
        let _ = stopped.await;
    }),
stop,
    );

    let outcome: anyhow::Result<()> = async {
    let channel = gateway_tls.connect(&post_url, Workload::Post).await?;
    let mut grpc = tonic::client::Grpc::new(channel);
    let initial = post::FeedRequest {
        user_id: viewer.to_string(),
        cursor: String::new(),
        limit: 1,
        request_id: "ranked-authority-native".into(),
        session_id: Uuid::new_v4().to_string(),
    };
    let denied = feed(&mut grpc, initial.clone(), None).await.unwrap_err();
    assert_eq!(denied.code(), Code::Unauthenticated);
    assert_eq!(counts(&app.pool).await?, (0, 0, 0, 0));
    assert_eq!(effects.load(Ordering::SeqCst), 0);
    let initial_root = root(&initial);
    let first = feed(&mut grpc, initial.clone(), Some(&initial_root))
        .await?
        .into_inner();
    assert_eq!(first.posts.len(), 1);
    assert_eq!(first.items.len(), 1);
    assert_eq!(first.request_id, initial.request_id);
    assert_eq!(first.rank_version, "feed-linear-v1");
    assert!(!first.fallback_used);
    assert!(first.failed_sources.is_empty());
    assert!(!first.next_cursor.is_empty());
    let snapshot = Uuid::parse_str(&first.snapshot_id)?;
    let rows = sqlx::query("select position,post_id,author_id from feed_recommendation_snapshot_items where snapshot_id=$1 order by position")
        .bind(snapshot).fetch_all(&app.pool).await?;
    assert_eq!(
        rows.len(),
        5,
        "blocked/muted collected rows cannot enter snapshot"
    );
    let ids = rows
        .iter()
        .map(|row| row.get::<Uuid, _>("post_id"))
        .collect::<Vec<_>>();
    assert_eq!(first.posts[0].id, ids[0].to_string());
    assert!(ids
        .iter()
        .all(|id| (10_000..10_005).contains(&id.as_u128())));
    let stages = sqlx::query("select purpose,post_ids from post_authority_resource_witnesses")
        .fetch_all(&app.pool)
        .await?;
    assert_eq!(stages.len(), 2);
    for stage in stages {
        let members: Vec<Uuid> = stage.try_get("post_ids")?;
        match stage.get::<String, _>("purpose").as_str() {
            "RANKED_CANDIDATES" => assert_eq!(
                members.len(),
                7,
                "actual merged rows precede current policy"
            ),
            "SNAPSHOT_PAGE" => assert_eq!(
                members,
                vec![ids[0]],
                "page lookahead is not observed authority"
            ),
            value => panic!("unexpected stage {value}"),
        }
    }
    assert_eq!(counts(&app.pool).await?, (2, 1, 1, 1));
    assert_eq!(
        effects.load(Ordering::SeqCst),
        3,
        "membership, ranked policy, page policy"
    );

    // Reused admission and a new valid root with a different framed body both
    // fail before Post snapshot/witness/impression effects or Friend calls.
    assert_eq!(
        feed(&mut grpc, initial.clone(), Some(&initial_root))
            .await
            .unwrap_err()
            .code(),
        Code::Unauthenticated
    );
    let mut continuation = post::FeedRequest {
        cursor: first.next_cursor.clone(),
        ..initial.clone()
    };
    let wrong_body = root(&initial);
    assert_eq!(
        feed(&mut grpc, continuation.clone(), Some(&wrong_body))
            .await
            .unwrap_err()
            .code(),
        Code::PermissionDenied
    );
    let other = post::FeedRequest {
        user_id: Uuid::new_v4().to_string(),
        ..continuation.clone()
    };
    let proof = root(&other);
    assert!(feed(&mut grpc, other, Some(&proof)).await.is_err());
    assert_eq!(counts(&app.pool).await?, (2, 1, 1, 1));
    assert_eq!(effects.load(Ordering::SeqCst), 3);

    // A persisted mute after snapshot creation suppresses page two, preserves
    // owner cursor progress and does not manufacture an impression/event.
    let second_author: Uuid = rows[1].try_get("author_id")?;
    sqlx::query(
        "insert into user_mutes(muter_id,muted_id,created_at) values($1,$2,clock_timestamp())",
    )
    .bind(viewer)
    .bind(second_author)
    .execute(&friend_app.pool)
    .await?;
    let proof = root(&continuation);
    let second = feed(&mut grpc, continuation.clone(), Some(&proof))
        .await?
        .into_inner();
    assert!(second.posts.is_empty() && second.items.is_empty());
    assert_eq!(second.snapshot_id, first.snapshot_id);
    assert!(!second.next_cursor.is_empty());
    assert_eq!(counts(&app.pool).await?, (3, 1, 1, 1));
    assert_eq!(effects.load(Ordering::SeqCst), 4);

    // Discovery hydration excludes the deleted row from an empty page stage.
    // Explicit missing-ID authority is reserved for fixed roots; no child
    // is needed when this discovery page has no remaining author.
    sqlx::query("update posts set deleted_at=clock_timestamp() where id=$1")
        .bind(ids[2])
        .execute(&app.pool)
        .await?;
    continuation.cursor = second.next_cursor;
    let proof = root(&continuation);
    let third = feed(&mut grpc, continuation.clone(), Some(&proof))
        .await?
        .into_inner();
    assert!(third.posts.is_empty() && third.items.is_empty());
    assert!(!third.next_cursor.is_empty());
    assert_eq!(counts(&app.pool).await?, (4, 1, 1, 1));
    assert_eq!(effects.load(Ordering::SeqCst), 4);
    let empty_pages: i64 = sqlx::query_scalar("select count(*) from post_authority_resource_witnesses where purpose='SNAPSHOT_PAGE' and cardinality(post_ids)=0 and cardinality(missing_post_ids)=0").fetch_one(&app.pool).await?;
    assert_eq!(empty_pages,1,"deleted discovery row creates no found/missing authority");
    let missing: i64 = sqlx::query_scalar("select count(*) from post_authority_resource_witnesses where cardinality(missing_post_ids)>0").fetch_one(&app.pool).await?;
    assert_eq!(missing,0,"discovery cannot grant fixed-root missing-ID authority");

    continuation.cursor = third.next_cursor;
    let proof = root(&continuation);
    let fourth = feed(&mut grpc, continuation.clone(), Some(&proof))
        .await?
        .into_inner();
    assert_eq!(fourth.posts.len(), 1);
    assert_eq!(fourth.posts[0].id, ids[3].to_string());
    assert_eq!(fourth.items.len(), 1);
    assert_eq!(fourth.items[0].post_id, ids[3].to_string());
    assert!(!fourth.next_cursor.is_empty());
    assert_eq!(counts(&app.pool).await?, (5, 1, 2, 2));
    assert_eq!(effects.load(Ordering::SeqCst), 5);

    // Unavailable policy for a never-delivered fifth page must fail, despite
    // an existing valid snapshot and successful Post hydration/publication.
    friend_app.pool.close().await;
    continuation.cursor = fourth.next_cursor;
    let proof = root(&continuation);
    assert_eq!(
        feed(&mut grpc, continuation.clone(), Some(&proof))
            .await
            .unwrap_err()
            .code(),
        Code::Unavailable
    );
    assert_eq!(counts(&app.pool).await?, (6, 1, 2, 2));
    assert_eq!(effects.load(Ordering::SeqCst), 6);
    let delivered: Vec<Uuid> =
        sqlx::query_scalar("select post_id from feed_recommendation_impressions order by position")
            .fetch_all(&app.pool)
            .await?;
    assert_eq!(delivered, vec![ids[0], ids[3]]);
    let withheld: i64 = sqlx::query_scalar(
        "select count(*) from feed_recommendation_events where subject_id=any($1::uuid[])",
    )
    .bind(vec![ids[1], ids[2], ids[4]])
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(withheld, 0);

    // Expired snapshot must fail before another hydration stage/policy call.
    sqlx::query("update feed_recommendation_snapshots set created_at=clock_timestamp()-interval '1 hour',expires_at=clock_timestamp()-interval '1 second' where snapshot_id=$1")
        .bind(snapshot).execute(&app.pool).await?;
    let proof = root(&continuation);
    assert!(feed(&mut grpc, continuation, Some(&proof)).await.is_err());
    assert_eq!(counts(&app.pool).await?, (6, 1, 2, 2));
    assert_eq!(effects.load(Ordering::SeqCst), 6);
    Ok(())
    }.await;
    if let Err(error) = &outcome {
        eprintln!("RANKED_SNAPSHOT_PRIMARY_ERROR: {error:#}");
    }
    lookup_pool.close().await;
    post_server.stop().await?;
    gateway_server.stop().await?;
    friend_server.stop().await?;
    friend_app.shutdown().await?;
    outcome?;
    println!("RANKED_SNAPSHOT_AUTHORITY_PG: actual generated Post handler/production composition; real ranked collector/snapshot/impressions and Friend PG; distinct ranked/page stages; lookahead excluded; mute/deletion/cursor/replay/storage/expiry boundaries; only delivered impressions/events; all listeners stopped");
    Ok(())
}
