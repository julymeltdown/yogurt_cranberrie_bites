pub fn describe() -> &'static str {
    "friend use cases"
}
pub mod accept_follow_request;
pub mod apply_user_deletion;
pub mod block_user;
pub mod cancel_follow_request;
pub mod check_follow;
pub mod follow;
pub mod get_counts;
pub mod get_people_recommendations;
pub mod get_relationship_policy;
pub mod get_stats_batch;
pub mod list_follow_requests;
pub mod list_followers;
pub mod list_following;
pub mod mute_user;
pub mod purge_expired_mutation_outcomes;
pub mod record_recommendation_event;
pub mod reject_follow_request;
pub mod unblock_user;
pub mod unfollow;
pub mod unmute_user;
