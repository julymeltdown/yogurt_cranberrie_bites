use crate::application::ports::GatewayRuntimePort;
use chrono::{DateTime, Utc};
use std::time::Instant;
use uuid::Uuid;

/// System clock and UUID values composed explicitly outside the application.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemGatewayRuntime;

impl GatewayRuntimePort for SystemGatewayRuntime {
    fn utc_now(&self) -> DateTime<Utc> {
        Utc::now()
    }
    fn monotonic_now(&self) -> Instant {
        Instant::now()
    }
    fn new_id(&self) -> Uuid {
        Uuid::new_v4()
    }
}
