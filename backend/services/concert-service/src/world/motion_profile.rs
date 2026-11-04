//! Optional C05 motion metadata. It describes the actual serialized simulation;
//! it grants neither membership nor client coordinate authority.
use super::simulation::{Binding, MAX_CATCH_UP_STEPS};
use concert_domain::world::{Limits, Position, STEP_MS};
use serde::Serialize;

pub const VERSION: &str = "1.0.0";

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MotionProfile {
    version: &'static str,
    geometry_hash: String,
    step_ms: u64,
    speed_px_per_second: f64,
    hold_ms: u64,
    max_catch_up_steps: u64,
    axes: &'static str,
    collision: &'static str,
    catch_up: &'static str,
    hold_boundary: &'static str,
}
impl MotionProfile {
    /// The production caller supplies the hash from its validated C06 release.
    pub(crate) fn new(geometry_hash: &str, limits: Limits) -> Option<Self> {
        let hash = geometry_hash.strip_prefix("sha256:")?;
        if hash.len() != 64
            || !hash
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        {
            return None;
        }
        Some(Self {
            version: VERSION,
            geometry_hash: geometry_hash.to_owned(),
            step_ms: STEP_MS,
            speed_px_per_second: limits.distance_per_step() * 1000.0 / STEP_MS as f64,
            hold_ms: limits.input_hold_ms(),
            max_catch_up_steps: MAX_CATCH_UP_STEPS,
            axes: "UNIT_DISK_ANALOG",
            collision: "POINT_SUPERCOVER_STOP",
            catch_up: "DISCARD_OLDEST_DUE_STEPS",
            hold_boundary: "AGE_LE_HOLD_MS",
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeldInput {
    pub seq: u64,
    pub axis_x: f64,
    pub axis_y: f64,
    pub remaining_hold_ms: u64,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MotionAnchor {
    pub server_tick: u64,
    pub position: MotionPosition,
    pub consumed_input_seq: u64,
    pub input_applied_ticks: u64,
    pub dropped_ticks: u64,
    pub held_input: Option<HeldInput>,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct MotionPosition {
    pub x: f64,
    pub y: f64,
}
impl From<Position> for MotionPosition {
    fn from(p: Position) -> Self {
        Self { x: p.x, y: p.y }
    }
}
#[derive(Clone, Debug)]
pub(crate) struct MotionCapture {
    pub binding: Binding,
    pub profile: MotionProfile,
    pub anchor: MotionAnchor,
}
