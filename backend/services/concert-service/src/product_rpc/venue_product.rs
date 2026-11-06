//! VENU-BE product producer. Route decoding is closed in `venue_admission`;
//! every venue owner guard is re-evaluated here under the held canonical scope.
use super::{
    proto, scoped::ProductRepresentation, venue_admission::VenueRequest,
    venue_meeting_admission::MeetingOperation, Operation, VerifiedProduct,
};
use crate::{
    adapters::{
        out::relationship_authority::guarded::OwnerGuardScope,
        out::{
            public_profile::PublicProfileReader,
            relationship_authority::BoundedRelationshipAuthority,
        },
    },
    venue_authority::VenueFriendAuthority,
    venue_location::{PgVenueLocation, VenueLocationAuthority},
    venue_meeting::{Direction, PgVenueMeeting},
    venue_notes::{
        NoteFriendAuthority, NoteMeetingAuthority, NotePlaceAuthority, NoteProfileAuthority,
        PgPlaceNotes, UnavailableModerationAuthority,
    },
};
use async_trait::async_trait;
use chrono::{DateTime, SecondsFormat, Utc};
use concert_application::venue_location::{VenueLocation, VenueLocationPolicyProvider};
use concert_application::venue_meeting::{LandmarkReview, PairFacts, VenueMeetingAuthority};
use concert_domain::venue_location::{
    CommandReceipt, LocationSnapshot, SharingProjection, VenueLocationError, VenueLocationPolicy,
    VisibleLocation,
};
use concert_domain::venue_meeting::{
    LandmarkSelection, MeetingError, MeetingPolicy, MeetingSelection,
};
use concert_domain::venue_notes::{
    LandmarkSelection as NoteLandmarkSelection, NoteAuthor, PlaceNoteError,
};
use concert_domain::{AuthUserId, EventId, ParticipantKey};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{PgConnection, PgPool, Row};
use std::sync::Arc;
use tonic::Status;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VenuePolicyConfig {
    pub venue_key: String,
    pub provider_id: String,
    pub source_revision: String,
    pub location_revision: i64,
    pub meeting_revision: i64,
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
    pub invitation_ms: i64,
    pub content_ms: i64,
    pub purge_ms: i64,
}

impl VenuePolicyConfig {
    pub fn validate(&self) -> Result<(), &'static str> {
        let text = |value: &str| !value.is_empty() && value.len() <= 128;
        let positive = |value: i64| (1..=concert_domain::MAX_REVISION).contains(&value);
        if !text(&self.venue_key) || !text(&self.provider_id) || !text(&self.source_revision) {
            return Err("Venue policy identity is invalid");
        }
        if !positive(self.location_revision) || !positive(self.meeting_revision) {
            return Err("Venue policy revision is invalid");
        }
        for value in [
            self.fresh_ms,
            self.last_known_ms,
            self.fix_max_age_ms,
            self.sharing_lease_ms,
            self.request_ttl_ms,
            self.check_in_ttl_ms,
            self.coordinate_retention_ms,
            self.command_retention_ms,
            self.invitation_ms,
            self.content_ms,
            self.purge_ms,
        ] {
            if !positive(value) {
                return Err("Venue policy duration is invalid");
            }
        }
        if self.fresh_ms > self.last_known_ms
            || self.last_known_ms > self.coordinate_retention_ms
            || self.fix_max_age_ms > self.last_known_ms
            || self.future_skew_ms < 0
            || self.future_skew_ms > concert_domain::MAX_REVISION
            || !self.max_accuracy_m.is_finite()
            || self.max_accuracy_m <= 0.0
            || !(1..=50).contains(&self.max_viewers)
            || self.purge_ms < self.content_ms
        {
            return Err("Venue policy ordering is invalid");
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct VenuePolicies {
    pool: PgPool,
    config: Arc<VenuePolicyConfig>,
}

impl VenuePolicies {
    pub fn new(pool: PgPool, config: Arc<VenuePolicyConfig>) -> Self {
        Self { pool, config }
    }
    async fn window(&self, event: EventId) -> Result<(String, DateTime<Utc>), VenueLocationError> {
        let row = sqlx::query(
            "select e.timezone,m.event_cutoff
               from public.concert_events e
               join public.concert_meeting_settings m on m.event_id=e.event_id
              where e.event_id=$1",
        )
        .bind(event.uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| VenueLocationError::DependencyUnavailable)?
        .ok_or(VenueLocationError::PolicyUnavailable)?;
        let timezone: String = row
            .try_get("timezone")
            .map_err(|_| VenueLocationError::DependencyUnavailable)?;
        let valid_until: DateTime<Utc> = row
            .try_get("event_cutoff")
            .map_err(|_| VenueLocationError::DependencyUnavailable)?;
        if valid_until <= Utc::now() {
            return Err(VenueLocationError::PolicyUnavailable);
        }
        Ok((timezone, valid_until))
    }
    pub async fn location(
        &self,
        event: EventId,
    ) -> Result<VenueLocationPolicy, VenueLocationError> {
        let (timezone, valid_until) = self.window(event).await?;
        let policy = VenueLocationPolicy {
            event_id: event,
            venue_key: self.config.venue_key.clone(),
            revision: self.config.location_revision,
            timezone,
            valid_until,
            fresh_ms: self.config.fresh_ms,
            last_known_ms: self.config.last_known_ms,
            fix_max_age_ms: self.config.fix_max_age_ms,
            future_skew_ms: self.config.future_skew_ms,
            sharing_lease_ms: self.config.sharing_lease_ms,
            request_ttl_ms: self.config.request_ttl_ms,
            check_in_ttl_ms: self.config.check_in_ttl_ms,
            coordinate_retention_ms: self.config.coordinate_retention_ms,
            command_retention_ms: self.config.command_retention_ms,
            max_accuracy_m: self.config.max_accuracy_m,
            max_viewers: self.config.max_viewers,
        };
        policy.validate(Utc::now())?;
        Ok(policy)
    }
    pub async fn meeting(&self, event: Uuid) -> Result<MeetingPolicy, MeetingError> {
        let (timezone, valid_until) = self
            .window(EventId::from_uuid(event))
            .await
            .map_err(meeting_policy_error)?;
        let policy = MeetingPolicy {
            policy_revision: self.config.meeting_revision,
            valid_until,
            invitation_ms: self.config.invitation_ms,
            content_ms: self.config.content_ms,
            purge_ms: self.config.purge_ms,
            event_timezone: timezone,
        };
        policy
            .valid()
            .then_some(policy)
            .ok_or(MeetingError::PolicyUnavailable)
    }
}

fn meeting_policy_error(error: VenueLocationError) -> MeetingError {
    match error {
        VenueLocationError::PolicyUnavailable => MeetingError::PolicyUnavailable,
        _ => MeetingError::StorageUnavailable,
    }
}

#[async_trait]
impl VenueLocationPolicyProvider for VenuePolicies {
    async fn policy(&self, event: EventId) -> Result<VenueLocationPolicy, VenueLocationError> {
        self.location(event).await
    }
}

#[derive(Clone)]
pub struct PgReviewedPlace {
    pool: PgPool,
}

struct ReviewedPlace {
    catalog_revision: i64,
    landmark_revision: i64,
}

impl PgReviewedPlace {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    async fn row(
        &self,
        connection: &mut PgConnection,
        event: Uuid,
        landmark_id: &str,
    ) -> Result<Option<ReviewedPlace>, PlaceNoteError> {
        let place = Uuid::parse_str(landmark_id).map_err(|_| PlaceNoteError::NotFound)?;
        let row = sqlx::query(
            "select e.catalog_revision,e.map_revision
               from public.concert_events_map_releases e
               join public.concert_map_release_places q
                 on q.map_id=e.map_id and q.map_revision=e.map_revision and q.place_id=$2
               join public.concert_place_current_state s
                 on s.map_id=q.map_id and s.map_revision=q.map_revision and s.place_id=q.place_id
              where e.event_id=$1 and q.review_state='REVIEWED'
                and q.verification_state='VERIFIED' and q.place_kind='PHYSICAL'
                and s.closure_state='OPEN'",
        )
        .bind(event)
        .bind(place)
        .fetch_optional(&mut *connection)
        .await
        .map_err(|_| PlaceNoteError::DependencyUnavailable)?;
        row.map(|row| {
            Ok(ReviewedPlace {
                catalog_revision: row
                    .try_get("catalog_revision")
                    .map_err(|_| PlaceNoteError::DependencyUnavailable)?,
                landmark_revision: row
                    .try_get("map_revision")
                    .map_err(|_| PlaceNoteError::DependencyUnavailable)?,
            })
        })
        .transpose()
    }
    async fn reviewed(
        &self,
        connection: &mut PgConnection,
        event: Uuid,
        selection: &LandmarkSelection,
    ) -> Result<ReviewedPlace, PlaceNoteError> {
        self.check(
            connection,
            event,
            &selection.landmark_id,
            selection.catalog_revision,
            selection.landmark_revision,
        )
        .await
    }
    async fn check(
        &self,
        connection: &mut PgConnection,
        event: Uuid,
        landmark_id: &str,
        catalog_revision: i64,
        landmark_revision: i64,
    ) -> Result<ReviewedPlace, PlaceNoteError> {
        let row = self
            .row(connection, event, landmark_id)
            .await?
            .ok_or(PlaceNoteError::NotFound)?;
        if row.catalog_revision != catalog_revision || row.landmark_revision != landmark_revision {
            return Err(PlaceNoteError::NotFound);
        }
        Ok(row)
    }
}

#[async_trait]
impl NotePlaceAuthority for PgReviewedPlace {
    async fn review(
        &self,
        connection: &mut PgConnection,
        event: Uuid,
        selection: &NoteLandmarkSelection,
    ) -> Result<(), PlaceNoteError> {
        self.check(
            connection,
            event,
            &selection.landmark_id,
            selection.catalog_revision,
            selection.landmark_revision,
        )
        .await
        .map(|_| ())
    }
    async fn current(
        &self,
        connection: &mut PgConnection,
        event: Uuid,
        landmark_id: &str,
    ) -> Result<(), PlaceNoteError> {
        self.row(connection, event, landmark_id)
            .await?
            .map(|_| ())
            .ok_or(PlaceNoteError::NotFound)
    }
}

#[async_trait]
impl VenueMeetingAuthority for PgReviewedPlace {
    async fn friend(&self, _: Uuid, _: Uuid, _: Uuid) -> Result<PairFacts, MeetingError> {
        Err(MeetingError::PolicyUnavailable)
    }
    async fn reviewed_landmark(
        &self,
        event: Uuid,
        selection: &LandmarkSelection,
    ) -> Result<LandmarkReview, MeetingError> {
        let mut connection = self
            .pool
            .acquire()
            .await
            .map_err(|_| MeetingError::StorageUnavailable)?;
        let row = self
            .row(&mut connection, event, &selection.landmark_id)
            .await
            .map_err(|error| match error {
                PlaceNoteError::NotFound => MeetingError::ResourceUnavailable,
                _ => MeetingError::PolicyUnavailable,
            })?
            .ok_or(MeetingError::ResourceUnavailable)?;
        if row.catalog_revision != selection.catalog_revision
            || row.landmark_revision != selection.landmark_revision
        {
            return Err(MeetingError::ResourceUnavailable);
        }
        meeting_review(&mut connection, event, row).await
    }
    async fn policy(&self, _: Uuid) -> Result<MeetingPolicy, MeetingError> {
        Err(MeetingError::PolicyUnavailable)
    }
}

async fn meeting_review(
    connection: &mut PgConnection,
    event: Uuid,
    row: ReviewedPlace,
) -> Result<LandmarkReview, MeetingError> {
    let cutoff: Option<DateTime<Utc>> = sqlx::query_scalar(
        "select event_cutoff from public.concert_meeting_settings where event_id=$1",
    )
    .bind(event)
    .fetch_optional(&mut *connection)
    .await
    .map_err(|_| MeetingError::StorageUnavailable)?;
    let valid_until = cutoff.ok_or(MeetingError::PolicyUnavailable)?;
    if valid_until <= Utc::now() {
        return Err(MeetingError::ResourceUnavailable);
    }
    Ok(LandmarkReview {
        catalog_revision: row.catalog_revision,
        landmark_revision: row.landmark_revision,
        can_meet: true,
        valid_until,
    })
}

struct NoteFriends {
    authority: VenueFriendAuthority,
}
#[async_trait]
impl NoteFriendAuthority for NoteFriends {
    async fn available(&self, _: &mut PgConnection) -> Result<(), PlaceNoteError> {
        Ok(())
    }
    async fn bilateral(
        &self,
        connection: &mut PgConnection,
        event: Uuid,
        author: Uuid,
        viewer: Uuid,
    ) -> Result<(), PlaceNoteError> {
        let pair = self
            .authority
            .pair_in(
                connection,
                EventId::from_uuid(event),
                AuthUserId::from_uuid(viewer),
                AuthUserId::from_uuid(author),
                Utc::now(),
            )
            .await
            .map_err(note_friend_error)?;
        pair.allowed.then_some(()).ok_or(PlaceNoteError::Denied)
    }
}

fn note_friend_error(error: VenueLocationError) -> PlaceNoteError {
    match error {
        VenueLocationError::Denied | VenueLocationError::NotFound => PlaceNoteError::Denied,
        _ => PlaceNoteError::DependencyUnavailable,
    }
}

struct NoteProfiles {
    profiles: Arc<PublicProfileReader>,
    scope: Arc<OwnerGuardScope>,
}
#[async_trait]
impl NoteProfileAuthority for NoteProfiles {
    async fn profile(
        &self,
        _: &mut PgConnection,
        _: Uuid,
        user: Uuid,
    ) -> Result<NoteAuthor, PlaceNoteError> {
        let subject = self
            .scope
            .public_profile_subject(user)
            .await
            .map_err(|_| PlaceNoteError::DependencyUnavailable)?;
        let author = self
            .profiles
            .read(&subject)
            .await
            .map_err(|_| PlaceNoteError::DependencyUnavailable)?;
        let value: Value = serde_json::from_slice(&author.json)
            .map_err(|_| PlaceNoteError::DependencyUnavailable)?;
        NoteAuthor::new(
            author.user_id,
            value["display_name"]
                .as_str()
                .ok_or(PlaceNoteError::DependencyUnavailable)?,
            value["bio"].as_str(),
            value["avatar"].clone(),
        )
        .map_err(|_| PlaceNoteError::DependencyUnavailable)
    }
}

struct NoteMeetings {
    authority: VenueFriendAuthority,
    places: PgReviewedPlace,
}
#[async_trait]
impl NoteMeetingAuthority for NoteMeetings {
    #[allow(clippy::too_many_arguments)]
    async fn composer_entry(
        &self,
        connection: &mut PgConnection,
        event: Uuid,
        viewer: Uuid,
        author: Uuid,
        landmark_id: &str,
        landmark_revision: i64,
        access_until: DateTime<Utc>,
    ) -> Result<Option<concert_domain::venue_notes::NoteMeetingEntry>, PlaceNoteError> {
        if viewer == author {
            return Ok(None);
        }
        let pair = self
            .authority
            .pair_in(
                connection,
                EventId::from_uuid(event),
                AuthUserId::from_uuid(viewer),
                AuthUserId::from_uuid(author),
                Utc::now(),
            )
            .await
            .map_err(note_friend_error)?;
        if !pair.allowed {
            return Ok(None);
        }
        if self
            .places
            .current(connection, event, landmark_id)
            .await
            .is_err()
        {
            return Ok(None);
        }
        Ok(Some(concert_domain::venue_notes::NoteMeetingEntry {
            landmark_id: landmark_id.to_owned(),
            recipient_user_id: author,
            landmark_revision,
            access_until,
        }))
    }
}

pub struct VenueProductProducer {
    pool: PgPool,
    config: Arc<VenuePolicyConfig>,
    profiles: Arc<PublicProfileReader>,
    relationships: Arc<dyn BoundedRelationshipAuthority>,
}

impl VenueProductProducer {
    pub fn new(
        pool: PgPool,
        config: Arc<VenuePolicyConfig>,
        profiles: Arc<PublicProfileReader>,
        relationships: Arc<dyn BoundedRelationshipAuthority>,
    ) -> Self {
        Self {
            pool,
            config,
            profiles,
            relationships,
        }
    }
    fn policies(&self) -> VenuePolicies {
        VenuePolicies::new(self.pool.clone(), self.config.clone())
    }
    fn friend(&self, context: &VerifiedProduct) -> Result<VenueFriendAuthority, Status> {
        VenueFriendAuthority::from_product(
            self.pool.clone(),
            self.relationships.clone(),
            context,
            AuthUserId::from_uuid(context.actor()),
        )
        .map_err(|_| Status::unavailable("Venue friendship authority unavailable"))
    }
    fn notes(
        &self,
        context: &VerifiedProduct,
        friend: VenueFriendAuthority,
        places: PgReviewedPlace,
        scope: Arc<OwnerGuardScope>,
    ) -> PgPlaceNotes {
        PgPlaceNotes::new(
            self.pool.clone(),
            Arc::new(places.clone()),
            Arc::new(NoteFriends {
                authority: friend.clone(),
            }),
            Arc::new(NoteProfiles {
                profiles: self.profiles.clone(),
                scope,
            }),
            Arc::new(NoteMeetings {
                authority: friend,
                places,
            }),
            Arc::new(UnavailableModerationAuthority),
        )
    }
    async fn venue(
        &self,
        context: &VerifiedProduct,
        scope: Arc<OwnerGuardScope>,
        request: proto::ProductRequest,
    ) -> Result<ProductRepresentation, Status> {
        let decoded = super::venue_admission::request(&request, context.operation())?;
        let key = super::canonical_uuid(&request.idempotency_key).ok();
        let event = context.event();
        let actor = context.actor();
        let participant = ParticipantKey {
            event_id: EventId::from_uuid(event),
            user_id: AuthUserId::from_uuid(actor),
        };
        match decoded {
            VenueRequest::LocationSnapshot => {
                let service: PgVenueLocation<_, _> = PgVenueLocation::production(
                    self.pool.clone(),
                    self.policies(),
                    self.relationships.clone(),
                    context,
                    participant.user_id,
                )
                .map_err(|_| Status::unavailable("Venue location authority unavailable"))?;
                match service.snapshot(participant).await {
                    Ok(snapshot) => Ok(json_response(
                        200,
                        self.snapshot_json(context, scope, snapshot),
                    )),
                    Err(error) => Ok(error_response(
                        context,
                        location_status(error),
                        location_reason(error),
                    )),
                }
            }
            VenueRequest::SetLocationSharing(command) => {
                let service: PgVenueLocation<_, _> = PgVenueLocation::production(
                    self.pool.clone(),
                    self.policies(),
                    self.relationships.clone(),
                    context,
                    participant.user_id,
                )
                .map_err(|_| Status::unavailable("Venue location authority unavailable"))?;
                let key =
                    key.ok_or_else(|| Status::invalid_argument("Invalid Venue command key"))?;
                match service.set_sharing(participant, key, command).await {
                    Ok(receipt) => Ok(json_response(200, receipt_json(receipt))),
                    Err(error) => Ok(error_response(
                        context,
                        location_status(error),
                        location_reason(error),
                    )),
                }
            }
            VenueRequest::PublishLocationFix(input) => {
                let service: PgVenueLocation<_, _> = PgVenueLocation::production(
                    self.pool.clone(),
                    self.policies(),
                    self.relationships.clone(),
                    context,
                    participant.user_id,
                )
                .map_err(|_| Status::unavailable("Venue location authority unavailable"))?;
                let key =
                    key.ok_or_else(|| Status::invalid_argument("Invalid Venue command key"))?;
                match service.publish_fix(participant, key, input).await {
                    Ok(receipt) => Ok(json_response(200, receipt_json(receipt))),
                    Err(error) => Ok(error_response(
                        context,
                        location_status(error),
                        location_reason(error),
                    )),
                }
            }
            VenueRequest::LocationCommand { .. } => {
                Ok(error_response(context, 503, "DEPENDENCY_UNAVAILABLE"))
            }
            VenueRequest::ListNotes { .. } => {
                Ok(error_response(context, 503, "DEPENDENCY_UNAVAILABLE"))
            }
            VenueRequest::CreateNote { landmark, command } => {
                let friend = self.friend(context)?;
                let places = PgReviewedPlace::new(self.pool.clone());
                let notes = self.notes(context, friend, places, scope.clone());
                let key =
                    key.ok_or_else(|| Status::invalid_argument("Invalid Venue command key"))?;
                match notes.create_note_receipt(actor, event, command, key).await {
                    Ok(note) => Ok(json_response(
                        201,
                        note_command_json(
                            key,
                            "createVenuePlaceNote",
                            note.id,
                            note.revision,
                            note.created_at,
                            note.expires_at,
                        ),
                    )),
                    Err(error) => {
                        let _ = landmark;
                        Ok(contextual_error(context, error))
                    }
                }
            }
            VenueRequest::GetNote { .. } | VenueRequest::ListReplies { .. } => {
                Ok(error_response(context, 503, "DEPENDENCY_UNAVAILABLE"))
            }
            VenueRequest::CreateReply {
                landmark,
                id,
                command,
            } => {
                let friend = self.friend(context)?;
                let places = PgReviewedPlace::new(self.pool.clone());
                let notes = self.notes(context, friend, places, scope.clone());
                let key =
                    key.ok_or_else(|| Status::invalid_argument("Invalid Venue command key"))?;
                match notes
                    .create_reply_receipt(actor, event, &landmark, id, command, key)
                    .await
                {
                    Ok(note) => Ok(json_response(
                        201,
                        note_command_json(
                            key,
                            "createVenuePlaceReply",
                            note.id,
                            note.revision,
                            note.created_at,
                            note.expires_at,
                        ),
                    )),
                    Err(error) => Ok(contextual_error(context, error)),
                }
            }
            VenueRequest::WithdrawNote {
                landmark,
                id,
                expected_revision,
            } => {
                let friend = self.friend(context)?;
                let places = PgReviewedPlace::new(self.pool.clone());
                let notes = self.notes(context, friend, places, scope);
                let key =
                    key.ok_or_else(|| Status::invalid_argument("Invalid Venue command key"))?;
                match notes
                    .withdraw_note_receipt(actor, event, &landmark, id, expected_revision, key)
                    .await
                {
                    Ok(mutation) => Ok(json_response(
                        200,
                        note_command_json(
                            key,
                            "withdrawVenuePlaceNote",
                            mutation.id,
                            mutation.revision,
                            mutation.created_at,
                            mutation.expires_at,
                        ),
                    )),
                    Err(error) => Ok(contextual_error(context, error)),
                }
            }
        }
    }
    async fn meeting(
        &self,
        context: &VerifiedProduct,
        request: proto::ProductRequest,
        operation: MeetingOperation,
    ) -> Result<ProductRepresentation, Status> {
        let actor = context.actor();
        let event = context.event();
        let friend = self.friend(context)?;
        let authority = MeetingAuthority {
            friend,
            places: PgReviewedPlace::new(self.pool.clone()),
            policies: self.policies(),
        };
        let service = PgVenueMeeting::new(self.pool.clone(), authority);
        let target = request.http_target.as_str();
        let key = super::canonical_uuid(&request.idempotency_key).ok();
        let result = match operation {
            MeetingOperation::Context => {
                let selection = super::venue_meeting_admission::read_selection(target, false)?;
                service.context(event, actor, &selection).await
            }
            MeetingOperation::Invitations => {
                let (direction, cursor) =
                    super::venue_meeting_admission::read_direction_cursor(target)?;
                service.inbox(event, actor, direction, cursor).await
            }
            MeetingOperation::CreateInvitation => {
                let command = super::venue_meeting_admission::create_command(&request.http_body)?;
                let key =
                    key.ok_or_else(|| Status::invalid_argument("Invalid Venue command key"))?;
                service.create_invitation(event, actor, key, &command).await
            }
            MeetingOperation::GetInvitation(id) => service.invitation(event, actor, id).await,
            MeetingOperation::DecideInvitation(id) => {
                let command = super::venue_meeting_admission::decide_command(&request.http_body)?;
                let key =
                    key.ok_or_else(|| Status::invalid_argument("Invalid Venue command key"))?;
                service.decide(event, actor, id, key, &command).await
            }
            MeetingOperation::Messages => {
                let (selection, cursor) =
                    super::venue_meeting_admission::read_selection_cursor(target, false)?;
                service.messages(event, actor, &selection, cursor).await
            }
            MeetingOperation::SendMessage => {
                let command = super::venue_meeting_admission::send_command(&request.http_body)?;
                let key =
                    key.ok_or_else(|| Status::invalid_argument("Invalid Venue command key"))?;
                service.send(event, actor, key, &command).await
            }
            MeetingOperation::Status(key) => {
                let selection = super::venue_meeting_admission::read_selection(target, false)?;
                service.command(event, actor, key, &selection).await
            }
        };
        match result {
            Ok(value) => Ok(json_response(200, value)),
            Err(error) => Ok(error_response(
                context,
                meeting_status(error),
                meeting_reason(error),
            )),
        }
    }
    fn snapshot_json(
        &self,
        _: &VerifiedProduct,
        scope: Arc<OwnerGuardScope>,
        snapshot: LocationSnapshot,
    ) -> Value {
        let access_until = scope
            .disclosure_ceiling()
            .map(|value| micros(value.expires_micros()))
            .unwrap_or(snapshot.access_until)
            .min(snapshot.access_until);
        json!({
            "event_id": snapshot.event_id,
            "actor_id": snapshot.actor_id,
            "server_time": ts(snapshot.server_time),
            "access_until": ts(access_until),
            "policy": {
                "event_id": snapshot.policy.event_id.uuid(),
                "venue_key": snapshot.policy.venue_key,
                "provider_id": self.config.provider_id.clone(),
                "source_revision": self.config.source_revision.clone(),
                "revision": snapshot.policy.revision,
                "timezone": snapshot.policy.timezone,
                "valid_until": ts(snapshot.policy.valid_until),
                "fresh_ms": snapshot.policy.fresh_ms,
                "last_known_ms": snapshot.policy.last_known_ms,
                "fix_max_age_ms": snapshot.policy.fix_max_age_ms,
                "future_skew_ms": snapshot.policy.future_skew_ms,
                "sharing_lease_ms": snapshot.policy.sharing_lease_ms,
                "request_ttl_ms": snapshot.policy.request_ttl_ms,
                "check_in_ttl_ms": snapshot.policy.check_in_ttl_ms,
                "coordinate_retention_ms": snapshot.policy.coordinate_retention_ms,
                "command_retention_ms": snapshot.policy.command_retention_ms,
                "max_accuracy_m": snapshot.policy.max_accuracy_m,
                "max_viewers": snapshot.policy.max_viewers,
            },
            "clock": {
                "event_id": snapshot.clock.event_id,
                "venue_key": snapshot.clock.venue_key,
                "policy_revision": snapshot.clock.policy_revision,
                "timezone": snapshot.clock.timezone,
                "server_time": ts(snapshot.clock.server_time),
                "end_of_day": ts(snapshot.clock.end_of_day),
                "valid_until": ts(snapshot.clock.valid_until),
            },
            "sharing": sharing_json(&snapshot.sharing),
            "self_location": snapshot.self_location.as_ref().map(visible_json),
            "friends": snapshot.friends.iter().map(visible_json).collect::<Vec<_>>(),
            "capabilities": {
                "can_share_phone": snapshot.capabilities.can_share_phone,
                "can_pause": snapshot.capabilities.can_pause,
                "can_revoke": snapshot.capabilities.can_revoke,
                "check_in_landmark_ids": snapshot.capabilities.check_in_landmark_ids,
                "geographic_projection": "UNAVAILABLE",
            },
            "incoming_requests": snapshot.incoming_requests,
            "outgoing_requests": snapshot.outgoing_requests,
        })
    }
}

struct MeetingAuthority {
    friend: VenueFriendAuthority,
    places: PgReviewedPlace,
    policies: VenuePolicies,
}

#[async_trait]
impl VenueMeetingAuthority for MeetingAuthority {
    async fn friend(
        &self,
        event: Uuid,
        actor: Uuid,
        peer: Uuid,
    ) -> Result<PairFacts, MeetingError> {
        let pair = self
            .friend
            .pair(
                EventId::from_uuid(event),
                AuthUserId::from_uuid(actor),
                AuthUserId::from_uuid(peer),
                Utc::now(),
            )
            .await
            .map_err(|error| match error {
                VenueLocationError::PolicyUnavailable
                | VenueLocationError::DependencyUnavailable => MeetingError::PolicyUnavailable,
                _ => MeetingError::ResourceUnavailable,
            })?;
        pair.allowed
            .then_some(PairFacts {
                relationship_revision: pair.relationship_revision,
            })
            .ok_or(MeetingError::ResourceUnavailable)
    }
    async fn reviewed_landmark(
        &self,
        event: Uuid,
        selection: &LandmarkSelection,
    ) -> Result<LandmarkReview, MeetingError> {
        self.places.reviewed_landmark(event, selection).await
    }
    async fn policy(&self, event: Uuid) -> Result<MeetingPolicy, MeetingError> {
        self.policies.meeting(event).await
    }
}

fn json_response(status: u32, value: Value) -> ProductRepresentation {
    ProductRepresentation {
        status,
        content_type: "application/json".into(),
        body: serde_json::to_vec(&value).expect("closed venue JSON"),
    }
}

fn error_response(
    context: &VerifiedProduct,
    status: u32,
    reason: &'static str,
) -> ProductRepresentation {
    json_response(
        status,
        json!({
            "code": format!("CONCERT_{status}"),
            "message": "Concert request unavailable",
            "traceId": context.request(),
            "timestamp": ts(Utc::now()),
            "details": {"reason": reason},
        }),
    )
}

fn contextual_error(context: &VerifiedProduct, error: PlaceNoteError) -> ProductRepresentation {
    match error {
        PlaceNoteError::InvalidInput => error_response(context, 400, "INVALID_INPUT"),
        PlaceNoteError::Denied | PlaceNoteError::ModerationDenied => {
            error_response(context, 403, "PERMISSION_DENIED")
        }
        PlaceNoteError::NotFound => error_response(context, 404, "RESOURCE_UNAVAILABLE"),
        PlaceNoteError::Conflict => error_response(context, 409, "VERSION_CONFLICT"),
        PlaceNoteError::EventReadOnly => error_response(context, 409, "EVENT_READ_ONLY"),
        PlaceNoteError::DependencyUnavailable | PlaceNoteError::InvalidState => {
            error_response(context, 503, "DEPENDENCY_UNAVAILABLE")
        }
    }
}

fn location_status(error: VenueLocationError) -> u32 {
    match error {
        VenueLocationError::InvalidInput => 400,
        VenueLocationError::Denied | VenueLocationError::NotFound => 404,
        VenueLocationError::Conflict => 409,
        VenueLocationError::PolicyUnavailable | VenueLocationError::DependencyUnavailable => 503,
    }
}
fn location_reason(error: VenueLocationError) -> &'static str {
    match error {
        VenueLocationError::InvalidInput => "INVALID_INPUT",
        VenueLocationError::Denied | VenueLocationError::NotFound => "RESOURCE_UNAVAILABLE",
        VenueLocationError::Conflict => "VERSION_CONFLICT",
        VenueLocationError::PolicyUnavailable | VenueLocationError::DependencyUnavailable => {
            "DEPENDENCY_UNAVAILABLE"
        }
    }
}
fn meeting_status(error: MeetingError) -> u32 {
    match error {
        MeetingError::InvalidInput => 400,
        MeetingError::ResourceUnavailable => 404,
        MeetingError::RevisionConflict | MeetingError::InvalidState => 409,
        MeetingError::PolicyUnavailable | MeetingError::StorageUnavailable => 503,
    }
}
fn meeting_reason(error: MeetingError) -> &'static str {
    match error {
        MeetingError::InvalidInput => "INVALID_INPUT",
        MeetingError::ResourceUnavailable => "RESOURCE_UNAVAILABLE",
        MeetingError::RevisionConflict | MeetingError::InvalidState => "REVISION_OR_KEY_CONFLICT",
        MeetingError::PolicyUnavailable | MeetingError::StorageUnavailable => {
            "DEPENDENCY_UNAVAILABLE"
        }
    }
}
fn ts(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Micros, true)
}
fn micros(value: i64) -> DateTime<Utc> {
    DateTime::from_timestamp_micros(value).unwrap_or_else(Utc::now)
}
fn sharing_json(value: &SharingProjection) -> Value {
    json!({
        "state": value.state.as_str(),
        "revision": value.revision,
        "viewer_ids": value.viewer_ids,
        "expires_at": value.expires_at.map(ts),
        "updated_at": value.updated_at.map(ts),
    })
}
fn visible_json(value: &VisibleLocation) -> Value {
    json!({
        "user_id": value.user_id,
        "source": value.source.as_str(),
        "position": {
            "latitude": value.position.latitude,
            "longitude": value.position.longitude,
            "accuracy_m": value.position.accuracy_m,
            "captured_at": ts(value.position.captured_at),
            "received_at": ts(value.position.received_at),
        },
        "landmark": Value::Null,
        "observed_at": ts(value.observed_at),
        "fresh_until": value.fresh_until.map(ts),
        "expires_at": ts(value.expires_at),
        "access_until": ts(value.access_until),
        "visibility_revision": value.visibility_revision,
        "physical_arrival_verified": false,
        "can_request_location": value.can_request_location,
    })
}
fn receipt_json(value: CommandReceipt) -> Value {
    json!({
        "command_key": value.command_key,
        "operation": value.operation,
        "state": "SUCCEEDED",
        "resource_kind": value.resource_kind,
        "resource_id": value.resource_id,
        "revision": value.revision,
        "created_at": ts(value.created_at),
        "expires_at": ts(value.expires_at),
    })
}
fn note_command_json(
    key: Uuid,
    operation: &'static str,
    id: Uuid,
    revision: i64,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
) -> Value {
    json!({
        "receipt_id": key,
        "operation_id": operation,
        "state": "SUCCEEDED",
        "resource": {"id": id, "revision": revision},
        "failure_reason": Value::Null,
        "created_at": ts(created_at),
        "expires_at": ts(expires_at),
    })
}

#[async_trait]
impl super::scoped::ScopedProductProducer for VenueProductProducer {
    async fn dispatch(
        &self,
        context: &VerifiedProduct,
        scope: Arc<OwnerGuardScope>,
        request: proto::ProductRequest,
    ) -> Result<ProductRepresentation, Status> {
        match context.operation() {
            Operation::Venue(_) => self.venue(context, scope, request).await,
            Operation::Meeting(operation) => self.meeting(context, request, operation).await,
            _ => Err(Status::unavailable("Product producer unavailable")),
        }
    }
}
