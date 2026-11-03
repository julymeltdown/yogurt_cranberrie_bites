use std::path::PathBuf;

fn source(path: &str) -> String {
    let service = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = if let Some(relative) = path.strip_prefix("src/application/") {
        service
            .join("../../crates/auth-application/src")
            .join(relative)
    } else {
        service.join(path)
    };
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

#[test]
fn application_password_contract_has_no_argon2_or_worker_pool_implementation() {
    let contract = source("src/application/password_work.rs");
    for forbidden in ["argon2::", "platform::cpu", "WorkerPool", "AdmissionClass"] {
        assert!(
            !contract.contains(forbidden),
            "application password contract leaked {forbidden}"
        );
    }
    assert!(contract.contains("trait PasswordWorkPort"));
    assert!(!contract.contains("process_default"));

    let adapter = source("src/adapters/out/security/admitted_argon2.rs");
    assert!(adapter.contains("impl PasswordWorkPort for AdmittedArgon2PasswordWork"));
    assert!(adapter.contains("Argon2"));
    assert!(adapter.contains("WorkerPool"));
    assert!(adapter.contains("CapacityExhausted"));
}

#[test]
fn sensitive_use_cases_require_explicit_security_capabilities() {
    for path in [
        "src/application/use_cases/signup.rs",
        "src/application/use_cases/login.rs",
        "src/application/use_cases/password_reset.rs",
    ] {
        let contents = source(path);
        assert!(
            contents.contains("Arc<dyn AuthTransactionsPort>"),
            "{path} does not own an explicit transaction port"
        );
        assert!(
            contents.contains("PasswordWork"),
            "{path} does not own an explicit password port"
        );
        assert!(!contents.contains("auth_transactions()"));
        assert!(!contents.contains("process_default"));
        assert!(!contents.contains("with_password_work"));
    }

    let oauth = source("src/application/use_cases/oauth_login.rs");
    assert!(oauth.contains("Arc<dyn AuthTransactionsPort>"));
    assert!(!oauth.contains("auth_transactions()"));
    assert!(!oauth.contains("resolve_legacy_user"));
}

#[test]
fn production_composition_selects_one_shared_real_password_adapter() {
    let config = source("src/config/mod.rs");
    assert!(config.contains("AdmittedArgon2PasswordWork::new(config.password_work)"));
    assert!(config.contains("let auth_transactions: Arc<dyn AuthTransactionsPort>"));
    assert!(config.contains("password_work.clone()"));
    assert!(!contains_rust_identifier_call(
        &config,
        "PasswordWork",
        "new"
    ));
}

#[test]
fn repositories_cannot_offer_optional_auth_transactions() {
    let ports = source("src/application/ports.rs");
    let repository = source("src/adapters/out/persistence/user_repository.rs");
    assert!(!ports.contains("fn auth_transactions"));
    assert!(!repository.contains("fn auth_transactions"));
}

fn contains_rust_identifier_call(source: &str, type_name: &str, method: &str) -> bool {
    let needle = format!("{type_name}::{method}");
    source.match_indices(&needle).any(|(offset, _)| {
        let before = source[..offset].chars().next_back();
        let after = source[offset + needle.len()..].chars().next();
        !matches!(before, Some(c) if c.is_ascii_alphanumeric() || c == '_')
            && !matches!(after, Some(c) if c.is_ascii_alphanumeric() || c == '_')
    })
}
#[test]
fn architecture_guard_distinguishes_password_work_from_argon_adapter() {
    assert!(!contains_rust_identifier_call(
        "AdmittedArgon2PasswordWork::new(config.password_work)",
        "PasswordWork",
        "new"
    ));
    assert!(contains_rust_identifier_call(
        "PasswordWork::new(config.password_work)",
        "PasswordWork",
        "new"
    ));
}

#[test]
fn login_and_oauth_lifecycles_require_explicit_time_and_security_capabilities() {
    for path in [
        "src/application/use_cases/login.rs",
        "src/application/use_cases/oauth_login.rs",
        "src/application/use_cases/oauth_link.rs",
        "src/application/use_cases/signup.rs",
        "src/application/use_cases/delete_user.rs",
        "src/application/use_cases/retry_user_deletion.rs",
        "src/application/use_cases/lifecycle_cleanup.rs",
        "src/application/use_cases/password_reset.rs",
    ] {
        let core = source(path);
        for forbidden in [
            "Utc::now()",
            "Uuid::new_v4()",
            "thread_rng()",
            "rand::",
            "OsRng",
        ] {
            assert!(
                !core.contains(forbidden),
                "{path} owns non-determinism: {forbidden}"
            );
        }
        assert!(core.contains("Arc<dyn AuthRuntimePort>"));
    }
    let oauth = source("src/application/use_cases/oauth_login.rs");
    assert!(oauth.contains("Arc<dyn OAuthAuthorizationCryptoPort>"));
    assert!(!oauth.contains("rand::"));
    let signup = source("src/application/use_cases/signup.rs");
    assert!(signup.contains("Arc<dyn VerificationCryptoPort>"));
    let domain =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../crates/auth-domain/src/session.rs");
    let content = std::fs::read_to_string(domain).unwrap();
    let production = content.split("#[cfg(test)]").next().unwrap();
    assert!(!production.contains("Uuid::new_v4()"));
}
