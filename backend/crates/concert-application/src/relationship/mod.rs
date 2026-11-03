//! Shared HTTP, world and projection policy port. Implementations resolve fresh authority.
use async_trait::async_trait;
pub use concert_domain::relationship::{
    Field, PolicyDecision, PolicyQuery, PolicyStamp, RelationshipError,
};

#[async_trait]
pub trait RelationshipPolicy: Send + Sync {
    /// No cached allow fallback. Mutation adapters must use a transactionally fenced
    /// implementation through commit, not a previously returned PolicyDecision.
    async fn authorize(&self, query: PolicyQuery) -> Result<PolicyDecision, RelationshipError>;
}
/// Safe production default until real owner authority is bound by composition.
pub struct UnavailableRelationshipPolicy;
#[async_trait]
impl RelationshipPolicy for UnavailableRelationshipPolicy {
    async fn authorize(&self, _: PolicyQuery) -> Result<PolicyDecision, RelationshipError> {
        Err(RelationshipError::Unavailable)
    }
}

pub mod social;
