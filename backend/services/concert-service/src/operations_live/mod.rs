//! BE36/BE38 request-bound bridges over the retained transactional producers.
//! Root retains canonical owner scopes through final HTTP delivery. This module
//! never acquires or completes a scope, creates operator roles, or publishes HTTP.
pub mod audience;
mod authority;
pub mod cursor;
mod owners;
mod roles;
mod runtime;
pub mod settlement;
pub mod report_product;
pub mod wire;

pub use owners::{OperationsOwners, ScopedOperationsRoles, ScopedReportEvidence};
pub use roles::{PgOperationsReview, PgOperationsRoles};
pub use runtime::{OperationsError, OperationsRuntime};
pub use wire::{OperationsReply, OperationsRequest};

use concert_application::commands::CommandError;
use tonic::{Code, Status};

fn unavailable(_: impl std::fmt::Display) -> CommandError {
    CommandError::Unavailable
}
fn authority_error(status: Status) -> CommandError {
    match status.code() {
        Code::Unauthenticated | Code::PermissionDenied => CommandError::Denied,
        _ => CommandError::Unavailable,
    }
}
