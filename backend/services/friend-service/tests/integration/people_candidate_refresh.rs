use crate::support::start_postgres_app;
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use friend_service::adapters::out::persistence::people_recommendation_repository::PgPeopleRecommendationRepository;
use friend_service::application::ports::PeopleRecommendationRepositoryPort;
use friend_service::domain::recommendation::PeopleCandidateSource;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

const VIEWER: &str = "ffffffff-ffff-4fff-8fff-fffffffffff0";
const LOW_BRIDGE: &str = "00000000-0000-0000-0000-000000000002";
const HIGH_BRIDGE: &str = "a0000000-0000-0000-0000-000000000003";
const MUTUAL: &str = "90000000-0000-0000-0000-000000000001";
const SECOND: &str = "90000000-0000-0000-0000-000000000002";
const DIRECT: &str = "90000000-0000-0000-0000-000000000003";
const DELETED: &str = "90000000-0000-0000-0000-000000000004";
const OVERLAP: &str = "90000000-0000-0000-0000-000000000005";

fn id(value: &str) -> Uuid {
    Uuid::parse_str(value).unwrap()
}

fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-10-11T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc)
}

async fn seed_graph(pool: &PgPool, reverse: bool) -> Result<()> {
    let mut edges = vec![
        (VIEWER, HIGH_BRIDGE, false),
        (VIEWER, LOW_BRIDGE, false),
        (VIEWER, DIRECT, false),
        (HIGH_BRIDGE, MUTUAL, false),
        (LOW_BRIDGE, MUTUAL, false),
        (HIGH_BRIDGE, SECOND, false),
        (HIGH_BRIDGE, DIRECT, false),
        (LOW_BRIDGE, VIEWER, false),
        (LOW_BRIDGE, DELETED, true),
        (OVERLAP, HIGH_BRIDGE, false),
        (OVERLAP, LOW_BRIDGE, false),
    ];
    if reverse {
        edges.reverse();
    }
    for (from, to, deleted) in edges {
        sqlx::query(
            "insert into follows (follower_id, followee_id, deleted_at) values ($1, $2, case when $3 then now() else null end)",
        )
        .bind(id(from))
        .bind(id(to))
        .bind(deleted)
        .execute(pool)
        .await?;
    }
    Ok(())
}

async fn old_candidate(pool: &PgPool, viewer: Uuid, expires: DateTime<Utc>) -> Result<()> {
    let features = json!({
        "mutual_count":0,"mutual_score":0.0,"adamic_adar":0.0,"jaccard":0.0,
        "profile_overlap":0.0,"interaction_affinity":0.0,"freshness":0.0,
        "candidate_quality":0.0,"impression_penalty":0.0,"dismissal_penalty":0.0
    });
    sqlx::query(
        "insert into people_recommendation_candidates (viewer_id,candidate_user_id,source,source_score,feature_snapshot_json,generated_at,expires_at,model_version) values ($1,$2,'EXPLORATION',0.1,$3,$4,$5,'old-model')",
    )
    .bind(viewer)
    .bind(Uuid::new_v4())
    .bind(features.to_string())
    .bind(now() - Duration::days(1))
    .bind(expires)
    .execute(pool)
    .await?;
    Ok(())
}

async fn table_snapshot(pool: &PgPool) -> Result<Value> {
    Ok(sqlx::query_scalar(
        "select coalesce(jsonb_agg(to_jsonb(candidate) order by viewer_id,candidate_user_id,source), '[]'::jsonb) from people_recommendation_candidates candidate",
    )
    .fetch_one(pool)
    .await?)
}

#[tokio::test]
async fn refresh_candidates_commits_all_sources_and_deterministic_uuid_cluster() -> Result<()> {
    let app = start_postgres_app(false).await?;
    let result = async {
        seed_graph(&app.pool, false).await?;
        old_candidate(&app.pool, id(VIEWER), now() + Duration::hours(1)).await?;
        let repository = PgPeopleRecommendationRepository::new(app.pool.clone());
        let count = repository
            .refresh_candidates(
                id(VIEWER),
                now(),
                now() + Duration::hours(1),
                "people-linear-v1",
                100,
            )
            .await?;
        let first = repository.list_candidates(id(VIEWER), now(), 2_000).await?;
        assert_eq!(count, first.len() as i64);
        assert!(count > 0);
        for source in [
            PeopleCandidateSource::MutualFollow,
            PeopleCandidateSource::FollowerOverlap,
            PeopleCandidateSource::Exploration,
        ] {
            assert!(first.iter().any(|candidate| candidate.source == source));
        }
        let mutual = first
            .iter()
            .find(|candidate| {
                candidate.candidate_user_id == id(MUTUAL)
                    && candidate.source == PeopleCandidateSource::MutualFollow
            })
            .unwrap();
        assert_eq!(mutual.mutual_cluster, Some(id(LOW_BRIDGE)));
        assert_eq!(mutual.features.mutual_count, 2);
        assert!((mutual.source_score - 0.2).abs() < 1e-12);
        assert!(mutual.features.adamic_adar.is_finite() && mutual.features.adamic_adar > 0.0);
        let second = first
            .iter()
            .find(|candidate| {
                candidate.candidate_user_id == id(SECOND)
                    && candidate.source == PeopleCandidateSource::MutualFollow
            })
            .unwrap();
        assert_eq!(second.mutual_cluster, Some(id(HIGH_BRIDGE)));
        assert_eq!(second.features.mutual_count, 1);
        assert!(first
            .iter()
            .all(|candidate| ![id(VIEWER), id(DIRECT), id(DELETED)]
                .contains(&candidate.candidate_user_id)));
        assert!(first
            .iter()
            .all(|candidate| candidate.model_version == "people-linear-v1"));
        sqlx::query("delete from follows")
            .execute(&app.pool)
            .await?;
        seed_graph(&app.pool, true).await?;
        repository
            .refresh_candidates(
                id(VIEWER),
                now(),
                now() + Duration::hours(1),
                "people-linear-v1",
                100,
            )
            .await?;
        let reversed = repository.list_candidates(id(VIEWER), now(), 2_000).await?;
        let repeated = reversed
            .iter()
            .find(|candidate| {
                candidate.candidate_user_id == id(MUTUAL)
                    && candidate.source == PeopleCandidateSource::MutualFollow
            })
            .unwrap();
        assert_eq!(repeated.mutual_cluster, mutual.mutual_cluster);
        assert_eq!(repeated.features, mutual.features);
        repository
            .refresh_candidates(
                id(VIEWER),
                now(),
                now() + Duration::hours(1),
                "people-linear-v1",
                1,
            )
            .await?;
        let bounded = repository.list_candidates(id(VIEWER), now(), 2_000).await?;
        let mutual_ids: Vec<_> = bounded
            .iter()
            .filter(|candidate| candidate.source == PeopleCandidateSource::MutualFollow)
            .map(|candidate| candidate.candidate_user_id)
            .collect();
        assert_eq!(mutual_ids, vec![id(MUTUAL)]);
        Ok(())
    }
    .await;
    app.shutdown().await?;
    result
}

#[tokio::test]
async fn refresh_candidates_rolls_back_partial_sources_and_recovers_on_retry() -> Result<()> {
    let app = start_postgres_app(false).await?;
    let result = async {
        seed_graph(&app.pool, false).await?;
        old_candidate(&app.pool, id(VIEWER), now() + Duration::hours(1)).await?;
        old_candidate(&app.pool, Uuid::new_v4(), now() - Duration::hours(1)).await?;
        old_candidate(&app.pool, Uuid::new_v4(), now() + Duration::hours(1)).await?;
        let before = table_snapshot(&app.pool).await?;
        sqlx::raw_sql(r#"
            create function reject_overlap_candidate() returns trigger language plpgsql as $$
            begin
                if new.source = 'FOLLOWER_OVERLAP' then
                    raise exception 'injected second-source insertion failure';
                end if;
                return new;
            end;
            $$;
            create trigger reject_overlap_candidate before insert on people_recommendation_candidates
            for each row execute function reject_overlap_candidate();
        "#).execute(&app.pool).await?;
        let repository = PgPeopleRecommendationRepository::new(app.pool.clone());
        let error = repository.refresh_candidates(id(VIEWER), now(), now() + Duration::hours(1), "people-linear-v1", 100).await.expect_err("second insertion stage must fail");
        assert!(error.to_string().contains("injected second-source insertion failure"));
        assert_eq!(table_snapshot(&app.pool).await?, before, "initial deletion and first-source writes must roll back together");
        sqlx::raw_sql("drop trigger reject_overlap_candidate on people_recommendation_candidates; drop function reject_overlap_candidate();").execute(&app.pool).await?;
        let count = repository.refresh_candidates(id(VIEWER), now(), now() + Duration::hours(1), "people-linear-v1", 100).await?;
        assert!(count > 0);
        let remaining_old: i64 = sqlx::query_scalar("select count(*) from people_recommendation_candidates where model_version='old-model'").fetch_one(&app.pool).await?;
        assert_eq!(remaining_old, 1, "only another viewer's still-live row survives successful replacement/expiry");
        Ok(())
    }.await;
    app.shutdown().await?;
    result
}

#[tokio::test]
async fn refresh_candidates_empty_graph_replaces_old_candidates() -> Result<()> {
    let app = start_postgres_app(false).await?;
    let result = async {
        old_candidate(&app.pool, id(VIEWER), now() + Duration::hours(1)).await?;
        let repository = PgPeopleRecommendationRepository::new(app.pool.clone());
        let count = repository
            .refresh_candidates(
                id(VIEWER),
                now(),
                now() + Duration::hours(1),
                "people-linear-v1",
                100,
            )
            .await?;
        assert_eq!(count, 0);
        assert!(repository
            .list_candidates(id(VIEWER), now(), 100)
            .await?
            .is_empty());
        Ok(())
    }
    .await;
    app.shutdown().await?;
    result
}
