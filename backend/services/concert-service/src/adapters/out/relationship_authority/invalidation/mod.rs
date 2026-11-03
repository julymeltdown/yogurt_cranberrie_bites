//! Authenticated owner replay only. No World effects, grants or acknowledgments.
mod budget;
mod client;
mod model;
mod validation;

pub use budget::{ReplayBudget, ReplayCancellation, MAX_OPERATION};
pub use client::InvalidationSource;
pub use model::{
    InvalidationOwner, OwnerCursor, ReadError, ReplayQuery, SignalReason, SignalScope,
    VerifiedOwnerHead, VerifiedOwnerPage, VerifiedSignal, MAX_CURSOR, MAX_PAGE_SIZE,
    MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES,
};
