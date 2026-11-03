//! One-unit reservations. Time comes from the repository's PostgreSQL clock.
use super::party::{text, valid_revision};
use crate::MAX_REVISION;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum GiftError {
    #[error("Invalid giveaway input")]
    InvalidInput,
    #[error("Resource unavailable")]
    Unavailable,
    #[error("Permission denied")]
    Denied,
    #[error("Revision changed")]
    VersionConflict,
    #[error("Invalid transition")]
    InvalidTransition,
    #[error("Giveaway sold out")]
    SoldOut,
    #[error("Duplicate active reservation")]
    DuplicateReservation,
    #[error("Reservation expired")]
    Expired,
    #[error("Pickup code invalid")]
    CodeInvalid,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GiftState {
    Draft,
    Open,
    Paused,
    Closed,
    Cancelled,
}
impl GiftState {
    pub fn active(self) -> bool {
        matches!(self, Self::Open | Self::Paused)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReservationState {
    Held,
    Ready,
    Claimed,
    Cancelled,
    Expired,
}
impl ReservationState {
    pub fn active(self) -> bool {
        matches!(self, Self::Held | Self::Ready)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GiftFields {
    pub title: String,
    pub description: String,
    pub image_media_id: Option<Uuid>,
    pub quantity_total: i32,
    pub pickup_starts_at: DateTime<Utc>,
    pub pickup_ends_at: DateTime<Utc>,
    pub public_place_id: Uuid,
}
impl GiftFields {
    pub fn validate(&self, now: DateTime<Utc>) -> Result<(), GiftError> {
        if !text(&self.title, 1, 120)
            || !text(&self.description, 0, 2000)
            || !(1..=10000).contains(&self.quantity_total)
            || self.pickup_starts_at >= self.pickup_ends_at
            || self.pickup_ends_at <= now
        {
            return Err(GiftError::InvalidInput);
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PickupMeeting {
    pub place_id: Uuid,
    pub map_revision: i64,
    pub place_revision: i64,
    pub instructions: String,
    pub identification: String,
    pub expires_at: DateTime<Utc>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reservation {
    pub id: Uuid,
    pub recipient: Uuid,
    pub state: ReservationState,
    pub revision: i64,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub claim_receipt_id: Option<Uuid>,
    pub claimed_at: Option<DateTime<Utc>>,
    // The keyed hash is stored separately by the service; never serialize plaintext in aggregate.
    pub code_expires_at: Option<DateTime<Utc>>,
    pub code_generation: i64,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Giveaway {
    pub id: Uuid,
    pub event: Uuid,
    pub owner: Uuid,
    pub fields: GiftFields,
    pub state: GiftState,
    pub revision: i64,
    pub pickup_grace_seconds: i32,
    pub reserved_unclaimed: i32,
    pub quantity_claimed: i32,
    pub meeting: Option<PickupMeeting>,
    pub meeting_revision: i64,
    pub created_at: DateTime<Utc>,
    pub reservations: Vec<Reservation>,
}
#[derive(Clone, PartialEq, Eq)]
pub enum GiftAction {
    Create(GiftFields),
    Update {
        expected: i64,
        fields: GiftFields,
        state: GiftState,
    },
    SetMeeting {
        expected: i64,
        meeting: PickupMeeting,
        physical_acknowledged: bool,
    },
    Reserve {
        giveaway_revision: i64,
    },
    Cancel {
        reservation: Uuid,
        expected: i64,
    },
    IssueCode {
        reservation: Uuid,
        expected: i64,
    },
    Claim {
        reservation: Uuid,
        expected: i64,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GiftChange {
    pub resource: Uuid,
    pub revision: i64,
}
impl Giveaway {
    pub fn create(
        id: Uuid,
        event: Uuid,
        owner: Uuid,
        fields: GiftFields,
        grace: i32,
        now: DateTime<Utc>,
    ) -> Result<Self, GiftError> {
        fields.validate(now)?;
        if !(0..=86400).contains(&grace) {
            return Err(GiftError::InvalidInput);
        }
        Ok(Self {
            id,
            event,
            owner,
            fields,
            state: GiftState::Draft,
            revision: 1,
            pickup_grace_seconds: grace,
            reserved_unclaimed: 0,
            quantity_claimed: 0,
            meeting: None,
            meeting_revision: 1,
            created_at: now,
            reservations: vec![],
        })
    }
    pub fn available(&self) -> i32 {
        self.fields.quantity_total - self.reserved_unclaimed - self.quantity_claimed
    }
    pub fn expires_at(&self) -> DateTime<Utc> {
        self.fields.pickup_ends_at + Duration::seconds(self.pickup_grace_seconds.into())
    }
    pub fn validate(&self) -> Result<(), GiftError> {
        if self.available() < 0
            || self.reserved_unclaimed < 0
            || self.quantity_claimed < 0
            || !valid_revision(self.revision)
            || !valid_revision(self.meeting_revision)
            || !(0..=86400).contains(&self.pickup_grace_seconds)
        {
            return Err(GiftError::InvalidInput);
        }
        if self.reserved_unclaimed
            != self
                .reservations
                .iter()
                .filter(|r| r.state.active())
                .count() as i32
            || self.quantity_claimed
                != self
                    .reservations
                    .iter()
                    .filter(|r| r.state == ReservationState::Claimed)
                    .count() as i32
        {
            return Err(GiftError::InvalidInput);
        }
        let mut ids = std::collections::HashSet::new();
        let mut active = std::collections::HashSet::new();
        for r in &self.reservations {
            if !ids.insert(r.id)
                || !valid_revision(r.revision)
                || (r.state.active() && !active.insert(r.recipient))
                || ((r.state == ReservationState::Claimed)
                    != (r.claim_receipt_id.is_some() && r.claimed_at.is_some()))
                || (!r.state.active() && r.code_expires_at.is_some())
            {
                return Err(GiftError::InvalidInput);
            }
        }
        Ok(())
    }
    pub fn change(
        &mut self,
        actor: Uuid,
        action: &GiftAction,
        new_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<GiftChange, GiftError> {
        let mut next = self.clone();
        let change = next.inner(actor, action, new_id, now)?;
        next.validate()?;
        *self = next;
        Ok(change)
    }
    fn owner_guard(&self, actor: Uuid, expected: i64) -> Result<(), GiftError> {
        if actor != self.owner {
            return Err(GiftError::Denied);
        }
        revision(self.revision, expected)?;
        if matches!(self.state, GiftState::Closed | GiftState::Cancelled) {
            return Err(GiftError::InvalidTransition);
        }
        Ok(())
    }
    fn meeting_valid(&self, now: DateTime<Utc>) -> bool {
        self.meeting.as_ref().is_some_and(|m| {
            m.place_id == self.fields.public_place_id
                && m.expires_at >= self.expires_at()
                && m.expires_at > now
        })
    }
    fn inner(
        &mut self,
        actor: Uuid,
        action: &GiftAction,
        new_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<GiftChange, GiftError> {
        match action {
            GiftAction::Create(_) => Err(GiftError::InvalidTransition),
            GiftAction::Update {
                expected,
                fields,
                state,
            } => {
                self.owner_guard(actor, *expected)?;
                fields.validate(
                    if matches!(state, GiftState::Cancelled | GiftState::Closed) {
                        std::cmp::min(now, fields.pickup_ends_at - Duration::nanoseconds(1))
                    } else {
                        now
                    },
                )?;
                if fields.quantity_total < self.reserved_unclaimed + self.quantity_claimed {
                    return Err(GiftError::InvalidInput);
                }
                if self.reserved_unclaimed > 0
                    && (fields.pickup_starts_at != self.fields.pickup_starts_at
                        || fields.pickup_ends_at != self.fields.pickup_ends_at
                        || fields.public_place_id != self.fields.public_place_id)
                {
                    return Err(GiftError::InvalidTransition);
                }
                let allowed = self.state == *state
                    || match self.state {
                        GiftState::Draft => matches!(state, GiftState::Open | GiftState::Cancelled),
                        GiftState::Open => matches!(
                            state,
                            GiftState::Paused | GiftState::Closed | GiftState::Cancelled
                        ),
                        GiftState::Paused => matches!(
                            state,
                            GiftState::Open | GiftState::Closed | GiftState::Cancelled
                        ),
                        _ => false,
                    };
                if !allowed || (*state == GiftState::Closed && self.reserved_unclaimed > 0) {
                    return Err(GiftError::InvalidTransition);
                }
                self.fields = fields.clone();
                if *state == GiftState::Open && !self.meeting_valid(now) {
                    return Err(GiftError::InvalidTransition);
                }
                if *state == GiftState::Cancelled {
                    for r in &mut self.reservations {
                        if r.state.active() {
                            r.state = ReservationState::Cancelled;
                            r.code_expires_at = None;
                            r.revision = bump(r.revision)?;
                        }
                    }
                    self.reserved_unclaimed = 0;
                    self.meeting = None;
                    self.meeting_revision = bump(self.meeting_revision)?;
                }
                self.state = *state;
                self.revision = bump(self.revision)?;
                Ok(GiftChange {
                    resource: self.id,
                    revision: self.revision,
                })
            }
            GiftAction::SetMeeting {
                expected,
                meeting,
                physical_acknowledged,
            } => {
                self.owner_guard(actor, *expected)?;
                if !physical_acknowledged
                    || meeting.place_id != self.fields.public_place_id
                    || !valid_revision(meeting.map_revision)
                    || !valid_revision(meeting.place_revision)
                    || !text(&meeting.instructions, 1, 1000)
                    || !text(&meeting.identification, 1, 240)
                    || meeting.expires_at < self.expires_at()
                    || meeting.expires_at <= now
                {
                    return Err(GiftError::InvalidInput);
                }
                self.meeting = Some(meeting.clone());
                self.meeting_revision = bump(self.meeting_revision)?;
                self.revision = bump(self.revision)?;
                Ok(GiftChange {
                    resource: self.id,
                    revision: self.revision,
                })
            }
            GiftAction::Reserve { giveaway_revision } => {
                revision(self.revision, *giveaway_revision)?;
                if actor == self.owner {
                    return Err(GiftError::Denied);
                }
                if self.state != GiftState::Open || !self.meeting_valid(now) {
                    return Err(GiftError::InvalidTransition);
                }
                if now >= self.fields.pickup_ends_at {
                    return Err(GiftError::Expired);
                }
                if self
                    .reservations
                    .iter()
                    .any(|r| r.recipient == actor && r.state.active())
                {
                    return Err(GiftError::DuplicateReservation);
                }
                if self.available() <= 0 {
                    return Err(GiftError::SoldOut);
                }
                if self.reservations.len() >= 10000 {
                    return Err(GiftError::InvalidInput);
                }
                self.reservations.push(Reservation {
                    id: new_id,
                    recipient: actor,
                    state: if now >= self.fields.pickup_starts_at {
                        ReservationState::Ready
                    } else {
                        ReservationState::Held
                    },
                    revision: 1,
                    expires_at: self.expires_at(),
                    created_at: now,
                    claim_receipt_id: None,
                    claimed_at: None,
                    code_expires_at: None,
                    code_generation: 0,
                });
                self.reserved_unclaimed += 1;
                self.revision = bump(self.revision)?;
                Ok(GiftChange {
                    resource: new_id,
                    revision: 1,
                })
            }
            GiftAction::Cancel {
                reservation,
                expected,
            } => {
                let r = self
                    .reservations
                    .iter_mut()
                    .find(|r| r.id == *reservation)
                    .ok_or(GiftError::Unavailable)?;
                if actor != self.owner && actor != r.recipient {
                    return Err(GiftError::Denied);
                }
                revision(r.revision, *expected)?;
                if !r.state.active() {
                    return Err(GiftError::InvalidTransition);
                }
                r.state = if now >= r.expires_at {
                    ReservationState::Expired
                } else {
                    ReservationState::Cancelled
                };
                r.code_expires_at = None;
                r.revision = bump(r.revision)?;
                self.reserved_unclaimed -= 1;
                self.revision = bump(self.revision)?;
                Ok(GiftChange {
                    resource: r.id,
                    revision: r.revision,
                })
            }
            GiftAction::IssueCode {
                reservation,
                expected,
            } => {
                if !self.state.active() || !self.meeting_valid(now) {
                    return Err(GiftError::InvalidTransition);
                }
                let r = self
                    .reservations
                    .iter_mut()
                    .find(|r| r.id == *reservation)
                    .ok_or(GiftError::Unavailable)?;
                if actor != r.recipient {
                    return Err(GiftError::Denied);
                }
                revision(r.revision, *expected)?;
                if now >= r.expires_at {
                    return Err(GiftError::Expired);
                }
                if r.state != ReservationState::Ready {
                    return Err(GiftError::InvalidTransition);
                }
                r.code_expires_at = Some(
                    DateTime::from_timestamp_micros(
                        std::cmp::min(now + Duration::seconds(300), r.expires_at)
                            .timestamp_micros(),
                    )
                    .ok_or(GiftError::InvalidInput)?,
                );
                r.code_generation = r
                    .code_generation
                    .checked_add(1)
                    .ok_or(GiftError::InvalidInput)?;
                r.revision = bump(r.revision)?;
                Ok(GiftChange {
                    resource: r.id,
                    revision: r.revision,
                })
            }
            GiftAction::Claim {
                reservation,
                expected,
            } => {
                if actor != self.owner {
                    return Err(GiftError::Denied);
                }
                let r = self
                    .reservations
                    .iter_mut()
                    .find(|r| r.id == *reservation)
                    .ok_or(GiftError::Unavailable)?;
                if r.state == ReservationState::Claimed {
                    return Ok(GiftChange {
                        resource: r.id,
                        revision: r.revision,
                    });
                }
                revision(r.revision, *expected)?;
                if now >= r.expires_at {
                    return Err(GiftError::Expired);
                }
                if !self.state.active() || r.state != ReservationState::Ready {
                    return Err(GiftError::InvalidTransition);
                }
                if !r.code_expires_at.is_some_and(|expiry| now < expiry) {
                    return Err(GiftError::CodeInvalid);
                }
                // Service must verify constant-time keyed code and shared attempt budget before this transition.
                r.state = ReservationState::Claimed;
                r.claim_receipt_id = Some(new_id);
                r.claimed_at = Some(now);
                r.code_expires_at = None;
                r.revision = bump(r.revision)?;
                self.reserved_unclaimed -= 1;
                self.quantity_claimed += 1;
                self.revision = bump(self.revision)?;
                Ok(GiftChange {
                    resource: r.id,
                    revision: r.revision,
                })
            }
        }
    }
    /// Bounded root sweep; caller locks root and persists changes/outbox in one transaction.
    pub fn advance_time(&mut self, now: DateTime<Utc>, limit: usize) -> Result<usize, GiftError> {
        if limit == 0 || limit > 100 {
            return Err(GiftError::InvalidInput);
        }
        let mut next = self.clone();
        let mut count = 0;
        for r in &mut next.reservations {
            if count >= limit {
                break;
            }
            if !r.state.active() {
                continue;
            }
            if now >= r.expires_at {
                r.state = ReservationState::Expired;
                r.code_expires_at = None;
                r.revision = bump(r.revision)?;
                next.reserved_unclaimed -= 1;
                count += 1;
            } else if r.state == ReservationState::Held && now >= next.fields.pickup_starts_at {
                r.state = ReservationState::Ready;
                r.revision = bump(r.revision)?;
                count += 1;
            }
        }
        if count > 0 {
            next.revision = bump(next.revision)?;
        }
        next.validate()?;
        *self = next;
        Ok(count)
    }
}
fn bump(v: i64) -> Result<i64, GiftError> {
    if !(1..MAX_REVISION).contains(&v) {
        Err(GiftError::InvalidInput)
    } else {
        Ok(v + 1)
    }
}
fn revision(actual: i64, expected: i64) -> Result<(), GiftError> {
    if valid_revision(expected) && actual == expected {
        Ok(())
    } else {
        Err(GiftError::VersionConflict)
    }
}
