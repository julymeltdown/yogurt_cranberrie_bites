//! Newly authored BE34/35 values. Public projections contain no authors or moderation data.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const SIDE: usize = 128;
pub const PIXELS: usize = SIDE * SIDE;
pub const MAX_DELTA: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlacePixel {
    pub expected_revision: i64,
    pub x: i32,
    pub y: i32,
    pub color_index: i32,
}
impl PlacePixel {
    /// This value represents a single explicitly confirmed command, never pointer movement.
    pub fn valid(self) -> bool {
        (1..=crate::MAX_REVISION).contains(&self.expected_revision)
            && (0..128).contains(&self.x)
            && (0..128).contains(&self.y)
            && (0..16).contains(&self.color_index)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MuralState {
    Editable,
    Paused,
    Frozen,
}
impl MuralState {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "EDITABLE" => Some(Self::Editable),
            "PAUSED" => Some(Self::Paused),
            "FROZEN" => Some(Self::Frozen),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicPixelOperation {
    pub operation_id: Uuid,
    pub event_id: Uuid,
    pub revision: i64,
    pub x: i32,
    pub y: i32,
    pub color_index: i32,
    pub committed_at: DateTime<Utc>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PixelOperation {
    #[serde(flatten)]
    pub public: PublicPixelOperation,
    pub next_allowed_at: DateTime<Utc>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct MuralSnapshot {
    pub snapshot_id: Uuid,
    pub event_id: Uuid,
    pub revision: i64,
    pub state: MuralState,
    pub width: usize,
    pub height: usize,
    pub palette: Vec<String>,
    pub pixels: Vec<u8>,
    pub created_at: DateTime<Utc>,
    pub server_time: DateTime<Utc>,
    pub cooldown_ms: i64,
    /// Caller-only value. Never broadcast one caller's snapshot to another actor.
    pub next_allowed_at: Option<DateTime<Utc>>,
}
impl MuralSnapshot {
    pub fn valid(&self) -> bool {
        !self.snapshot_id.is_nil()
            && !self.event_id.is_nil()
            && (1..=crate::MAX_REVISION).contains(&self.revision)
            && self.width == SIDE
            && self.height == SIDE
            && self.pixels.len() == PIXELS
            && self.pixels.iter().all(|v| *v < 16)
            && self.palette.len() == 16
            && self.palette.iter().all(|color| {
                color.len() == 7
                    && color.starts_with('#')
                    && color.as_bytes()[1..]
                        .iter()
                        .all(|c| c.is_ascii_digit() || (b'A'..=b'F').contains(c))
            })
            && (0..=3600000).contains(&self.cooldown_ms)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct MuralDelta {
    pub event_id: Uuid,
    pub from_revision: i64,
    pub to_revision: i64,
    pub reset_required: bool,
    pub operations: Vec<PublicPixelOperation>,
}
impl MuralDelta {
    pub fn reset(event_id: Uuid, from_revision: i64, to_revision: i64) -> Self {
        Self {
            event_id,
            from_revision,
            to_revision,
            reset_required: true,
            operations: vec![],
        }
    }
    pub fn contiguous(&self) -> bool {
        !self.reset_required
            && self.from_revision >= 1
            && self.to_revision >= self.from_revision
            && self.operations.len() <= MAX_DELTA
            && self.to_revision - self.from_revision == self.operations.len() as i64
            && self.operations.iter().enumerate().all(|(n, op)| {
                !op.operation_id.is_nil()
                    && op.event_id == self.event_id
                    && op.revision == self.from_revision + n as i64 + 1
                    && PlacePixel {
                        expected_revision: op.revision,
                        x: op.x,
                        y: op.y,
                        color_index: op.color_index,
                    }
                    .valid()
            })
    }
}
