pub fn describe() -> &'static str {
    "notification use cases"
}
pub mod apply_user_deletion;
pub mod create_notification;
pub mod delete_notification;
pub mod list_notifications;
pub mod mark_all_read;
pub mod mark_read;
pub mod purge_expired_notifications;
