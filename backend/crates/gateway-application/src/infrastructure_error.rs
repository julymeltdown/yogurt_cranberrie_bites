//! Controlled infrastructure failures without driver, codec or diagnostic payloads.
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GatewayInfrastructureError {
    Storage,
    Encoding,
    Unexpected,
}
pub type GatewayInfrastructureResult<T> = Result<T, GatewayInfrastructureError>;
impl fmt::Display for GatewayInfrastructureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Storage => "Gateway storage operation failed",
            Self::Encoding => "Gateway data encoding operation failed",
            Self::Unexpected => "Gateway infrastructure operation failed",
        })
    }
}
impl std::error::Error for GatewayInfrastructureError {}
