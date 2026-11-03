use platform::grpc_auth::Workload;
use std::fmt;
use uuid::Uuid;

pub const MAX_CURSOR: i64 = 9_007_199_254_740_991;
pub const MAX_PAGE_SIZE: u16 = 256;
pub const MAX_REQUEST_BYTES: usize = 4096;
pub const MAX_RESPONSE_BYTES: usize = 128 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InvalidationOwner {
    Auth,
    Friend,
}
impl InvalidationOwner {
    pub(super) fn workload(self) -> Workload {
        match self {
            Self::Auth => Workload::Auth,
            Self::Friend => Workload::Friend,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadError {
    InvalidConfiguration,
    InvalidInput,
    Cancelled,
    DeadlineExceeded,
    Unauthenticated,
    PermissionDenied,
    CheckpointMismatch,
    OwnerMismatch,
    QueryMismatch,
    EpochChanged,
    FutureCursor,
    SequenceGap,
    DuplicateSignal,
    ConflictingSignal,
    MalformedResponse,
    ResponseTooLarge,
    Unavailable,
}
impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invalidation read failed: {self:?}")
    }
}
impl std::error::Error for ReadError {}

/// A caller-restored position is input, never authenticated owner evidence.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct OwnerCursor {
    owner: InvalidationOwner,
    epoch: Uuid,
    sequence: i64,
}
impl OwnerCursor {
    pub fn new(owner: InvalidationOwner, epoch: Uuid, sequence: i64) -> Result<Self, ReadError> {
        if epoch.is_nil() || !(0..=MAX_CURSOR).contains(&sequence) {
            return Err(ReadError::InvalidInput);
        }
        Ok(Self {
            owner,
            epoch,
            sequence,
        })
    }
    pub fn owner(&self) -> InvalidationOwner {
        self.owner
    }
    pub fn epoch(&self) -> Uuid {
        self.epoch
    }
    pub fn sequence(&self) -> i64 {
        self.sequence
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct ReplayQuery {
    pub(super) consumer: Uuid,
    pub(super) request: Uuid,
    pub(super) after: Option<OwnerCursor>,
    pub(super) limit: Option<u16>,
}
impl ReplayQuery {
    pub fn consumer_id(&self) -> Uuid {
        self.consumer
    }
    pub fn request_id(&self) -> Uuid {
        self.request
    }
    pub fn after(&self) -> Option<OwnerCursor> {
        self.after
    }
    pub fn limit(&self) -> Option<u16> {
        self.limit
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignalReason {
    AuthSessionChanged,
    AuthUserAuthorityChanged,
    AuthCredentialsChanged,
    AuthUserRemoved,
    FriendRelationshipChanged,
    FriendUserRemoved,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SignalScope {
    User { user: Uuid },
    Family { user: Uuid, family: Uuid },
    Pair { actor: Uuid, subject: Uuid },
}

/// Only the sibling authenticated client/validation modules construct these.
/// There is no public deserializer, raw/protobuf conversion or unchecked factory.
#[derive(Clone, PartialEq, Eq)]
pub struct VerifiedSignal {
    pub(super) cursor: OwnerCursor,
    pub(super) id: Uuid,
    pub(super) reason: SignalReason,
    pub(super) scope: SignalScope,
}
impl VerifiedSignal {
    pub fn owner(&self) -> InvalidationOwner {
        self.cursor.owner()
    }
    pub fn cursor(&self) -> OwnerCursor {
        self.cursor
    }
    pub fn signal_id(&self) -> Uuid {
        self.id
    }
    pub fn reason(&self) -> SignalReason {
        self.reason
    }
    pub fn scope(&self) -> SignalScope {
        self.scope
    }
    /// Consumers retaining an earlier sealed record can detect conflicting
    /// redelivery before recording effects. This performs no dedup/checkpoint I/O.
    pub fn check_redelivery(&self, previous: &Self) -> Result<(), ReadError> {
        if self.owner() != previous.owner() {
            return Err(ReadError::OwnerMismatch);
        }
        if self.id != previous.id {
            return Err(ReadError::InvalidInput);
        }
        if self != previous {
            return Err(ReadError::ConflictingSignal);
        }
        Ok(())
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct VerifiedOwnerHead {
    pub(super) query: ReplayQuery,
    pub(super) cursor: OwnerCursor,
}
impl VerifiedOwnerHead {
    pub fn owner(&self) -> InvalidationOwner {
        self.cursor.owner()
    }
    pub fn consumer_id(&self) -> Uuid {
        self.query.consumer_id()
    }
    pub fn query(&self) -> &ReplayQuery {
        &self.query
    }
    pub fn cursor(&self) -> OwnerCursor {
        self.cursor
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct VerifiedOwnerPage {
    pub(super) query: ReplayQuery,
    pub(super) next: OwnerCursor,
    pub(super) head: OwnerCursor,
    pub(super) signals: Vec<VerifiedSignal>,
}
impl VerifiedOwnerPage {
    pub fn owner(&self) -> InvalidationOwner {
        self.head.owner()
    }
    pub fn consumer_id(&self) -> Uuid {
        self.query.consumer_id()
    }
    pub fn query(&self) -> &ReplayQuery {
        &self.query
    }
    pub fn next(&self) -> OwnerCursor {
        self.next
    }
    pub fn head(&self) -> OwnerCursor {
        self.head
    }
    pub fn signals(&self) -> &[VerifiedSignal] {
        &self.signals
    }
    pub fn has_more(&self) -> bool {
        self.next.sequence() < self.head.sequence()
    }
}
macro_rules! redacted {($($ty:ty),+)=>{$(impl fmt::Debug for $ty {fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result {f.write_str(concat!(stringify!($ty),"([REDACTED])"))}})+};}
redacted!(
    OwnerCursor,
    ReplayQuery,
    SignalScope,
    VerifiedSignal,
    VerifiedOwnerHead,
    VerifiedOwnerPage
);
