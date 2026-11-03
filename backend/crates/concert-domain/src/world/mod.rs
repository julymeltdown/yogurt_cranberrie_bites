//! BE12 pure virtual movement. No network, database, physical-arrival or client-position authority.
use crate::place::Geometry;
use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};
use uuid::Uuid;

pub const STEP_MS: u64 = 50;
pub const MAX_SEQUENCE: u64 = 9_007_199_254_740_991;
pub const MAX_GRID_CELLS: usize = crate::place::MAX_GRID_CELLS;
pub const MAX_SWEEP_CELLS: usize = 512;
pub const MAX_INTERACTIONS: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum WorldError {
    #[error("invalid normalized input")]
    Input,
    #[error("invalid or oversized virtual geometry")]
    Geometry,
    #[error("unknown or blocked spawn")]
    Spawn,
    #[error("invalid movement limits")]
    Limits,
}

/// A server-produced foot position, in C06 virtual pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum Button {
    #[serde(rename = "interact")]
    Interact,
}

/// Constructed only after validation; carries no coordinates, speed or duration.
#[derive(Clone, Debug, Deserialize)]
#[serde(try_from = "WireInput")]
pub struct Input {
    stream_id: Uuid,
    room_epoch: Uuid,
    seq: u64,
    axis_x: f64,
    axis_y: f64,
    interact: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireInput {
    protocol_version: String,
    #[serde(rename = "type")]
    kind: String,
    stream_id: String,
    room_epoch: String,
    seq: u64,
    axis_x: f64,
    axis_y: f64,
    buttons: Vec<Button>,
}
impl TryFrom<WireInput> for Input {
    type Error = WorldError;
    fn try_from(w: WireInput) -> Result<Self, Self::Error> {
        if w.protocol_version != "1.0.0" || w.kind != "input" || w.buttons.len() > 1 {
            return Err(WorldError::Input);
        }
        let canonical = |s: &str| {
            Uuid::parse_str(s)
                .ok()
                .filter(|id| id.to_string() == s)
                .ok_or(WorldError::Input)
        };
        Self::new(
            canonical(&w.stream_id)?,
            canonical(&w.room_epoch)?,
            w.seq,
            w.axis_x,
            w.axis_y,
            !w.buttons.is_empty(),
        )
    }
}
impl Input {
    pub fn new(
        stream_id: Uuid,
        room_epoch: Uuid,
        seq: u64,
        axis_x: f64,
        axis_y: f64,
        interact: bool,
    ) -> Result<Self, WorldError> {
        let length = axis_x.hypot(axis_y);
        if seq == 0
            || seq > MAX_SEQUENCE
            || !axis_x.is_finite()
            || !axis_y.is_finite()
            || axis_x.abs() > 1.0
            || axis_y.abs() > 1.0
            || length > 1.0 + 1e-9
        {
            return Err(WorldError::Input);
        }
        // C05 allows only rounding error above length one; never amplify analog input.
        let divisor = length.max(1.0);
        Ok(Self {
            stream_id,
            room_epoch,
            seq,
            axis_x: axis_x / divisor,
            axis_y: axis_y / divisor,
            interact,
        })
    }
    pub fn stream_id(&self) -> Uuid {
        self.stream_id
    }
    pub fn room_epoch(&self) -> Uuid {
        self.room_epoch
    }
    pub fn seq(&self) -> u64 {
        self.seq
    }
    pub fn axes(&self) -> (f64, f64) {
        (self.axis_x, self.axis_y)
    }
    pub fn interact(&self) -> bool {
        self.interact
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    speed_px_per_second: f64,
    interaction_range_px: f64,
    input_hold_ms: u64,
}
impl Limits {
    /// Explicit server tuning, not client fields. Hard bounds limit collision work.
    pub fn new(
        speed_px_per_second: f64,
        interaction_range_px: f64,
        input_hold_ms: u64,
    ) -> Result<Self, WorldError> {
        if !speed_px_per_second.is_finite()
            || !(0.0..=1000.0).contains(&speed_px_per_second)
            || speed_px_per_second == 0.0
            || !interaction_range_px.is_finite()
            || !(0.0..=256.0).contains(&interaction_range_px)
            || !(50..=1000).contains(&input_hold_ms)
        {
            return Err(WorldError::Limits);
        }
        Ok(Self {
            speed_px_per_second,
            interaction_range_px,
            input_hold_ms,
        })
    }
    pub fn distance_per_step(self) -> f64 {
        self.speed_px_per_second / 20.0
    }
    pub fn input_hold_ms(self) -> u64 {
        self.input_hold_ms
    }
}

#[derive(Clone, Debug)]
pub struct WorldMap {
    width: usize,
    height: usize,
    tile_width: f64,
    tile_height: f64,
    blocked: Vec<u8>,
    spawns: BTreeMap<String, Position>,
    interactions: BTreeMap<String, Position>,
}
impl WorldMap {
    /// Rechecks even public/mutated Geometry values. Release/hash/provenance validation belongs to BE08.
    pub fn from_geometry(g: &Geometry) -> Result<Self, WorldError> {
        let width = usize::try_from(g.width).map_err(|_| WorldError::Geometry)?;
        let height = usize::try_from(g.height).map_err(|_| WorldError::Geometry)?;
        let cells = width.checked_mul(height).ok_or(WorldError::Geometry)?;
        if width == 0
            || height == 0
            || cells > MAX_GRID_CELLS
            || g.tile_width < 1
            || g.tile_height < 1
            || f64::from(g.width) * f64::from(g.tile_width) > 1_048_576.0
            || f64::from(g.height) * f64::from(g.tile_height) > 1_048_576.0
            || g.collision.len() != height
            || g.collision
                .iter()
                .any(|row| row.len() != width || row.iter().any(|c| *c > 1))
            || g.spawns.is_empty()
            || g.interactions.is_empty()
            || g.spawns.len() > cells
            || g.interactions.len() > cells.min(MAX_INTERACTIONS)
        {
            return Err(WorldError::Geometry);
        }
        let mut map = Self {
            width,
            height,
            tile_width: f64::from(g.tile_width),
            tile_height: f64::from(g.tile_height),
            blocked: g.collision.iter().flatten().copied().collect(),
            spawns: BTreeMap::new(),
            interactions: BTreeMap::new(),
        };
        for s in &g.spawns {
            crate::AssetId::new(s.id.clone()).map_err(|_| WorldError::Geometry)?;
            let p = map.center(s.x, s.y)?;
            if map.spawns.insert(s.id.clone(), p).is_some() {
                return Err(WorldError::Geometry);
            }
        }
        if !map.spawns.contains_key(&g.default_spawn_id) {
            return Err(WorldError::Spawn);
        }
        for i in &g.interactions {
            crate::AssetId::new(i.id.clone()).map_err(|_| WorldError::Geometry)?;
            let p = map.center(i.x, i.y)?;
            if map.interactions.insert(i.id.clone(), p).is_some() {
                return Err(WorldError::Geometry);
            }
        }
        // One flood fill bounds work by map size, including publicly mutated Geometry.
        let first = *map.spawns.values().next().ok_or(WorldError::Spawn)?;
        let mut seen = HashSet::new();
        let mut todo = vec![map.cell(first)];
        while let Some(cell @ (x, y)) = todo.pop() {
            if !map.walkable_cell(x, y) || !seen.insert(cell) {
                continue;
            }
            todo.extend([(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]);
        }
        if map
            .spawns
            .values()
            .chain(map.interactions.values())
            .any(|p| !seen.contains(&map.cell(*p)))
        {
            return Err(WorldError::Geometry);
        }
        Ok(map)
    }
    fn center(&self, x: i32, y: i32) -> Result<Position, WorldError> {
        if !self.walkable_cell(x, y) {
            return Err(WorldError::Spawn);
        }
        Ok(Position {
            x: (f64::from(x) + 0.5) * self.tile_width,
            y: (f64::from(y) + 0.5) * self.tile_height,
        })
    }
    fn cell(&self, p: Position) -> (i32, i32) {
        (
            (p.x / self.tile_width).floor() as i32,
            (p.y / self.tile_height).floor() as i32,
        )
    }
    fn walkable_cell(&self, x: i32, y: i32) -> bool {
        x >= 0
            && y >= 0
            && (x as usize) < self.width
            && (y as usize) < self.height
            && self.blocked[y as usize * self.width + x as usize] == 0
    }
    pub fn walkable(&self, p: Position) -> bool {
        if !p.x.is_finite() || !p.y.is_finite() || p.x < 0.0 || p.y < 0.0 {
            return false;
        }
        let (x, y) = self.cell(p);
        let edge_x = p.x % self.tile_width == 0.0;
        let edge_y = p.y % self.tile_height == 0.0;
        self.walkable_cell(x, y)
            && (!edge_x || self.walkable_cell(x - 1, y))
            && (!edge_y || self.walkable_cell(x, y - 1))
            && (!(edge_x && edge_y) || self.walkable_cell(x - 1, y - 1))
    }
    pub fn spawn(&self, id: &str) -> Result<Position, WorldError> {
        self.spawns.get(id).copied().ok_or(WorldError::Spawn)
    }

    /// Supercover DDA checks all cells along the segment, including both corner neighbors.
    /// A point on a grid edge also checks its adjacent cell, so a line cannot skim a wall.
    pub fn clear_segment(&self, from: Position, to: Position) -> bool {
        if !self.walkable(from) || !self.walkable(to) {
            return false;
        }
        let (mut x, mut y) = self.cell(from);
        let end = self.cell(to);
        let dx = (to.x - from.x) / self.tile_width;
        let dy = (to.y - from.y) / self.tile_height;
        let sx = if dx > 0.0 {
            1
        } else if dx < 0.0 {
            -1
        } else {
            0
        };
        let sy = if dy > 0.0 {
            1
        } else if dy < 0.0 {
            -1
        } else {
            0
        };
        let edge_x = dx == 0.0 && from.x % self.tile_width == 0.0;
        let edge_y = dy == 0.0 && from.y % self.tile_height == 0.0;
        let tx_delta = if dx == 0.0 {
            f64::INFINITY
        } else {
            1.0 / dx.abs()
        };
        let ty_delta = if dy == 0.0 {
            f64::INFINITY
        } else {
            1.0 / dy.abs()
        };
        let mut tx = if sx == 0 {
            f64::INFINITY
        } else {
            ((f64::from(x + if sx > 0 { 1 } else { 0 }) * self.tile_width - from.x)
                / self.tile_width)
                / dx
        };
        let mut ty = if sy == 0 {
            f64::INFINITY
        } else {
            ((f64::from(y + if sy > 0 { 1 } else { 0 }) * self.tile_height - from.y)
                / self.tile_height)
                / dy
        };
        for _ in 0..MAX_SWEEP_CELLS {
            if !self.walkable_cell(x, y)
                || (edge_x && !self.walkable_cell(x - 1, y))
                || (edge_y && !self.walkable_cell(x, y - 1))
            {
                return false;
            }
            if (x, y) == end {
                return true;
            }
            if (tx - ty).abs() <= 1e-12 {
                if !self.walkable_cell(x + sx, y) || !self.walkable_cell(x, y + sy) {
                    return false;
                }
                x += sx;
                y += sy;
                tx += tx_delta;
                ty += ty_delta;
            } else if tx < ty {
                x += sx;
                tx += tx_delta;
            } else {
                y += sy;
                ty += ty_delta;
            }
        }
        false // Fail closed at the work bound; never skip remaining cells.
    }
    pub fn step(&self, p: Position, input: &Input, limits: Limits) -> Position {
        let d = limits.distance_per_step();
        let target = Position {
            x: p.x + input.axis_x * d,
            y: p.y + input.axis_y * d,
        };
        if self.clear_segment(p, target) {
            target
        } else {
            p
        }
    }
    /// Virtual interaction only. No physical arrival, party/pickup authorization or side effect.
    pub fn interaction_in_range(&self, p: Position, id: &str, limits: Limits) -> bool {
        self.interactions.get(id).is_some_and(|target| {
            (target.x - p.x).hypot(target.y - p.y) <= limits.interaction_range_px
                && self.clear_segment(p, *target)
        })
    }
    pub fn nearby_interaction(&self, p: Position, limits: Limits) -> Option<&str> {
        // Lexicographic deterministic selection; interaction count is bounded by MAX_INTERACTIONS.
        self.interactions
            .keys()
            .find(|id| self.interaction_in_range(p, id, limits))
            .map(String::as_str)
    }
}
