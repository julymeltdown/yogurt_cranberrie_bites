use crate::MAX_REVISION;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PartyError {
    #[error("Invalid party input")]
    InvalidInput,
    #[error("Resource unavailable")]
    Unavailable,
    #[error("Permission denied")]
    Denied,
    #[error("Revision changed")]
    VersionConflict,
    #[error("Invalid transition")]
    InvalidTransition,
    #[error("Party full")]
    Full,
    #[error("Duplicate application")]
    DuplicateApplication,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PartyState {
    Open,
    Closed,
    Cancelled,
    Completed,
}
impl PartyState {
    pub fn active(self) -> bool {
        matches!(self, Self::Open | Self::Closed)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ApplicationState {
    Pending,
    Approved,
    Rejected,
    Cancelled,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MemberState {
    Active,
    Removed,
    Left,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartyFields {
    pub title: String,
    pub description: String,
    pub ticket_section: Option<String>,
    pub capacity: i32,
    pub meeting_starts_at: DateTime<Utc>,
    pub meeting_ends_at: DateTime<Utc>,
    pub requirements: String,
    pub public_place_id: Uuid,
}
impl PartyFields {
    pub fn validate(&self, now: DateTime<Utc>) -> Result<(), PartyError> {
        if !text(&self.title, 1, 120)
            || !text(&self.description, 0, 2000)
            || self
                .ticket_section
                .as_ref()
                .is_some_and(|s| !text(s, 1, 120))
            || !text(&self.requirements, 0, 1000)
            || !(2..=50).contains(&self.capacity)
            || self.meeting_starts_at >= self.meeting_ends_at
            || self.meeting_ends_at <= now
        {
            return Err(PartyError::InvalidInput);
        }
        Ok(())
    }
}
pub fn text(s: &str, min: usize, max: usize) -> bool {
    let count = s.chars().count();
    count >= min
        && count <= max
        && (min == 0 || !s.trim().is_empty())
        && !s.chars().any(char::is_control)
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Application {
    pub id: Uuid,
    pub user: Uuid,
    pub note: String,
    pub state: ApplicationState,
    pub revision: i64,
    pub created_at: DateTime<Utc>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    pub user: Uuid,
    pub owner: bool,
    pub state: MemberState,
    pub revision: i64,
    pub joined_at: DateTime<Utc>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Party {
    pub id: Uuid,
    pub event: Uuid,
    pub owner: Uuid,
    pub fields: PartyFields,
    pub state: PartyState,
    pub revision: i64,
    pub meeting_revision: i64,
    /// Synchronous audience fence. Every private reader/subscriber must recheck it.
    pub audience_revision: i64,
    pub created_at: DateTime<Utc>,
    pub applications: Vec<Application>,
    pub members: Vec<Member>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PartyAction {
    Create(PartyFields),
    Update {
        expected: i64,
        fields: PartyFields,
        state: PartyState,
    },
    Cancel {
        expected: i64,
    },
    Transfer {
        expected: i64,
        user: Uuid,
    },
    Apply {
        party_revision: i64,
        note: String,
    },
    Decide {
        application: Uuid,
        expected: i64,
        party_revision: i64,
        approve: bool,
    },
    CancelApplication {
        application: Uuid,
        expected: i64,
    },
    Remove {
        user: Uuid,
        expected: i64,
        party_revision: i64,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PartyChange {
    pub resource: Uuid,
    pub revision: i64,
}
impl Party {
    pub fn create(
        id: Uuid,
        event: Uuid,
        owner: Uuid,
        fields: PartyFields,
        now: DateTime<Utc>,
    ) -> Result<Self, PartyError> {
        fields.validate(now)?;
        Ok(Self {
            id,
            event,
            owner,
            fields,
            state: PartyState::Open,
            revision: 1,
            meeting_revision: 1,
            audience_revision: 1,
            created_at: now,
            applications: vec![],
            members: vec![Member {
                user: owner,
                owner: true,
                state: MemberState::Active,
                revision: 1,
                joined_at: now,
            }],
        })
    }
    pub fn member_count(&self) -> i32 {
        self.members
            .iter()
            .filter(|m| m.state == MemberState::Active)
            .count() as i32
    }
    pub fn member_access(&self, user: Uuid) -> bool {
        self.state.active()
            && self
                .members
                .iter()
                .any(|m| m.user == user && m.state == MemberState::Active)
    }
    /// Every private application lookup separately checks applicant/owner and current shared policy.
    pub fn can_read_application(&self, actor: Uuid, id: Uuid) -> bool {
        self.applications
            .iter()
            .any(|a| a.id == id && (a.user == actor || self.owner == actor))
    }
    /// Clone/commit keeps errors side-effect free, including revision overflow.
    pub fn change(
        &mut self,
        actor: Uuid,
        action: &PartyAction,
        new_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<PartyChange, PartyError> {
        let mut next = self.clone();
        let change = next.change_inner(actor, action, new_id, now)?;
        next.validate()?;
        *self = next;
        Ok(change)
    }
    pub fn validate(&self) -> Result<(), PartyError> {
        if self.member_count() < 1
            || self.member_count() > self.fields.capacity
            || self
                .members
                .iter()
                .filter(|m| m.owner && m.state == MemberState::Active && m.user == self.owner)
                .count()
                != 1
            || self.members.iter().filter(|m| m.owner).count() != 1
            || !valid_revision(self.revision)
            || !valid_revision(self.meeting_revision)
            || !valid_revision(self.audience_revision)
            || self.members.iter().any(|m| !valid_revision(m.revision))
            || self
                .applications
                .iter()
                .any(|a| !valid_revision(a.revision))
        {
            return Err(PartyError::InvalidInput);
        }
        let mut ids = std::collections::HashSet::new();
        if self.members.iter().any(|m| !ids.insert(m.user)) {
            return Err(PartyError::InvalidInput);
        }
        ids.clear();
        if self.applications.iter().any(|a| !ids.insert(a.id)) {
            return Err(PartyError::InvalidInput);
        }
        let mut pending = std::collections::HashSet::new();
        if self
            .applications
            .iter()
            .filter(|a| a.state == ApplicationState::Pending)
            .any(|a| !pending.insert(a.user))
        {
            return Err(PartyError::DuplicateApplication);
        }
        Ok(())
    }
    fn owner_guard(&self, actor: Uuid, expected: i64) -> Result<(), PartyError> {
        if actor != self.owner {
            return Err(PartyError::Denied);
        }
        revision(self.revision, expected)?;
        if !self.state.active() {
            return Err(PartyError::InvalidTransition);
        }
        Ok(())
    }
    fn revoke(&mut self) -> Result<(), PartyError> {
        self.audience_revision = bump(self.audience_revision)?;
        Ok(())
    }
    fn terminal(&mut self) -> Result<(), PartyError> {
        for a in &mut self.applications {
            if a.state == ApplicationState::Pending {
                a.state = ApplicationState::Cancelled;
                a.revision = bump(a.revision)?;
            }
        }
        self.revoke()
    }
    fn change_inner(
        &mut self,
        actor: Uuid,
        action: &PartyAction,
        new_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<PartyChange, PartyError> {
        let mut result = self.id;
        let result_revision;
        match action {
            PartyAction::Create(_) => return Err(PartyError::InvalidTransition),
            PartyAction::Update {
                expected,
                fields,
                state,
            } => {
                self.owner_guard(actor, *expected)?;
                fields.validate(now)?;
                if fields.capacity < self.member_count() {
                    return Err(PartyError::Full);
                }
                if *state == PartyState::Cancelled {
                    return Err(PartyError::InvalidTransition);
                }
                if fields.public_place_id != self.fields.public_place_id
                    || fields.meeting_starts_at != self.fields.meeting_starts_at
                    || fields.meeting_ends_at != self.fields.meeting_ends_at
                {
                    self.meeting_revision = bump(self.meeting_revision)?;
                    self.revoke()?;
                }
                self.fields = fields.clone();
                self.state = *state;
                if !state.active() {
                    self.terminal()?;
                }
                self.revision = bump(self.revision)?;
                result_revision = self.revision;
            }
            PartyAction::Cancel { expected } => {
                self.owner_guard(actor, *expected)?;
                self.state = PartyState::Cancelled;
                self.terminal()?;
                self.revision = bump(self.revision)?;
                result_revision = self.revision;
            }
            PartyAction::Transfer { expected, user } => {
                self.owner_guard(actor, *expected)?;
                if *user == actor || !self.member_access(*user) {
                    return Err(PartyError::Denied);
                }
                for m in &mut self.members {
                    if m.user == actor || m.user == *user {
                        m.owner = m.user == *user;
                        m.revision = bump(m.revision)?;
                    }
                }
                self.owner = *user;
                self.revoke()?;
                self.revision = bump(self.revision)?;
                result_revision = self.revision;
            }
            PartyAction::Apply {
                party_revision,
                note,
            } => {
                revision(self.revision, *party_revision)?;
                if self.state != PartyState::Open || self.fields.meeting_ends_at <= now {
                    return Err(PartyError::InvalidTransition);
                }
                if self.owner == actor || self.member_access(actor) {
                    return Err(PartyError::Denied);
                }
                if self
                    .applications
                    .iter()
                    .any(|a| a.user == actor && a.state == ApplicationState::Pending)
                {
                    return Err(PartyError::DuplicateApplication);
                }
                if !text(note, 0, 500) {
                    return Err(PartyError::InvalidInput);
                }
                if self.applications.len() >= 1000 {
                    return Err(PartyError::InvalidInput);
                }
                self.applications.push(Application {
                    id: new_id,
                    user: actor,
                    note: note.clone(),
                    state: ApplicationState::Pending,
                    revision: 1,
                    created_at: now,
                });
                result = new_id;
                result_revision = 1;
            }
            PartyAction::Decide {
                application,
                expected,
                party_revision,
                approve,
            } => {
                self.owner_guard(actor, *party_revision)?;
                let index = self
                    .applications
                    .iter()
                    .position(|a| a.id == *application)
                    .ok_or(PartyError::Unavailable)?;
                let a = &self.applications[index];
                revision(a.revision, *expected)?;
                if a.state != ApplicationState::Pending {
                    return Err(PartyError::InvalidTransition);
                }
                if *approve {
                    if self.state != PartyState::Open || self.fields.meeting_ends_at <= now {
                        return Err(PartyError::InvalidTransition);
                    }
                    if self.member_count() >= self.fields.capacity {
                        return Err(PartyError::Full);
                    }
                    let user = a.user;
                    if let Some(m) = self.members.iter_mut().find(|m| m.user == user) {
                        if m.state == MemberState::Active {
                            return Err(PartyError::Denied);
                        }
                        m.state = MemberState::Active;
                        m.revision = bump(m.revision)?;
                        m.joined_at = now;
                    } else {
                        self.members.push(Member {
                            user,
                            owner: false,
                            state: MemberState::Active,
                            revision: 1,
                            joined_at: now,
                        });
                    }
                    self.revision = bump(self.revision)?;
                }
                let a = &mut self.applications[index];
                a.state = if *approve {
                    ApplicationState::Approved
                } else {
                    ApplicationState::Rejected
                };
                a.revision = bump(a.revision)?;
                result = a.id;
                result_revision = a.revision;
            }
            PartyAction::CancelApplication {
                application,
                expected,
            } => {
                let a = self
                    .applications
                    .iter_mut()
                    .find(|a| a.id == *application && a.user == actor)
                    .ok_or(PartyError::Denied)?;
                revision(a.revision, *expected)?;
                if a.state != ApplicationState::Pending {
                    return Err(PartyError::InvalidTransition);
                }
                a.state = ApplicationState::Cancelled;
                a.revision = bump(a.revision)?;
                result = a.id;
                result_revision = a.revision;
            }
            PartyAction::Remove {
                user,
                expected,
                party_revision,
            } => {
                if actor != self.owner && actor != *user {
                    return Err(PartyError::Denied);
                }
                revision(self.revision, *party_revision)?;
                if !self.state.active() || *user == self.owner {
                    return Err(PartyError::InvalidTransition);
                }
                let m = self
                    .members
                    .iter_mut()
                    .find(|m| m.user == *user)
                    .ok_or(PartyError::Unavailable)?;
                revision(m.revision, *expected)?;
                if m.state != MemberState::Active {
                    return Err(PartyError::InvalidTransition);
                }
                m.state = if actor == *user {
                    MemberState::Left
                } else {
                    MemberState::Removed
                };
                m.revision = bump(m.revision)?;
                result = m.user;
                result_revision = m.revision;
                self.revoke()?;
                self.revision = bump(self.revision)?;
            }
        }
        Ok(PartyChange {
            resource: result,
            revision: result_revision,
        })
    }
}
pub fn valid_revision(v: i64) -> bool {
    (1..=MAX_REVISION).contains(&v)
}
pub fn bump(v: i64) -> Result<i64, PartyError> {
    if v >= MAX_REVISION || v < 1 {
        Err(PartyError::InvalidInput)
    } else {
        Ok(v + 1)
    }
}
pub fn revision(actual: i64, expected: i64) -> Result<(), PartyError> {
    if valid_revision(expected) && actual == expected {
        Ok(())
    } else {
        Err(PartyError::VersionConflict)
    }
}
