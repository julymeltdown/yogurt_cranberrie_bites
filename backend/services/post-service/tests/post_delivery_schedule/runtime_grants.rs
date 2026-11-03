//! A real non-owner compatibility journey and the retained capability boundary.
use super::{recommendation_admission::seed, Fixture};
use anyhow::Result;
use post_service::application::ports::FeedRecommendationRepositoryPort;
use sqlx::{postgres::PgPoolOptions, Row};
use uuid::Uuid;

#[tokio::test]
async fn owner_migration_enables_legacy_reduction_without_protected_table_writes() -> Result<()> {
    let mut f = Fixture::new(29).await?;
    let role = format!("runtime_grants_{}", Uuid::new_v4().simple());
    let password = Uuid::new_v4().simple().to_string();
    sqlx::raw_sql(&format!("create role {role} login nosuperuser nocreatedb nocreaterole noinherit noreplication nobypassrls password '{password}'; revoke create on schema public from public;"))
        .execute(&f.pool).await?;
    let grants = include_str!("../../runtime-grants.sql").replace(":\"runtime_role\"", &role);
    sqlx::raw_sql(&grants).execute(&f.pool).await?;
    let runtime = PgPoolOptions::new()
        .max_connections(4)
        .connect_with(
            f.pool
                .connect_options()
                .as_ref()
                .clone()
                .username(&role)
                .password(&password),
        )
        .await?;
    f.runtime_role = Some(role);
    f.controller_pool = Some(std::mem::replace(&mut f.pool, runtime));
    let owner = f.controller_pool.as_ref().unwrap();
    let (repo, event) = seed(&f).await?;
    let error = repo
        .append_event(&event)
        .await
        .expect_err("schema 29 must expose the invoker/protected-write mismatch");
    assert!(
        error
            .to_string()
            .contains("permission denied for table feed_affinity_legacy_effect_observations"),
        "unexpected pre-upgrade error: {error}"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from feed_recommendation_events where event_id=$1"
        )
        .bind(event.event_id)
        .fetch_one(owner)
        .await?,
        0
    );
    sqlx::migrate::Migrator::new(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations"),
    )
    .await?
    .run(owner)
    .await?;
    let guard = sqlx::query("select prosecdef, proconfig from pg_proc where oid='public.feed_affinity_apply_legacy_effect(text,uuid,uuid,uuid,uuid,uuid,text,timestamptz,float8,float8)'::regprocedure").fetch_one(owner).await?;
    assert!(guard.get::<bool, _>("prosecdef"));
    assert!(guard
        .get::<Vec<String>, _>("proconfig")
        .contains(&"search_path=pg_catalog, public, pg_temp".to_string()));
    assert!(repo.append_event(&event).await?);
    assert!(!repo.append_event(&event).await?);
    assert_eq!(sqlx::query_scalar::<_,i64>("select count(*) from feed_affinity_legacy_effect_observations where namespace='RECOMMENDATION' and source_id=$1").bind(event.event_id).fetch_one(owner).await?,1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from feed_affinity_test_provisioning")
            .fetch_one(owner)
            .await?,
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from feed_affinity_resolution_receipts")
            .fetch_one(owner)
            .await?,
        0
    );
    for table in [
        "_sqlx_migrations",
        "feed_affinity_capture_epoch",
        "feed_affinity_pair_baselines",
        "feed_affinity_pair_provenance",
        "feed_affinity_source_pairs",
        "feed_affinity_source_contributions",
        "feed_affinity_legacy_effect_observations",
        "feed_affinity_pair_heads",
        "feed_affinity_resolution_receipts",
        "feed_recommendation_retry_contexts",
    ] {
        let writable: bool = sqlx::query_scalar("select has_table_privilege(current_user,$1,'INSERT,UPDATE,DELETE,TRUNCATE,REFERENCES,TRIGGER')").bind(format!("public.{table}")).fetch_one(&f.pool).await?;
        assert!(!writable, "runtime can write protected {table}");
        let error = sqlx::query(&format!(
            "insert into public.{table} select * from public.{table} where false"
        ))
        .execute(&f.pool)
        .await
        .unwrap_err();
        assert_eq!(
            error.as_database_error().and_then(|e| e.code()).as_deref(),
            Some("42501")
        );
    }
    let author: Uuid = sqlx::query_scalar("select author_id from posts where id=$1")
        .bind(event.subject_id)
        .fetch_one(&f.pool)
        .await?;
    // A caller-controlled search path cannot redirect the elevated function's
    // hash helper. Exact duplicate rejection must use the real public source.
    sqlx::raw_sql("create schema shadow; create function shadow.feed_affinity_identity_hash(jsonb) returns text language sql as $$ select repeat('f',64) $$;")
        .execute(owner).await?;
    sqlx::query(&format!(
        "grant usage on schema shadow to {}",
        f.runtime_role.as_ref().unwrap()
    ))
    .execute(owner)
    .await?;
    let mut connection = f.pool.acquire().await?;
    sqlx::query("set search_path=shadow,public")
        .execute(&mut *connection)
        .await?;
    for (delta, constraint) in [
        (0.99, "feed_affinity_legacy_effect_binding"),
        (0.08, "feed_affinity_legacy_effect_identity"),
    ] {
        let error = sqlx::query("select public.feed_affinity_apply_legacy_effect('RECOMMENDATION',$1,$2,$3,$4,$5,'CLICK',$6,$7,0)")
            .bind(event.event_id).bind(event.viewer_id).bind(author).bind(event.actor_id).bind(event.subject_id).bind(event.occurred_at).bind(delta)
            .execute(&mut *connection).await.unwrap_err();
        let database = error.as_database_error().unwrap();
        assert_eq!(database.code().as_deref(), Some("23514"));
        assert_eq!(database.constraint(), Some(constraint));
    }
    drop(connection);
    assert_eq!(
        sqlx::query_scalar::<_, f64>("select affinity_score from feed_author_affinities")
            .fetch_one(owner)
            .await?,
        0.08
    );
    f.shutdown().await
}
