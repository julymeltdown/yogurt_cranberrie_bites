//! Wall B cell planning. Inputs are confirmed commands; this module grants no authority.
//! Before/after cells are private server history and never a public mural delta.
use super::{PlacePixel, PIXELS, SIDE};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use uuid::Uuid;

pub const FILL_LIMIT: usize = 256;
pub const CELL_ALLOWANCE: u32 = 512;
pub const WINDOW_SECONDS: i64 = 600;
pub const BACKGROUND_INDEX: u8 = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ErasePixel {
    pub expected_revision: i64,
    pub x: i32,
    pub y: i32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FillRegion {
    pub expected_revision: i64,
    pub x: i32,
    pub y: i32,
    pub color_index: i32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UndoAction {
    pub expected_revision: i64,
    pub target_action_id: Uuid,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ActionKind {
    Pencil,
    Erase,
    Fill,
    Undo,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellChange {
    pub index: u16,
    pub before: u8,
    pub after: u8,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanError {
    InvalidInput,
    NoChange,
    FillTooLarge,
    HistoryMismatch,
}

fn grid_valid(grid: &[u8]) -> bool {
    grid.len() == PIXELS && grid.iter().all(|c| *c < 16)
}
fn position(x: i32, y: i32) -> Result<usize, PlanError> {
    if !(0..SIDE as i32).contains(&x) || !(0..SIDE as i32).contains(&y) {
        return Err(PlanError::InvalidInput);
    }
    Ok(y as usize * SIDE + x as usize)
}
pub fn pencil(grid: &[u8], input: PlacePixel) -> Result<Vec<CellChange>, PlanError> {
    if !grid_valid(grid) || !input.valid() {
        return Err(PlanError::InvalidInput);
    }
    let index = position(input.x, input.y)?;
    if grid[index] == input.color_index as u8 {
        return Err(PlanError::NoChange);
    }
    Ok(vec![CellChange {
        index: index as u16,
        before: grid[index],
        after: input.color_index as u8,
    }])
}
pub fn erase(grid: &[u8], input: ErasePixel) -> Result<Vec<CellChange>, PlanError> {
    pencil(
        grid,
        PlacePixel {
            expected_revision: input.expected_revision,
            x: input.x,
            y: input.y,
            color_index: BACKGROUND_INDEX.into(),
        },
    )
}
/// Four-connected traversal stops at the first cell exceeding the cap. No partial plan.
pub fn fill(grid: &[u8], input: FillRegion) -> Result<Vec<CellChange>, PlanError> {
    let seed = pencil(
        grid,
        PlacePixel {
            expected_revision: input.expected_revision,
            x: input.x,
            y: input.y,
            color_index: input.color_index,
        },
    )?[0]
        .index as usize;
    let original = grid[seed];
    let mut seen = [false; PIXELS];
    seen[seed] = true;
    let mut queue = vec![seed];
    let mut cursor = 0;
    while cursor < queue.len() {
        let index = queue[cursor];
        cursor += 1;
        let x = index % SIDE;
        let y = index / SIDE;
        for neighbour in [
            (x > 0).then(|| index - 1),
            (x + 1 < SIDE).then(|| index + 1),
            (y > 0).then(|| index - SIDE),
            (y + 1 < SIDE).then(|| index + SIDE),
        ]
        .into_iter()
        .flatten()
        {
            if !seen[neighbour] && grid[neighbour] == original {
                if queue.len() == FILL_LIMIT {
                    return Err(PlanError::FillTooLarge);
                }
                seen[neighbour] = true;
                queue.push(neighbour);
            }
        }
    }
    queue.sort_unstable();
    Ok(queue
        .into_iter()
        .map(|index| CellChange {
            index: index as u16,
            before: original,
            after: input.color_index as u8,
        })
        .collect())
}
/// Only pass history loaded from the owning database, never client before-colors.
pub fn compensate(grid: &[u8], history: &[CellChange]) -> Result<Vec<CellChange>, PlanError> {
    if !grid_valid(grid) || history.is_empty() || history.len() > FILL_LIMIT {
        return Err(PlanError::HistoryMismatch);
    }
    let mut indices = BTreeSet::new();
    history
        .iter()
        .map(|cell| {
            let index = usize::from(cell.index);
            if index >= PIXELS
                || cell.before >= 16
                || cell.after >= 16
                || cell.before == cell.after
                || !indices.insert(index)
                || grid[index] != cell.after
            {
                return Err(PlanError::HistoryMismatch);
            }
            Ok(CellChange {
                index: cell.index,
                before: cell.after,
                after: cell.before,
            })
        })
        .collect()
}

#[derive(Clone, Copy, Debug)]
pub struct Charge {
    pub committed_at: DateTime<Utc>,
    pub cells: u32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Allowance {
    pub used: u32,
    pub remaining: u32,
    pub next_available_at: Option<DateTime<Utc>>,
}
/// Exact database-time window (now - 600 seconds, now], independent of receipts.
pub fn allowance(now: DateTime<Utc>, charges: &[Charge], cost: u32) -> Allowance {
    let lower = now - Duration::seconds(WINDOW_SECONDS);
    let mut active: Vec<_> = charges
        .iter()
        .filter(|c| c.committed_at > lower && c.committed_at <= now)
        .collect();
    active.sort_by_key(|c| c.committed_at);
    let used = active.iter().fold(0u32, |n, c| n.saturating_add(c.cells));
    let remaining = CELL_ALLOWANCE.saturating_sub(used);
    let mut retained = used;
    let next_available_at = if cost <= remaining {
        None
    } else {
        active.into_iter().find_map(|c| {
            retained = retained.saturating_sub(c.cells);
            (retained.saturating_add(cost) <= CELL_ALLOWANCE)
                .then_some(c.committed_at + Duration::seconds(WINDOW_SECONDS))
        })
    };
    Allowance {
        used,
        remaining,
        next_available_at,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation_id", content = "body", deny_unknown_fields)]
pub enum ActionBody {
    #[serde(rename = "placeConcertPixel")]
    Pencil(PlacePixel),
    #[serde(rename = "eraseConcertMuralPixel")]
    Erase(ErasePixel),
    #[serde(rename = "fillConcertMuralRegion")]
    Fill(FillRegion),
    #[serde(rename = "undoConcertMuralAction")]
    Undo(UndoAction),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ToolAvailability {
    pub pencil: bool,
    pub erase: bool,
    pub fill: bool,
    pub undo: bool,
}

/// Returned only to its canonical actor over an authenticated private response.
/// Contains no before-colors, other authors, public delta or local fallback authority.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnAction {
    pub action_id: Uuid,
    pub receipt_id: Uuid,
    pub event_id: Uuid,
    pub actor_id: Uuid,
    pub command: ActionBody,
    pub idempotency_key: Uuid,
    pub kind: ActionKind,
    pub revision: i64,
    pub changed_cells: u32,
    pub committed_at: DateTime<Utc>,
    pub next_allowed_at: DateTime<Utc>,
    pub undo_expires_at: DateTime<Utc>,
    pub reset_required: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct UndoEligibility {
    pub action_id: Uuid,
    pub revision: i64,
    pub expires_at: DateTime<Utc>,
    pub changed_cells: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct UnresolvedAction {
    pub command: ActionBody,
    pub idempotency_key: Uuid,
    pub receipt_id: Uuid,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ToolCapabilities {
    pub version: u8,
    pub event_id: Uuid,
    pub actor_id: Uuid,
    pub revision: i64,
    pub server_time: DateTime<Utc>,
    pub background_index: Option<u8>,
    pub fill_limit: u32,
    pub cell_allowance: u32,
    pub window_seconds: i64,
    pub remaining_cells: u32,
    pub next_cell_available_at: Option<DateTime<Utc>>,
    pub next_allowed_at: Option<DateTime<Utc>>,
    pub editable: bool,
    pub available: ToolAvailability,
    pub undo_window_seconds: i64,
    pub undo: Option<UndoEligibility>,
    pub latest_action: Option<OwnAction>,
    pub unresolved: Option<UnresolvedAction>,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn region(n: usize) -> Vec<u8> {
        let mut g = vec![1; PIXELS];
        g[..n].fill(0);
        g
    }
    fn input() -> FillRegion {
        FillRegion {
            expected_revision: 1,
            x: 0,
            y: 0,
            color_index: 2,
        }
    }
    #[test]
    fn fill_accepts_256_and_rejects_257_without_mutating_input() {
        let grid = region(256);
        let plan = fill(&grid, input()).unwrap();
        assert_eq!(plan.len(), 256);
        assert!(plan.iter().all(|c| c.before == 0 && c.after == 2));
        assert_eq!(grid, region(256));
        let grid = region(257);
        assert_eq!(fill(&grid, input()), Err(PlanError::FillTooLarge));
        assert_eq!(grid, region(257));
    }
    #[test]
    fn fill_is_four_connected_and_never_wraps_rows() {
        let mut grid = vec![1; PIXELS];
        grid[127] = 0;
        grid[128] = 0;
        grid[256] = 0;
        grid[129] = 0;
        assert_eq!(
            fill(&grid, FillRegion { x: 127, ..input() }).unwrap().len(),
            1
        );
        grid.fill(1);
        grid[0] = 0;
        grid[129] = 0;
        assert_eq!(fill(&grid, input()).unwrap().len(), 1);
    }
    #[test]
    fn no_change_and_invalid_inputs_produce_no_plan() {
        let grid = vec![0; PIXELS];
        assert_eq!(
            erase(
                &grid,
                ErasePixel {
                    expected_revision: 1,
                    x: 0,
                    y: 0
                }
            ),
            Err(PlanError::NoChange)
        );
        assert_eq!(
            fill(
                &grid,
                FillRegion {
                    color_index: 0,
                    ..input()
                }
            ),
            Err(PlanError::NoChange)
        );
        for p in [
            FillRegion { x: -1, ..input() },
            FillRegion { y: 128, ..input() },
            FillRegion {
                color_index: 16,
                ..input()
            },
            FillRegion {
                expected_revision: 0,
                ..input()
            },
        ] {
            assert_eq!(fill(&grid, p), Err(PlanError::InvalidInput));
        }
        assert_eq!(fill(&grid[..127], input()), Err(PlanError::InvalidInput));
    }
    #[test]
    fn compensation_requires_exact_private_after_values_and_unique_valid_indices() {
        let grid = vec![2; PIXELS];
        let history = vec![CellChange {
            index: 0,
            before: 1,
            after: 2,
        }];
        assert_eq!(
            compensate(&grid, &history).unwrap(),
            vec![CellChange {
                index: 0,
                before: 2,
                after: 1
            }]
        );
        for bad in [
            vec![],
            vec![history[0].clone(), history[0].clone()],
            vec![CellChange {
                index: 0,
                before: 1,
                after: 3,
            }],
            vec![CellChange {
                index: 0,
                before: 16,
                after: 2,
            }],
            vec![CellChange {
                index: 16384,
                before: 1,
                after: 2,
            }],
        ] {
            assert_eq!(compensate(&grid, &bad), Err(PlanError::HistoryMismatch));
        }
    }
    #[test]
    fn rolling_window_is_open_left_closed_right_and_undo_pays_full_cost() {
        let now = DateTime::<Utc>::from_timestamp(1_800_000_000, 0).unwrap();
        let charges = [
            Charge {
                committed_at: now - Duration::seconds(600),
                cells: 256,
            },
            Charge {
                committed_at: now - Duration::seconds(599),
                cells: 256,
            },
            Charge {
                committed_at: now,
                cells: 256,
            },
            Charge {
                committed_at: now + Duration::microseconds(1),
                cells: 256,
            },
        ];
        assert_eq!(
            allowance(now, &charges, 256),
            Allowance {
                used: 512,
                remaining: 0,
                next_available_at: Some(now + Duration::seconds(1))
            }
        );
        assert_eq!(
            allowance(now + Duration::seconds(600), &charges[..3], 256),
            Allowance {
                used: 0,
                remaining: 512,
                next_available_at: None
            }
        );
        assert_eq!(allowance(now, &charges[1..2], 256).remaining, 256);
        assert_eq!(
            allowance(now, &charges[1..2], 257).next_available_at,
            Some(now + Duration::seconds(1))
        );
    }
}
