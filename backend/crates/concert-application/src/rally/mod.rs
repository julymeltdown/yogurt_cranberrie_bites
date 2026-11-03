//! Fresh party meeting/membership authority. No policy is inferred from user input.
use crate::beacon::MeetingActor;
use chrono::{DateTime, Utc};
use uuid::Uuid;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RallyQuery {
    pub actor: MeetingActor,
    pub party: Uuid,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RallyMember {
    pub user: Uuid,
    pub membership_revision: i64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartyMeeting {
    pub query: RallyQuery,
    pub owner: Uuid,
    pub meeting_revision: i64,
    pub audience_revision: i64,
    pub public_place_id: Uuid,
    pub ends_at: DateTime<Utc>,
    pub members: Vec<RallyMember>,
}
impl PartyMeeting {
    pub fn valid_for(&self, q: RallyQuery) -> bool {
        self.query == q
            && !q.party.is_nil()
            && !self.public_place_id.is_nil()
            && (1..=concert_domain::MAX_REVISION).contains(&self.meeting_revision)
            && (1..=concert_domain::MAX_REVISION).contains(&self.audience_revision)
            && !self.members.is_empty()
            && self.members.len() <= 50
            && self.members.iter().all(|m| {
                !m.user.is_nil()
                    && (1..=concert_domain::MAX_REVISION).contains(&m.membership_revision)
            })
            && self
                .members
                .iter()
                .map(|m| m.user)
                .collect::<std::collections::HashSet<_>>()
                .len()
                == self.members.len()
            && self.members.iter().any(|m| m.user == q.actor.actor)
            && self.members.iter().any(|m| m.user == self.owner)
    }
}
