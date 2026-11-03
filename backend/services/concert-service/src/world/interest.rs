//! Recipient projection under the serialized room owner. No cached permissive policy.
use super::simulation::{
    Binding, MembershipPort, PlayerState, Simulation, AUTH_LEASE_MAX_MS, ROOM_CAPACITY,
};
use concert_domain::{world::MAX_SEQUENCE, AvatarSelection, Revision};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectionError {
    InvalidFrame,
    PolicyUnavailable,
    Authority,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Facing {
    Down,
    Left,
    Right,
    Up,
}
impl Facing {
    fn wire(self) -> &'static str {
        match self {
            Self::Down => "down",
            Self::Left => "left",
            Self::Right => "right",
            Self::Up => "up",
        }
    }
}
/// Only public identity fields; selection must already pass BE07's reviewed catalog policy.
#[derive(Clone, Debug, PartialEq)]
pub struct PublicIdentity {
    pub display_name: String,
    pub avatar_revision: Revision,
    pub avatar: AvatarSelection,
    pub facing: Facing,
    pub moving: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectedPlayer {
    pub(crate) identity: PublicIdentity,
    pub(crate) state: PlayerState,
}
impl ProjectedPlayer {
    pub fn wire(&self, user: Uuid) -> Value {
        let a = &self.identity.avatar;
        json!({"userId":user,"displayName":self.identity.display_name,
            "position":{"x":self.state.position.x,"y":self.state.position.y},
            "facing":self.identity.facing.wire(),"moving":self.identity.moving,
            "avatar":{"revision":self.identity.avatar_revision.get(),"config":{
                "contract_version":"1.0.0","kind":"avatar_config",
                "manifest_id":a.manifest_id.as_str(),"manifest_version":a.manifest_version.get(),
                "item_ids":{"body":a.body.as_str(),"eyes":a.eyes.as_str(),"hair":a.hair.as_str(),
                "outfit":a.outfit.as_str(),"accessory":a.accessory.as_ref().map(|x|x.as_str())},
                "skin_tone_id":a.skin_tone.as_str()}}})
    }
}
/// Immutable snapshot of a single serialized BE12 advance. This is not a public roster.
pub struct RoomFrame {
    pub(crate) scope: Binding,
    pub(crate) tick: u64,
    players: BTreeMap<Uuid, (Binding, ProjectedPlayer)>,
}
impl RoomFrame {
    /// Roster must be captured under the same owner lock as simulation/admission and policy.
    /// No client coordinates, avatar JSON, or private profile DTOs enter the projection.
    pub fn capture(
        simulation: &Simulation,
        scope: Binding,
        tick: u64,
        roster: impl IntoIterator<Item = (Binding, PublicIdentity)>,
    ) -> Result<Self, ProjectionError> {
        let mut players = BTreeMap::new();
        for (b, identity) in roster {
            if players.len() >= ROOM_CAPACITY
                || !same_room(&scope, &b)
                || identity.display_name.trim().is_empty()
                || identity.display_name.chars().count() > 40
                || identity.display_name.chars().any(char::is_control)
                || identity.avatar.manifest_id != scope.resources.avatar_manifest_id
                || identity.avatar.manifest_version != scope.resources.avatar_manifest_version
            {
                return Err(ProjectionError::InvalidFrame);
            }
            let user = b.key.user_id.uuid();
            let state = simulation
                .state(user)
                .ok_or(ProjectionError::InvalidFrame)?;
            if !state.position.x.is_finite()
                || !state.position.y.is_finite()
                || !(0.0..=1048576.0).contains(&state.position.x)
                || !(0.0..=1048576.0).contains(&state.position.y)
                || state.ack_input_seq > MAX_SEQUENCE
                || players
                    .insert(user, (b, ProjectedPlayer { identity, state }))
                    .is_some()
            {
                return Err(ProjectionError::InvalidFrame);
            }
        }
        if players.is_empty() || players.len() != simulation.player_count() || tick > MAX_SEQUENCE {
            return Err(ProjectionError::InvalidFrame);
        }
        Ok(Self {
            scope,
            tick,
            players,
        })
    }
}
pub(crate) fn same_room(a: &Binding, b: &Binding) -> bool {
    a.key.event_id == b.key.event_id
        && a.owner_id == b.owner_id
        && a.room_id == b.room_id
        && a.room_epoch == b.room_epoch
        && a.resources == b.resources
}
/// Request-bound projection observations, not a fabricated cross-service transaction stamp.
/// A production adapter MUST serialize policy invalidation with publication, and re-resolve
/// BE37 Presence(event,actor=recipient,subject=player), current stamps and authority for each
/// member, including self. Either-direction/inherited block excludes the subject.
/// Unknown, stale, restricted or unavailable self authority is an error, never an empty allow.
pub struct RecipientView {
    pub binding: Binding,
    /// Local owner invalidation generation; NOT a cross-service authority/revision stamp.
    pub invalidation_id: Uuid,
    pub checked_at_ms: u64,
    pub valid_until_ms: u64,
    pub visible_users: BTreeSet<Uuid>,
}
pub trait RecipientPolicy {
    fn current(
        &self,
        recipient: &Binding,
        subjects: &[Binding],
        now_ms: u64,
    ) -> Result<RecipientView, ProjectionError>;
}
pub struct UnavailableRecipientPolicy;
impl RecipientPolicy for UnavailableRecipientPolicy {
    fn current(
        &self,
        _: &Binding,
        _: &[Binding],
        _: u64,
    ) -> Result<RecipientView, ProjectionError> {
        Err(ProjectionError::PolicyUnavailable)
    }
}
pub(crate) fn project(
    frame: &RoomFrame,
    recipient: &Binding,
    now: u64,
    membership: &impl MembershipPort,
    policy: &impl RecipientPolicy,
) -> Result<(Uuid, BTreeMap<Uuid, ProjectedPlayer>), ProjectionError> {
    if !same_room(&frame.scope, recipient)
        || !frame
            .players
            .get(&recipient.key.user_id.uuid())
            .is_some_and(|(b, _)| b == recipient)
    {
        return Err(ProjectionError::Authority);
    }
    let subjects: Vec<_> = frame.players.values().map(|(b, _)| b.clone()).collect();
    let view = policy.current(recipient, &subjects, now)?;
    if view.invalidation_id.is_nil()
        || view.binding != *recipient
        || view.checked_at_ms != now
        || view.valid_until_ms <= now
        || view.valid_until_ms - now > AUTH_LEASE_MAX_MS
        || !view.visible_users.contains(&recipient.key.user_id.uuid())
        || view
            .visible_users
            .iter()
            .any(|u| !frame.players.contains_key(u))
    {
        return Err(ProjectionError::PolicyUnavailable);
    }
    let mut result = BTreeMap::new();
    for user in view.visible_users {
        let (b, player) = &frame.players[&user];
        let lease = membership
            .current(b, now)
            .map_err(|_| ProjectionError::Authority)?;
        if lease.binding != *b
            || lease.lease_until_ms <= now
            || lease.access_expires_at_ms <= now
            || lease.lease_until_ms - now > AUTH_LEASE_MAX_MS
        {
            return Err(ProjectionError::Authority);
        }
        result.insert(user, player.clone());
    }
    Ok((view.invalidation_id, result))
}
