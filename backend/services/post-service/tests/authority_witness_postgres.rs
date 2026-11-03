//! Real PostgreSQL storage acceptance. Admission/producer provenance and RPC
//! integration remain separate; these fixtures explicitly construct parent DTOs.
#[path = "support/native_postgres.rs"]
mod native_postgres;
use chrono::{Duration, Utc};
use post_application::authority_witness::*;
use post_service::adapters::out::persistence::authority_witness_repository::PgAuthorityWitnessRepository;
use sqlx::{postgres::PgPoolOptions, PgPool};
use uuid::Uuid;

/// Owns the disposable PostgreSQL server for one test when no controller-owned
/// native parent is supplied. Dropping it after the pool stops the container.
type OwnedPostgres = Option<test_support::postgres::PostgresContainer>;

async fn fixture() -> anyhow::Result<(PgPool, OwnedPostgres)> {
    // A controller may supply an owned native parent; otherwise each test gets
    // a fresh real PostgreSQL container, as the other owner receipt tests do.
    let (postgres, url) = if std::env::var_os("POST_TEST_DATABASE_URL").is_some() {
        (None, native_postgres::database_url().await?)
    } else {
        let postgres = test_support::postgres::start_postgres().await?;
        let url = format!(
            "postgres://postgres:postgres@{}:{}/postgres",
            postgres.get_host().await?,
            postgres.get_host_port_ipv4(5432).await?
        );
        (Some(postgres), url)
    };
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&url)
        .await?;
    let migrations = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    sqlx::migrate::Migrator::new(migrations.as_path())
        .await?
        .run(&pool)
        .await?;
    Ok((pool, postgres))
}
fn member(post: u128, author: u128) -> WitnessMemberDto {
    WitnessMemberDto::from_owned_query(Uuid::from_u128(post), Uuid::from_u128(author)).unwrap()
}
fn discovery(stage: Uuid, digest: [u8; 32], members: Vec<WitnessMemberDto>) -> AuthorityWitnessDto {
    let now = Utc::now();
    let parent = WitnessParentDto::from_admitted_request(
        digest,
        [2; 32],
        Uuid::from_u128(1),
        WitnessRootDto::Search,
        vec![],
        now + Duration::seconds(60),
        now,
    )
    .unwrap();
    let query = WitnessQueryDto::new(stage, WitnessPurposeDto::SearchBatch, [3; 32], 0).unwrap();
    AuthorityWitnessDto::capture(parent, query, members, vec![], now).unwrap()
}
async fn count(pool: &PgPool) -> i64 {
    sqlx::query_scalar("select count(*) from post_authority_resource_witnesses")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn independent_pools_observe_one_atomic_immutable_publication() -> anyhow::Result<()> {
    let (pool, _postgres) = fixture().await?;
    let other_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect_with(pool.connect_options().as_ref().clone())
        .await?;
    let a = PgAuthorityWitnessRepository::new(pool.clone());
    let b = PgAuthorityWitnessRepository::new(other_pool.clone());
    let stage = Uuid::new_v4();
    let witness = discovery(stage, [1; 32], (2..1002).map(|n| member(n, n)).collect());
    let (first, second) = tokio::join!(a.publish(&witness), b.publish(&witness));
    first?;
    second?;
    assert_eq!(count(&pool).await, 1);
    let requested = (2..102).map(Uuid::from_u128).collect();
    let subset = b
        .resolve(&ResolveWitnessDto::authors(stage, [1; 32], requested)?)
        .await?;
    assert_eq!(subset.members().len(), 100);
    assert_eq!(subset.stage_id(), stage);
    assert_eq!(subset.parent_digest(), &[1; 32]);
    assert!(subset.missing().is_empty());
    assert_eq!(subset.expires_at(), witness.parent().expires_at());
    pool.close().await;
    other_pool.close().await;
    Ok(())
}

#[tokio::test]
async fn another_parent_stage_or_unadmitted_target_is_denied_without_new_rows() -> anyhow::Result<()>
{
    let (pool, _postgres) = fixture().await?;
    let repo = PgAuthorityWitnessRepository::new(pool.clone());
    let stage = Uuid::new_v4();
    repo.publish(&discovery(stage, [1; 32], vec![member(2, 3)]))
        .await?;
    for request in [
        ResolveWitnessDto::authors(stage, [4; 32], vec![Uuid::from_u128(3)])?,
        ResolveWitnessDto::authors(Uuid::new_v4(), [1; 32], vec![Uuid::from_u128(3)])?,
    ] {
        assert!(matches!(
            repo.resolve(&request).await,
            Err(WitnessErrorDto::MissingOrExpired)
        ));
    }
    assert!(matches!(
        repo.resolve(&ResolveWitnessDto::authors(
            stage,
            [1; 32],
            vec![Uuid::from_u128(9)]
        )?)
        .await,
        Err(WitnessErrorDto::NotAdmitted)
    ));
    assert!(
        matches!(
            repo.resolve(&ResolveWitnessDto::posts(
                stage,
                [1; 32],
                vec![Uuid::from_u128(2)]
            )?)
            .await,
            Err(WitnessErrorDto::NotAdmitted)
        ),
        "discovery cannot become fixed-ID authorization"
    );
    assert!(ResolveWitnessDto::authors(stage, [1; 32], vec![Uuid::from_u128(3); 2]).is_err());
    assert!(
        ResolveWitnessDto::authors(stage, [1; 32], (1..102).map(Uuid::from_u128).collect())
            .is_err()
    );
    assert_eq!(count(&pool).await, 1);
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn conflicting_retry_cannot_replace_membership_or_extend_the_stage() -> anyhow::Result<()> {
    let (pool, _postgres) = fixture().await?;
    let repo = PgAuthorityWitnessRepository::new(pool.clone());
    let stage = Uuid::new_v4();
    let initial = discovery(stage, [1; 32], vec![member(2, 3)]);
    repo.publish(&initial).await?;
    repo.publish(&initial).await?;
    for replacement in [
        discovery(stage, [1; 32], vec![member(2, 4)]),
        discovery(stage, [4; 32], vec![member(2, 3)]),
    ] {
        assert!(matches!(
            repo.publish(&replacement).await,
            Err(WitnessErrorDto::StageConflict)
        ));
    }
    let update = sqlx::query("update post_authority_resource_witnesses set parent_expires_at=parent_expires_at+interval '1 second' where stage_id=$1")
        .bind(stage).execute(&pool).await;
    let error = update.unwrap_err();
    assert_eq!(
        error
            .as_database_error()
            .and_then(|error| error.code())
            .as_deref(),
        Some("23514")
    );
    let subset = repo
        .resolve(&ResolveWitnessDto::authors(
            stage,
            [1; 32],
            vec![Uuid::from_u128(3)],
        )?)
        .await?;
    assert_eq!(subset.members()[0].author_id(), Uuid::from_u128(3));
    assert_eq!(subset.expires_at(), initial.parent().expires_at());
    assert_eq!(count(&pool).await, 1);
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn expired_stages_deny_and_bounded_cleanup_preserves_live_membership() -> anyhow::Result<()> {
    let (pool, _postgres) = fixture().await?;
    let repo = PgAuthorityWitnessRepository::new(pool.clone());
    let stage = Uuid::new_v4();
    repo.publish(&discovery(stage, [1; 32], vec![member(2, 3)]))
        .await?;
    let expired_ids = [Uuid::new_v4(), Uuid::new_v4()];
    for expired in expired_ids {
        sqlx::query("insert into post_authority_resource_witnesses
            select $1,parent_digest,request_digest,actor_id,root_kind,root_resources,purpose,query_digest,
                batch_sequence,post_ids,author_ids,missing_post_ids,
                statement_timestamp()-interval '90 seconds',statement_timestamp()-interval '30 seconds'
            from post_authority_resource_witnesses where stage_id=$2")
            .bind(expired).bind(stage).execute(&pool).await?;
        assert!(matches!(
            repo.resolve(&ResolveWitnessDto::authors(
                expired,
                [1; 32],
                vec![Uuid::from_u128(3)]
            )?)
            .await,
            Err(WitnessErrorDto::MissingOrExpired)
        ));
    }
    assert_eq!(count(&pool).await, 3);
    assert_eq!(
        repo.purge_expired(&PurgeWitnessesDto::new(1)?)
            .await?
            .removed(),
        1
    );
    assert_eq!(count(&pool).await, 2);
    assert_eq!(
        repo.purge_expired(&PurgeWitnessesDto::new(1000)?)
            .await?
            .removed(),
        1
    );
    assert_eq!(
        repo.purge_expired(&PurgeWitnessesDto::new(1000)?)
            .await?
            .removed(),
        0
    );
    assert_eq!(count(&pool).await, 1);
    assert!(repo
        .resolve(&ResolveWitnessDto::authors(
            stage,
            [1; 32],
            vec![Uuid::from_u128(3)]
        )?)
        .await
        .is_ok());
    // A publisher delayed beyond its parent's expiry cannot acknowledge a new
    // stage. The database clock gates insertion, not the caller's old timestamp.
    let old = Utc::now() - Duration::seconds(90);
    let parent = WitnessParentDto::from_admitted_request(
        [1; 32],
        [2; 32],
        Uuid::from_u128(1),
        WitnessRootDto::Search,
        vec![],
        old + Duration::seconds(60),
        old,
    )?;
    let late = AuthorityWitnessDto::capture(
        parent,
        WitnessQueryDto::new(Uuid::new_v4(), WitnessPurposeDto::SearchBatch, [3; 32], 0)?,
        vec![member(2, 3)],
        vec![],
        old,
    )?;
    assert!(matches!(
        repo.publish(&late).await,
        Err(WitnessErrorDto::MissingOrExpired)
    ));
    assert_eq!(count(&pool).await, 1, "no stale stage was inserted");
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn fixed_post_scope_retains_explicit_missing_results_and_rejects_unrelated_ids(
) -> anyhow::Result<()> {
    let (pool, _postgres) = fixture().await?;
    let repo = PgAuthorityWitnessRepository::new(pool.clone());
    let stage = Uuid::new_v4();
    let now = Utc::now();
    let parent = WitnessParentDto::from_admitted_request(
        [1; 32],
        [2; 32],
        Uuid::from_u128(1),
        WitnessRootDto::NotificationPosts,
        vec![Uuid::from_u128(2), Uuid::from_u128(4)],
        now + Duration::seconds(30),
        now,
    )?;
    let query = WitnessQueryDto::new(stage, WitnessPurposeDto::FixedPosts, [3; 32], 0)?;
    let witness = AuthorityWitnessDto::capture(
        parent,
        query,
        vec![member(2, 3)],
        vec![Uuid::from_u128(4)],
        now,
    )?;
    repo.publish(&witness).await?;
    let subset = repo
        .resolve(&ResolveWitnessDto::posts(
            stage,
            [1; 32],
            vec![Uuid::from_u128(2), Uuid::from_u128(4)],
        )?)
        .await?;
    assert_eq!(subset.members().len(), 1);
    assert_eq!(subset.missing(), &[Uuid::from_u128(4)]);
    assert!(matches!(
        repo.resolve(&ResolveWitnessDto::posts(
            stage,
            [1; 32],
            vec![Uuid::from_u128(5)]
        )?)
        .await,
        Err(WitnessErrorDto::NotAdmitted)
    ));
    assert!(matches!(
        repo.resolve(&ResolveWitnessDto::authors(
            stage,
            [1; 32],
            vec![Uuid::from_u128(3)]
        )?)
        .await,
        Err(WitnessErrorDto::NotAdmitted)
    ));
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn storage_errors_do_not_invent_publication_or_membership() -> anyhow::Result<()> {
    let (pool, _postgres) = fixture().await?;
    let repo = PgAuthorityWitnessRepository::new(pool.clone());
    let stage = Uuid::new_v4();
    pool.close().await;
    assert!(matches!(
        repo.publish(&discovery(stage, [1; 32], vec![member(2, 3)]))
            .await,
        Err(WitnessErrorDto::Unavailable)
    ));
    assert!(matches!(
        repo.resolve(&ResolveWitnessDto::authors(
            stage,
            [1; 32],
            vec![Uuid::from_u128(3)]
        )?)
        .await,
        Err(WitnessErrorDto::Unavailable)
    ));
    assert!(matches!(
        repo.resolve_current(&ResolveWitnessDto::authors(
            stage,
            [1; 32],
            vec![Uuid::from_u128(3)]
        )?)
        .await,
        Err(WitnessErrorDto::Unavailable)
    ));
    assert!(matches!(
        repo.purge_expired(&PurgeWitnessesDto::new(10)?).await,
        Err(WitnessErrorDto::Unavailable)
    ));
    Ok(())
}

async fn live_post(pool: &PgPool, post: u128, author: u128) -> anyhow::Result<()> {
    sqlx::query("insert into posts(id,author_id,content,visibility,created_at) values ($1,$2,'resource lookup','PUBLIC',clock_timestamp())")
        .bind(Uuid::from_u128(post)).bind(Uuid::from_u128(author)).execute(pool).await?;
    Ok(())
}

#[tokio::test]
async fn current_resolution_rejects_changed_author_deleted_hidden_and_unobserved_posts(
) -> anyhow::Result<()> {
    let (pool, _postgres) = fixture().await?;
    let repo = PgAuthorityWitnessRepository::new(pool.clone());
    live_post(&pool, 2, 3).await?;
    live_post(&pool, 4, 3).await?;
    let stage = Uuid::new_v4();
    repo.publish(&discovery(stage, [1; 32], vec![member(2, 3)]))
        .await?;
    let request = ResolveWitnessDto::authors(stage, [1; 32], vec![Uuid::from_u128(3)])?;
    let subset = repo.resolve_current(&request).await?;
    assert_eq!(subset.members()[0].post_id(), Uuid::from_u128(2));
    for mutation in [
        "update posts set author_id='00000000-0000-0000-0000-000000000009' where id='00000000-0000-0000-0000-000000000002'",
        "update posts set author_id='00000000-0000-0000-0000-000000000003',deleted_at=clock_timestamp() where id='00000000-0000-0000-0000-000000000002'",
        "update posts set deleted_at=null,moderation_hidden_at=clock_timestamp() where id='00000000-0000-0000-0000-000000000002'",
        "delete from posts where id='00000000-0000-0000-0000-000000000002'",
    ] {
        sqlx::query(mutation).execute(&pool).await?;
        assert!(matches!(repo.resolve_current(&request).await, Err(WitnessErrorDto::NotAdmitted)), "{mutation}");
        assert!(repo.resolve(&request).await.is_ok(), "immutable observation survives; current access denies");
        assert_eq!(count(&pool).await, 1, "lookup does not republish or switch to unobserved post 4");
    }
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn current_fixed_resolution_rejects_newly_present_missing_post_and_foreign_parent(
) -> anyhow::Result<()> {
    let (pool, _postgres) = fixture().await?;
    let repo = PgAuthorityWitnessRepository::new(pool.clone());
    live_post(&pool, 2, 3).await?;
    let stage = Uuid::new_v4();
    let now = Utc::now();
    let ids = vec![Uuid::from_u128(2), Uuid::from_u128(4)];
    let parent = WitnessParentDto::from_admitted_request(
        [1; 32],
        [2; 32],
        Uuid::from_u128(1),
        WitnessRootDto::NotificationPosts,
        ids.clone(),
        now + Duration::seconds(60),
        now,
    )?;
    repo.publish(&AuthorityWitnessDto::capture(
        parent,
        WitnessQueryDto::new(stage, WitnessPurposeDto::FixedPosts, [3; 32], 0)?,
        vec![member(2, 3)],
        vec![Uuid::from_u128(4)],
        now,
    )?)
    .await?;
    let request = ResolveWitnessDto::posts(stage, [1; 32], ids.clone())?;
    assert_eq!(
        repo.resolve_current(&request).await?.missing(),
        &[Uuid::from_u128(4)]
    );
    assert!(matches!(
        repo.resolve_current(&ResolveWitnessDto::posts(stage, [9; 32], ids)?)
            .await,
        Err(WitnessErrorDto::MissingOrExpired)
    ));
    live_post(&pool, 4, 5).await?;
    assert!(matches!(
        repo.resolve_current(&request).await,
        Err(WitnessErrorDto::NotAdmitted)
    ));
    assert_eq!(count(&pool).await, 1);
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn current_access_waiting_on_posts_lock_cannot_return_after_parent_expiry(
) -> anyhow::Result<()> {
    let (pool, _postgres) = fixture().await?;
    let repo = PgAuthorityWitnessRepository::new(pool.clone());
    live_post(&pool, 2, 3).await?;
    let stage = Uuid::new_v4();
    let witness = short_lived_discovery(stage);
    repo.publish(&witness).await?;
    let mut blocker = pool.begin().await?;
    sqlx::query("lock table posts in access exclusive mode")
        .execute(&mut *blocker)
        .await?;
    let resolving = tokio::spawn(async move {
        repo.resolve_current(&ResolveWitnessDto::authors(
            stage,
            [1; 32],
            vec![Uuid::from_u128(3)],
        )?)
        .await
    });
    let observation = async {
        loop {
            let waiting: bool = sqlx::query_scalar("select exists(select 1 from pg_stat_activity where datname=current_database() and pid<>pg_backend_pid() and wait_event_type='Lock' and query like '%from posts%')")
                .fetch_one(&pool).await?;
            if waiting {
                return Ok::<_, anyhow::Error>(true);
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    };
    let observed = tokio::time::timeout(std::time::Duration::from_secs(2), observation)
        .await
        .unwrap_or(Ok(false))?;
    wait_for_parent_expiry(&pool, witness.parent().expires_at()).await?;
    blocker.rollback().await?;
    assert!(matches!(
        resolving.await?,
        Err(WitnessErrorDto::MissingOrExpired)
    ));
    assert!(observed, "lookup must actually wait on authoritative posts");
    assert_eq!(count(&pool).await, 1);
    pool.close().await;
    Ok(())
}

fn short_lived_discovery(stage: Uuid) -> AuthorityWitnessDto {
    let now = Utc::now();
    let parent = WitnessParentDto::from_admitted_request(
        [1; 32],
        [2; 32],
        Uuid::from_u128(1),
        WitnessRootDto::Search,
        vec![],
        now + Duration::seconds(3),
        now,
    )
    .unwrap();
    AuthorityWitnessDto::capture(
        parent,
        WitnessQueryDto::new(stage, WitnessPurposeDto::SearchBatch, [3; 32], 0).unwrap(),
        vec![member(2, 3)],
        vec![],
        now,
    )
    .unwrap()
}

async fn observed_witness_lock_wait(pool: &PgPool) -> anyhow::Result<bool> {
    let observation = async {
        loop {
            let waiting: bool = sqlx::query_scalar(
                "select exists(select 1 from pg_stat_activity
                 where datname=current_database() and pid<>pg_backend_pid()
                 and wait_event_type='Lock'
                 and query like '%post_authority_resource_witnesses%')",
            )
            .fetch_one(pool)
            .await?;
            if waiting {
                return Ok::<_, anyhow::Error>(true);
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    };
    tokio::time::timeout(std::time::Duration::from_secs(2), observation)
        .await
        .unwrap_or(Ok(false))
}

async fn wait_for_parent_expiry(
    pool: &PgPool,
    expires_at: chrono::DateTime<Utc>,
) -> anyhow::Result<()> {
    // A bounded DB-clock wait after observing the actual blocker, not a guessed
    // scheduling delay. The original publishing/lookup handle stays live.
    sqlx::query("select pg_sleep(greatest(0, extract(epoch from ($1::timestamptz-clock_timestamp()))+0.03))")
        .bind(expires_at).execute(pool).await?;
    Ok(())
}

#[tokio::test]
async fn publication_waiting_on_unique_index_cannot_ack_or_insert_after_expiry(
) -> anyhow::Result<()> {
    let (pool, _postgres) = fixture().await?;
    let repo = PgAuthorityWitnessRepository::new(pool.clone());
    let stage = Uuid::new_v4();
    // Seed one long-lived source row so the blocker can insert a stage without
    // going through the publishing adapter being exercised by this test.
    let source = Uuid::new_v4();
    repo.publish(&discovery(source, [1; 32], vec![member(2, 3)]))
        .await?;
    let witness = short_lived_discovery(stage);
    let expires_at = witness.parent().expires_at();
    let mut blocker = pool.begin().await?;
    sqlx::query("insert into post_authority_resource_witnesses
        select $1,parent_digest,request_digest,actor_id,root_kind,root_resources,purpose,query_digest,
            batch_sequence,post_ids,author_ids,missing_post_ids,created_at,parent_expires_at
        from post_authority_resource_witnesses where stage_id=$2")
        .bind(stage).bind(source).execute(&mut *blocker).await?;
    let publishing = tokio::spawn(async move { repo.publish(&witness).await });
    let observed = observed_witness_lock_wait(&pool).await?;
    wait_for_parent_expiry(&pool, expires_at).await?;
    // Rollback makes the waiting insertion succeed rather than conflict, which
    // exercises the post-write recheck and transactional rollback specifically.
    blocker.rollback().await?;
    let result = publishing.await?;
    assert!(
        observed,
        "the publisher must actually wait on the unique-index lock"
    );
    assert!(matches!(result, Err(WitnessErrorDto::MissingOrExpired)));
    assert_eq!(
        count(&pool).await,
        1,
        "expired waiting insertion was rolled back"
    );
    let absent: bool = sqlx::query_scalar(
        "select not exists(select 1 from post_authority_resource_witnesses where stage_id=$1)",
    )
    .bind(stage)
    .fetch_one(&pool)
    .await?;
    assert!(absent);
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn resolution_waiting_on_table_lock_cannot_return_expired_membership() -> anyhow::Result<()> {
    let (pool, _postgres) = fixture().await?;
    let repo = PgAuthorityWitnessRepository::new(pool.clone());
    let stage = Uuid::new_v4();
    let witness = short_lived_discovery(stage);
    repo.publish(&witness).await?;
    let mut blocker = pool.begin().await?;
    sqlx::query("lock table post_authority_resource_witnesses in access exclusive mode")
        .execute(&mut *blocker)
        .await?;
    let resolving = tokio::spawn(async move {
        repo.resolve(&ResolveWitnessDto::authors(
            stage,
            [1; 32],
            vec![Uuid::from_u128(3)],
        )?)
        .await
    });
    let observed = observed_witness_lock_wait(&pool).await?;
    wait_for_parent_expiry(&pool, witness.parent().expires_at()).await?;
    blocker.rollback().await?;
    let result = resolving.await?;
    assert!(observed, "lookup must actually wait on the table lock");
    assert!(matches!(result, Err(WitnessErrorDto::MissingOrExpired)));
    assert_eq!(count(&pool).await, 1, "lookup has no write effects");
    pool.close().await;
    Ok(())
}
