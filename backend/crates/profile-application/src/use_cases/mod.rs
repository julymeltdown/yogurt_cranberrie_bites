pub fn describe() -> &'static str {
    "profile use cases"
}
pub mod apply_user_deletion;
pub mod delete_profile;
pub mod get_avatar;
pub mod get_profile;
pub mod get_profile_by_username;
pub mod get_profiles_batch;
pub mod purge_expired_mutation_outcomes;
pub mod purge_expired_profiles;
pub mod search_profiles;
pub mod update_privacy;
pub mod update_profile;
pub mod upload_avatar;
