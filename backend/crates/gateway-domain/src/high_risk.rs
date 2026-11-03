//! Which requests must be confirmed against the issuer's current session state.
//!
//! An access token is trusted until it expires (`AUTH_ACCESS_TTL_SECONDS`, 15
//! minutes by default). For ordinary reads and writes that is the accepted bound
//! on how long a logged-out or demoted session keeps working. The actions listed
//! here cannot wait that long: each one is confirmed with the issuer, and refused
//! if the session has ended, the required role is gone, or the issuer cannot
//! answer.

/// Role required for the administration surface.
pub const ADMIN_ROLE: &str = "ROLE_ADMIN";

/// A class of action that needs the issuer's current view of the session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HighRiskAction {
    /// Every `/admin/...` route, reads included: they expose or change what
    /// other users see. Requires a live session that currently holds
    /// [`ADMIN_ROLE`].
    Administration,
    /// Changes to the account itself that cannot be undone by the user or that
    /// hand the account to another credential: deleting the account and linking
    /// an external identity provider. Requires a live session.
    AccountSecurity,
}

impl HighRiskAction {
    /// Metric label.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Administration => "administration",
            Self::AccountSecurity => "account_security",
        }
    }

    /// The role the session must currently hold, if any.
    pub fn required_role(self) -> Option<&'static str> {
        match self {
            Self::Administration => Some(ADMIN_ROLE),
            Self::AccountSecurity => None,
        }
    }
}

/// Classify a request by its method and matched route template.
///
/// `route` is the template the router matched (`/admin/clients/:client_id`),
/// not the raw path, so a path parameter cannot make a route look like another.
pub fn classify(method: &str, route: &str) -> Option<HighRiskAction> {
    if route == "/admin" || route.starts_with("/admin/") {
        return Some(HighRiskAction::Administration);
    }
    match (method, route) {
        ("DELETE", "/auth/account") | ("POST", "/auth/oauth/:provider/link") => {
            Some(HighRiskAction::AccountSecurity)
        }
        _ => None,
    }
}

/// The issuer's current answer about one session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionState {
    /// The session is live; `roles` are the account's roles now.
    Active { roles: Vec<String> },
    /// Logged out, revoked, expired, or the account is inactive.
    Ended,
}

impl SessionState {
    /// Whether this state permits `action`.
    pub fn permits(&self, action: HighRiskAction) -> SessionDecision {
        match self {
            Self::Ended => SessionDecision::SessionEnded,
            Self::Active { roles } => match action.required_role() {
                Some(role) if !roles.iter().any(|held| held == role) => {
                    SessionDecision::RoleRevoked
                }
                _ => SessionDecision::Allowed,
            },
        }
    }
}

/// Outcome of checking a session for a high-risk action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionDecision {
    Allowed,
    /// The session no longer exists; the caller must sign in again.
    SessionEnded,
    /// The session is live but no longer holds the required role.
    RoleRevoked,
}
