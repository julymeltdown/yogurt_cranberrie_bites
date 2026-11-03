//! Reuse the existing marker/inode/Unix-socket-owned native bootstrap fixture.
#[path = "../bootstrap/mod.rs"]
mod bootstrap;
pub use bootstrap::*;
use concert_application::avatar::*;
use concert_domain::ParticipantKey;
use std::sync::atomic::{AtomicUsize, Ordering};
pub struct Authority {
    pub key: ParticipantKey,
    pub op: AvatarOperation,
    pub digest: String,
    pub calls: AtomicUsize,
    pub fail_at: usize,
}
impl Authority {
    pub fn new(key: ParticipantKey, op: AvatarOperation, digest: &str) -> Self {
        Self {
            key,
            op,
            digest: digest.into(),
            calls: AtomicUsize::new(0),
            fail_at: usize::MAX,
        }
    }
}
impl AvatarAuthority for Authority {
    fn check(&self) -> Result<AvatarGrant, AvatarError> {
        if self.calls.fetch_add(1, Ordering::SeqCst) + 1 >= self.fail_at {
            return Err(AvatarError::Unauthenticated);
        }
        Ok(AvatarGrant {
            key: self.key,
            operation: self.op,
            release_digest: self.digest.clone(),
        })
    }
}
