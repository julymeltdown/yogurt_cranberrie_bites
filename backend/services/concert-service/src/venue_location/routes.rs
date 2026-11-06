//! Closed VENU-BE-L01 public route set. Parsing a route is lookup only, never authority.
use concert_domain::venue_location::VenueLocationError;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VenueLocationOperation {
    Snapshot,
    SetSharing,
    PublishFix,
    Command,
}
impl VenueLocationOperation {
    pub fn id(self) -> &'static str {
        match self {
            Self::Snapshot => "getVenueLocationSnapshot",
            Self::SetSharing => "setVenueLocationSharing",
            Self::PublishFix => "publishVenueLocationFix",
            Self::Command => "getVenueLocationCommand",
        }
    }
    pub fn route(event: Uuid, method: &str, target: &str) -> Result<Self, VenueLocationError> {
        if event.is_nil() || target.len() > 4096 || target.chars().any(char::is_control) {
            return Err(VenueLocationError::InvalidInput);
        }
        let base = format!("/api/concert/events/{event}/venue/location");
        let (path, query) = target
            .split_once('?')
            .map_or((target, None), |(path, query)| (path, Some(query)));
        let suffix = path.strip_prefix(&base).ok_or(VenueLocationError::Denied)?;
        match (method, suffix, query) {
            ("GET", "", None) => Ok(Self::Snapshot),
            ("PUT", "/sharing", None) => Ok(Self::SetSharing),
            ("PUT", "/fix", None) => Ok(Self::PublishFix),
            ("GET", rest, Some(_)) if canonical_command(rest) => Ok(Self::Command),
            _ => Err(VenueLocationError::Denied),
        }
    }
}

fn canonical_command(suffix: &str) -> bool {
    suffix
        .strip_prefix("/commands/")
        .is_some_and(|value| Uuid::parse_str(value).is_ok_and(|id| !id.is_nil()))
}

pub const ROUTES: &[(&str, &str, &str)] = &[
    (
        "getVenueLocationSnapshot",
        "GET",
        "/api/concert/events/{eventId}/venue/location",
    ),
    (
        "setVenueLocationSharing",
        "PUT",
        "/api/concert/events/{eventId}/venue/location/sharing",
    ),
    (
        "publishVenueLocationFix",
        "PUT",
        "/api/concert/events/{eventId}/venue/location/fix",
    ),
    (
        "getVenueLocationCommand",
        "GET",
        "/api/concert/events/{eventId}/venue/location/commands/{key}",
    ),
];
