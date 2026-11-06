//! VENU-BE-M01 closed route decoding before any owner work. The bilateral
//! friend read does not exist yet, so production composition refuses these
//! operations with 503 rather than reconstructing friendship locally.
use super::{canonical_uuid, proto::ProductRequest, Operation};
use concert_application::venue_meeting::{CreateInvitation, DecideInvitation, SendMessage};
use concert_domain::venue_meeting::{
    Decision, LandmarkSelection, MeetingSelection, MAX_REQUEST_BYTES,
};
use serde::Deserialize;
use std::collections::BTreeMap;
use tonic::Status;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeetingOperation {
    Context,
    Invitations,
    CreateInvitation,
    GetInvitation(Uuid),
    DecideInvitation(Uuid),
    Messages,
    SendMessage,
    Status(Uuid),
}

impl MeetingOperation {
    pub fn id(self) -> &'static str {
        match self {
            Self::Context => "resolveVenueMeetingContext",
            Self::Invitations => "listVenueMeetingInvitations",
            Self::CreateInvitation => "createVenueMeetingInvitation",
            Self::GetInvitation(_) => "getVenueMeetingInvitation",
            Self::DecideInvitation(_) => "decideVenueMeetingInvitation",
            Self::Messages => "listVenueMeetingMessages",
            Self::SendMessage => "sendVenueMeetingMessage",
            Self::Status(_) => "getVenueMeetingCommand",
        }
    }
    pub fn mutation(self) -> bool {
        matches!(self, Self::CreateInvitation | Self::DecideInvitation(_) | Self::SendMessage)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LandmarkBody {
    landmark_id: String,
    catalog_revision: i64,
    landmark_revision: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SelectionBody {
    selected_friend_id: String,
    landmark: Option<LandmarkBody>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateBody {
    selection: SelectionBody,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DecideBody {
    selection: SelectionBody,
    expected_revision: i64,
    decision: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SendBody {
    selection: SelectionBody,
    body: String,
    invitation_id: Option<String>,
}

fn invalid() -> Status {
    Status::invalid_argument("Invalid Venue meeting request")
}

fn selection(value: SelectionBody) -> Result<MeetingSelection, Status> {
    let friend = canonical_uuid(&value.selected_friend_id)?;
    let landmark = value
        .landmark
        .map(|landmark| {
            LandmarkSelection::new(
                landmark.landmark_id,
                landmark.catalog_revision,
                landmark.landmark_revision,
            )
        })
        .transpose()
        .map_err(|_| invalid())?;
    MeetingSelection::new(friend, landmark).map_err(|_| invalid())
}

fn query(target: &str) -> Result<BTreeMap<String, String>, Status> {
    let Some((_, query)) = target.split_once('?') else {
        return Ok(BTreeMap::new());
    };
    if query.is_empty() || query.contains('%') || query.contains('+') {
        return Err(invalid());
    }
    let mut fields = BTreeMap::new();
    for pair in query.split('&') {
        let (name, value) = pair.split_once('=').ok_or_else(invalid)?;
        if name.is_empty()
            || value.is_empty()
            || fields.insert(name.to_owned(), value.to_owned()).is_some()
        {
            return Err(invalid());
        }
    }
    Ok(fields)
}

fn decimal(value: &str, minimum: i64) -> Result<i64, Status> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(invalid());
    }
    value
        .parse::<i64>()
        .ok()
        .filter(|value| (minimum..=concert_domain::MAX_REVISION).contains(value))
        .ok_or_else(invalid)
}

fn selection_query(
    fields: &mut BTreeMap<String, String>,
    landmark_required: bool,
) -> Result<MeetingSelection, Status> {
    let friend = canonical_uuid(&fields.remove("selectedFriendId").ok_or_else(invalid)?)?;
    let landmark_id = fields.remove("landmarkId");
    let catalog = fields.remove("catalogRevision");
    let landmark_revision = fields.remove("landmarkRevision");
    let landmark = match (landmark_id, catalog, landmark_revision) {
        (None, None, None) if !landmark_required => None,
        (Some(id), Some(catalog), Some(revision)) if !id.is_empty() => Some(
            LandmarkSelection::new(id, decimal(&catalog, 1)?, decimal(&revision, 1)?)
                .map_err(|_| invalid())?,
        ),
        _ => return Err(invalid()),
    };
    if landmark_required && landmark.is_none() {
        return Err(invalid());
    }
    MeetingSelection::new(friend, landmark).map_err(|_| invalid())
}

pub(super) fn route(
    event: Uuid,
    method: &str,
    target: &str,
) -> Result<MeetingOperation, Status> {
    let (path, _) = target.split_once('?').map_or((target, None), |(p, q)| (p, Some(q)));
    let prefix = format!("/api/concert/events/{event}/venue-meeting");
    let suffix = path.strip_prefix(&prefix).ok_or_else(invalid)?;
    let pieces: Vec<_> = suffix.trim_start_matches('/').split('/').collect();
    match (method, suffix, pieces.as_slice()) {
        ("GET", "", [""]) => Ok(MeetingOperation::Context),
        ("GET", "/invitations", ["invitations"]) => Ok(MeetingOperation::Invitations),
        ("POST", "/invitations", ["invitations"]) => Ok(MeetingOperation::CreateInvitation),
        ("GET", _, ["invitations", id]) => Ok(MeetingOperation::GetInvitation(
            canonical_uuid(id)?,
        )),
        ("POST", _, ["invitations", id, "decision"]) => Ok(MeetingOperation::DecideInvitation(
            canonical_uuid(id)?,
        )),
        ("GET", "/messages", ["messages"]) => Ok(MeetingOperation::Messages),
        ("POST", "/messages", ["messages"]) => Ok(MeetingOperation::SendMessage),
        ("GET", _, ["commands", key]) => Ok(MeetingOperation::Status(canonical_uuid(key)?)),
        _ => Err(invalid()),
    }
}

pub(super) fn request(
    request: &ProductRequest,
    expected: Operation,
) -> Result<(), Status> {
    let Operation::Meeting(operation) = expected else {
        return Err(invalid());
    };
    canonical_uuid(&request.user_id)?;
    let event = canonical_uuid(&request.event_id)?;
    if route(event, &request.http_method, &request.http_target)? != operation {
        return Err(invalid());
    }
    let mut fields = query(&request.http_target)?;
    if operation.mutation() {
        if request.content_type != "application/json"
            || request.http_body.is_empty()
            || request.http_body.len() > MAX_REQUEST_BYTES
        {
            return Err(invalid());
        }
        canonical_uuid(&request.idempotency_key)?;
        match operation {
            MeetingOperation::CreateInvitation => {
                let body: CreateBody =
                    serde_json::from_slice(&request.http_body).map_err(|_| invalid())?;
                CreateInvitation::new(selection(body.selection)?).map_err(|_| invalid())?;
            }
            MeetingOperation::DecideInvitation(_) => {
                let body: DecideBody =
                    serde_json::from_slice(&request.http_body).map_err(|_| invalid())?;
                let decision = Decision::parse(&body.decision).map_err(|_| invalid())?;
                DecideInvitation::new(
                    selection(body.selection)?,
                    body.expected_revision,
                    decision,
                )
                .map_err(|_| invalid())?;
            }
            MeetingOperation::SendMessage => {
                let body: SendBody =
                    serde_json::from_slice(&request.http_body).map_err(|_| invalid())?;
                let invitation = body
                    .invitation_id
                    .map(|value| canonical_uuid(&value))
                    .transpose()?;
                SendMessage::new(selection(body.selection)?, body.body, invitation)
                    .map_err(|_| invalid())?;
            }
            _ => return Err(invalid()),
        }
        return Ok(());
    }
    if !request.http_body.is_empty()
        || !request.content_type.is_empty()
        || !request.idempotency_key.is_empty()
    {
        return Err(invalid());
    }
    match operation {
        MeetingOperation::Context => {
            selection_query(&mut fields, false)?;
        }
        MeetingOperation::Invitations => {
            let direction = fields.remove("direction").ok_or_else(invalid)?;
            if !matches!(direction.as_str(), "INCOMING" | "OUTGOING") {
                return Err(invalid());
            }
            if let Some(cursor) = fields.remove("cursor") {
                decimal(&cursor, 0)?;
            }
        }
        MeetingOperation::GetInvitation(_) | MeetingOperation::DecideInvitation(_) => {
            selection_query(&mut fields, true)?;
        }
        MeetingOperation::Messages | MeetingOperation::Status(_) => {
            selection_query(&mut fields, false)?;
            if operation == MeetingOperation::Messages {
                if let Some(cursor) = fields.remove("cursor") {
                    decimal(&cursor, 0)?;
                }
            }
        }
        MeetingOperation::CreateInvitation | MeetingOperation::SendMessage => {
            return Err(invalid());
        }
    }
    if !fields.is_empty() {
        return Err(invalid());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(event: Uuid, tail: &str) -> String {
        format!("/api/concert/events/{event}/venue-meeting/{tail}")
    }

    #[test]
    fn frozen_routes_are_exact_and_ids_are_canonical() {
        let event = Uuid::new_v4();
        let id = Uuid::new_v4();
        assert_eq!(
            route(event, "GET", &target(event, "context?selectedFriendId=1")).is_err(),
            true
        );
        assert_eq!(
            route(event, "GET", &target(event, "invitations")).unwrap(),
            MeetingOperation::Invitations
        );
        assert_eq!(
            route(event, "POST", &target(event, "invitations")).unwrap(),
            MeetingOperation::CreateInvitation
        );
        assert_eq!(
            route(event, "GET", &target(event, &format!("invitations/{id}"))).unwrap(),
            MeetingOperation::GetInvitation(id)
        );
        assert_eq!(
            route(event, "POST", &target(event, &format!("invitations/{id}/decision"))).unwrap(),
            MeetingOperation::DecideInvitation(id)
        );
        assert_eq!(
            route(event, "GET", &target(event, "messages")).unwrap(),
            MeetingOperation::Messages
        );
        assert_eq!(
            route(event, "POST", &target(event, "messages")).unwrap(),
            MeetingOperation::SendMessage
        );
        assert_eq!(
            route(event, "GET", &target(event, &format!("commands/{id}"))).unwrap(),
            MeetingOperation::Status(id)
        );
        assert!(route(event, "GET", &target(event, "bare-user-dm")).is_err());
        let nil = "00000000-0000-0000-0000-000000000000";
        assert!(route(event, "GET", &target(event, &format!("invitations/{nil}"))).is_err());
    }

    #[test]
    fn invite_and_message_shapes_reject_unknown_or_missing_landmark() {
        let event = Uuid::new_v4();
        let actor = Uuid::new_v4();
        let friend = Uuid::new_v4();
        let key = Uuid::new_v4();
        let request = |target: String, body: serde_json::Value, expected| ProductRequest {
            user_id: actor.to_string(),
            event_id: event.to_string(),
            session_family_id: Uuid::new_v4().to_string(),
            request_id: Uuid::new_v4().to_string(),
            http_method: "POST".into(),
            http_target: target,
            http_body: serde_json::to_vec(&body).unwrap(),
            content_type: "application/json".into(),
            idempotency_key: key.to_string(),
            ..Default::default()
        };
        let create = request(
            target(event, "invitations"),
            serde_json::json!({"selection":{"selectedFriendId":friend,"landmark":{"landmarkId":"gate-a","catalogRevision":1,"landmarkRevision":2}}}),
            (),
        );
        assert!(request_checked(&create, MeetingOperation::CreateInvitation));
        let without = request(
            target(event, "invitations"),
            serde_json::json!({"selection":{"selectedFriendId":friend}}),
            (),
        );
        assert!(!request_checked(&without, MeetingOperation::CreateInvitation));
        let unknown = request(
            target(event, "invitations"),
            serde_json::json!({"selection":{"selectedFriendId":friend,"landmark":{"landmarkId":"gate-a","catalogRevision":1,"landmarkRevision":2}},"extra":1}),
            (),
        );
        assert!(!request_checked(&unknown, MeetingOperation::CreateInvitation));
        let send = request(
            target(event, "messages"),
            serde_json::json!({"selection":{"selectedFriendId":friend},"body":"hello"}),
            (),
        );
        assert!(request_checked(&send, MeetingOperation::SendMessage));
    }

    fn request_checked(request: &ProductRequest, operation: MeetingOperation) -> bool {
        request_checked_inner(request, Operation::Meeting(operation))
    }

    fn request_checked_inner(request: &ProductRequest, operation: Operation) -> bool {
        super::request(request, operation).is_ok()
    }
}
