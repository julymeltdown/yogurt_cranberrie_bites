//! Captured Product HTTP input shared by the route and signed dispatch adapter.

use uuid::Uuid;

pub const MAX_BODY: usize = 10 * 1024 * 1024;

pub struct ProductHttpRequest {
    pub client_address: Option<crate::adapters::r#in::http::client_identity::VerifiedClientAddress>,
    pub event: Uuid,
    pub request_id: Uuid,
    pub method: String,
    pub target: String,
    pub body: Vec<u8>,
    pub content_type: String,
    pub idempotency_key: String,
}
