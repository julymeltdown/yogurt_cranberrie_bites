//! BE41 bounded owner progress. A checkpoint is correlation, never authority.
//! Persistence, current privileged authority, and owner receipt verification are
//! adapter responsibilities. Unknown downstream outcomes cannot advance a cursor.
use chrono::{DateTime, Utc};
use concert_domain::event_lifecycle::{Retention, Step};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub const MAX_BATCH: u16 = 100;
pub const MAX_CURSOR_BYTES: usize = 2048;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Owner {
    Mural,
    PublicSnapshot,
    SocialGrants,
    Beacons,
    World,
    Community,
    PublicExport,
    PrivateRetention,
    PostRetention,
    MediaRetention,
}
impl Owner {
    pub fn name(self) -> &'static str {
        match self {
            Self::Mural => "MURAL",
            Self::PublicSnapshot => "PUBLIC_SNAPSHOT",
            Self::SocialGrants => "SOCIAL_GRANTS",
            Self::Beacons => "BEACONS",
            Self::World => "WORLD",
            Self::Community => "COMMUNITY",
            Self::PublicExport => "PUBLIC_EXPORT",
            Self::PrivateRetention => "PRIVATE_RETENTION",
            Self::PostRetention => "POST_RETENTION",
            Self::MediaRetention => "MEDIA_RETENTION",
        }
    }
}
/// READ_ONLY itself must use the retained actual native/composite driver.
/// No configurable empty owner list can certify a later step.
pub fn required_owners(step: Step) -> &'static [Owner] {
    match step {
        Step::ReadOnly => &[],
        Step::FreezeMural => &[Owner::Mural],
        Step::Snapshot => &[Owner::PublicSnapshot],
        Step::ExpireGrants => &[Owner::SocialGrants],
        Step::ExpireBeacons => &[Owner::Beacons, Owner::World, Owner::Community],
        Step::Export => &[Owner::PublicExport],
        Step::Cleanup => &[
            Owner::PrivateRetention,
            Owner::PostRetention,
            Owner::MediaRetention,
            Owner::World,
            Owner::Community,
        ],
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scope {
    pub actor: Uuid,
    pub event: Uuid,
    pub job: Uuid,
    pub step: Step,
    pub owner: Owner,
}
impl Scope {
    pub fn validate(self) -> Result<Self, Error> {
        if [self.actor, self.event, self.job].iter().any(Uuid::is_nil)
            || !required_owners(self.step).contains(&self.owner)
        {
            return Err(Error::Invalid);
        }
        Ok(self)
    }
}
/// Opaque owner cursor. Never serialize into public progress, audit, or logs.
#[derive(Clone, PartialEq, Eq)]
pub struct Cursor(Vec<u8>);
impl Cursor {
    pub fn new(bytes: Vec<u8>) -> Result<Self, Error> {
        if bytes.is_empty() || bytes.len() > MAX_CURSOR_BYTES {
            return Err(Error::Invalid);
        }
        Ok(Self(bytes))
    }
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("Invalid cleanup input")]
    Invalid,
    #[error("Cleanup binding conflict")]
    Conflict,
    #[error("Cleanup source unavailable")]
    Unavailable,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Pending,
    PartialFailure,
    Complete,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    AuthorityUnavailable,
    OwnerUnavailable,
    OwnerDenied,
    SourceChanged,
}
#[derive(Clone)]
pub struct BatchRequest {
    scope: Scope,
    retention: Retention,
    ordinal: u64,
    cursor: Option<Cursor>,
    limit: u16,
    binding: [u8; 32],
    operation: Uuid,
}
impl BatchRequest {
    pub fn new(
        scope: Scope,
        retention: Retention,
        ordinal: u64,
        cursor: Option<Cursor>,
        limit: u16,
    ) -> Result<Self, Error> {
        scope.validate()?;
        retention.validate().map_err(|_| Error::Invalid)?;
        if !(1..=MAX_BATCH).contains(&limit)
            || ordinal > i64::MAX as u64
            || (ordinal == 0) != cursor.is_none()
        {
            return Err(Error::Invalid);
        }
        let mut hash = Sha256::new();
        hash.update(b"mcr-event-cleanup-batch-v1\0");
        hash.update(scope.actor.as_bytes());
        hash.update(scope.event.as_bytes());
        hash.update(scope.job.as_bytes());
        hash.update(scope.step.name());
        hash.update([0]);
        hash.update(scope.owner.name());
        hash.update([0]);
        hash.update(retention.policy_revision.to_be_bytes());
        hash.update(retention.closed_at.timestamp().to_be_bytes());
        hash.update(retention.closed_at.timestamp_subsec_nanos().to_be_bytes());
        hash.update(retention.cleanup_after.timestamp().to_be_bytes());
        hash.update(
            retention
                .cleanup_after
                .timestamp_subsec_nanos()
                .to_be_bytes(),
        );
        hash.update(ordinal.to_be_bytes());
        hash.update(limit.to_be_bytes());
        hash.update(Sha256::digest(
            cursor.as_ref().map_or(&[][..], Cursor::bytes),
        ));
        let binding: [u8; 32] = hash.finalize().into();
        let mut id: [u8; 16] = binding[..16].try_into().map_err(|_| Error::Invalid)?;
        id[6] = (id[6] & 15) | 0x50;
        id[8] = (id[8] & 63) | 0x80;
        Ok(Self {
            scope,
            retention,
            ordinal,
            cursor,
            limit,
            binding,
            operation: Uuid::from_bytes(id),
        })
    }
    pub fn scope(&self) -> Scope {
        self.scope
    }
    pub fn retention(&self) -> Retention {
        self.retention
    }
    pub fn ordinal(&self) -> u64 {
        self.ordinal
    }
    pub fn cursor(&self) -> Option<&Cursor> {
        self.cursor.as_ref()
    }
    pub fn limit(&self) -> u16 {
        self.limit
    }
    pub fn binding(&self) -> [u8; 32] {
        self.binding
    }
    pub fn operation(&self) -> Uuid {
        self.operation
    }
    pub fn due(&self, at: DateTime<Utc>) -> bool {
        self.scope.step != Step::Cleanup || at >= self.retention.cleanup_after
    }
}
/// Returned only by a trusted owning adapter after its exact durable receipt has
/// been verified. A nonnil UUID or affected-row count alone is not that proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Receipt {
    pub binding: [u8; 32],
    pub operation: Uuid,
    pub id: Uuid,
    pub revision: i64,
    pub processed: u16,
}
pub enum Outcome {
    Pending,
    Page { receipt: Receipt, next: Cursor },
    Converged { receipt: Receipt },
}
#[derive(Clone)]
pub struct Checkpoint {
    request: BatchRequest,
    state: State,
    processed: u64,
    last_receipt: Option<Receipt>,
    failure: Option<Failure>,
}
impl Checkpoint {
    pub fn new(request: BatchRequest) -> Result<Self, Error> {
        if request.ordinal != 0 {
            return Err(Error::Invalid);
        }
        Ok(Self {
            request,
            state: State::Pending,
            processed: 0,
            last_receipt: None,
            failure: None,
        })
    }
    /// Storage projection only. The adapter must also verify this head against
    /// its immutable batch ledger; these structural checks do not confer authority.
    pub fn restore(
        request: BatchRequest,
        state: State,
        processed: u64,
        last_receipt: Option<Receipt>,
        failure: Option<Failure>,
    ) -> Result<Self, Error> {
        let bad = processed > i64::MAX as u64
            || processed < request.ordinal
            || (state == State::PartialFailure) != failure.is_some()
            || (request.ordinal == 0
                && state != State::Complete
                && (processed != 0 || last_receipt.is_some()))
            || ((request.ordinal > 0 || state == State::Complete) && last_receipt.is_none());
        if bad {
            return Err(Error::Invalid);
        }
        if let Some(r) = last_receipt {
            if r.id.is_nil()
                || !(1..=concert_domain::MAX_REVISION).contains(&r.revision)
                || r.processed > request.limit
                || processed < u64::from(r.processed)
                || (state == State::Complete
                    && (r.binding != request.binding || r.operation != request.operation))
                || (state != State::Complete
                    && (r.processed == 0
                        || r.operation == request.operation
                        || r.binding == request.binding))
            {
                return Err(Error::Conflict);
            }
        }
        Ok(Self {
            request,
            state,
            processed,
            last_receipt,
            failure,
        })
    }
    pub fn request(&self) -> &BatchRequest {
        &self.request
    }
    pub fn state(&self) -> State {
        self.state
    }
    pub fn processed(&self) -> u64 {
        self.processed
    }
    pub fn last_receipt(&self) -> Option<Receipt> {
        self.last_receipt
    }
    pub fn failure(&self) -> Option<Failure> {
        self.failure
    }
    /// Failure retains the SAME operation/cursor; retry is never a new command.
    pub fn failed(&mut self, failure: Failure) -> Result<(), Error> {
        if self.state == State::Complete {
            return Err(Error::Conflict);
        }
        self.failure = Some(failure);
        self.state = State::PartialFailure;
        Ok(())
    }
    pub fn apply(&mut self, outcome: Outcome) -> Result<(), Error> {
        if self.state == State::Complete {
            return Err(Error::Conflict);
        }
        let (receipt, next) = match outcome {
            Outcome::Pending => {
                self.state = State::Pending;
                self.failure = None;
                return Ok(());
            }
            Outcome::Page { receipt, next } => (receipt, Some(next)),
            Outcome::Converged { receipt } => (receipt, None),
        };
        if receipt.binding != self.request.binding
            || receipt.operation != self.request.operation
            || receipt.id.is_nil()
            || !(1..=concert_domain::MAX_REVISION).contains(&receipt.revision)
            || receipt.processed > self.request.limit
            || self
                .last_receipt
                .is_some_and(|r| receipt.revision < r.revision)
            || next
                .as_ref()
                .is_some_and(|c| receipt.processed == 0 || Some(c) == self.request.cursor.as_ref())
        {
            return Err(Error::Conflict);
        }
        let processed = self
            .processed
            .checked_add(u64::from(receipt.processed))
            .filter(|count| *count <= i64::MAX as u64)
            .ok_or(Error::Invalid)?;
        let next_request = if let Some(cursor) = next {
            Some(BatchRequest::new(
                self.request.scope,
                self.request.retention,
                self.request.ordinal.checked_add(1).ok_or(Error::Invalid)?,
                Some(cursor),
                self.request.limit,
            )?)
        } else {
            None
        };
        self.processed = processed;
        self.last_receipt = Some(receipt);
        self.failure = None;
        self.state = if let Some(request) = next_request {
            self.request = request;
            State::Pending
        } else {
            State::Complete
        };
        Ok(())
    }
}
