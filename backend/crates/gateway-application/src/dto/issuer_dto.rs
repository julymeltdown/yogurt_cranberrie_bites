use gateway_domain::high_risk::SessionState;
use std::fmt;

/// Opaque identifiers for one issuer-authoritative session lookup.
/// Validation remains with the issuer; this transfer does not normalize IDs.
#[derive(Clone, PartialEq, Eq)]
pub struct SessionAuthorityRequestDto {
    pub user_id: String,
    pub session_id: String,
}

impl fmt::Debug for SessionAuthorityRequestDto {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SessionAuthorityRequestDto")
            .field("user_id", &"[REDACTED]")
            .field("session_id", &"[REDACTED]")
            .finish()
    }
}

/// The issuer's answer, before application domain permission decisions.
#[derive(Clone, PartialEq, Eq)]
pub enum SessionStateDto {
    Active { roles: Vec<String> },
    Ended,
}

impl fmt::Debug for SessionStateDto {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active { .. } => formatter
                .debug_struct("SessionStateDto::Active")
                .field("roles", &"[REDACTED]")
                .finish(),
            Self::Ended => formatter.write_str("SessionStateDto::Ended"),
        }
    }
}

impl From<SessionState> for SessionStateDto {
    fn from(state: SessionState) -> Self {
        match state {
            SessionState::Active { roles } => Self::Active { roles },
            SessionState::Ended => Self::Ended,
        }
    }
}

impl From<&SessionState> for SessionStateDto {
    fn from(state: &SessionState) -> Self {
        match state {
            SessionState::Active { roles } => Self::Active {
                roles: roles.clone(),
            },
            SessionState::Ended => Self::Ended,
        }
    }
}

impl From<SessionStateDto> for SessionState {
    fn from(state: SessionStateDto) -> Self {
        match state {
            SessionStateDto::Active { roles } => Self::Active { roles },
            SessionStateDto::Ended => Self::Ended,
        }
    }
}

impl From<&SessionStateDto> for SessionState {
    fn from(state: &SessionStateDto) -> Self {
        match state {
            SessionStateDto::Active { roles } => Self::Active {
                roles: roles.clone(),
            },
            SessionStateDto::Ended => Self::Ended,
        }
    }
}

/// Unparsed issuer JWK Set document. Boundary verification owns JSON parsing.
#[derive(Clone, PartialEq, Eq)]
pub struct SigningKeysDto {
    pub jwks_json: String,
}

impl fmt::Debug for SigningKeysDto {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SigningKeysDto")
            .field("jwks_json", &"[REDACTED]")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::{SessionAuthorityRequestDto, SessionStateDto, SigningKeysDto};
    use gateway_domain::high_risk::{HighRiskAction, SessionDecision, SessionState};

    #[test]
    fn active_transfers_preserve_role_order_duplicates_and_permission_rules() {
        for (roles, expected_admin) in [
            (Vec::<String>::new(), SessionDecision::RoleRevoked),
            (
                vec!["ROLE_USER".into(), "role_admin".into()],
                SessionDecision::RoleRevoked,
            ),
            (
                vec![
                    "private-role-사용자".into(),
                    "ROLE_ADMIN".into(),
                    "ROLE_ADMIN".into(),
                    " ROLE_USER ".into(),
                ],
                SessionDecision::Allowed,
            ),
        ] {
            let expected_transfer = SessionStateDto::Active {
                roles: roles.clone(),
            };
            let expected_domain = SessionState::Active {
                roles: roles.clone(),
            };
            assert_eq!(
                SessionStateDto::from(expected_domain.clone()),
                expected_transfer
            );
            assert_eq!(SessionStateDto::from(&expected_domain), expected_transfer);
            let owned = SessionState::from(expected_transfer.clone());
            let borrowed = SessionState::from(&expected_transfer);
            assert_eq!(owned, expected_domain);
            assert_eq!(borrowed, expected_domain);
            for domain in [owned, borrowed] {
                assert_eq!(
                    domain.permits(HighRiskAction::Administration),
                    expected_admin
                );
                assert_eq!(
                    domain.permits(HighRiskAction::AccountSecurity),
                    SessionDecision::Allowed
                );
            }
        }
    }

    #[test]
    fn ended_transfer_stays_ended_and_differs_from_empty_active_roles() {
        assert_eq!(
            SessionStateDto::from(SessionState::Ended),
            SessionStateDto::Ended
        );
        assert_eq!(
            SessionStateDto::from(&SessionState::Ended),
            SessionStateDto::Ended
        );
        for domain in [
            SessionState::from(SessionStateDto::Ended),
            SessionState::from(&SessionStateDto::Ended),
        ] {
            assert_eq!(domain, SessionState::Ended);
            assert_eq!(
                domain.permits(HighRiskAction::Administration),
                SessionDecision::SessionEnded
            );
            assert_eq!(
                domain.permits(HighRiskAction::AccountSecurity),
                SessionDecision::SessionEnded
            );
        }
        assert_ne!(
            SessionStateDto::Ended,
            SessionStateDto::Active { roles: Vec::new() }
        );
    }

    #[test]
    fn request_and_state_diagnostics_hide_identifiers_and_role_values() {
        let request = SessionAuthorityRequestDto {
            user_id: " private-user-사용자 ".into(),
            session_id: "private-session/not-a-uuid".into(),
        };
        assert_eq!(request.user_id, " private-user-사용자 ");
        assert_eq!(request.session_id, "private-session/not-a-uuid");
        let state = SessionStateDto::Active {
            roles: vec!["private-role".into()],
        };
        let request_debug = format!("{request:?}");
        let state_debug = format!("{state:?}");
        assert!(!request_debug.contains("private-user"));
        assert!(!request_debug.contains("private-session"));
        assert!(!state_debug.contains("private-role"));
        assert!(request_debug.contains("[REDACTED]"));
        assert!(state_debug.contains("[REDACTED]"));
    }

    #[test]
    fn key_document_transfer_keeps_raw_bytes_and_hides_diagnostics() {
        for raw in [
            "",
            "not JSON: private-key",
            "\0private-key",
            " { \"keys\": [{\"kid\":\"private-key-사용자\",\"d\":\"private-exponent\"}] }\n",
        ] {
            let transfer = SigningKeysDto {
                jwks_json: raw.into(),
            };
            assert_eq!(transfer.jwks_json.as_bytes(), raw.as_bytes());
            let diagnostic = format!("{transfer:?}");
            assert!(!diagnostic.contains("private-key"));
            assert!(!diagnostic.contains("private-exponent"));
            assert!(diagnostic.contains("[REDACTED]"));
        }
    }
}
