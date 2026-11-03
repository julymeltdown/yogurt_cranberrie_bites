//! Pure request-admission policy. No runtime, database or transport dependencies.
//! The parent middleware still owns admission permits, dispatch and draining.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RouteClass {
    Authentication,
    Mutation,
}

impl RouteClass {
    pub(super) fn of(path: &str) -> Self {
        // Match complete path segments, never lookalikes such as /authorizations.
        // Callers pass Uri::path(), not a query string or decoded user input.
        if path == "/auth"
            || path.starts_with("/auth/")
            || path == "/oauth"
            || path.starts_with("/oauth/")
        {
            Self::Authentication
        } else {
            Self::Mutation
        }
    }
}

pub(super) fn valid_key(key: &str) -> bool {
    (1..=128).contains(&key.len())
        && key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actual_auth_namespaces_use_the_reserved_budget() {
        for path in [
            "/auth",
            "/auth/",
            "/auth/login",
            "/auth/email/signup",
            "/auth/refresh",
            "/auth/password/reset/request",
            "/oauth",
            "/oauth/",
            "/oauth/google/callback",
        ] {
            assert_eq!(RouteClass::of(path), RouteClass::Authentication, "{path}");
        }
    }

    #[test]
    fn lookalike_prefixes_cannot_bypass_the_mutation_journal() {
        for path in [
            "/author",
            "/authorize",
            "/authentication",
            "/authentic/resource",
            "/auth-v2/login",
            "/oauth2/token",
            "/oauth-client",
            "/oauthwhatever",
        ] {
            assert_eq!(RouteClass::of(path), RouteClass::Mutation, "{path}");
        }
    }

    #[test]
    fn unrelated_paths_remain_mutations() {
        for path in [
            "/",
            "",
            "/api/posts",
            "/api/profile/user-1",
            "/api/friends/requests",
            "/admin/auth",
        ] {
            assert_eq!(RouteClass::of(path), RouteClass::Mutation, "{path}");
        }
    }

    #[test]
    fn matching_is_case_sensitive_and_does_not_decode_separators() {
        for path in [
            "/Auth/login",
            "/AUTH/login",
            "/auth%2flogin",
            "/oauth%2Fgoogle",
            "/auth\\login",
        ] {
            assert_eq!(RouteClass::of(path), RouteClass::Mutation, "{path}");
        }
    }

    #[test]
    fn valid_key_boundaries_are_inclusive() {
        assert!(!valid_key(""));
        assert!(valid_key("a"));
        assert!(valid_key(&"a".repeat(128)));
        assert!(!valid_key(&"a".repeat(129)));
        assert!(valid_key("operation:123-abc_DEF.4"));
    }

    #[test]
    fn all_ascii_characters_have_the_exact_published_allowlist() {
        for byte in 0..=127u8 {
            let text = String::from_utf8(vec![byte]).unwrap();
            assert_eq!(
                valid_key(&text),
                byte.is_ascii_alphanumeric() || b"-_.:".contains(&byte)
            );
        }
    }

    #[test]
    fn unicode_and_control_characters_are_never_keys() {
        for text in [
            "한글", "café", "a b", "a\r\nb", "a\0b", "🔑", "\u{200b}", "\u{7f}",
        ] {
            assert!(!valid_key(text));
        }
    }
}
