//! VENU-BE-L01 consent values. Server time is supplied by the adapter, never a client.
use crate::{AuthUserId, EventId, EVENT_TIMEZONE, MAX_REVISION};
use chrono::{DateTime, Duration, Utc};
use std::collections::BTreeSet;
use uuid::Uuid;

pub const MAX_VIEWERS: usize = 50;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum VenueLocationError {
    #[error("invalid location input")]
    InvalidInput,
    #[error("location disclosure is denied")]
    Denied,
    #[error("location resource is unavailable")]
    NotFound,
    #[error("location revision changed")]
    Conflict,
    #[error("venue location policy is unavailable")]
    PolicyUnavailable,
    #[error("location authority is unavailable")]
    DependencyUnavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SharingState {
    Off,
    Active,
    Paused,
    Revoked,
    Expired,
}
impl SharingState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "OFF",
            Self::Active => "ACTIVE",
            Self::Paused => "PAUSED",
            Self::Revoked => "REVOKED",
            Self::Expired => "EXPIRED",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SharingRecord {
    pub state: SharingState,
    pub revision: i64,
    pub viewers: Vec<Uuid>,
    pub expires_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}
impl SharingRecord {
    pub fn off() -> Self {
        Self {
            state: SharingState::Off,
            revision: 0,
            viewers: Vec::new(),
            expires_at: None,
            updated_at: None,
        }
    }
    pub fn effective(&self, now: DateTime<Utc>) -> SharingState {
        match (self.state, self.expires_at) {
            (SharingState::Active, Some(expires)) if now >= expires => SharingState::Expired,
            (SharingState::Active, Some(_)) => SharingState::Active,
            (SharingState::Active, None) => SharingState::Expired,
            (state, _) => state,
        }
    }
    pub fn current_viewers(&self, now: DateTime<Utc>) -> &[Uuid] {
        if self.effective(now) == SharingState::Active {
            &self.viewers
        } else {
            &[]
        }
    }
    pub fn projection(&self, now: DateTime<Utc>) -> SharingProjection {
        let state = self.effective(now);
        SharingProjection {
            state,
            revision: self.revision,
            viewer_ids: if state == SharingState::Active {
                self.viewers.clone()
            } else {
                Vec::new()
            },
            expires_at: if state == SharingState::Active {
                self.expires_at
            } else {
                None
            },
            updated_at: self.updated_at,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SharingProjection {
    pub state: SharingState,
    pub revision: i64,
    pub viewer_ids: Vec<Uuid>,
    pub expires_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SharingCommand {
    Active {
        expected_revision: i64,
        viewer_ids: Vec<Uuid>,
    },
    Paused {
        expected_revision: i64,
        viewer_ids: Vec<Uuid>,
    },
    Revoked {
        expected_revision: i64,
        viewer_ids: Vec<Uuid>,
    },
}
impl SharingCommand {
    pub fn expected_revision(&self) -> i64 {
        match self {
            Self::Active {
                expected_revision, ..
            }
            | Self::Paused {
                expected_revision, ..
            }
            | Self::Revoked {
                expected_revision, ..
            } => *expected_revision,
        }
    }
    pub fn state(&self) -> SharingState {
        match self {
            Self::Active { .. } => SharingState::Active,
            Self::Paused { .. } => SharingState::Paused,
            Self::Revoked { .. } => SharingState::Revoked,
        }
    }
    pub fn viewer_ids(&self) -> &[Uuid] {
        match self {
            Self::Active { viewer_ids, .. }
            | Self::Paused { viewer_ids, .. }
            | Self::Revoked { viewer_ids, .. } => viewer_ids,
        }
    }
    pub fn validate(&self, policy: &VenueLocationPolicy) -> Result<(), VenueLocationError> {
        if !(0..=MAX_REVISION).contains(&self.expected_revision()) {
            return Err(VenueLocationError::InvalidInput);
        }
        let viewers = self.viewer_ids();
        if viewers.iter().any(|id| id.is_nil()) {
            return Err(VenueLocationError::InvalidInput);
        }
        match self {
            Self::Active { .. } => {
                let unique: BTreeSet<_> = viewers.iter().collect();
                if viewers.is_empty()
                    || unique.len() != viewers.len()
                    || viewers.len() > policy.max_viewers
                    || viewers.len() > MAX_VIEWERS
                {
                    return Err(VenueLocationError::InvalidInput);
                }
            }
            Self::Paused { .. } | Self::Revoked { .. } if !viewers.is_empty() => {
                return Err(VenueLocationError::InvalidInput);
            }
            Self::Paused { .. } | Self::Revoked { .. } => {}
        }
        Ok(())
    }
    pub fn lease_expiry(
        &self,
        now: DateTime<Utc>,
        policy: &VenueLocationPolicy,
        access_deadline: DateTime<Utc>,
    ) -> Result<Option<DateTime<Utc>>, VenueLocationError> {
        if self.state() != SharingState::Active {
            return Ok(None);
        }
        let lease = now
            .checked_add_signed(Duration::milliseconds(policy.sharing_lease_ms))
            .ok_or(VenueLocationError::PolicyUnavailable)?;
        Ok(Some(lease.min(policy.valid_until).min(access_deadline)))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct VenueLocationPolicy {
    pub event_id: EventId,
    pub venue_key: String,
    pub revision: i64,
    pub timezone: String,
    pub valid_until: DateTime<Utc>,
    pub fresh_ms: i64,
    pub last_known_ms: i64,
    pub fix_max_age_ms: i64,
    pub future_skew_ms: i64,
    pub sharing_lease_ms: i64,
    pub request_ttl_ms: i64,
    pub check_in_ttl_ms: i64,
    pub coordinate_retention_ms: i64,
    pub command_retention_ms: i64,
    pub max_accuracy_m: f64,
    pub max_viewers: usize,
}
impl VenueLocationPolicy {
    pub fn validate(&self, now: DateTime<Utc>) -> Result<(), VenueLocationError> {
        let positives = [
            self.fresh_ms,
            self.last_known_ms,
            self.fix_max_age_ms,
            self.sharing_lease_ms,
            self.request_ttl_ms,
            self.check_in_ttl_ms,
            self.coordinate_retention_ms,
            self.command_retention_ms,
        ];
        let valid = !self.venue_key.is_empty()
            && self.venue_key.len() <= 128
            && self.timezone == EVENT_TIMEZONE
            && self.valid_until > now
            && (1..=MAX_REVISION).contains(&self.revision)
            && positives
                .iter()
                .all(|value| (1..=MAX_REVISION).contains(value))
            && self.fresh_ms <= self.last_known_ms
            && self.last_known_ms <= self.coordinate_retention_ms
            && self.fix_max_age_ms <= self.last_known_ms
            && self.future_skew_ms >= 0
            && self.future_skew_ms <= MAX_REVISION
            && self.max_accuracy_m.is_finite()
            && self.max_accuracy_m > 0.0
            && (1..=MAX_VIEWERS).contains(&self.max_viewers);
        if valid {
            Ok(())
        } else {
            Err(VenueLocationError::PolicyUnavailable)
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PhoneFixInput {
    pub expected_sharing_revision: i64,
    pub latitude: f64,
    pub longitude: f64,
    pub accuracy_m: f64,
    pub captured_at: DateTime<Utc>,
}
impl PhoneFixInput {
    pub fn validate(
        &self,
        now: DateTime<Utc>,
        policy: &VenueLocationPolicy,
    ) -> Result<(), VenueLocationError> {
        let oldest = now
            .checked_sub_signed(Duration::milliseconds(policy.fix_max_age_ms))
            .ok_or(VenueLocationError::PolicyUnavailable)?;
        let newest = now
            .checked_add_signed(Duration::milliseconds(policy.future_skew_ms))
            .ok_or(VenueLocationError::PolicyUnavailable)?;
        let valid = (1..=MAX_REVISION).contains(&self.expected_sharing_revision)
            && self.latitude.is_finite()
            && (-90.0..=90.0).contains(&self.latitude)
            && self.longitude.is_finite()
            && (-180.0..=180.0).contains(&self.longitude)
            && self.accuracy_m.is_finite()
            && self.accuracy_m >= 0.0
            && self.accuracy_m <= policy.max_accuracy_m
            && self.captured_at >= oldest
            && self.captured_at <= newest;
        if valid {
            Ok(())
        } else {
            Err(VenueLocationError::InvalidInput)
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PhoneFix {
    pub latitude: f64,
    pub longitude: f64,
    pub accuracy_m: f64,
    pub captured_at: DateTime<Utc>,
    pub received_at: DateTime<Utc>,
    pub sharing_revision: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PhonePosition {
    pub latitude: f64,
    pub longitude: f64,
    pub accuracy_m: f64,
    pub captured_at: DateTime<Utc>,
    pub received_at: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VisibleSource {
    Phone,
    LastKnown,
}
impl VisibleSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Phone => "PHONE",
            Self::LastKnown => "LAST_KNOWN",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct VisibleLocation {
    pub user_id: Uuid,
    pub source: VisibleSource,
    pub position: PhonePosition,
    pub observed_at: DateTime<Utc>,
    pub fresh_until: Option<DateTime<Utc>>,
    pub expires_at: DateTime<Utc>,
    pub access_until: DateTime<Utc>,
    pub visibility_revision: i64,
    pub physical_arrival_verified: bool,
    pub can_request_location: bool,
}

pub fn project_phone(
    now: DateTime<Utc>,
    owner: Uuid,
    fix: &PhoneFix,
    visibility_revision: i64,
    lease_expiry: DateTime<Utc>,
    policy: &VenueLocationPolicy,
) -> Option<VisibleLocation> {
    if now >= lease_expiry || fix.sharing_revision != visibility_revision {
        return None;
    }
    let fresh_until = fix
        .captured_at
        .checked_add_signed(Duration::milliseconds(policy.fresh_ms))?;
    let last_known_until = fix
        .captured_at
        .checked_add_signed(Duration::milliseconds(policy.last_known_ms))?;
    let retention_until = fix
        .captured_at
        .checked_add_signed(Duration::milliseconds(policy.coordinate_retention_ms))?;
    if fix.captured_at > now || now >= retention_until {
        return None;
    }
    let source = if now <= fresh_until {
        VisibleSource::Phone
    } else if now <= last_known_until {
        VisibleSource::LastKnown
    } else {
        return None;
    };
    Some(VisibleLocation {
        user_id: owner,
        source,
        position: PhonePosition {
            latitude: fix.latitude,
            longitude: fix.longitude,
            accuracy_m: fix.accuracy_m,
            captured_at: fix.captured_at,
            received_at: fix.received_at,
        },
        observed_at: fix.captured_at,
        fresh_until: (source == VisibleSource::Phone).then_some(fresh_until),
        expires_at: lease_expiry,
        access_until: lease_expiry.min(policy.valid_until).min(retention_until),
        visibility_revision,
        physical_arrival_verified: false,
        can_request_location: true,
    })
}

#[derive(Clone, Debug, PartialEq)]
pub struct VenueClock {
    pub event_id: Uuid,
    pub venue_key: String,
    pub policy_revision: i64,
    pub timezone: String,
    pub server_time: DateTime<Utc>,
    pub end_of_day: DateTime<Utc>,
    pub valid_until: DateTime<Utc>,
}

pub fn seoul_end_of_day(now: DateTime<Utc>) -> DateTime<Utc> {
    let offset = Duration::hours(9);
    let local = now + offset;
    let next = local
        .date_naive()
        .succ_opt()
        .unwrap_or(local.date_naive())
        .and_hms_opt(0, 0, 0)
        .unwrap_or(local.naive_utc());
    DateTime::<Utc>::from_naive_utc_and_offset(next, Utc) - offset
}

#[derive(Clone, Debug, PartialEq)]
pub struct LocationCapabilities {
    pub can_share_phone: bool,
    pub can_pause: bool,
    pub can_revoke: bool,
    pub check_in_landmark_ids: Vec<String>,
    pub geographic_projection: String,
}
impl Default for LocationCapabilities {
    fn default() -> Self {
        Self {
            can_share_phone: false,
            can_pause: false,
            can_revoke: false,
            check_in_landmark_ids: Vec::new(),
            geographic_projection: "UNAVAILABLE".to_owned(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LocationSnapshot {
    pub event_id: Uuid,
    pub actor_id: Uuid,
    pub server_time: DateTime<Utc>,
    pub access_until: DateTime<Utc>,
    pub policy: VenueLocationPolicy,
    pub clock: VenueClock,
    pub sharing: SharingProjection,
    pub self_location: Option<VisibleLocation>,
    pub friends: Vec<VisibleLocation>,
    pub capabilities: LocationCapabilities,
    pub incoming_requests: Vec<serde_json::Value>,
    pub outgoing_requests: Vec<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CommandReceipt {
    pub command_key: Uuid,
    pub operation: &'static str,
    pub resource_kind: &'static str,
    pub resource_id: Uuid,
    pub revision: i64,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

pub fn receipt(
    command_key: Uuid,
    operation: &'static str,
    resource_kind: &'static str,
    owner: AuthUserId,
    revision: i64,
    now: DateTime<Utc>,
    policy: &VenueLocationPolicy,
) -> Result<CommandReceipt, VenueLocationError> {
    if command_key.is_nil() || !(1..=MAX_REVISION).contains(&revision) {
        return Err(VenueLocationError::InvalidInput);
    }
    let expires_at = now
        .checked_add_signed(Duration::milliseconds(policy.command_retention_ms))
        .ok_or(VenueLocationError::PolicyUnavailable)?
        .min(policy.valid_until);
    Ok(CommandReceipt {
        command_key,
        operation,
        resource_kind,
        resource_id: owner.uuid(),
        revision,
        created_at: now,
        expires_at,
    })
}
