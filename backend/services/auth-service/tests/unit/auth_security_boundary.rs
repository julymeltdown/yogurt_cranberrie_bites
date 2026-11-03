use auth_service::application::errors::AuthError;
use auth_service::application::ports::UserRepositoryPort;
use auth_service::application::use_cases::login::{LoginUseCase, LoginUseCaseDeps};
use auth_service::application::use_cases::password_reset::{
    PasswordResetLinkBaseUrl, PasswordResetUseCase,
};
use auth_service::application::use_cases::signup::SignupUseCase;
use std::sync::Arc;

#[tokio::test]
async fn unknown_login_requests_equalization_and_never_opens_a_session() {
    let users = crate::support::InMemoryUserRepository::default();
    let audits = crate::support::InMemoryAuditRepository::default();
    let refresh = crate::support::InMemoryRefreshTokenStore::default();
    let trace = crate::support::security::SecurityTrace::default();
    let transactions = crate::support::security::InMemoryAuthTransactions::new(users.clone())
        .with_audits(audits.clone())
        .with_refresh_tokens(refresh.clone())
        .with_trace(trace.clone());
    let calls = transactions.calls.clone();
    let password =
        crate::support::security::RecordingPasswordWork::default().with_trace(trace.clone());
    let use_case = LoginUseCase::new(LoginUseCaseDeps {
        runtime: std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
        user_repository: users,
        login_audit_repository: audits,
        refresh_token_store: refresh,
        token_service: crate::support::TestTokenService::default(),
        transactions: crate::support::security::transaction_port(transactions),
        password_work: password.as_port(),
    });

    let error = use_case
        .login("unknown@example.com", "candidate", None, None)
        .await
        .expect_err("unknown user");
    assert_eq!(error.to_string(), "Invalid credentials");
    assert_eq!(password.equalized(), 1);
    assert_eq!(password.calls().await, vec!["equalize_login"]);
    assert_eq!(trace.entries().await, vec!["equalize_login"]);
    assert!(!calls.lock().await.contains(&"begin_session"));
}

#[tokio::test]
async fn successful_login_verifies_before_opening_the_atomic_session_boundary() {
    let users = crate::support::InMemoryUserRepository::default();
    crate::support::seed_active_user(&users, "ordered-login@example.com")
        .await
        .expect("seed");
    let audits = crate::support::InMemoryAuditRepository::default();
    let refresh = crate::support::InMemoryRefreshTokenStore::default();
    let trace = crate::support::security::SecurityTrace::default();
    let transactions = crate::support::security::InMemoryAuthTransactions::new(users.clone())
        .with_audits(audits.clone())
        .with_refresh_tokens(refresh.clone())
        .with_trace(trace.clone());
    let password =
        crate::support::security::RecordingPasswordWork::default().with_trace(trace.clone());
    let use_case = LoginUseCase::new(LoginUseCaseDeps {
        runtime: std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
        user_repository: users,
        login_audit_repository: audits,
        refresh_token_store: refresh,
        token_service: crate::support::TestTokenService::default(),
        transactions: crate::support::security::transaction_port(transactions),
        password_work: password.as_port(),
    });

    use_case
        .login("ordered-login@example.com", "Passw0rd!", None, None)
        .await
        .expect("login");
    assert_eq!(
        trace.entries().await,
        vec!["verify_login", "roles", "begin_session"]
    );
}

#[tokio::test]
async fn signup_busy_is_observed_before_the_atomic_registration_port() {
    let users = crate::support::InMemoryUserRepository::default();
    let verifications = crate::support::InMemoryVerificationRepository::default();
    let trace = crate::support::security::SecurityTrace::default();
    let transactions = crate::support::security::InMemoryAuthTransactions::new(users.clone())
        .with_verifications(verifications.clone())
        .with_trace(trace.clone());
    let calls = transactions.calls.clone();
    let password =
        crate::support::security::RecordingPasswordWork::default().with_trace(trace.clone());
    password.set_busy_signup(true);
    let use_case = SignupUseCase::new(
        users.clone(),
        crate::support::security::transaction_port(transactions),
        password.as_port(),
        std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
        std::sync::Arc::new(
            auth_service::adapters::out::security::email_verification::SecureEmailVerification,
        ),
    );

    let error = use_case
        .signup_email("busy@example.com", "Passw0rd!")
        .await
        .expect_err("busy signup");
    assert!(matches!(Some(&error), Some(AuthError::Busy)));
    assert_eq!(password.calls().await, vec!["hash_for_signup"]);
    assert_eq!(trace.entries().await, vec!["hash_for_signup"]);
    assert!(!calls.lock().await.contains(&"register_email"));
    assert!(users
        .find_by_email("busy@example.com")
        .await
        .expect("users")
        .map(auth_service::domain::user::User::from)
        .is_none());
    assert!(verifications
        .stored_code("busy@example.com")
        .await
        .is_none());
}

#[tokio::test]
async fn successful_signup_hashes_before_calling_the_atomic_registration_port() {
    let users = crate::support::InMemoryUserRepository::default();
    let verifications = crate::support::InMemoryVerificationRepository::default();
    let trace = crate::support::security::SecurityTrace::default();
    let transactions = crate::support::security::InMemoryAuthTransactions::new(users.clone())
        .with_verifications(verifications.clone())
        .with_trace(trace.clone());
    let transaction_calls = transactions.calls.clone();
    let password =
        crate::support::security::RecordingPasswordWork::default().with_trace(trace.clone());
    let use_case = SignupUseCase::new(
        users.clone(),
        crate::support::security::transaction_port(transactions),
        password.as_port(),
        std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
        std::sync::Arc::new(
            auth_service::adapters::out::security::email_verification::SecureEmailVerification,
        ),
    );

    let response = use_case
        .signup_email("ordered@example.com", "Passw0rd!")
        .await
        .expect("signup");
    assert_eq!(password.calls().await, vec!["hash_for_signup"]);
    assert_eq!(
        trace.entries().await,
        vec!["hash_for_signup", "register_email"]
    );
    assert_eq!(
        transaction_calls.lock().await.as_slice(),
        ["register_email"]
    );
    assert!(users
        .find_by_id(response.user_id)
        .await
        .expect("users")
        .map(auth_service::domain::user::User::from)
        .is_some());
}

#[tokio::test]
async fn reset_busy_preserves_the_token_and_never_calls_atomic_reset() {
    let users = crate::support::InMemoryUserRepository::default();
    crate::support::seed_active_user(&users, "reset@example.com")
        .await
        .expect("seed");
    let reset_tokens = crate::support::InMemoryPasswordResetStore::default();
    let mailer = crate::support::RecordingResetMailer::default();
    let trace = crate::support::security::SecurityTrace::default();
    let transactions = crate::support::security::InMemoryAuthTransactions::new(users.clone())
        .with_reset_tokens(reset_tokens)
        .with_mailer(mailer.clone())
        .with_trace(trace.clone());
    let calls = transactions.calls.clone();
    let password =
        crate::support::security::RecordingPasswordWork::default().with_trace(trace.clone());
    password.set_busy_reset(true);
    let use_case = PasswordResetUseCase::new(
        users,
        crate::support::security::transaction_port(transactions),
        password.as_port(),
        PasswordResetLinkBaseUrl::new("https://app.example.com/auth/reset").expect("url"),
        Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
        Arc::new(auth_service::adapters::out::security::password_reset::SecurePasswordReset),
    );
    use_case
        .request_reset("reset@example.com")
        .await
        .expect("request");
    let token = mailer.sent.lock().await[0]
        .1
        .split_once("#token=")
        .map(|(_, token)| token.to_owned())
        .expect("token");

    let error = use_case
        .confirm_reset(&token, "NewPassw0rd!")
        .await
        .expect_err("busy reset");
    assert!(matches!(Some(&error), Some(AuthError::Busy)));
    assert_eq!(password.calls().await, vec!["hash_for_reset"]);
    assert_eq!(
        trace.entries().await,
        vec![
            "queue_password_reset",
            "reset_token_exists",
            "hash_for_reset"
        ]
    );
    assert_eq!(
        calls.lock().await.as_slice(),
        ["queue_password_reset", "reset_token_exists"]
    );

    password.set_busy_reset(false);
    use_case
        .confirm_reset(&token, "NewPassw0rd!")
        .await
        .expect("token survived busy admission");
    assert!(calls.lock().await.contains(&"reset_password"));
}

#[tokio::test]
async fn login_uses_injected_clock_for_failure_audit_and_lockout_window() {
    use auth_service::application::auth_runtime::AuthRuntimePort;
    use auth_service::application::ports::LoginAuditRepositoryPort;
    use chrono::{Duration, TimeZone, Utc};
    use std::sync::Arc;
    struct FixedRuntime(chrono::DateTime<Utc>);
    impl AuthRuntimePort for FixedRuntime {
        fn now(&self) -> chrono::DateTime<Utc> {
            self.0
        }
        fn new_id(&self) -> uuid::Uuid {
            uuid::Uuid::from_u128(7)
        }
    }
    let now = Utc.timestamp_opt(1_700_000_000, 0).unwrap();
    let users = crate::support::InMemoryUserRepository::default();
    crate::support::seed_active_user(&users, "fixed-clock@example.com")
        .await
        .expect("seed");
    let user = users
        .find_by_email("fixed-clock@example.com")
        .await
        .unwrap()
        .map(auth_service::domain::user::User::from)
        .unwrap();
    let audits = crate::support::InMemoryAuditRepository::default();
    audits
        .record(
            &(&auth_service::domain::session::LoginAudit::failure(
                uuid::Uuid::new_v4(),
                user.id,
                None,
                None,
                now - Duration::seconds(901),
            ))
                .into(),
        )
        .await
        .unwrap();
    let refresh = crate::support::InMemoryRefreshTokenStore::default();
    let transactions = crate::support::security::InMemoryAuthTransactions::new(users.clone())
        .with_audits(audits.clone())
        .with_refresh_tokens(refresh.clone());
    let password = crate::support::security::RecordingPasswordWork::default();
    let use_case = LoginUseCase::new_with_lockout_policy(
        LoginUseCaseDeps {
            user_repository: users,
            login_audit_repository: audits.clone(),
            refresh_token_store: refresh,
            token_service: crate::support::TestTokenService::default(),
            transactions: crate::support::security::transaction_port(transactions),
            password_work: password.as_port(),
            runtime: Arc::new(FixedRuntime(now)),
        },
        auth_service::application::use_cases::login::LoginLockoutPolicy {
            max_failures: 1,
            window: Duration::seconds(900),
        },
    );
    use_case
        .login("fixed-clock@example.com", "bad-password", None, None)
        .await
        .expect_err("bad credential");
    assert_eq!(password.calls().await, vec!["verify_login"]);
    let recorded = audits.audits.lock().await;
    assert_eq!(recorded.last().unwrap().occurred_at, now);
    drop(recorded);
    use_case
        .login("fixed-clock@example.com", "bad-password", None, None)
        .await
        .expect_err("locked");
    assert_eq!(
        password.calls().await,
        vec!["verify_login", "equalize_login"]
    );
}
