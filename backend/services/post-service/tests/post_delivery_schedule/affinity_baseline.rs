//! Controller-issued, blocked ordered foundation. Runtime is a separate login
//! without ownership or controller membership. No production activation claim.
use super::{affinity_source::outbox, projection, recommendation_admission::seed, Fixture};
use anyhow::Result;
use chrono::{Duration, Utc};
use post_service::adapters::out::persistence::user_lifecycle_repository::PgUserLifecycleRepository;
use post_service::application::dto::mutation_dto::MutationError;
use post_service::application::ports::{
    FeedRecommendationRepositoryPort, UserLifecycleRepositoryPort,
};
use sqlx::{postgres::PgPoolOptions, Row};
use uuid::Uuid;

async fn controlled() -> Result<Fixture> {
    controlled_version(25).await
}

pub(super) async fn controlled_version(version: i64) -> Result<Fixture> {
    let mut f = Fixture::new(version).await?;
    f.provisioning
        .issue_test_baseline_authority(&f.pool)
        .await?;
    let role = format!("affinity_runtime_{}", Uuid::new_v4().simple());
    let password = Uuid::new_v4().simple().to_string();
    let controller = &f.pool;
    sqlx::query(&format!("create role {role} login nosuperuser nocreatedb nocreaterole noinherit noreplication nobypassrls password '{password}'"))
        .execute(controller).await?;
    f.runtime_role = Some(role.clone());
    sqlx::raw_sql(&format!(
        "grant usage on schema public to {role};
        grant select,insert,update,delete on all tables in schema public to {role};
        grant usage,select on all sequences in schema public to {role};
        revoke insert,update,delete,truncate,references,trigger on
          feed_affinity_test_provisioning,feed_affinity_pair_baselines,
          feed_affinity_pair_provenance,feed_affinity_source_pairs,
          feed_affinity_source_contributions,feed_affinity_legacy_effect_observations,
          _sqlx_migrations from {role};
        revoke update,delete,truncate on post_user_deletion_tombstones from {role};
        revoke update,delete,truncate on timeline_projection_receipts from {role};
        grant execute on function feed_affinity_forget_subject(uuid) to {role};"
    ))
    .execute(controller)
    .await?;
    if version >= 26 {
        sqlx::raw_sql(&format!("revoke all on feed_affinity_write_permits from {role};
            revoke insert,update,delete,truncate,references,trigger on feed_affinity_pair_heads,feed_affinity_resolution_receipts from {role};
            grant execute on function feed_affinity_resolve_ordered(text,uuid,uuid,uuid,uuid,integer,text) to {role};"))
            .execute(controller).await?;
    }
    if version >= 27 {
        sqlx::raw_sql(&format!("revoke insert,update,delete,truncate,references,trigger on feed_recommendation_retry_contexts from {role};
            grant execute on function feed_recommendation_receipt_matches(public.feed_recommendation_events) to {role};"))
            .execute(controller).await?;
    }
    let runtime = PgPoolOptions::new()
        .max_connections(8)
        .connect_with(
            controller
                .connect_options()
                .as_ref()
                .clone()
                .username(&role)
                .password(&password),
        )
        .await?;
    if sqlx::query_scalar::<_, bool>(
        "select to_regprocedure('public.post_timeline_affinity_ready(uuid)') is not null",
    )
    .fetch_one(&f.pool)
    .await?
    {
        sqlx::raw_sql(&format!(
            "grant execute on function post_timeline_affinity_ready(uuid) to {role};"
        ))
        .execute(&f.pool)
        .await?;
    }
    let attributes = sqlx::query("select current_user::text as name,rolsuper,rolcreatedb,rolcreaterole,rolreplication,rolbypassrls from pg_roles where rolname=current_user")
        .fetch_one(&runtime).await?;
    assert_eq!(attributes.get::<String, _>("name"), role);
    for field in [
        "rolsuper",
        "rolcreatedb",
        "rolcreaterole",
        "rolreplication",
        "rolbypassrls",
    ] {
        assert!(!attributes.get::<bool, _>(field));
    }
    f.controller_pool = Some(std::mem::replace(&mut f.pool, runtime));
    Ok(f)
}

async fn count(f: &Fixture, table: &str) -> Result<i64> {
    Ok(sqlx::query_scalar(&format!("select count(*) from {table}"))
        .fetch_one(&f.pool)
        .await?)
}

#[tokio::test]
async fn ordinary_schema25_keeps_capture_only_without_issuing_authority() -> Result<()> {
    let f = Fixture::new(25).await?;
    let (repo, event) = seed(&f).await?;
    assert!(repo.append_event(&event).await?);
    assert_eq!(count(&f, "feed_affinity_test_provisioning").await?, 0);
    assert_eq!(count(&f, "feed_affinity_pair_baselines").await?, 0);
    assert_eq!(
        sqlx::query_scalar::<_, String>("select origin_mode from feed_affinity_pair_provenance")
            .fetch_one(&f.pool)
            .await?,
        "HOLD_CAPTURE_ONLY"
    );
    f.shutdown().await
}

#[tokio::test]
async fn even_privileged_synthetic_genesis_cannot_commit_without_an_actual_owner_source(
) -> Result<()> {
    let f = controlled().await?;
    let mut tx = f.controller_pool.as_ref().unwrap().begin().await?;
    sqlx::query(
        "select feed_affinity_capture_source('OUTBOX',$1,$2,$3,$2,$4,now(),0.08,0,repeat('0',64))",
    )
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .execute(&mut *tx)
    .await?;
    assert!(tx.commit().await.is_err());
    for table in [
        "feed_affinity_pair_baselines",
        "feed_affinity_pair_provenance",
        "feed_affinity_source_pairs",
        "feed_affinity_source_contributions",
    ] {
        assert_eq!(count(&f, table).await?, 0);
    }
    f.shutdown().await
}

#[tokio::test]
async fn actual_first_source_creates_zero_null_baseline_and_ordered_provenance_atomically(
) -> Result<()> {
    let f = controlled().await?;
    let (_, event) = seed(&f).await?;
    assert_eq!(count(&f, "feed_affinity_pair_baselines").await?, 0);
    let source = outbox(&f, &event, "POST_LIKED", false).await?;
    let row=sqlx::query("select b.*,o.origin_mode,o.first_source_id,o.legacy_write_observed,o.scalar_observed,o.capture_prefix_preexisted,
        b.provisioning_id=p.provisioning_id and b.feature_epoch=p.feature_epoch and b.writer_generation=p.writer_generation and b.epoch_id=p.capture_epoch as bound
        from feed_affinity_pair_baselines b join feed_affinity_pair_provenance o using(epoch_id,viewer_id,author_id)
        cross join feed_affinity_test_provisioning p").fetch_one(&f.pool).await?;
    assert_eq!(row.get::<f64, _>("positive_score"), 0.0);
    assert_eq!(row.get::<f64, _>("negative_score"), 0.0);
    assert_eq!(row.get::<i64, _>("resolved_sequence"), 0);
    assert!(row
        .get::<Option<chrono::DateTime<Utc>>, _>("last_interacted_at")
        .is_none());
    assert!(row.get::<bool, _>("bound"));
    assert_eq!(row.get::<String, _>("origin_mode"), "ORDERED_FRESH_BLOCKED");
    assert_eq!(row.get::<Uuid, _>("first_source_id"), source.event_id);
    for field in [
        "legacy_write_observed",
        "scalar_observed",
        "capture_prefix_preexisted",
    ] {
        assert!(!row.get::<bool, _>(field));
    }
    assert_eq!(count(&f, "feed_author_affinities").await?, 0);
    f.shutdown().await
}

#[tokio::test]
async fn runtime_cannot_provision_impersonate_controller_or_fabricate_metadata() -> Result<()> {
    let f = controlled().await?;
    for statement in [
        "select feed_affinity_issue_test_provisioning(null,null,null,null,null,null,null,null)",
        "select feed_affinity_capture_source('OUTBOX',gen_random_uuid(),gen_random_uuid(),gen_random_uuid(),null,gen_random_uuid(),now(),0.08,0,repeat('0',64))",
        "delete from feed_affinity_test_provisioning",
        "update feed_affinity_test_provisioning set retired=true",
        "insert into feed_affinity_source_pairs select capture_epoch,gen_random_uuid(),gen_random_uuid(),1 from feed_affinity_test_provisioning",
        "update feed_affinity_pair_baselines set resolved_sequence=0",
        "delete from feed_affinity_pair_provenance",
    ] {
        let error=sqlx::query(statement).execute(&f.pool).await.unwrap_err();
        assert_eq!(error.as_database_error().unwrap().code().as_deref(),Some("42501"),"{statement}: {error}");
    }
    let owner: String = sqlx::query_scalar("select current_user::text")
        .fetch_one(f.controller_pool.as_ref().unwrap())
        .await?;
    assert!(
        sqlx::query(&format!("set role \"{}\"", owner.replace('"', "\"\"")))
            .execute(&f.pool)
            .await
            .is_err()
    );
    assert_ne!(
        sqlx::query_scalar::<_, String>("select current_user::text")
            .fetch_one(&f.pool)
            .await?,
        owner
    );
    f.shutdown().await
}

#[tokio::test]
async fn concurrent_first_sources_create_one_baseline_and_contiguous_capture_only_prefix(
) -> Result<()> {
    let f = controlled().await?;
    let (_, event) = seed(&f).await?;
    let mut other = event.clone();
    other.subject_id = Uuid::new_v4();
    sqlx::query("insert into posts(id,author_id,content,visibility,created_at) select $1,author_id,'second baseline post',visibility,now() from posts where id=$2")
        .bind(other.subject_id).bind(event.subject_id).execute(&f.pool).await?;
    let (a, b) = tokio::join!(
        outbox(&f, &event, "POST_LIKED", false),
        outbox(&f, &other, "POST_COMMENTED", false)
    );
    a?;
    b?;
    assert_eq!(count(&f, "feed_affinity_pair_baselines").await?, 1);
    assert_eq!(sqlx::query_scalar::<_,Vec<i64>>("select array_agg(source_sequence order by source_sequence) from feed_affinity_source_contributions").fetch_one(&f.pool).await?,vec![1,2]);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select resolved_sequence from feed_affinity_pair_baselines")
            .fetch_one(&f.pool)
            .await?,
        0
    );
    f.shutdown().await
}

#[tokio::test]
async fn plain_zero_legacy_and_guc_writes_fail_and_recommendation_acceptance_rolls_back(
) -> Result<()> {
    let f = controlled().await?;
    let (repo, event) = seed(&f).await?;
    let source = outbox(&f, &event, "POST_LIKED", false).await?;
    let author: Uuid = sqlx::query_scalar("select author_id from posts where id=$1")
        .bind(event.subject_id)
        .fetch_one(&f.pool)
        .await?;
    for positive in [0.0, 0.08] {
        let mut tx = f.pool.begin().await?;
        sqlx::query("select set_config('port_rust.affinity_ordered_writer','true',true)")
            .execute(&mut *tx)
            .await?;
        assert!(sqlx::query("insert into feed_author_affinities(viewer_id,author_id,affinity_score,negative_score,last_interacted_at) values($1,$2,$3,0,now())")
            .bind(event.viewer_id).bind(author).bind(positive).execute(&mut *tx).await.is_err());
        tx.rollback().await?;
    }
    assert!(sqlx::query("insert into feed_author_affinity(viewer_id,author_id,affinity_score,updated_at) values($1,$2,0,now())")
        .bind(event.viewer_id).bind(author).execute(&f.pool).await.is_err());
    assert!(sqlx::query(
        "select feed_affinity_apply_legacy_effect('OUTBOX',$1,$2,$3,$2,$4,'LIKE',$5,0.08,0)"
    )
    .bind(source.event_id)
    .bind(event.viewer_id)
    .bind(author)
    .bind(event.subject_id)
    .bind(source.occurred_at)
    .execute(&f.pool)
    .await
    .is_err());
    let error = repo.append_event(&event).await.unwrap_err();
    assert_eq!(
        error.downcast_ref::<MutationError>(),
        Some(&MutationError::Unavailable)
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from feed_recommendation_events where action<>'IMPRESSION'"
        )
        .fetch_one(&f.pool)
        .await?,
        0
    );
    for table in [
        "feed_recent_actions",
        "feed_author_affinities",
        "feed_affinity_legacy_effect_observations",
    ] {
        assert_eq!(count(&f, table).await?, 0);
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select allocated_sequence from feed_affinity_source_pairs")
            .fetch_one(&f.pool)
            .await?,
        1
    );
    f.shutdown().await
}

#[tokio::test]
async fn direct_recommendation_source_cannot_commit_without_its_required_resolution() -> Result<()>
{
    let f = controlled().await?;
    let (_, event) = seed(&f).await?;
    let mut tx = f.pool.begin().await?;
    sqlx::query("insert into feed_recommendation_events(event_id,schema_version,occurred_at,viewer_id,subject_type,subject_id,actor_id,surface,action,source,model_version,impression_id)
        values($1,1,$2,$3,'POST',$4,$3,'HOME_FEED','CLICK','native','feed-linear-v1',$5)")
        .bind(event.event_id).bind(event.occurred_at).bind(event.viewer_id).bind(event.subject_id).bind(event.impression_id).execute(&mut *tx).await?;
    assert!(tx.commit().await.is_err());
    assert_eq!(count(&f, "feed_affinity_pair_baselines").await?, 0);
    assert_eq!(count(&f, "feed_affinity_source_pairs").await?, 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from feed_recommendation_events where action<>'IMPRESSION'"
        )
        .fetch_one(&f.pool)
        .await?,
        0
    );
    f.shutdown().await
}

#[tokio::test]
async fn preexisting_history_and_bad_manifest_cannot_issue_or_reinterpret_fresh_authority(
) -> Result<()> {
    let f = Fixture::new(25).await?;
    let (repo, event) = seed(&f).await?;
    repo.append_event(&event).await?;
    assert!(f
        .provisioning
        .issue_test_baseline_authority(&f.pool)
        .await
        .is_err());
    assert_eq!(count(&f, "feed_affinity_test_provisioning").await?, 0);
    assert_eq!(count(&f, "feed_affinity_pair_baselines").await?, 0);
    assert_eq!(
        sqlx::query_scalar::<_, String>("select origin_mode from feed_affinity_pair_provenance")
            .fetch_one(&f.pool)
            .await?,
        "HOLD_CAPTURE_ONLY"
    );
    f.shutdown().await?;
    let f = Fixture::new(25).await?;
    sqlx::query("update _sqlx_migrations set checksum=decode('00','hex') where version=25")
        .execute(&f.pool)
        .await?;
    assert!(f
        .provisioning
        .issue_test_baseline_authority(&f.pool)
        .await
        .is_err());
    assert_eq!(count(&f, "feed_affinity_test_provisioning").await?, 0);
    f.shutdown().await
}

#[tokio::test]
async fn retired_or_mismatched_authority_fails_closed_without_legacy_fallback() -> Result<()> {
    let f = controlled().await?;
    let (_, event) = seed(&f).await?;
    outbox(&f, &event, "POST_LIKED", false).await?;
    let controller = f.controller_pool.as_ref().unwrap();
    sqlx::query("update _sqlx_migrations set success=false where version=25")
        .execute(controller)
        .await?;
    assert!(outbox(&f, &event, "POST_COMMENTED", false).await.is_err());
    sqlx::query("update _sqlx_migrations set success=true where version=25")
        .execute(controller)
        .await?;
    sqlx::query("update feed_affinity_test_provisioning set retired=true")
        .execute(controller)
        .await?;
    assert!(outbox(&f, &event, "POST_COMMENTED", false).await.is_err());
    assert!(
        sqlx::query("update feed_affinity_test_provisioning set retired=false")
            .execute(controller)
            .await
            .is_err()
    );
    assert!(sqlx::query("delete from feed_affinity_test_provisioning")
        .execute(controller)
        .await
        .is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select allocated_sequence from feed_affinity_source_pairs")
            .fetch_one(&f.pool)
            .await?,
        1
    );
    assert_eq!(count(&f, "feed_author_affinities").await?, 0);
    f.shutdown().await
}

#[tokio::test]
async fn actual_timeline_claim_cannot_acknowledge_the_unresolved_affinity_effect() -> Result<()> {
    let f = controlled().await?;
    let (post, author) = projection::seed(&f).await?;
    let create = projection::event(&f, post, author, author, "POST_CREATED").await?;
    projection::apply(&f, &create).await?;
    let like = projection::event(&f, post, author, Uuid::new_v4(), "POST_LIKED").await?;
    let error = projection::apply(&f, &like).await.unwrap_err();
    assert_eq!(
        error.downcast_ref::<MutationError>(),
        Some(&MutationError::Unavailable)
    );
    assert_eq!(projection::cursor(&f, post).await?, 1);
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "select state from post_event_delivery_jobs where event_id=$1 and target='TIMELINE'"
        )
        .bind(like.event_id)
        .fetch_one(&f.pool)
        .await?,
        "LEASED"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from timeline_projection_receipts where event_id=$1"
        )
        .bind(like.event_id)
        .fetch_one(&f.pool)
        .await?,
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from event_publications where event_id=$1 and target='TIMELINE'"
        )
        .bind(like.event_id)
        .fetch_one(&f.pool)
        .await?,
        0
    );
    for table in [
        "feed_author_affinities",
        "feed_projection_inbox",
        "post_engagement_velocity",
    ] {
        assert_eq!(count(&f, table).await?, 0);
    }
    f.shutdown().await
}

#[tokio::test]
async fn actual_account_erasure_removes_baseline_and_delayed_source_cannot_reinitialize_it(
) -> Result<()> {
    let f = controlled().await?;
    let (_, event) = seed(&f).await?;
    outbox(&f, &event, "POST_LIKED", false).await?;
    PgUserLifecycleRepository::new(f.pool.clone())
        .apply_user_deletion(
            Uuid::new_v4(),
            event.viewer_id,
            Uuid::new_v4(),
            Utc::now() + Duration::days(7),
        )
        .await?;
    for statement in [
        "update post_user_deletion_tombstones set retention_until=now()",
        "delete from post_user_deletion_tombstones",
        "truncate post_user_deletion_tombstones",
    ] {
        let error = sqlx::query(statement).execute(&f.pool).await.unwrap_err();
        assert_eq!(
            error.as_database_error().unwrap().code().as_deref(),
            Some("42501")
        );
        assert!(sqlx::query(statement)
            .execute(f.controller_pool.as_ref().unwrap())
            .await
            .is_err());
    }
    for table in [
        "feed_affinity_pair_baselines",
        "feed_affinity_pair_provenance",
        "feed_affinity_source_pairs",
        "feed_affinity_source_contributions",
    ] {
        assert_eq!(count(&f, table).await?, 0);
    }
    assert!(outbox(&f, &event, "POST_LIKED", false).await.is_err());
    assert_eq!(count(&f, "feed_affinity_pair_baselines").await?, 0);
    f.shutdown().await
}

#[tokio::test]
async fn deleted_post_suppression_retains_the_claim_until_pair_resolution_exists() -> Result<()> {
    let f = controlled().await?;
    let (post, author) = projection::seed(&f).await?;
    let create = projection::event(&f, post, author, author, "POST_CREATED").await?;
    projection::apply(&f, &create).await?;
    let like = projection::event(&f, post, author, Uuid::new_v4(), "POST_LIKED").await?;
    sqlx::query("update posts set deleted_at=now() where id=$1")
        .bind(post)
        .execute(&f.pool)
        .await?;
    let error = projection::apply(&f, &like).await.unwrap_err();
    assert_eq!(
        error.downcast_ref::<MutationError>(),
        Some(&MutationError::Unavailable)
    );
    assert_eq!(projection::cursor(&f, post).await?, 1);
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "select state from post_event_delivery_jobs where event_id=$1 and target='TIMELINE'"
        )
        .bind(like.event_id)
        .fetch_one(&f.pool)
        .await?,
        "LEASED"
    );
    for query in [
        "select count(*) from timeline_projection_receipts where event_id=$1",
        "select count(*) from event_publications where event_id=$1 and target='TIMELINE'",
    ] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(query)
                .bind(like.event_id)
                .fetch_one(&f.pool)
                .await?,
            0
        );
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select resolved_sequence from feed_affinity_pair_baselines")
            .fetch_one(&f.pool)
            .await?,
        0
    );
    f.shutdown().await
}

#[tokio::test]
async fn sql_receipt_and_cursor_fences_reject_an_older_writer_omitting_the_adapter_gate(
) -> Result<()> {
    let f = controlled().await?;
    let (post, author) = projection::seed(&f).await?;
    let create = projection::event(&f, post, author, author, "POST_CREATED").await?;
    projection::apply(&f, &create).await?;
    let like = projection::event(&f, post, author, Uuid::new_v4(), "POST_LIKED").await?;
    sqlx::query("update posts set deleted_at=now() where id=$1")
        .bind(post)
        .execute(&f.pool)
        .await?;
    let claim = projection::authority(&f, post).await?;
    assert_eq!(claim.event.event_id, like.event_id);
    let rewrite = "update timeline_projection_receipts r set event_id=$1,aggregate_version=$2,
        identity_sha256=c.identity_sha256,disposition='SUPPRESSED',delivery_event_id=$1,delivery_attempt_id=$3
        from feed_affinity_source_contributions c where r.event_id=$4 and c.namespace='OUTBOX' and c.source_id=$1";
    let error = sqlx::query(rewrite)
        .bind(like.event_id)
        .bind(like.aggregate_version)
        .bind(claim.attempt_id)
        .bind(create.event_id)
        .execute(&f.pool)
        .await
        .unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().code().as_deref(),
        Some("42501")
    );
    // The owner role has table privileges, so this independently proves the
    // row guard protects older writers as well as the restricted runtime.
    let error = sqlx::query(rewrite)
        .bind(like.event_id)
        .bind(like.aggregate_version)
        .bind(claim.attempt_id)
        .bind(create.event_id)
        .execute(f.controller_pool.as_ref().unwrap())
        .await
        .unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().constraint(),
        Some("timeline_projection_receipt_immutable")
    );
    let error=sqlx::query("insert into timeline_projection_receipts(event_id,aggregate_id,aggregate_version,identity_sha256,disposition,delivery_event_id,delivery_attempt_id)
        select $1,$2,$3,identity_sha256,'SUPPRESSED',$1,$4 from feed_affinity_source_contributions where namespace='OUTBOX' and source_id=$1")
        .bind(like.event_id).bind(post).bind(like.aggregate_version).bind(claim.attempt_id).execute(&f.pool).await.unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().constraint(),
        Some("feed_affinity_ordered_writer_required")
    );
    let error =
        sqlx::query("update timeline_projection_cursors set last_version=$2 where aggregate_id=$1")
            .bind(post)
            .bind(like.aggregate_version)
            .execute(&f.pool)
            .await
            .unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().constraint(),
        Some("feed_affinity_ordered_writer_required")
    );
    assert_eq!(projection::cursor(&f, post).await?, 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select aggregate_version from timeline_projection_receipts where event_id=$1"
        )
        .bind(create.event_id)
        .fetch_one(&f.pool)
        .await?,
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select resolved_sequence from feed_affinity_pair_baselines")
            .fetch_one(&f.pool)
            .await?,
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from event_publications where event_id=$1")
            .bind(like.event_id)
            .fetch_one(&f.pool)
            .await?,
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from timeline_projection_receipts where event_id=$1"
        )
        .bind(like.event_id)
        .fetch_one(&f.pool)
        .await?,
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "select state from post_event_delivery_jobs where event_id=$1 and target='TIMELINE'"
        )
        .bind(like.event_id)
        .fetch_one(&f.pool)
        .await?,
        "LEASED"
    );
    f.shutdown().await
}

#[tokio::test]
async fn terminal_account_erasure_may_acknowledge_without_recreating_erased_pair_state(
) -> Result<()> {
    let f = controlled().await?;
    let (post, author) = projection::seed(&f).await?;
    let create = projection::event(&f, post, author, author, "POST_CREATED").await?;
    projection::apply(&f, &create).await?;
    let actor = Uuid::new_v4();
    let like = projection::event(&f, post, author, actor, "POST_LIKED").await?;
    PgUserLifecycleRepository::new(f.pool.clone())
        .apply_user_deletion(
            Uuid::new_v4(),
            actor,
            Uuid::new_v4(),
            Utc::now() + Duration::days(7),
        )
        .await?;
    projection::apply(&f, &like).await?;
    assert_eq!(projection::cursor(&f, post).await?, 2);
    for table in [
        "feed_affinity_pair_baselines",
        "feed_affinity_pair_provenance",
        "feed_affinity_source_pairs",
        "feed_affinity_source_contributions",
        "feed_author_affinities",
    ] {
        assert_eq!(count(&f, table).await?, 0);
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from post_user_deletion_tombstones where user_id=$1"
        )
        .bind(actor)
        .fetch_one(&f.pool)
        .await?,
        1
    );
    f.shutdown().await
}
