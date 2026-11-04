//! Real Notification transport integration. The generated contract alone grants
//! no actor, target, transaction settlement or public-route authority.
pub mod proto {
    include!(concat!(
        env!("OUT_DIR"),
        "/notification_authority/notification.concert.v1.rs"
    ));
}
mod client;
pub use client::{NotificationClient, NotificationRecoveryBinding};
