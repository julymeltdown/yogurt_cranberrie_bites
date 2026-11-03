use std::fmt;
use uuid::Uuid;
pub const MAX_SEQUENCE: i64 = 9_007_199_254_740_991;
pub const MAX_PAGE: u32 = 256;
pub const MAX_RESPONSE_BYTES: usize = 128 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ReadError {
    #[error("invalid invalidation query")]
    InvalidInput,
    #[error("invalidation cursor requires bootstrap")]
    FailedPrecondition,
    #[error("invalidation read cancelled")]
    Cancelled,
    #[error("invalidation read deadline elapsed")]
    DeadlineExceeded,
    #[error("invalidation journal unavailable")]
    Unavailable,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    epoch: Uuid,
    sequence: i64,
}
impl Cursor {
    pub fn new(epoch: Uuid, sequence: i64) -> Result<Self, ReadError> {
        if epoch.is_nil() || !(0..=MAX_SEQUENCE).contains(&sequence) {
            return Err(ReadError::InvalidInput);
        }
        Ok(Self { epoch, sequence })
    }
    pub fn epoch(self) -> Uuid {
        self.epoch
    }
    pub fn sequence(self) -> i64 {
        self.sequence
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct HeadQuery {
    consumer: Uuid,
    request: Uuid,
}
impl HeadQuery {
    pub fn new(consumer: Uuid, request: Uuid) -> Result<Self, ReadError> {
        if consumer.is_nil() || request.is_nil() {
            return Err(ReadError::InvalidInput);
        }
        Ok(Self { consumer, request })
    }
    pub fn consumer_id(self) -> Uuid {
        self.consumer
    }
    pub fn request_id(self) -> Uuid {
        self.request
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PageQuery {
    identity: HeadQuery,
    after: Cursor,
    limit: u32,
}
impl PageQuery {
    pub fn new(identity: HeadQuery, after: Cursor, limit: u32) -> Result<Self, ReadError> {
        if !(1..=MAX_PAGE).contains(&limit) {
            return Err(ReadError::InvalidInput);
        }
        Ok(Self {
            identity,
            after,
            limit,
        })
    }
    pub fn identity(self) -> HeadQuery {
        self.identity
    }
    pub fn after(self) -> Cursor {
        self.after
    }
    pub fn limit(self) -> u32 {
        self.limit
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    RelationshipChanged,
    UserRemoved,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Pair { first: Uuid, second: Uuid },
    User { user: Uuid },
}
#[derive(Clone, PartialEq, Eq)]
pub struct Signal {
    pub(crate) cursor: Cursor,
    pub(crate) id: Uuid,
    pub(crate) reason: Reason,
    pub(crate) scope: Scope,
}
impl Signal {
    pub fn cursor(&self) -> Cursor {
        self.cursor
    }
    pub fn id(&self) -> Uuid {
        self.id
    }
    pub fn reason(&self) -> Reason {
        self.reason
    }
    pub fn scope(&self) -> Scope {
        self.scope
    }
    pub(crate) fn validate(&self) -> Result<(), ReadError> {
        if self.id.is_nil() || self.cursor.sequence == 0 {
            return Err(ReadError::Unavailable);
        }
        match (self.reason, self.scope) {
            (Reason::RelationshipChanged, Scope::Pair { first, second })
                if !first.is_nil() && first < second =>
            {
                Ok(())
            }
            (Reason::UserRemoved, Scope::User { user }) if !user.is_nil() => Ok(()),
            _ => Err(ReadError::Unavailable),
        }
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct Head {
    pub(crate) query: HeadQuery,
    pub(crate) head: Cursor,
}
impl Head {
    pub fn query(&self) -> HeadQuery {
        self.query
    }
    pub fn cursor(&self) -> Cursor {
        self.head
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct Page {
    pub(crate) query: PageQuery,
    pub(crate) head: Cursor,
    pub(crate) next: Cursor,
    pub(crate) signals: Vec<Signal>,
}
impl Page {
    pub fn query(&self) -> PageQuery {
        self.query
    }
    pub fn head(&self) -> Cursor {
        self.head
    }
    pub fn next(&self) -> Cursor {
        self.next
    }
    pub fn signals(&self) -> &[Signal] {
        &self.signals
    }
    pub(crate) fn validate(&self) -> Result<(), ReadError> {
        if self.query.after.epoch != self.head.epoch
            || self.query.after.sequence > self.head.sequence
        {
            return Err(ReadError::FailedPrecondition);
        }
        let expected = (self.head.sequence - self.query.after.sequence)
            .min(i64::from(self.query.limit)) as usize;
        if self.signals.len() != expected {
            return Err(ReadError::Unavailable);
        }
        let mut ids = std::collections::BTreeSet::new();
        for (index, signal) in self.signals.iter().enumerate() {
            signal.validate()?;
            if signal.cursor.epoch != self.head.epoch
                || signal.cursor.sequence != self.query.after.sequence + index as i64 + 1
                || !ids.insert(signal.id)
            {
                return Err(ReadError::Unavailable);
            }
        }
        let next = self.signals.last().map_or(self.query.after, |s| s.cursor);
        if self.next != next {
            return Err(ReadError::Unavailable);
        }
        Ok(())
    }
}
macro_rules! redacted { ($($name:ty),+) => { $(impl fmt::Debug for $name { fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result {f.write_str(concat!(stringify!($name),"([REDACTED])"))} })+ }; }
redacted!(Cursor, HeadQuery, PageQuery, Scope, Signal, Head, Page);
