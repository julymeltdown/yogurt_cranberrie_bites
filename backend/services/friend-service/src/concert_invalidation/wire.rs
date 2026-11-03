use super::*;
use crate::adapters::r#in::grpc::concert_relationship::proto;
use prost::Message;
use uuid::Uuid;
fn uuid(value: &str) -> Result<Uuid, ReadError> {
    let id = Uuid::parse_str(value).map_err(|_| ReadError::InvalidInput)?;
    if id.is_nil() || id.to_string() != value {
        return Err(ReadError::InvalidInput);
    }
    Ok(id)
}
impl TryFrom<proto::ReadInvalidationHeadRequest> for HeadQuery {
    type Error = ReadError;
    fn try_from(q: proto::ReadInvalidationHeadRequest) -> Result<Self, Self::Error> {
        Self::new(uuid(&q.consumer_id)?, uuid(&q.request_id)?)
    }
}
impl TryFrom<proto::ReadInvalidationsRequest> for PageQuery {
    type Error = ReadError;
    fn try_from(q: proto::ReadInvalidationsRequest) -> Result<Self, Self::Error> {
        let after = q.after.ok_or(ReadError::InvalidInput)?;
        Self::new(
            HeadQuery::new(uuid(&q.consumer_id)?, uuid(&q.request_id)?)?,
            Cursor::new(uuid(&after.owner_epoch)?, after.sequence)?,
            q.limit,
        )
    }
}
impl From<Cursor> for proto::InvalidationCursor {
    fn from(c: Cursor) -> Self {
        Self {
            owner_epoch: c.epoch().to_string(),
            sequence: c.sequence(),
        }
    }
}
impl From<HeadQuery> for proto::ReadInvalidationHeadRequest {
    fn from(q: HeadQuery) -> Self {
        Self {
            consumer_id: q.consumer_id().to_string(),
            request_id: q.request_id().to_string(),
        }
    }
}
impl From<PageQuery> for proto::ReadInvalidationsRequest {
    fn from(q: PageQuery) -> Self {
        Self {
            consumer_id: q.identity().consumer_id().to_string(),
            request_id: q.identity().request_id().to_string(),
            after: Some(q.after().into()),
            limit: q.limit(),
        }
    }
}
impl Head {
    pub fn to_wire(&self) -> Result<proto::ReadInvalidationHeadResponse, ReadError> {
        let reply = proto::ReadInvalidationHeadResponse {
            query: Some(self.query.into()),
            head: Some(self.head.into()),
        };
        if reply.encoded_len() > MAX_RESPONSE_BYTES {
            Err(ReadError::Unavailable)
        } else {
            Ok(reply)
        }
    }
}
impl Page {
    pub fn to_wire(&self) -> Result<proto::ReadInvalidationsResponse, ReadError> {
        self.validate()?;
        let signals = self
            .signals
            .iter()
            .map(|s| {
                let (reason, scope) = match s.scope {
                    Scope::Pair { first, second } => (
                        proto::InvalidationReason::FriendRelationshipChanged,
                        proto::invalidation_signal::Scope::Pair(proto::InvalidationPairScope {
                            actor_id: first.to_string(),
                            subject_id: second.to_string(),
                        }),
                    ),
                    Scope::User { user } => (
                        proto::InvalidationReason::FriendUserRemoved,
                        proto::invalidation_signal::Scope::User(proto::InvalidationUserScope {
                            user_id: user.to_string(),
                        }),
                    ),
                };
                proto::InvalidationSignal {
                    cursor: Some(s.cursor.into()),
                    signal_id: s.id.to_string(),
                    reason: reason as i32,
                    scope: Some(scope),
                }
            })
            .collect();
        let reply = proto::ReadInvalidationsResponse {
            query: Some(self.query.into()),
            next: Some(self.next.into()),
            head: Some(self.head.into()),
            signals,
        };
        if reply.encoded_len() > MAX_RESPONSE_BYTES {
            Err(ReadError::Unavailable)
        } else {
            Ok(reply)
        }
    }
}
impl From<ReadError> for tonic::Status {
    fn from(e: ReadError) -> Self {
        match e {
            ReadError::InvalidInput => Self::invalid_argument(e.to_string()),
            ReadError::FailedPrecondition => Self::failed_precondition(e.to_string()),
            ReadError::Cancelled => Self::cancelled(e.to_string()),
            ReadError::DeadlineExceeded => Self::deadline_exceeded(e.to_string()),
            ReadError::Unavailable => Self::unavailable(e.to_string()),
        }
    }
}
