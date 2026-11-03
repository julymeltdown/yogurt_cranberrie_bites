//! Compatibility names for the shared client failure contract.

pub use crate::client_error::{
    GatewayClientError as PostClientError, GatewayClientMessage as PostClientMessage,
    GatewayClientResult as PostClientResult,
    MAX_GATEWAY_CLIENT_MESSAGE_CHARS as MAX_POST_CLIENT_MESSAGE_CHARS,
};
