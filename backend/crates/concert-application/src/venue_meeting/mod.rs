//! VENU-BE-M01 transaction guards. Every cell re-evaluates current authority;
//! an invitation, landmark or identifier never grants conversation access.
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_domain::venue_meeting::{
    valid_message_body, Decision, InvitationState, LandmarkSelection, MeetingError,
    MeetingPolicy, MeetingSelection, StoredInvitation,
};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PairFacts {
    pub relationship_revision: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LandmarkReview {
    pub catalog_revision: i64,
    pub landmark_revision: i64,
    pub can_meet: bool,
    pub valid_until: DateTime<Utc>,
}

#[async_trait]
pub trait VenueMeetingAuthority: Send + Sync {
    /// The bilateral `VENUE_FRIEND_V1` owner read. The retained directional
    /// Friend flag is insufficient and must never be reconstructed here.
    async fn friend(
        &self,
        event: Uuid,
        actor: Uuid,
        peer: Uuid,
    ) -> Result<PairFacts, MeetingError>;
    async fn reviewed_landmark(
        &self,
        event: Uuid,
        selection: &LandmarkSelection,
    ) -> Result<LandmarkReview, MeetingError>;
    async fn policy(&self, event: Uuid) -> Result<MeetingPolicy, MeetingError>;
}

pub struct UnavailableVenueMeetingAuthority;

#[async_trait]
impl VenueMeetingAuthority for UnavailableVenueMeetingAuthority {
    async fn friend(
        &self,
        _: Uuid,
        _: Uuid,
        _: Uuid,
    ) -> Result<PairFacts, MeetingError> {
        Err(MeetingError::PolicyUnavailable)
    }
    async fn reviewed_landmark(
        &self,
        _: Uuid,
        _: &LandmarkSelection,
    ) -> Result<LandmarkReview, MeetingError> {
        Err(MeetingError::PolicyUnavailable)
    }
    async fn policy(&self, _: Uuid) -> Result<MeetingPolicy, MeetingError> {
        Err(MeetingError::PolicyUnavailable)
    }
}

pub async fn require_friend<A: VenueMeetingAuthority>(
    authority: &A,
    event: Uuid,
    actor: Uuid,
    peer: Uuid,
) -> Result<PairFacts, MeetingError> {
    if actor.is_nil() || peer.is_nil() || actor == peer {
        return Err(MeetingError::InvalidInput);
    }
    authority.friend(event, actor, peer).await
}

pub async fn require_selection_friend<A: VenueMeetingAuthority>(
    authority: &A,
    event: Uuid,
    actor: Uuid,
    selection: &MeetingSelection,
) -> Result<PairFacts, MeetingError> {
    require_friend(authority, event, actor, selection.selected_friend_id).await
}

pub async fn require_invitation_landmark<A: VenueMeetingAuthority>(
    authority: &A,
    event: Uuid,
    selection: &MeetingSelection,
    now: DateTime<Utc>,
) -> Result<LandmarkReview, MeetingError> {
    let landmark = selection
        .landmark
        .as_ref()
        .ok_or(MeetingError::InvalidInput)?;
    let review = authority.reviewed_landmark(event, landmark).await?;
    if review.catalog_revision != landmark.catalog_revision
        || review.landmark_revision != landmark.landmark_revision
        || !review.can_meet
        || review.valid_until <= now
    {
        return Err(MeetingError::ResourceUnavailable);
    }
    Ok(review)
}

pub async fn require_policy<A: VenueMeetingAuthority>(
    authority: &A,
    event: Uuid,
) -> Result<MeetingPolicy, MeetingError> {
    let policy = authority.policy(event).await?;
    if !policy.valid() {
        return Err(MeetingError::PolicyUnavailable);
    }
    Ok(policy)
}

pub fn safe_ceiling(
    policy: &MeetingPolicy,
    landmark: Option<&LandmarkReview>,
    now: DateTime<Utc>,
) -> Result<DateTime<Utc>, MeetingError> {
    let ceiling = landmark
        .map(|review| review.valid_until)
        .map_or(policy.valid_until, |value| value.min(policy.valid_until));
    if ceiling <= now {
        return Err(MeetingError::PolicyUnavailable);
    }
    Ok(ceiling)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateInvitation {
    pub selection: MeetingSelection,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecideInvitation {
    pub selection: MeetingSelection,
    pub expected_revision: i64,
    pub decision: Decision,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SendMessage {
    pub selection: MeetingSelection,
    pub body: String,
    pub invitation_id: Option<Uuid>,
}

impl CreateInvitation {
    pub fn new(selection: MeetingSelection) -> Result<Self, MeetingError> {
        if selection.landmark.is_none() {
            return Err(MeetingError::InvalidInput);
        }
        Ok(Self { selection })
    }
}

impl DecideInvitation {
    pub fn new(
        selection: MeetingSelection,
        expected_revision: i64,
        decision: Decision,
    ) -> Result<Self, MeetingError> {
        if selection.landmark.is_none()
            || !(1..=concert_domain::MAX_REVISION).contains(&expected_revision)
        {
            return Err(MeetingError::InvalidInput);
        }
        Ok(Self {
            selection,
            expected_revision,
            decision,
        })
    }
}

impl SendMessage {
    pub fn new(
        selection: MeetingSelection,
        body: impl Into<String>,
        invitation_id: Option<Uuid>,
    ) -> Result<Self, MeetingError> {
        let body = body.into();
        if !valid_message_body(&body) || invitation_id.is_some_and(|id| id.is_nil()) {
            return Err(MeetingError::InvalidInput);
        }
        Ok(Self {
            selection,
            body,
            invitation_id,
        })
    }
}

pub fn decision_transition(
    invitation: &StoredInvitation,
    actor: Uuid,
    decision: Decision,
    now: DateTime<Utc>,
) -> Result<InvitationState, MeetingError> {
    invitation.decision_state(actor, decision, now)
}
