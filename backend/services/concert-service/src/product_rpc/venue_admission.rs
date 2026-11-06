//! VENU-BE L/N closed route decoding before owner work. Parsing a route is
//! lookup only; the producer still reauthorizes every venue guard.
use super::{canonical_uuid, proto::ProductRequest, Operation};
use concert_application::venue_notes::{
    CreatePlaceNote, CreatePlaceReply, NoteListQuery, MAX_LIMIT,
};
use concert_domain::venue_location::{PhoneFixInput, SharingCommand};
use concert_domain::venue_notes::{valid_landmark_id, LandmarkSelection, NoteAudience, NoteExpiry};
use serde::Deserialize;
use std::collections::BTreeMap;
use tonic::Status;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VenueOperation {
    LocationSnapshot,
    SetLocationSharing,
    PublishLocationFix,
    LocationCommand(Uuid),
    ListNotes,
    CreateNote,
    GetNote(Uuid),
    ListReplies(Uuid),
    CreateReply(Uuid),
    WithdrawNote(Uuid),
}

impl VenueOperation {
    pub fn id(self) -> &'static str {
        match self {
            Self::LocationSnapshot => "getVenueLocationSnapshot",
            Self::SetLocationSharing => "setVenueLocationSharing",
            Self::PublishLocationFix => "publishVenueLocationFix",
            Self::LocationCommand(_) => "getVenueLocationCommand",
            Self::ListNotes => "listVenuePlaceNotes",
            Self::CreateNote => "createVenuePlaceNote",
            Self::GetNote(_) => "getVenuePlaceNote",
            Self::ListReplies(_) => "listVenuePlaceReplies",
            Self::CreateReply(_) => "createVenuePlaceReply",
            Self::WithdrawNote(_) => "withdrawVenuePlaceNote",
        }
    }
    pub fn mutation(self) -> bool {
        matches!(
            self,
            Self::SetLocationSharing
                | Self::PublishLocationFix
                | Self::CreateNote
                | Self::CreateReply(_)
                | Self::WithdrawNote(_)
        )
    }
}

fn invalid() -> Status {
    Status::invalid_argument("Invalid Venue product request")
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
struct SharingBody {
    expected_revision: i64,
    state: String,
    viewer_ids: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
struct FixBody {
    expected_sharing_revision: i64,
    source: String,
    latitude: f64,
    longitude: f64,
    accuracy_m: f64,
    captured_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
struct NoteLandmarkBody {
    landmark_id: String,
    catalog_revision: i64,
    landmark_revision: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
struct NoteBody {
    text: String,
    audience: String,
    expiry: String,
    landmark_selection: NoteLandmarkBody,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
struct ReplyBody {
    text: String,
    expected_parent_revision: i64,
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

fn decimal(value: &str, minimum: i64, maximum: i64) -> Result<i64, Status> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(invalid());
    }
    value
        .parse::<i64>()
        .ok()
        .filter(|value| (minimum..=maximum).contains(value))
        .ok_or_else(invalid)
}

fn note_query(
    fields: &mut BTreeMap<String, String>,
    landmark: &str,
) -> Result<NoteListQuery, Status> {
    let limit = fields
        .remove("limit")
        .map(|value| decimal(&value, 1, MAX_LIMIT as i64).map(|value| value as usize))
        .transpose()?;
    let before = fields.remove("before");
    if !fields.is_empty() {
        return Err(invalid());
    }
    NoteListQuery::new(landmark, limit, before.as_deref()).map_err(|_| invalid())
}

pub(super) fn route(event: Uuid, method: &str, target: &str) -> Result<VenueOperation, Status> {
    let (path, _) = target
        .split_once('?')
        .map_or((target, None), |(path, query)| (path, Some(query)));
    let prefix = format!("/api/concert/events/{event}/venue");
    let suffix = path.strip_prefix(&prefix).ok_or_else(invalid)?;
    let pieces: Vec<_> = suffix.trim_start_matches('/').split('/').collect();
    match (method, pieces.as_slice()) {
        ("GET", ["location"]) => Ok(VenueOperation::LocationSnapshot),
        ("PUT", ["location", "sharing"]) => Ok(VenueOperation::SetLocationSharing),
        ("PUT", ["location", "fix"]) => Ok(VenueOperation::PublishLocationFix),
        ("GET", ["location", "commands", key]) => {
            Ok(VenueOperation::LocationCommand(canonical_uuid(key)?))
        }
        ("GET", ["landmarks", landmark, "notes"]) => {
            landmark_route(landmark)?;
            Ok(VenueOperation::ListNotes)
        }
        ("POST", ["landmarks", landmark, "notes"]) => {
            landmark_route(landmark)?;
            Ok(VenueOperation::CreateNote)
        }
        ("GET", ["landmarks", landmark, "notes", id]) => {
            landmark_route(landmark)?;
            Ok(VenueOperation::GetNote(canonical_uuid(id)?))
        }
        ("GET", ["landmarks", landmark, "notes", id, "replies"]) => {
            landmark_route(landmark)?;
            Ok(VenueOperation::ListReplies(canonical_uuid(id)?))
        }
        ("POST", ["landmarks", landmark, "notes", id, "replies"]) => {
            landmark_route(landmark)?;
            Ok(VenueOperation::CreateReply(canonical_uuid(id)?))
        }
        ("DELETE", ["landmarks", landmark, "notes", id]) => {
            landmark_route(landmark)?;
            Ok(VenueOperation::WithdrawNote(canonical_uuid(id)?))
        }
        _ => Err(invalid()),
    }
}

fn landmark_route(value: &str) -> Result<(), Status> {
    valid_landmark_id(value).then_some(()).ok_or_else(invalid)
}

fn note_landmark(target: &str) -> String {
    let path = target.split_once('?').map_or(target, |(path, _)| path);
    path.split('/')
        .skip_while(|piece| *piece != "landmarks")
        .nth(1)
        .unwrap_or_default()
        .to_owned()
}

fn selection(body: &NoteLandmarkBody) -> Result<LandmarkSelection, Status> {
    LandmarkSelection::new(
        &body.landmark_id,
        body.catalog_revision,
        body.landmark_revision,
    )
    .map_err(|_| invalid())
}

/// Typed request payload used by the producer. Reconstructed from the exact
/// original target/body so no normalized alias can differ from the guarded one.
#[derive(Clone, Debug, PartialEq)]
pub enum VenueRequest {
    LocationSnapshot,
    SetLocationSharing(SharingCommand),
    PublishLocationFix(PhoneFixInput),
    LocationCommand {
        key: Uuid,
        operation: String,
    },
    ListNotes {
        landmark: String,
        query: NoteListQuery,
    },
    CreateNote {
        landmark: String,
        command: CreatePlaceNote,
    },
    GetNote {
        landmark: String,
        id: Uuid,
        limit: Option<usize>,
        before: Option<String>,
    },
    ListReplies {
        landmark: String,
        id: Uuid,
        query: NoteListQuery,
    },
    CreateReply {
        landmark: String,
        id: Uuid,
        command: CreatePlaceReply,
    },
    WithdrawNote {
        landmark: String,
        id: Uuid,
        expected_revision: i64,
    },
}

fn require_write(request: &ProductRequest, body_limit: usize) -> Result<(), Status> {
    if request.content_type != "application/json"
        || request.http_body.is_empty()
        || request.http_body.len() > body_limit
    {
        return Err(invalid());
    }
    canonical_uuid(&request.idempotency_key)?;
    Ok(())
}

fn require_read(request: &ProductRequest) -> Result<(), Status> {
    if !request.http_body.is_empty()
        || !request.content_type.is_empty()
        || !request.idempotency_key.is_empty()
    {
        return Err(invalid());
    }
    Ok(())
}

pub(super) fn request(
    request: &ProductRequest,
    expected: Operation,
) -> Result<VenueRequest, Status> {
    let Operation::Venue(operation) = expected else {
        return Err(invalid());
    };
    let event = canonical_uuid(&request.event_id)?;
    canonical_uuid(&request.user_id)?;
    if route(event, &request.http_method, &request.http_target)? != operation {
        return Err(invalid());
    }
    let mut fields = query(&request.http_target)?;
    match operation {
        VenueOperation::LocationSnapshot => {
            require_read(request)?;
            if !fields.is_empty() {
                return Err(invalid());
            }
            Ok(VenueRequest::LocationSnapshot)
        }
        VenueOperation::LocationCommand(key) => {
            require_read(request)?;
            let operation = fields.remove("operation").ok_or_else(invalid)?;
            if !matches!(
                operation.as_str(),
                "setVenueLocationSharing"
                    | "publishVenueLocationFix"
                    | "requestVenueLocation"
                    | "decideVenueLocationRequest"
                    | "setVenueLocationCheckIn"
            ) || !fields.is_empty()
            {
                return Err(invalid());
            }
            Ok(VenueRequest::LocationCommand { key, operation })
        }
        VenueOperation::SetLocationSharing => {
            require_write(request, 8192)?;
            if !fields.is_empty() {
                return Err(invalid());
            }
            let body: SharingBody =
                serde_json::from_slice(&request.http_body).map_err(|_| invalid())?;
            let viewers = body
                .viewer_ids
                .iter()
                .map(|value| canonical_uuid(value))
                .collect::<Result<Vec<_>, _>>()?;
            let command = match body.state.as_str() {
                "ACTIVE" => SharingCommand::Active {
                    expected_revision: body.expected_revision,
                    viewer_ids: viewers,
                },
                "PAUSED" => SharingCommand::Paused {
                    expected_revision: body.expected_revision,
                    viewer_ids: viewers,
                },
                "REVOKED" => SharingCommand::Revoked {
                    expected_revision: body.expected_revision,
                    viewer_ids: viewers,
                },
                _ => return Err(invalid()),
            };
            // Static shape validation only; policy bounds are rechecked by the owner.
            if !(0..=concert_domain::MAX_REVISION).contains(&body.expected_revision)
                || command.viewer_ids().len() > 50
            {
                return Err(invalid());
            }
            Ok(VenueRequest::SetLocationSharing(command))
        }
        VenueOperation::PublishLocationFix => {
            require_write(request, 8192)?;
            if !fields.is_empty() {
                return Err(invalid());
            }
            let body: FixBody =
                serde_json::from_slice(&request.http_body).map_err(|_| invalid())?;
            if body.source != "DEVICE_GEOLOCATION_REPORTED"
                || !(1..=concert_domain::MAX_REVISION).contains(&body.expected_sharing_revision)
                || !body.latitude.is_finite()
                || !(-90.0..=90.0).contains(&body.latitude)
                || !body.longitude.is_finite()
                || !(-180.0..=180.0).contains(&body.longitude)
                || !body.accuracy_m.is_finite()
                || body.accuracy_m < 0.0
            {
                return Err(invalid());
            }
            Ok(VenueRequest::PublishLocationFix(PhoneFixInput {
                expected_sharing_revision: body.expected_sharing_revision,
                latitude: body.latitude,
                longitude: body.longitude,
                accuracy_m: body.accuracy_m,
                captured_at: body.captured_at,
            }))
        }
        VenueOperation::ListNotes => {
            require_read(request)?;
            let landmark = note_landmark(&request.http_target);
            let query = note_query(&mut fields, &landmark)?;
            Ok(VenueRequest::ListNotes { landmark, query })
        }
        VenueOperation::CreateNote => {
            require_write(request, 8192)?;
            if !fields.is_empty() {
                return Err(invalid());
            }
            let landmark = note_landmark(&request.http_target);
            let body: NoteBody =
                serde_json::from_slice(&request.http_body).map_err(|_| invalid())?;
            let audience = body
                .audience
                .parse::<NoteAudience>()
                .map_err(|_| invalid())?;
            let expiry = body.expiry.parse::<NoteExpiry>().map_err(|_| invalid())?;
            let command = CreatePlaceNote::new(
                &body.text,
                audience,
                expiry,
                selection(&body.landmark_selection)?,
            )
            .map_err(|_| invalid())?;
            if command.landmark_selection.landmark_id != landmark {
                return Err(invalid());
            }
            Ok(VenueRequest::CreateNote { landmark, command })
        }
        VenueOperation::GetNote(id) => {
            require_read(request)?;
            let limit = fields
                .remove("limit")
                .map(|value| decimal(&value, 1, MAX_LIMIT as i64).map(|value| value as usize))
                .transpose()?;
            let before = fields.remove("before");
            if !fields.is_empty() {
                return Err(invalid());
            }
            if let Some(before) = before.as_deref() {
                concert_domain::venue_notes::NoteCursor::parse(before).map_err(|_| invalid())?;
            }
            Ok(VenueRequest::GetNote {
                landmark: note_landmark(&request.http_target),
                id,
                limit,
                before,
            })
        }
        VenueOperation::ListReplies(id) => {
            require_read(request)?;
            let landmark = note_landmark(&request.http_target);
            let query = note_query(&mut fields, &landmark)?;
            Ok(VenueRequest::ListReplies {
                landmark,
                id,
                query,
            })
        }
        VenueOperation::CreateReply(id) => {
            require_write(request, 8192)?;
            if !fields.is_empty() {
                return Err(invalid());
            }
            let landmark = note_landmark(&request.http_target);
            let body: ReplyBody =
                serde_json::from_slice(&request.http_body).map_err(|_| invalid())?;
            let command = CreatePlaceReply::new(&body.text, body.expected_parent_revision)
                .map_err(|_| invalid())?;
            Ok(VenueRequest::CreateReply {
                landmark,
                id,
                command,
            })
        }
        VenueOperation::WithdrawNote(id) => {
            if !request.http_body.is_empty() || !request.content_type.is_empty() {
                return Err(invalid());
            }
            canonical_uuid(&request.idempotency_key)?;
            let expected = fields.remove("expected_revision").ok_or_else(invalid)?;
            if !fields.is_empty() {
                return Err(invalid());
            }
            let expected_revision = decimal(&expected, 1, concert_domain::MAX_REVISION)?;
            Ok(VenueRequest::WithdrawNote {
                landmark: note_landmark(&request.http_target),
                id,
                expected_revision,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(event: Uuid, tail: &str) -> String {
        format!("/api/concert/events/{event}/venue/{tail}")
    }

    fn read(event: Uuid, tail: &str) -> ProductRequest {
        ProductRequest {
            user_id: Uuid::new_v4().to_string(),
            event_id: event.to_string(),
            http_method: "GET".into(),
            http_target: target(event, tail),
            ..Default::default()
        }
    }

    #[test]
    fn frozen_venue_routes_are_exact_and_closed() {
        let event = Uuid::new_v4();
        let id = Uuid::new_v4();
        for (method, tail, expected) in [
            (
                "GET",
                "location".to_owned(),
                VenueOperation::LocationSnapshot,
            ),
            (
                "PUT",
                "location/sharing".to_owned(),
                VenueOperation::SetLocationSharing,
            ),
            (
                "PUT",
                "location/fix".to_owned(),
                VenueOperation::PublishLocationFix,
            ),
            (
                "GET",
                format!("location/commands/{id}"),
                VenueOperation::LocationCommand(id),
            ),
            (
                "GET",
                "landmarks/lm-main/notes".to_owned(),
                VenueOperation::ListNotes,
            ),
            (
                "POST",
                "landmarks/lm-main/notes".to_owned(),
                VenueOperation::CreateNote,
            ),
            (
                "GET",
                format!("landmarks/lm-main/notes/{id}"),
                VenueOperation::GetNote(id),
            ),
            (
                "GET",
                format!("landmarks/lm-main/notes/{id}/replies"),
                VenueOperation::ListReplies(id),
            ),
            (
                "POST",
                format!("landmarks/lm-main/notes/{id}/replies"),
                VenueOperation::CreateReply(id),
            ),
            (
                "DELETE",
                format!("landmarks/lm-main/notes/{id}"),
                VenueOperation::WithdrawNote(id),
            ),
        ] {
            assert_eq!(
                route(event, method, &target(event, &tail)).unwrap(),
                expected,
                "{method} {tail}"
            );
        }
        for (method, tail) in [
            ("GET", "location/requests".to_owned()),
            ("DELETE", "landmarks/lm-main/notes/not-a-uuid".to_owned()),
            ("GET", "landmarks/bad%2Fid/notes".to_owned()),
            ("POST", "landmarks/lm-main/notes".to_owned()),
        ] {
            assert!(route(event, method, &target(event, &tail)).is_err());
        }
    }

    #[test]
    fn note_queries_are_closed_and_cursor_is_canonical() {
        let event = Uuid::new_v4();
        let id = Uuid::new_v4();
        let cursor = format!("{}.{}", chrono::Utc::now().timestamp_micros(), id);
        let valid = read(
            event,
            &format!("landmarks/lm-main/notes?limit=20&before={cursor}"),
        );
        assert!(request(&valid, Operation::Venue(VenueOperation::ListNotes)).is_ok());
        for tail in [
            "landmarks/lm-main/notes?limit=0",
            "landmarks/lm-main/notes?limit=51",
            "landmarks/lm-main/notes?limit=01",
            "landmarks/lm-main/notes?limit=1&limit=2",
            "landmarks/lm-main/notes?before=0.00000000-0000-0000-0000-000000000000",
            "landmarks/lm-main/notes?%6cimit=1",
            "landmarks/lm-main/notes?unknown=1",
        ] {
            let input = read(event, tail);
            assert!(
                request(&input, Operation::Venue(VenueOperation::ListNotes)).is_err(),
                "{tail}"
            );
        }
    }

    #[test]
    fn venue_writes_require_key_body_and_exact_target() {
        let event = Uuid::new_v4();
        let id = Uuid::new_v4();
        let mut input = ProductRequest {
            user_id: Uuid::new_v4().to_string(),
            event_id: event.to_string(),
            http_method: "POST".into(),
            http_target: target(event, "landmarks/lm-main/notes"),
            http_body: serde_json::to_vec(&serde_json::json!({
                "text": "hello",
                "audience": "EVERYONE",
                "expiry": "ONE_HOUR",
                "landmark_selection": {
                    "landmark_id": "lm-main",
                    "catalog_revision": 1,
                    "landmark_revision": 1
                }
            }))
            .unwrap(),
            content_type: "application/json".into(),
            idempotency_key: Uuid::new_v4().to_string(),
            ..Default::default()
        };
        assert!(request(&input, Operation::Venue(VenueOperation::CreateNote)).is_ok());
        input.idempotency_key.clear();
        assert!(request(&input, Operation::Venue(VenueOperation::CreateNote)).is_err());
        input.idempotency_key = Uuid::new_v4().to_string();
        input.http_target = target(event, &format!("landmarks/lm-other/notes"));
        assert!(request(&input, Operation::Venue(VenueOperation::CreateNote)).is_err());
        input.http_target = target(event, &format!("landmarks/lm-main/notes/{id}"));
        input.http_method = "DELETE".into();
        input.http_body.clear();
        input.content_type.clear();
        input.http_target.push_str("?expected_revision=1");
        assert!(request(&input, Operation::Venue(VenueOperation::WithdrawNote(id))).is_ok());
    }
}
