//! Conservative provenance does not authorize a fresh baseline or ordered
//! replay. These probes exercise actual owner writes and account erasure.
use super::{affinity_source::outbox, recommendation_admission::seed, Fixture};
use anyhow::Result;
use chrono::{Duration, Utc};
use post_service::adapters::out::persistence::user_lifecycle_repository::PgUserLifecycleRepository;
use post_service::application::ports::{
    FeedFeatureRepositoryPort, FeedRecommendationRepositoryPort, UserLifecycleRepositoryPort,
};
use post_service::domain::ranking::FeedEngagementAction;
use sqlx::Row;
use uuid::Uuid;

#[tokio::test]
async fn current_owner_effect_is_observed_without_becoming_an_ordered_receipt() -> Result<()> {
    let f = Fixture::new(24).await?;
    let (repo, event) = seed(&f).await?;
    assert!(repo.append_event(&event).await?);
    let row = origin(&f).await?;
    assert_eq!(row.get::<String, _>("origin_mode"), "HOLD_CAPTURE_ONLY");
    assert_eq!(row.get::<i64, _>("first_source_sequence"), 1);
    assert_eq!(row.get::<Uuid, _>("first_source_id"), event.event_id);
    assert!(row.get::<bool, _>("legacy_write_observed"));
    assert!(row.get::<bool, _>("scalar_observed"));
    assert!(!row.get::<bool, _>("capture_prefix_preexisted"));
    assert_eq!(
        sqlx::query_scalar::<_, f64>("select affinity_score from feed_author_affinities")
            .fetch_one(&f.pool)
            .await?,
        0.08
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select allocated_sequence from feed_affinity_source_pairs")
            .fetch_one(&f.pool)
            .await?,
        1
    );
    assert!(!repo.append_event(&event).await?);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from feed_affinity_pair_provenance")
            .fetch_one(&f.pool)
            .await?,
        1
    );
    f.shutdown().await
}

#[tokio::test]
async fn upgrade_never_backfills_fresh_or_replays_existing_capture_only_scores() -> Result<()> {
    let f = Fixture::new(23).await?;
    let (repo, event) = seed(&f).await?;
    repo.append_event(&event).await?;
    let before: serde_json::Value =
        sqlx::query_scalar("select to_jsonb(a) from feed_author_affinities a")
            .fetch_one(&f.pool)
            .await?;
    f.upgrade().await?;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from feed_affinity_pair_provenance")
            .fetch_one(&f.pool)
            .await?,
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, serde_json::Value>(
            "select to_jsonb(a) from feed_author_affinities a"
        )
        .fetch_one(&f.pool)
        .await?,
        before
    );
    let mut next = event.clone();
    next.event_id = Uuid::new_v4();
    repo.append_event(&next).await?;
    let row = origin(&f).await?;
    assert_eq!(row.get::<i64, _>("first_source_sequence"), 2);
    assert!(row.get::<bool, _>("capture_prefix_preexisted"));
    assert!(row.get::<bool, _>("scalar_observed"));
    assert_eq!(row.get::<String, _>("origin_mode"), "HOLD_CAPTURE_ONLY");
    f.shutdown().await
}

#[tokio::test]
async fn rollback_does_not_leave_a_false_legacy_write_or_first_source_observation() -> Result<()> {
    let f = Fixture::new(24).await?;
    let (repo, event) = seed(&f).await?;
    sqlx::raw_sql("create function provenance_fail_after_score() returns trigger language plpgsql as $$ begin raise exception 'actual score transaction fails'; end $$; create trigger zz_provenance_fail after insert on feed_author_affinities for each row execute function provenance_fail_after_score();")
        .execute(&f.pool).await?;
    assert!(repo.append_event(&event).await.is_err());
    for table in [
        "feed_affinity_pair_provenance",
        "feed_affinity_source_pairs",
        "feed_affinity_source_contributions",
        "feed_author_affinities",
    ] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(&format!("select count(*) from {table}"))
                .fetch_one(&f.pool)
                .await?,
            0,
            "rolled back {table}"
        );
    }
    sqlx::query("drop trigger zz_provenance_fail on feed_author_affinities")
        .execute(&f.pool)
        .await?;
    repo.append_event(&event).await?;
    assert_eq!(origin(&f).await?.get::<i64, _>("first_source_sequence"), 1);
    f.shutdown().await
}

#[tokio::test]
async fn disappearing_legacy_scalar_cannot_erase_the_observed_writer_history() -> Result<()> {
    let f = Fixture::new(24).await?;
    let (repo, event) = seed(&f).await?;
    let author: Uuid = sqlx::query_scalar("select author_id from posts where id=$1")
        .bind(event.subject_id)
        .fetch_one(&f.pool)
        .await?;
    sqlx::query(
        "insert into feed_author_affinity(viewer_id,author_id,affinity_score) values($1,$2,.4)",
    )
    .bind(event.viewer_id)
    .bind(author)
    .execute(&f.pool)
    .await?;
    sqlx::query("delete from feed_author_affinity where viewer_id=$1 and author_id=$2")
        .bind(event.viewer_id)
        .bind(author)
        .execute(&f.pool)
        .await?;
    let before = origin(&f).await?;
    assert!(before.get::<bool, _>("legacy_scalar_observed"));
    assert!(before
        .get::<Option<i64>, _>("first_source_sequence")
        .is_none());
    repo.append_event(&event).await?;
    assert!(origin(&f).await?.get::<bool, _>("legacy_scalar_observed"));
    assert!(
        sqlx::query("update feed_affinity_pair_provenance set legacy_write_observed=false")
            .execute(&f.pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("update feed_affinity_pair_provenance set origin_mode='ORDERED_ACTIVE'")
            .execute(&f.pool)
            .await
            .is_err()
    );
    f.shutdown().await
}

#[tokio::test]
async fn concurrent_sources_keep_one_first_identity_without_false_preexisting_prefix() -> Result<()>
{
    let f = Fixture::new(24).await?;
    let (repo, event) = seed(&f).await?;
    let (a, b) = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        tokio::join!(
            outbox(&f, &event, "POST_LIKED", false),
            repo.append_event(&event)
        )
    })
    .await?;
    a?;
    b?;
    let row = origin(&f).await?;
    let first: (String,Uuid,String)=sqlx::query_as("select namespace,source_id,identity_sha256 from feed_affinity_source_contributions where source_sequence=1")
        .fetch_one(&f.pool).await?;
    assert_eq!(
        (
            row.get::<String, _>("first_namespace"),
            row.get::<Uuid, _>("first_source_id"),
            row.get::<String, _>("first_identity_sha256")
        ),
        first
    );
    assert!(!row.get::<bool, _>("capture_prefix_preexisted"));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select allocated_sequence from feed_affinity_source_pairs")
            .fetch_one(&f.pool)
            .await?,
        2
    );
    f.shutdown().await
}

#[tokio::test]
async fn distinct_actor_erasure_removes_first_source_identity_and_never_rebases() -> Result<()> {
    let f = Fixture::new(24).await?;
    let (repo, mut event) = seed(&f).await?;
    let actor = Uuid::new_v4();
    event.actor_id = Some(actor);
    repo.append_event(&event).await?;
    PgUserLifecycleRepository::new(f.pool.clone())
        .apply_user_deletion(
            Uuid::new_v4(),
            actor,
            Uuid::new_v4(),
            Utc::now() + Duration::days(7),
        )
        .await?;
    let erased = origin(&f).await?;
    assert!(erased.get::<bool, _>("first_source_erased"));
    assert_eq!(erased.get::<i64, _>("first_source_sequence"), 1);
    assert!(erased.get::<Option<Uuid>, _>("first_source_id").is_none());
    assert!(erased
        .get::<Option<String>, _>("first_identity_sha256")
        .is_none());
    assert!(repo.append_event(&event).await.is_err());
    let mut other = event;
    other.event_id = Uuid::new_v4();
    other.actor_id = None;
    repo.append_event(&other).await?;
    let final_row = origin(&f).await?;
    assert!(final_row.get::<bool, _>("first_source_erased"));
    assert!(final_row
        .get::<Option<Uuid>, _>("first_source_id")
        .is_none());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select allocated_sequence from feed_affinity_source_pairs")
            .fetch_one(&f.pool)
            .await?,
        2
    );
    f.shutdown().await
}

#[tokio::test]
async fn viewer_deletion_erases_origins_in_the_real_account_transaction() -> Result<()> {
    let f = Fixture::new(24).await?;
    let (repo, event) = seed(&f).await?;
    repo.append_event(&event).await?;
    PgUserLifecycleRepository::new(f.pool.clone())
        .apply_user_deletion(
            Uuid::new_v4(),
            event.viewer_id,
            Uuid::new_v4(),
            Utc::now() + Duration::days(7),
        )
        .await?;
    for table in [
        "feed_affinity_pair_provenance",
        "feed_affinity_source_pairs",
        "feed_affinity_source_contributions",
    ] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(&format!("select count(*) from {table}"))
                .fetch_one(&f.pool)
                .await?,
            0
        );
    }
    assert!(repo.append_event(&event).await.is_err());
    f.shutdown().await
}

#[tokio::test]
async fn source_bound_observation_failure_rolls_back_the_actual_owner_score_write() -> Result<()> {
    let f = Fixture::new(24).await?;
    let (repo, event) = seed(&f).await?;
    sqlx::raw_sql("create function reject_legacy_effect_observation() returns trigger language plpgsql as $$ begin raise exception 'source-bound observation fails'; end $$; create trigger reject_legacy_effect_observation before insert on feed_affinity_legacy_effect_observations for each row execute function reject_legacy_effect_observation();")
        .execute(&f.pool).await?;
    assert!(repo.append_event(&event).await.is_err());
    for table in [
        "feed_affinity_pair_provenance",
        "feed_affinity_legacy_effect_observations",
        "feed_affinity_source_pairs",
        "feed_affinity_source_contributions",
        "feed_author_affinities",
        "feed_recent_actions",
    ] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(&format!("select count(*) from {table}"))
                .fetch_one(&f.pool)
                .await?,
            0
        );
    }
    sqlx::query(
        "drop trigger reject_legacy_effect_observation on feed_affinity_legacy_effect_observations",
    )
    .execute(&f.pool)
    .await?;
    repo.append_event(&event).await?;
    let binding:bool=sqlx::query_scalar("select o.source_identity_sha256=c.identity_sha256 and o.source_sequence=c.source_sequence from feed_affinity_legacy_effect_observations o join feed_affinity_source_contributions c using(namespace,source_id)")
        .fetch_one(&f.pool).await?;
    assert!(binding);
    assert!(!repo.append_event(&event).await?);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from feed_affinity_legacy_effect_observations"
        )
        .fetch_one(&f.pool)
        .await?,
        1
    );
    assert!(sqlx::query(
        "update feed_affinity_legacy_effect_observations set effect_identity_sha256=repeat('0',64)"
    )
    .execute(&f.pool)
    .await
    .is_err());
    f.shutdown().await
}

#[tokio::test]
async fn captured_source_cannot_observe_another_pair_post_actor_action_or_time() -> Result<()> {
    let f = Fixture::new(24).await?;
    let (repo, event) = seed(&f).await?;
    let source = outbox(&f, &event, "POST_LIKED", false).await?;
    let author: Uuid = sqlx::query_scalar("select author_id from posts where id=$1")
        .bind(event.subject_id)
        .fetch_one(&f.pool)
        .await?;
    let other_author = Uuid::new_v4();
    let other_post = Uuid::new_v4();
    let same_author_post = Uuid::new_v4();
    for (post, owner) in [(other_post, other_author), (same_author_post, author)] {
        sqlx::query("insert into posts(id,author_id,content,visibility,created_at) values($1,$2,'wrong binding','PUBLIC',now())").bind(post).bind(owner).execute(&f.pool).await?;
    }
    for owner in [author, other_author] {
        sqlx::query("insert into feed_author_affinities(viewer_id,author_id,affinity_score,last_interacted_at) values($1,$2,.4,now())")
            .bind(event.viewer_id).bind(owner).execute(&f.pool).await?;
    }
    let before: serde_json::Value = sqlx::query_scalar(
        "select jsonb_agg(to_jsonb(a) order by author_id) from feed_author_affinities a",
    )
    .fetch_one(&f.pool)
    .await?;
    for (post, owner, actor, action, time) in [
        (
            other_post,
            other_author,
            event.viewer_id,
            FeedEngagementAction::Like,
            source.occurred_at,
        ),
        (
            same_author_post,
            author,
            event.viewer_id,
            FeedEngagementAction::Like,
            source.occurred_at,
        ),
        (
            event.subject_id,
            author,
            Uuid::new_v4(),
            FeedEngagementAction::Like,
            source.occurred_at,
        ),
        (
            event.subject_id,
            author,
            event.viewer_id,
            FeedEngagementAction::Comment,
            source.occurred_at,
        ),
        (
            event.subject_id,
            author,
            event.viewer_id,
            FeedEngagementAction::Like,
            source.occurred_at + Duration::seconds(1),
        ),
    ] {
        assert!(repo
            .record_engagement(source.event_id, post, actor, owner, action, time)
            .await
            .is_err());
        assert_eq!(
            sqlx::query_scalar::<_, serde_json::Value>(
                "select jsonb_agg(to_jsonb(a) order by author_id) from feed_author_affinities a"
            )
            .fetch_one(&f.pool)
            .await?,
            before
        );
        for table in [
            "feed_projection_inbox",
            "post_engagement_velocity",
            "feed_affinity_legacy_effect_observations",
        ] {
            assert_eq!(
                sqlx::query_scalar::<_, i64>(&format!("select count(*) from {table}"))
                    .fetch_one(&f.pool)
                    .await?,
                0,
                "no forged binding in {table}"
            );
        }
    }
    f.shutdown().await
}

#[tokio::test]
async fn direct_coupled_routine_cannot_record_an_observation_without_its_scalar_mutation(
) -> Result<()> {
    let f = Fixture::new(24).await?;
    let (_, event) = seed(&f).await?;
    let source = outbox(&f, &event, "POST_LIKED", false).await?;
    let author: Uuid = sqlx::query_scalar("select author_id from posts where id=$1")
        .bind(event.subject_id)
        .fetch_one(&f.pool)
        .await?;
    sqlx::query("insert into feed_author_affinities(viewer_id,author_id,affinity_score,last_interacted_at) values($1,$2,.4,now())")
        .bind(event.viewer_id).bind(author).execute(&f.pool).await?;
    sqlx::raw_sql("create function reject_bound_scalar_mutation() returns trigger language plpgsql as $$ begin raise exception 'scalar mutation rejected'; end $$; create trigger reject_bound_scalar_mutation before update on feed_author_affinities for each row execute function reject_bound_scalar_mutation();")
        .execute(&f.pool).await?;
    assert!(sqlx::query("select feed_affinity_apply_legacy_effect('OUTBOX',$1,$2,$3,$2,$4,'LIKE',$5,.08::float8,0::float8)")
        .bind(source.event_id).bind(event.viewer_id).bind(author).bind(event.subject_id).bind(source.occurred_at)
        .execute(&f.pool).await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from feed_affinity_legacy_effect_observations"
        )
        .fetch_one(&f.pool)
        .await?,
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, f64>("select affinity_score from feed_author_affinities")
            .fetch_one(&f.pool)
            .await?,
        0.4
    );
    sqlx::query("drop trigger reject_bound_scalar_mutation on feed_author_affinities")
        .execute(&f.pool)
        .await?;
    sqlx::query("select feed_affinity_apply_legacy_effect('OUTBOX',$1,$2,$3,$2,$4,'LIKE',$5,.08::float8,0::float8)")
        .bind(source.event_id).bind(event.viewer_id).bind(author).bind(event.subject_id).bind(source.occurred_at)
        .execute(&f.pool).await?;
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from feed_affinity_legacy_effect_observations"
        )
        .fetch_one(&f.pool)
        .await?,
        1
    );
    let actual: f64 = sqlx::query_scalar("select affinity_score from feed_author_affinities")
        .fetch_one(&f.pool)
        .await?;
    assert!((actual - (0.4 * 0.95 + 0.08)).abs() < 1e-12);
    f.shutdown().await
}

async fn origin(f: &Fixture) -> Result<sqlx::postgres::PgRow> {
    Ok(sqlx::query("select * from feed_affinity_pair_provenance")
        .fetch_one(&f.pool)
        .await?)
}

#[tokio::test]
async fn silently_skipped_or_altered_score_write_cannot_create_a_source_observation() -> Result<()>
{
    let f = Fixture::new(24).await?;
    let (repo, event) = seed(&f).await?;
    let source = outbox(&f, &event, "POST_LIKED", false).await?;
    let author: Uuid = sqlx::query_scalar("select author_id from posts where id=$1")
        .bind(event.subject_id)
        .fetch_one(&f.pool)
        .await?;
    sqlx::query("insert into feed_author_affinities(viewer_id,author_id,affinity_score,last_interacted_at) values($1,$2,.4,now())")
        .bind(event.viewer_id).bind(author).execute(&f.pool).await?;
    for body in [
        "return null;",
        "new.affinity_score:=old.affinity_score; return new;",
    ] {
        sqlx::raw_sql(&format!("create or replace function skip_bound_score() returns trigger language plpgsql as $$ begin {body} end $$;create trigger skip_bound_score before update on feed_author_affinities for each row execute function skip_bound_score();"))
            .execute(&f.pool).await?;
        let err = repo
            .record_engagement(
                source.event_id,
                event.subject_id,
                event.viewer_id,
                author,
                FeedEngagementAction::Like,
                source.occurred_at,
            )
            .await
            .expect_err("no observation without the expected actual mutation");
        assert!(err.to_string().contains("scalar mutation"));
        assert_eq!(
            sqlx::query_scalar::<_, f64>("select affinity_score from feed_author_affinities")
                .fetch_one(&f.pool)
                .await?,
            0.4
        );
        for table in [
            "feed_affinity_legacy_effect_observations",
            "feed_projection_inbox",
            "post_engagement_velocity",
        ] {
            assert_eq!(
                sqlx::query_scalar::<_, i64>(&format!("select count(*) from {table}"))
                    .fetch_one(&f.pool)
                    .await?,
                0
            );
        }
        sqlx::query("drop trigger skip_bound_score on feed_author_affinities")
            .execute(&f.pool)
            .await?;
    }
    f.shutdown().await
}

#[tokio::test]
async fn unbound_direct_routine_fences_a_deleted_distinct_actor_before_any_scalar_write(
) -> Result<()> {
    let f = Fixture::new(24).await?;
    let (_, event) = seed(&f).await?;
    let author: Uuid = sqlx::query_scalar("select author_id from posts where id=$1")
        .bind(event.subject_id)
        .fetch_one(&f.pool)
        .await?;
    let actor = Uuid::new_v4();
    PgUserLifecycleRepository::new(f.pool.clone())
        .apply_user_deletion(
            Uuid::new_v4(),
            actor,
            Uuid::new_v4(),
            Utc::now() + Duration::days(7),
        )
        .await?;
    let err=sqlx::query("select feed_affinity_apply_legacy_effect('RECOMMENDATION',$1,$2,$3,$4,$5,'CLICK',$6,.08::float8,0::float8)")
        .bind(Uuid::new_v4()).bind(event.viewer_id).bind(author).bind(actor).bind(event.subject_id).bind(event.occurred_at)
        .execute(&f.pool).await.expect_err("unbound source cannot bypass actual actor retirement");
    assert!(err.to_string().contains("deleted"));
    for table in [
        "feed_author_affinities",
        "feed_affinity_pair_provenance",
        "feed_affinity_legacy_effect_observations",
    ] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(&format!("select count(*) from {table}"))
                .fetch_one(&f.pool)
                .await?,
            0
        );
    }
    f.shutdown().await
}
