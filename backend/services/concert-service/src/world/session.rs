//! Captured credential deadline and fail-closed subscription lifecycle; no credential material.
use super::simulation::{AuthorityError, Binding, MembershipPort, AUTH_LEASE_MAX_MS};
use chrono::{DateTime, Utc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RevokeReason {
    SessionExpired,
    SessionRevoked,
    Suspended,
    EventClosed,
    PolicyUnavailable,
    ConnectionReplaced,
}
impl RevokeReason {
    pub fn wire(self) -> &'static str {
        match self {
            Self::SessionExpired => "SESSION_EXPIRED",
            Self::SessionRevoked => "SESSION_REVOKED",
            Self::Suspended => "SUSPENDED",
            Self::EventClosed => "EVENT_CLOSED",
            Self::PolicyUnavailable => "POLICY_UNAVAILABLE",
            Self::ConnectionReplaced => "CONNECTION_REPLACED",
        }
    }
    pub fn close_code(self) -> u16 {
        match self {
            Self::SessionExpired | Self::SessionRevoked | Self::ConnectionReplaced => 4401,
            Self::Suspended | Self::EventClosed => 4403,
            Self::PolicyUnavailable => 4500,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionError {
    Revoked(RevokeReason),
    Binding,
    Clock,
}
/// Both deadlines come from the same verified admission credential. UTC conversion belongs
/// to the trusted adapter and must conservatively map onto the room's monotonic clock.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccessDeadline {
    pub monotonic_ms: u64,
    pub utc: DateTime<Utc>,
}
pub struct Session {
    binding: Binding,
    deadline: AccessDeadline,
    revoked: Option<RevokeReason>,
    last_now: u64,
}
impl Session {
    pub fn new(
        binding: Binding,
        deadline: AccessDeadline,
        now: u64,
        membership: &impl MembershipPort,
    ) -> Result<Self, SessionError> {
        if [
            binding.key.event_id.uuid(),
            binding.key.user_id.uuid(),
            binding.session_id,
            binding.connection_id,
            binding.owner_id,
            binding.room_id,
            binding.room_epoch,
            binding.stream_id,
        ]
        .iter()
        .any(|id| id.is_nil())
        {
            return Err(SessionError::Binding);
        }
        let mut session = Self {
            binding,
            deadline,
            revoked: None,
            last_now: now,
        };
        session.check(now, membership)?;
        Ok(session)
    }
    pub fn binding(&self) -> &Binding {
        &self.binding
    }
    pub fn deadline(&self) -> &AccessDeadline {
        &self.deadline
    }
    pub fn revoke(&mut self, reason: RevokeReason) {
        self.revoked.get_or_insert(reason);
    }
    pub fn check(
        &mut self,
        now: u64,
        membership: &impl MembershipPort,
    ) -> Result<(), SessionError> {
        if let Some(reason) = self.revoked {
            return Err(SessionError::Revoked(reason));
        }
        if now < self.last_now {
            self.revoke(RevokeReason::PolicyUnavailable);
            return Err(SessionError::Clock);
        }
        self.last_now = now;
        let reason = if self.deadline.monotonic_ms <= now {
            Some(RevokeReason::SessionExpired)
        } else {
            match membership.current(&self.binding, now) {
                Err(AuthorityError::Revoked) => Some(RevokeReason::SessionRevoked),
                Err(AuthorityError::Unavailable) => Some(RevokeReason::PolicyUnavailable),
                Ok(lease) if lease.binding != self.binding => {
                    Some(RevokeReason::ConnectionReplaced)
                }
                Ok(lease)
                    if lease.lease_until_ms <= now
                        || lease.lease_until_ms - now > AUTH_LEASE_MAX_MS
                        || lease.access_expires_at_ms != self.deadline.monotonic_ms =>
                {
                    Some(RevokeReason::PolicyUnavailable)
                }
                Ok(_) => None,
            }
        };
        if let Some(reason) = reason {
            self.revoke(reason);
            Err(SessionError::Revoked(reason))
        } else {
            Ok(())
        }
    }
}
