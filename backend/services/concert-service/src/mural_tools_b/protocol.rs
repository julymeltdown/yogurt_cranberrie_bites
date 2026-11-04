//! Closed original-byte parsing; no arbitrary product forwarding or client authority.
use concert_application::mural::tools_b::ToolCommand;
use concert_domain::mural::{tools_b::*, PlacePixel};
use tonic::Status;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WallOperation {
    Snapshot,
    Delta,
    Status,
    Pixel,
    PixelResult(Uuid),
    Tools,
    ActionResult(Uuid),
    Erase,
    Fill,
    Undo,
}
impl WallOperation {
    pub fn id(self) -> &'static str {
        match self {
            Self::Status => "getConcertCommandStatus",
            Self::Snapshot => "getConcertMuralSnapshot",
            Self::Delta => "getConcertMuralDelta",
            Self::Pixel => "placeConcertPixel",
            Self::PixelResult(_) => "getConcertMuralOwnResult",
            Self::Tools => "getConcertMuralTools",
            Self::ActionResult(_) => "getOwnConcertMuralAction",
            Self::Erase => "eraseConcertMuralPixel",
            Self::Fill => "fillConcertMuralRegion",
            Self::Undo => "undoConcertMuralAction",
        }
    }
    pub fn mutation(self) -> bool {
        matches!(self, Self::Pixel | Self::Erase | Self::Fill | Self::Undo)
    }
    pub fn parse(event: Uuid, method: &str, target: &str) -> Result<Self, Status> {
        if method == "GET" && status_query(event, target).is_ok() {
            return Ok(Self::Status);
        }
        let prefix = format!("/api/concert/events/{event}/mural/");
        let suffix = target.strip_prefix(&prefix).ok_or_else(invalid)?;
        let op = match (method, suffix) {
            ("GET", "snapshot") => Self::Snapshot,
            ("GET", s)
                if s.starts_with("operations?after_revision=")
                    && s.split_once('=')
                        .filter(|(_, n)| {
                            !n.is_empty()
                                && (n.len() == 1 || !n.starts_with('0'))
                                && n.bytes().all(|b| b.is_ascii_digit())
                        })
                        .and_then(|(_, n)| n.parse::<i64>().ok())
                        .is_some_and(|n| (1..=concert_domain::MAX_REVISION).contains(&n))
                    && !s.contains('&') =>
            {
                Self::Delta
            }
            ("POST", "pixels") => Self::Pixel,
            ("POST", "erase") => Self::Erase,
            ("POST", "fill") => Self::Fill,
            ("POST", "undo") => Self::Undo,
            ("GET", "tools") => Self::Tools,
            ("GET", s) if s.starts_with("actions/") => {
                Self::ActionResult(crate::product_rpc::canonical_uuid(&s[8..])?)
            }
            ("GET", s) if s.starts_with("commands/") && s.ends_with("/result") => {
                Self::PixelResult(crate::product_rpc::canonical_uuid(&s[9..s.len() - 7])?)
            }
            _ => return Err(invalid()),
        };
        Ok(op)
    }
}
fn invalid() -> Status {
    Status::invalid_argument("Invalid Wall command")
}
pub fn body(op: WallOperation, bytes: &[u8]) -> Result<ActionBody, Status> {
    if bytes.len() > 1024 {
        return Err(invalid());
    }
    let value = match op {
        WallOperation::Pixel => {
            ActionBody::Pencil(serde_json::from_slice::<PlacePixel>(bytes).map_err(|_| invalid())?)
        }
        WallOperation::Erase => {
            ActionBody::Erase(serde_json::from_slice::<ErasePixel>(bytes).map_err(|_| invalid())?)
        }
        WallOperation::Fill => {
            ActionBody::Fill(serde_json::from_slice::<FillRegion>(bytes).map_err(|_| invalid())?)
        }
        WallOperation::Undo => {
            ActionBody::Undo(serde_json::from_slice::<UndoAction>(bytes).map_err(|_| invalid())?)
        }
        _ => return Err(invalid()),
    };
    let valid = match value {
        ActionBody::Pencil(p) => p.valid(),
        ActionBody::Erase(p) => PlacePixel {
            expected_revision: p.expected_revision,
            x: p.x,
            y: p.y,
            color_index: 0,
        }
        .valid(),
        ActionBody::Fill(p) => PlacePixel {
            expected_revision: p.expected_revision,
            x: p.x,
            y: p.y,
            color_index: p.color_index,
        }
        .valid(),
        ActionBody::Undo(p) => {
            (1..=concert_domain::MAX_REVISION).contains(&p.expected_revision)
                && !p.target_action_id.is_nil()
        }
    };
    if !valid {
        return Err(invalid());
    }
    Ok(value)
}
pub fn command(body: ActionBody) -> Result<ToolCommand, Status> {
    match body {
        ActionBody::Erase(p) => Ok(ToolCommand::Erase(p)),
        ActionBody::Fill(p) => Ok(ToolCommand::Fill(p)),
        ActionBody::Undo(p) => Ok(ToolCommand::Undo(p)),
        _ => Err(invalid()),
    }
}

/// Reuse the canonical duplicate-safe parser, then close the operation domain.
pub fn status_query(
    event: Uuid,
    target: &str,
) -> Result<crate::product_rpc::CommandStatusQuery, Status> {
    if target.split_once('?').map(|p| p.0)
        != Some(format!("/api/concert/events/{event}/commands").as_str())
    {
        return Err(invalid());
    }
    let query = crate::product_rpc::wall_command_status_query(target)?;
    if !matches!(
        query.operation.id(),
        "placeConcertPixel"
            | "eraseConcertMuralPixel"
            | "fillConcertMuralRegion"
            | "undoConcertMuralAction"
    ) {
        return Err(invalid());
    }
    Ok(query)
}
