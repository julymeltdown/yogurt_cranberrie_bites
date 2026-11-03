use super::{super::grpc::proto, model::*};
use prost::Message;
use uuid::Uuid;

fn canonical(value: &str) -> Result<Uuid, ReadError> {
    let id = Uuid::parse_str(value).map_err(|_| ReadError::MalformedResponse)?;
    if id.is_nil() || id.to_string() != value {
        return Err(ReadError::MalformedResponse);
    }
    Ok(id)
}
fn cursor(
    owner: InvalidationOwner,
    value: Option<proto::InvalidationCursor>,
) -> Result<OwnerCursor, ReadError> {
    let value = value.ok_or(ReadError::MalformedResponse)?;
    OwnerCursor::new(owner, canonical(&value.owner_epoch)?, value.sequence)
        .map_err(|_| ReadError::MalformedResponse)
}
fn metadata<T: Message>(response: tonic::Response<T>) -> Result<T, ReadError> {
    if response
        .metadata()
        .get("cache-control")
        .and_then(|v| v.to_str().ok())
        != Some("no-store")
    {
        return Err(ReadError::MalformedResponse);
    }
    let value = response.into_inner();
    if value.encoded_len() > MAX_RESPONSE_BYTES {
        return Err(ReadError::ResponseTooLarge);
    }
    Ok(value)
}
pub(super) fn head(
    owner: InvalidationOwner,
    expected: proto::ReadInvalidationHeadRequest,
    response: tonic::Response<proto::ReadInvalidationHeadResponse>,
) -> Result<VerifiedOwnerHead, ReadError> {
    let value = metadata(response)?;
    if value.query.as_ref() != Some(&expected) {
        return Err(ReadError::QueryMismatch);
    }
    let position = cursor(owner, value.head)?;
    Ok(VerifiedOwnerHead {
        query: ReplayQuery {
            consumer: canonical(&expected.consumer_id)?,
            request: canonical(&expected.request_id)?,
            after: None,
            limit: None,
        },
        cursor: position,
    })
}
fn signal(
    owner: InvalidationOwner,
    value: proto::InvalidationSignal,
) -> Result<VerifiedSignal, ReadError> {
    use proto::{invalidation_signal::Scope, InvalidationReason as Reason};
    let position = cursor(owner, value.cursor)?;
    if position.sequence() == 0 {
        return Err(ReadError::MalformedResponse);
    }
    let id = canonical(&value.signal_id)?;
    let reason = Reason::try_from(value.reason).map_err(|_| ReadError::MalformedResponse)?;
    let (reason, scope) = match (owner, reason, value.scope) {
        (InvalidationOwner::Auth, Reason::AuthSessionChanged, Some(Scope::Family(f))) => (
            SignalReason::AuthSessionChanged,
            SignalScope::Family {
                user: canonical(&f.user_id)?,
                family: canonical(&f.session_family_id)?,
            },
        ),
        (InvalidationOwner::Auth, Reason::AuthUserAuthorityChanged, Some(Scope::User(u))) => (
            SignalReason::AuthUserAuthorityChanged,
            SignalScope::User {
                user: canonical(&u.user_id)?,
            },
        ),
        (InvalidationOwner::Auth, Reason::AuthCredentialsChanged, Some(Scope::User(u))) => (
            SignalReason::AuthCredentialsChanged,
            SignalScope::User {
                user: canonical(&u.user_id)?,
            },
        ),
        (InvalidationOwner::Auth, Reason::AuthUserRemoved, Some(Scope::User(u))) => (
            SignalReason::AuthUserRemoved,
            SignalScope::User {
                user: canonical(&u.user_id)?,
            },
        ),
        (InvalidationOwner::Friend, Reason::FriendUserRemoved, Some(Scope::User(u))) => (
            SignalReason::FriendUserRemoved,
            SignalScope::User {
                user: canonical(&u.user_id)?,
            },
        ),
        (InvalidationOwner::Friend, Reason::FriendRelationshipChanged, Some(Scope::Pair(p))) => {
            let actor = canonical(&p.actor_id)?;
            let subject = canonical(&p.subject_id)?;
            if actor == subject {
                return Err(ReadError::MalformedResponse);
            }
            (
                SignalReason::FriendRelationshipChanged,
                SignalScope::Pair { actor, subject },
            )
        }
        (
            InvalidationOwner::Auth,
            Reason::FriendRelationshipChanged | Reason::FriendUserRemoved,
            _,
        )
        | (
            InvalidationOwner::Friend,
            Reason::AuthSessionChanged
            | Reason::AuthUserAuthorityChanged
            | Reason::AuthCredentialsChanged
            | Reason::AuthUserRemoved,
            _,
        ) => return Err(ReadError::OwnerMismatch),
        _ => return Err(ReadError::MalformedResponse),
    };
    Ok(VerifiedSignal {
        cursor: position,
        id,
        reason,
        scope,
    })
}
pub(super) fn page(
    owner: InvalidationOwner,
    expected: proto::ReadInvalidationsRequest,
    response: tonic::Response<proto::ReadInvalidationsResponse>,
) -> Result<VerifiedOwnerPage, ReadError> {
    let value = metadata(response)?;
    if value.query.as_ref() != Some(&expected) {
        return Err(ReadError::QueryMismatch);
    }
    let after = cursor(owner, expected.after)?;
    let head = cursor(owner, value.head)?;
    let next = cursor(owner, value.next)?;
    if head.epoch() != after.epoch() || next.epoch() != after.epoch() {
        return Err(ReadError::EpochChanged);
    }
    if after.sequence() > head.sequence() || next.sequence() > head.sequence() {
        return Err(ReadError::FutureCursor);
    }
    if value.signals.len() > expected.limit as usize {
        return Err(ReadError::MalformedResponse);
    }
    let mut sequence = after.sequence();
    let mut signals = Vec::with_capacity(value.signals.len());
    let mut seen = std::collections::HashMap::<Uuid, VerifiedSignal>::new();
    for raw in value.signals {
        let s = signal(owner, raw)?;
        if s.cursor().epoch() != after.epoch() {
            return Err(ReadError::EpochChanged);
        }
        if let Some(old) = seen.get(&s.signal_id()) {
            return Err(if old == &s {
                ReadError::DuplicateSignal
            } else {
                ReadError::ConflictingSignal
            });
        }
        sequence = sequence
            .checked_add(1)
            .ok_or(ReadError::MalformedResponse)?;
        if s.cursor().sequence() != sequence {
            return Err(ReadError::SequenceGap);
        }
        seen.insert(s.signal_id(), s.clone());
        signals.push(s);
    }
    if next.sequence() != sequence {
        return Err(ReadError::SequenceGap);
    }
    if signals.len() < expected.limit as usize && next != head {
        return Err(ReadError::SequenceGap);
    }
    Ok(VerifiedOwnerPage {
        query: ReplayQuery {
            consumer: canonical(&expected.consumer_id)?,
            request: canonical(&expected.request_id)?,
            after: Some(after),
            limit: Some(expected.limit as u16),
        },
        next,
        head,
        signals,
    })
}

#[cfg(test)]
mod tests;
