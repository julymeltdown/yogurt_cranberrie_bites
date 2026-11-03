//! Public service failures contain only controlled application contracts and
//! existing pure policy markers. No arbitrary library/diagnostic conversion exists.
use std::fmt;

#[derive(Debug)]
pub enum GatewayServiceError {
    Client(crate::client_error::GatewayClientError),
    Infrastructure(crate::infrastructure_error::GatewayInfrastructureError),
    ModerationInput(crate::moderation_input::ModerationInputError),
    ModerationRepository(crate::moderation_repository_error::ModerationRepositoryError),
    SearchCursor(crate::search_cursor::InvalidSearchCursor),
    CursorEncoding(crate::search_cursor::SearchCursorEncodingError),
    ProfileNotVisible(gateway_domain::profile::ProfileNotVisible),
    ModerationCursor(gateway_domain::moderation::InvalidContentModerationDecisionCursor),
    ModerationTargetUnavailable(gateway_domain::moderation::ModerationTargetUnavailable),
    NotificationPageBound,
    NotificationAuthorizationIncomplete,
    ProfileRelationshipIncomplete,
    InvalidModerationAction,
    RouteContractMissing,
    RouteContractVersionMissing,
    PolicyMissing,
    RolloutMissing,
    ClientProfileMissing,
    AggregationRecipeMissing,
}

pub type GatewayServiceResult<T> = Result<T, GatewayServiceError>;

impl fmt::Display for GatewayServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Client(error) => write!(f, "{error}"),
            Self::Infrastructure(error) => write!(f, "{error}"),
            Self::ModerationInput(error) => write!(f, "{error}"),
            Self::ModerationRepository(error) => write!(f, "{error}"),
            Self::SearchCursor(error) => write!(f, "{error}"),
            Self::CursorEncoding(error) => write!(f, "{error}"),
            Self::ProfileNotVisible(error) => write!(f, "{error}"),
            Self::ModerationCursor(error) => write!(f, "{error}"),
            Self::ModerationTargetUnavailable(error) => write!(f, "{error}"),
            Self::NotificationPageBound => f.write_str("Notification page exceeds its bound"),
            Self::NotificationAuthorizationIncomplete => {
                f.write_str("Incomplete notification authorization")
            }
            Self::ProfileRelationshipIncomplete => {
                f.write_str("Incomplete profile relationship policy batch")
            }
            Self::InvalidModerationAction => {
                f.write_str("staged moderation decision is not a content action")
            }
            Self::RouteContractMissing => f.write_str("Route contract not found"),
            Self::RouteContractVersionMissing => f.write_str("Route contract version not found"),
            Self::PolicyMissing => f.write_str("Policy set not found"),
            Self::RolloutMissing => f.write_str("Rollout not found"),
            Self::ClientProfileMissing => f.write_str("Client profile not found"),
            Self::AggregationRecipeMissing => f.write_str("Aggregation recipe not found"),
        }
    }
}
impl std::error::Error for GatewayServiceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Client(error) => Some(error),
            Self::Infrastructure(error) => Some(error),
            Self::ModerationInput(error) => Some(error),
            Self::ModerationRepository(error) => Some(error),
            Self::SearchCursor(error) => Some(error),
            Self::CursorEncoding(error) => Some(error),
            Self::ProfileNotVisible(error) => Some(error),
            Self::ModerationCursor(error) => Some(error),
            Self::ModerationTargetUnavailable(error) => Some(error),
            Self::NotificationPageBound
            | Self::NotificationAuthorizationIncomplete
            | Self::ProfileRelationshipIncomplete
            | Self::InvalidModerationAction
            | Self::RouteContractMissing
            | Self::RouteContractVersionMissing
            | Self::PolicyMissing
            | Self::RolloutMissing
            | Self::ClientProfileMissing
            | Self::AggregationRecipeMissing => None,
        }
    }
}
impl From<crate::client_error::GatewayClientError> for GatewayServiceError {
    fn from(value: crate::client_error::GatewayClientError) -> Self {
        Self::Client(value)
    }
}
impl From<crate::infrastructure_error::GatewayInfrastructureError> for GatewayServiceError {
    fn from(value: crate::infrastructure_error::GatewayInfrastructureError) -> Self {
        Self::Infrastructure(value)
    }
}
impl From<crate::moderation_input::ModerationInputError> for GatewayServiceError {
    fn from(value: crate::moderation_input::ModerationInputError) -> Self {
        Self::ModerationInput(value)
    }
}
impl From<crate::moderation_repository_error::ModerationRepositoryError> for GatewayServiceError {
    fn from(value: crate::moderation_repository_error::ModerationRepositoryError) -> Self {
        Self::ModerationRepository(value)
    }
}
impl From<crate::search_cursor::InvalidSearchCursor> for GatewayServiceError {
    fn from(value: crate::search_cursor::InvalidSearchCursor) -> Self {
        Self::SearchCursor(value)
    }
}
impl From<crate::search_cursor::SearchCursorEncodingError> for GatewayServiceError {
    fn from(value: crate::search_cursor::SearchCursorEncodingError) -> Self {
        Self::CursorEncoding(value)
    }
}
impl From<gateway_domain::profile::ProfileNotVisible> for GatewayServiceError {
    fn from(value: gateway_domain::profile::ProfileNotVisible) -> Self {
        Self::ProfileNotVisible(value)
    }
}
impl From<gateway_domain::moderation::InvalidContentModerationDecisionCursor>
    for GatewayServiceError
{
    fn from(value: gateway_domain::moderation::InvalidContentModerationDecisionCursor) -> Self {
        Self::ModerationCursor(value)
    }
}
impl From<gateway_domain::moderation::ModerationTargetUnavailable> for GatewayServiceError {
    fn from(value: gateway_domain::moderation::ModerationTargetUnavailable) -> Self {
        Self::ModerationTargetUnavailable(value)
    }
}
