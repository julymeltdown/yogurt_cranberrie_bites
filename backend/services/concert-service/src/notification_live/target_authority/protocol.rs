//! Decode only the exact original parent and ordered typed snapshot.
use super::{proto, unavailable};
use crate::adapters::out::relationship_authority::guarded::OwnerGuardScope;
use chrono::{DateTime, Utc};
use concert_application::notification::{Candidate, Context};
use concert_domain::notification::{Target, TargetType};
use prost::Message;
use sha2::{Digest, Sha256};
use tonic::Status;
use uuid::Uuid;

pub(super) fn uuid(value: &str) -> Result<Uuid, Status> {
    Uuid::parse_str(value)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == value)
        .ok_or_else(|| Status::invalid_argument("Invalid target authority identifier"))
}
pub(super) fn decode(
    input: &proto::TargetAuthorityRequest,
    scope: &OwnerGuardScope,
) -> Result<(Context, Vec<Candidate>, [u8; 32]), Status> {
    if input.encoded_len() > super::MAX_FRAME {
        return Err(Status::resource_exhausted(
            "Target snapshot exceeds callback frame",
        ));
    }
    uuid(&input.call_id)?;
    let parent = input.parent.as_ref().ok_or_else(unavailable)?;
    if parent.binding.as_ref() != Some(&scope.wire_binding())
        || parent.root_view.as_ref() != Some(&scope.root_read_view())
        || parent.auth_view.as_ref() != Some(&scope.auth_view())
        || parent.friend_view.as_ref() != Some(&scope.friend_view())
    {
        return Err(Status::permission_denied("Target callback parent mismatch"));
    }
    let b = scope.binding();
    let operation =
        super::super::NotificationOperation::route(b.event(), b.http_method(), b.http_target())?;
    if operation.id() != b.operation()
        || scope.wire_binding().parent_method != "/concert.product.v1.ProductService/Dispatch"
    {
        return Err(Status::permission_denied(
            "Target callback requires original Inbox operation",
        ));
    }
    let c = input.context.as_ref().ok_or_else(unavailable)?;
    let context = Context {
        actor_id: uuid(&c.actor_id)?,
        event_id: uuid(&c.event_id)?,
        session_family_id: uuid(&c.session_family_id)?,
        request_id: uuid(&c.request_id)?,
        authority_digest: c
            .authority_digest
            .as_slice()
            .try_into()
            .map_err(|_| unavailable())?,
        observed_at: DateTime::from_timestamp_micros(c.observed_at_unix_micros)
            .ok_or_else(unavailable)?,
        expires_at: DateTime::from_timestamp_micros(c.expires_at_unix_micros)
            .ok_or_else(unavailable)?,
    };
    context.check(Utc::now()).map_err(|_| unavailable())?;
    if context.actor_id != b.actor()
        || context.event_id != b.event()
        || context.session_family_id != b.session_family()
        || context.request_id != b.request_id()
        || context.authority_digest != b.digest()
        || c.expires_at_unix_micros != b.deadline().expires_micros()
    {
        return Err(Status::permission_denied(
            "Target callback context mismatch",
        ));
    }
    let candidates = input
        .candidates
        .iter()
        .map(|c| {
            let target = c.target.as_ref().ok_or_else(unavailable)?;
            let target_type =
                match proto::TargetType::try_from(target.target_type).map_err(|_| unavailable())? {
                    proto::TargetType::TargetParty => TargetType::Party,
                    proto::TargetType::Application => TargetType::Application,
                    proto::TargetType::TargetGiveaway => TargetType::Giveaway,
                    proto::TargetType::Reservation => TargetType::Reservation,
                    proto::TargetType::SocialRequest => TargetType::SocialRequest,
                    proto::TargetType::TargetNotice => TargetType::Notice,
                    // No source owner means unknown; never infer deletion/staleness.
                    _ => return Err(unavailable()),
                };
            Ok(Candidate {
                notification_id: uuid(&c.notification_id)?,
                source_actor_id: c.source_actor_id.as_deref().map(uuid).transpose()?,
                target: Some(Target {
                    target_type,
                    target_id: uuid(&target.target_id)?,
                }),
            })
        })
        .collect::<Result<Vec<_>, Status>>()?;
    Ok((
        context,
        candidates,
        Sha256::digest(input.encode_to_vec()).into(),
    ))
}
