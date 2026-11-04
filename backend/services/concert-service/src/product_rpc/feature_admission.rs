//! Closed Intro/Notification decoding before ProductOwner dispatch. Admission
//! supplies no canonical owner, target policy, delivery capability or renewal.
use super::{canonical_uuid, proto::ProductRequest, Operation};
use crate::{intro_live::protocol, notification_live::NotificationRequest};
use concert_domain::{AuthUserId, EventId};
use tonic::Status;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntroOperation {
    List,
    Get(Uuid),
    Own,
    Set,
    Publish,
    Close,
}
impl IntroOperation {
    pub fn id(self) -> &'static str {
        match self {
            Self::List => "listConcertIntros",
            Self::Get(_) => "getConcertIntro",
            Self::Own => "getOwnConcertIntro",
            Self::Set => "setConcertIntro",
            Self::Publish => "publishConcertIntro",
            Self::Close => "closeConcertIntro",
        }
    }
    pub fn mutation(self) -> bool {
        matches!(self, Self::Set | Self::Publish | Self::Close)
    }
}

pub(super) fn intro_route(
    event: Uuid,
    method: &str,
    target: &str,
) -> Result<IntroOperation, Status> {
    match protocol::parse_intro_target(event, method, target).map_err(|_| invalid())? {
        ("listConcertIntros", None) => Ok(IntroOperation::List),
        ("getConcertIntro", Some(id)) => Ok(IntroOperation::Get(id)),
        ("getOwnConcertIntro", None) => Ok(IntroOperation::Own),
        ("setConcertIntro", None) => Ok(IntroOperation::Set),
        ("publishConcertIntro", None) => Ok(IntroOperation::Publish),
        ("closeConcertIntro", None) => Ok(IntroOperation::Close),
        _ => Err(invalid()),
    }
}

pub(super) fn request(request: &ProductRequest, expected: Operation) -> Result<(), Status> {
    if request
        .http_target
        .split_once('?')
        .is_some_and(|(_, query)| query.is_empty())
    {
        return Err(invalid());
    }
    match expected {
        Operation::Intro(operation) => intro_request(request, operation),
        Operation::Notification(operation)
            if NotificationRequest::parse(request)?.operation() == operation =>
        {
            Ok(())
        }
        _ => Err(invalid()),
    }
}

fn intro_request(request: &ProductRequest, expected: IntroOperation) -> Result<(), Status> {
    let actor = canonical_uuid(&request.user_id)?;
    let event = canonical_uuid(&request.event_id)?;
    if intro_route(event, &request.http_method, &request.http_target)? != expected {
        return Err(invalid());
    }
    if !expected.mutation() {
        if !request.http_body.is_empty()
            || !request.content_type.is_empty()
            || !request.idempotency_key.is_empty()
        {
            return Err(invalid());
        }
        if expected == IntroOperation::List {
            // Use the existing strict form decoder for lexical control checks;
            // the Intro parser still owns fields, duplicates and actual bounds.
            if let Some((_, query)) = request.http_target.split_once('?') {
                for pair in query.split('&') {
                    let (name, value) = pair.split_once('=').ok_or_else(invalid)?;
                    let name = super::query::decode(name)?;
                    let value = super::query::decode(value)?;
                    // The owner trims q; enforce the wire maximum before that
                    // normalization as required by the frozen query schema.
                    if name == "q" && value.chars().count() > 100 {
                        return Err(invalid());
                    }
                }
            }
            protocol::parse_query(&request.http_target).map_err(|_| invalid())?;
        }
        return Ok(());
    }
    if request.content_type != "application/json" || request.http_body.len() > 16 * 1024 {
        return Err(invalid());
    }
    let key = canonical_uuid(&request.idempotency_key)?;
    // Serde structs can also decode sequences. Require the frozen C04 object
    // shapes without normalizing bytes or losing duplicate-field detection in
    // the real typed command parser below.
    let shape: serde_json::Value =
        serde_json::from_slice(&request.http_body).map_err(|_| invalid())?;
    if !shape.is_object()
        || (expected == IntroOperation::Set
            && (!shape.get("age_bounds").is_some_and(|v| v.is_object())
                || shape
                    .get("expires_at")
                    .and_then(|v| v.as_str())
                    .is_some_and(|v| !v.ends_with('Z'))))
    {
        return Err(invalid());
    }
    protocol::parse_intro_command(
        AuthUserId::from_uuid(actor),
        EventId::from_uuid(event),
        key,
        expected.id(),
        &request.http_body,
    )
    .map_err(|_| invalid())?;
    Ok(())
}
fn invalid() -> Status {
    Status::invalid_argument("Invalid Concert feature request")
}
