use chrono::Utc;
use friend_service::adapters::out::persistence::follow_repository::PgFollowRepository;
use friend_service::application::ports::FollowRepositoryPort;
use friend_service::domain::follow_edge::FollowEdge;
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;
use uuid::Uuid;

#[tokio::test]
async fn postgres_repository_methods_propagate_pool_errors_without_real_database() {
    let repository = repository_without_database();
    let follower_id = uuid(31);
    let followee_id = uuid(32);
    let edge = FollowEdge::new(follower_id, followee_id, Utc::now()).expect("edge");

    assert!(repository.follow(&edge).await.is_err());
    assert!(repository.unfollow(follower_id, followee_id).await.is_err());
    assert!(repository
        .list_followers(followee_id, None, 10)
        .await
        .is_err());
    assert!(repository
        .list_following(follower_id, None, 10)
        .await
        .is_err());
    assert!(repository
        .is_following(follower_id, followee_id)
        .await
        .is_err());
    assert!(repository.counts(follower_id).await.is_err());
    assert!(repository.batch_counts(&[follower_id]).await.is_err());
}

fn repository_without_database() -> PgFollowRepository {
    let pool = PgPoolOptions::new()
        .acquire_timeout(Duration::from_millis(5))
        .connect_lazy("postgres://postgres:postgres@127.0.0.1:1/friend_unit")
        .expect("lazy pool");
    PgFollowRepository::new(pool)
}

fn uuid(suffix: u128) -> Uuid {
    Uuid::from_u128(suffix)
}
