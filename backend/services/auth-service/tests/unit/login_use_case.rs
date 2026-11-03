use auth_service::adapters::out::security::admitted_argon2::AdmittedArgon2PasswordWork;
use auth_service::application::errors::AuthError;
use auth_service::application::password_work::{
    PasswordWorkKind, PasswordWorkLimits, PasswordWorkPort,
};
use auth_service::application::ports::{
    LoginAuditRepositoryPort, RefreshTokenStorePort, TokenServicePort, UserRepositoryPort,
};
use auth_service::application::use_cases::login::{
    LoginLockoutPolicy, LoginUseCase as RealLoginUseCase, LoginUseCaseDeps,
};
use chrono::{Duration, Utc};
use std::sync::Arc;

/// Test-only factory retaining the old concise call shape while explicitly
/// injecting both mandatory security capabilities into the real use case.
struct LoginUseCase;

impl LoginUseCase {
    fn build_real(
        users: crate::support::InMemoryUserRepository,
        audits: crate::support::InMemoryAuditRepository,
        refresh_store: crate::support::InMemoryRefreshTokenStore,
        token_service: crate::support::TestTokenService,
    ) -> RealLoginUseCase<
        crate::support::InMemoryUserRepository,
        crate::support::InMemoryAuditRepository,
        crate::support::InMemoryRefreshTokenStore,
        crate::support::TestTokenService,
    > {
        Self::new_with_password_work(
            users,
            audits,
            refresh_store,
            token_service,
            Arc::new(
                AdmittedArgon2PasswordWork::new(PasswordWorkLimits::default())
                    .expect("password work"),
            ),
        )
    }

    fn new_with_password_work(
        users: crate::support::InMemoryUserRepository,
        audits: crate::support::InMemoryAuditRepository,
        refresh_store: crate::support::InMemoryRefreshTokenStore,
        token_service: crate::support::TestTokenService,
        password_work: Arc<AdmittedArgon2PasswordWork>,
    ) -> RealLoginUseCase<
        crate::support::InMemoryUserRepository,
        crate::support::InMemoryAuditRepository,
        crate::support::InMemoryRefreshTokenStore,
        crate::support::TestTokenService,
    > {
        let transactions = crate::support::security::InMemoryAuthTransactions::new(users.clone())
            .with_audits(audits.clone())
            .with_refresh_tokens(refresh_store.clone());
        RealLoginUseCase::new(LoginUseCaseDeps {
            runtime: std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
            user_repository: users,
            login_audit_repository: audits,
            refresh_token_store: refresh_store,
            token_service,
            transactions: crate::support::security::transaction_port(transactions),
            password_work,
        })
    }

    fn new_with_lockout_policy(
        users: crate::support::InMemoryUserRepository,
        audits: crate::support::InMemoryAuditRepository,
        refresh_store: crate::support::InMemoryRefreshTokenStore,
        token_service: crate::support::TestTokenService,
        lockout_policy: LoginLockoutPolicy,
    ) -> RealLoginUseCase<
        crate::support::InMemoryUserRepository,
        crate::support::InMemoryAuditRepository,
        crate::support::InMemoryRefreshTokenStore,
        crate::support::TestTokenService,
    > {
        let transactions = crate::support::security::InMemoryAuthTransactions::new(users.clone())
            .with_audits(audits.clone())
            .with_refresh_tokens(refresh_store.clone());
        RealLoginUseCase::new_with_lockout_policy(
            LoginUseCaseDeps {
                runtime: std::sync::Arc::new(
                    auth_service::adapters::out::runtime::SystemAuthRuntime,
                ),
                user_repository: users,
                login_audit_repository: audits,
                refresh_token_store: refresh_store,
                token_service,
                transactions: crate::support::security::transaction_port(transactions),
                password_work: Arc::new(
                    AdmittedArgon2PasswordWork::new(PasswordWorkLimits::default())
                        .expect("password work"),
                ),
            },
            lockout_policy,
        )
    }
}

#[tokio::test]
async fn known_and_unknown_accounts_have_the_same_public_login_failure() {
    let users = crate::support::InMemoryUserRepository::default();
    crate::support::seed_active_user(&users, "member@example.com")
        .await
        .expect("seed known account");
    let audits = crate::support::InMemoryAuditRepository::default();
    let refresh_store = crate::support::InMemoryRefreshTokenStore::default();
    let password_work = Arc::new(
        AdmittedArgon2PasswordWork::new(PasswordWorkLimits::default()).expect("password work"),
    );
    let use_case = LoginUseCase::new_with_password_work(
        users,
        audits.clone(),
        refresh_store,
        crate::support::TestTokenService::default(),
        password_work.clone(),
    );

    let known_account = use_case
        .login("member@example.com", "WrongPassw0rd!", None, None)
        .await
        .expect_err("a wrong password must not authenticate");
    let unknown = use_case
        .login("nobody@example.com", "WrongPassw0rd!", None, None)
        .await
        .expect_err("an unknown account must not authenticate");
    assert_eq!(known_account.to_string(), "Invalid credentials");
    assert_eq!(unknown.to_string(), known_account.to_string());
    assert_eq!(password_work.stats(PasswordWorkKind::Login).admitted, 2);
    assert_eq!(audits.audits.lock().await.len(), 1);
}

#[tokio::test]
async fn login_keeps_a_reserved_worker_while_signup_and_reset_are_saturated() {
    let users = crate::support::InMemoryUserRepository::default();
    crate::support::seed_active_user(&users, "hana@example.com")
        .await
        .expect("seed");
    let password_work = crate::support::small_password_work(0);
    let use_case = LoginUseCase::new_with_password_work(
        users,
        crate::support::InMemoryAuditRepository::default(),
        crate::support::InMemoryRefreshTokenStore::default(),
        crate::support::TestTokenService::default(),
        password_work.clone(),
    );

    let signup = crate::support::occupy(password_work.admission(PasswordWorkKind::Signup)).await;
    let reset = crate::support::occupy(password_work.admission(PasswordWorkKind::Reset)).await;
    for refused in [
        password_work.hash_for_signup("NewPassw0rd!").await,
        password_work.hash_for_reset("NewPassw0rd!").await,
    ] {
        let refused = refused.expect_err("each capped class is at its limit");
        assert!(matches!(Some(&refused), Some(AuthError::Busy)));
    }

    let response = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        use_case.login("hana@example.com", "Passw0rd!", None, None),
    )
    .await
    .expect("login must not wait behind signup or reset work")
    .expect("login");
    assert!(!response.access_token.is_empty());

    signup.release().await;
    reset.release().await;
}

#[tokio::test]
async fn a_saturated_login_class_answers_busy_without_recording_a_failure() {
    let users = crate::support::InMemoryUserRepository::default();
    crate::support::seed_active_user(&users, "hana@example.com")
        .await
        .expect("seed");
    let audits = crate::support::InMemoryAuditRepository::default();
    let password_work = crate::support::small_password_work(0);
    let use_case = LoginUseCase::new_with_password_work(
        users,
        audits.clone(),
        crate::support::InMemoryRefreshTokenStore::default(),
        crate::support::TestTokenService::default(),
        password_work.clone(),
    );

    let mut slots = Vec::new();
    for _ in 0..3 {
        slots.push(crate::support::occupy(password_work.admission(PasswordWorkKind::Login)).await);
    }
    let error = use_case
        .login("hana@example.com", "Passw0rd!", None, None)
        .await
        .expect_err("no login capacity is left");
    assert!(matches!(Some(&error), Some(AuthError::Busy)));
    assert!(audits.audits.lock().await.is_empty());
    assert_eq!(password_work.stats(PasswordWorkKind::Login).rejected, 1);

    for slot in slots {
        slot.release().await;
    }
    use_case
        .login("hana@example.com", "Passw0rd!", None, None)
        .await
        .expect("login succeeds once capacity returns");
}

#[tokio::test]
async fn login_issues_tokens_and_records_audit() {
    let users = crate::support::InMemoryUserRepository::default();
    let audits = crate::support::InMemoryAuditRepository::default();
    let refresh_store = crate::support::InMemoryRefreshTokenStore::default();
    let token_service = crate::support::TestTokenService::default();
    crate::support::seed_active_user(&users, "hana@example.com")
        .await
        .expect("seed");
    let use_case = LoginUseCase::build_real(users, audits.clone(), refresh_store, token_service);

    let response = use_case
        .login(
            "hana@example.com",
            "Passw0rd!",
            Some("127.0.0.1".into()),
            Some("unit-test".into()),
        )
        .await
        .expect("login");

    assert_eq!(response.token_type, "Bearer");
    assert!(!response.access_token.is_empty());
    assert!(!response.refresh_token.is_empty());
    assert_eq!(audits.audits.lock().await.len(), 1);
}

#[tokio::test]
async fn refresh_rotates_refresh_token() {
    let users = crate::support::InMemoryUserRepository::default();
    let audits = crate::support::InMemoryAuditRepository::default();
    let refresh_store = crate::support::InMemoryRefreshTokenStore::default();
    let token_service = crate::support::TestTokenService::default();
    crate::support::seed_active_user(&users, "hana@example.com")
        .await
        .expect("seed");
    let use_case =
        LoginUseCase::build_real(users, audits, refresh_store.clone(), token_service.clone());

    let login = use_case
        .login("hana@example.com", "Passw0rd!", None, None)
        .await
        .expect("login");
    let refreshed = use_case
        .refresh(&login.refresh_token)
        .await
        .expect("refresh");

    assert_ne!(login.refresh_token, refreshed.refresh_token);
    assert!(token_service
        .decode_refresh_token(&refreshed.refresh_token)
        .is_ok());
}

#[tokio::test]
async fn refresh_denies_deleted_user_and_revokes_all_user_tokens() {
    let users = crate::support::InMemoryUserRepository::default();
    let audits = crate::support::InMemoryAuditRepository::default();
    let refresh_store = crate::support::InMemoryRefreshTokenStore::default();
    let token_service = crate::support::TestTokenService::default();
    let user_id = crate::support::seed_active_user(&users, "hana@example.com")
        .await
        .expect("seed");
    let use_case = LoginUseCase::build_real(
        users.clone(),
        audits,
        refresh_store.clone(),
        token_service.clone(),
    );

    let first = use_case
        .login("hana@example.com", "Passw0rd!", None, None)
        .await
        .expect("first login");
    let second = use_case
        .login("hana@example.com", "Passw0rd!", None, None)
        .await
        .expect("second login");
    let first_jti = token_service
        .decode_refresh_token(&first.refresh_token)
        .expect("decode first refresh token")
        .jti;
    let second_jti = token_service
        .decode_refresh_token(&second.refresh_token)
        .expect("decode second refresh token")
        .jti;
    users
        .soft_delete_user(
            user_id,
            user_id,
            "unit_test",
            Utc::now() + Duration::days(30),
        )
        .await
        .expect("soft delete");

    let denied_error = use_case
        .refresh(&first.refresh_token)
        .await
        .expect_err("deleted user refresh should be denied");
    assert!(denied_error.to_string().contains("no longer valid"));
    assert!(refresh_store
        .find(&first_jti)
        .await
        .expect("find first token")
        .is_none());
    assert!(refresh_store
        .find(&second_jti)
        .await
        .expect("find second token")
        .is_none());
}

#[tokio::test]
async fn refresh_denies_missing_user_and_revokes_all_user_tokens() {
    let users = crate::support::InMemoryUserRepository::default();
    let audits = crate::support::InMemoryAuditRepository::default();
    let refresh_store = crate::support::InMemoryRefreshTokenStore::default();
    let token_service = crate::support::TestTokenService::default();
    let user_id = crate::support::seed_active_user(&users, "hana@example.com")
        .await
        .expect("seed");
    let use_case = LoginUseCase::build_real(
        users.clone(),
        audits,
        refresh_store.clone(),
        token_service.clone(),
    );

    let first = use_case
        .login("hana@example.com", "Passw0rd!", None, None)
        .await
        .expect("first login");
    let second = use_case
        .login("hana@example.com", "Passw0rd!", None, None)
        .await
        .expect("second login");
    let first_jti = token_service
        .decode_refresh_token(&first.refresh_token)
        .expect("decode first refresh token")
        .jti;
    let second_jti = token_service
        .decode_refresh_token(&second.refresh_token)
        .expect("decode second refresh token")
        .jti;
    users.remove_by_id(user_id).await.expect("remove user");

    let denied_error = use_case
        .refresh(&first.refresh_token)
        .await
        .expect_err("missing user refresh should be denied");
    assert!(denied_error.to_string().contains("no longer valid"));
    assert!(refresh_store
        .find(&first_jti)
        .await
        .expect("find first token")
        .is_none());
    assert!(refresh_store
        .find(&second_jti)
        .await
        .expect("find second token")
        .is_none());
}

#[tokio::test]
async fn concurrent_refresh_within_grace_is_retryable_and_keeps_the_family() {
    let users = crate::support::InMemoryUserRepository::default();
    let audits = crate::support::InMemoryAuditRepository::default();
    let refresh_store = crate::support::InMemoryRefreshTokenStore::default();
    let token_service = crate::support::TestTokenService::default();
    crate::support::seed_active_user(&users, "hana@example.com")
        .await
        .expect("seed");
    let use_case = LoginUseCase::build_real(users, audits, refresh_store.clone(), token_service)
        .with_refresh_grace_window(Duration::seconds(30));

    let login = use_case
        .login("hana@example.com", "Passw0rd!", None, None)
        .await
        .expect("login");
    let refreshed = use_case
        .refresh(&login.refresh_token)
        .await
        .expect("refresh");
    let conflict = use_case
        .refresh(&login.refresh_token)
        .await
        .expect_err("the superseded token must not rotate twice");
    assert!(conflict.to_string().contains("Refresh token conflict"));
    assert!(use_case.refresh(&refreshed.refresh_token).await.is_ok());
}

#[tokio::test]
async fn refresh_reuse_outside_the_grace_window_revokes_the_family() {
    let users = crate::support::InMemoryUserRepository::default();
    let audits = crate::support::InMemoryAuditRepository::default();
    let refresh_store = crate::support::InMemoryRefreshTokenStore::default();
    let token_service = crate::support::TestTokenService::default();
    crate::support::seed_active_user(&users, "hana@example.com")
        .await
        .expect("seed");
    let use_case = LoginUseCase::build_real(users, audits, refresh_store.clone(), token_service)
        .with_refresh_grace_window(Duration::zero());

    let login = use_case
        .login("hana@example.com", "Passw0rd!", None, None)
        .await
        .expect("login");
    let refreshed = use_case
        .refresh(&login.refresh_token)
        .await
        .expect("refresh");
    let reuse = use_case
        .refresh(&login.refresh_token)
        .await
        .expect_err("reuse of a superseded token must be rejected");
    assert!(reuse.to_string().contains("no longer valid"));
    assert!(use_case.refresh(&refreshed.refresh_token).await.is_err());
}

#[tokio::test]
async fn a_stale_tab_refreshing_after_logout_does_not_disturb_other_devices() {
    let users = crate::support::InMemoryUserRepository::default();
    let audits = crate::support::InMemoryAuditRepository::default();
    let refresh_store = crate::support::InMemoryRefreshTokenStore::default();
    let token_service = crate::support::TestTokenService::default();
    crate::support::seed_active_user(&users, "hana@example.com")
        .await
        .expect("seed");
    let use_case =
        LoginUseCase::build_real(users, audits, refresh_store.clone(), token_service.clone());

    let first = use_case
        .login("hana@example.com", "Passw0rd!", None, None)
        .await
        .expect("first login");
    let second = use_case
        .login("hana@example.com", "Passw0rd!", None, None)
        .await
        .expect("second login");
    use_case.logout(&first.refresh_token).await.expect("logout");
    assert!(use_case.refresh(&first.refresh_token).await.is_err());
    assert!(use_case.refresh(&second.refresh_token).await.is_ok());
}

#[tokio::test]
async fn repeated_login_failures_lock_out_user_temporarily() {
    let users = crate::support::InMemoryUserRepository::default();
    let audits = crate::support::InMemoryAuditRepository::default();
    let refresh_store = crate::support::InMemoryRefreshTokenStore::default();
    let token_service = crate::support::TestTokenService::default();
    let user_id = crate::support::seed_active_user(&users, "hana@example.com")
        .await
        .expect("seed");
    let use_case = LoginUseCase::new_with_lockout_policy(
        users,
        audits.clone(),
        refresh_store.clone(),
        token_service,
        LoginLockoutPolicy {
            max_failures: 2,
            window: Duration::seconds(900),
        },
    );

    assert!(use_case
        .login("hana@example.com", "wrong-one", None, None)
        .await
        .is_err());
    assert!(use_case
        .login("hana@example.com", "wrong-two", None, None)
        .await
        .is_err());
    let locked_error = use_case
        .login("hana@example.com", "Passw0rd!", None, None)
        .await
        .expect_err("lockout should block valid password");
    assert!(locked_error.to_string().contains("Invalid credentials"));
    assert!(refresh_store
        .find("any-refresh-token")
        .await
        .expect("find")
        .is_none());
    assert_eq!(
        audits
            .count_recent_failures(user_id, chrono::Utc::now() - Duration::seconds(900))
            .await
            .expect("count failures"),
        2
    );
}

#[tokio::test]
async fn requests_during_lockout_cannot_extend_it() {
    let users = crate::support::InMemoryUserRepository::default();
    let audits = crate::support::InMemoryAuditRepository::default();
    let refresh_store = crate::support::InMemoryRefreshTokenStore::default();
    let user_id = crate::support::seed_active_user(&users, "hana@example.com")
        .await
        .expect("seed");
    let use_case = LoginUseCase::new_with_lockout_policy(
        users,
        audits.clone(),
        refresh_store,
        crate::support::TestTokenService::default(),
        LoginLockoutPolicy {
            max_failures: 2,
            window: Duration::seconds(900),
        },
    );

    for attempt in 0..2 {
        assert!(use_case
            .login("hana@example.com", &format!("wrong-{attempt}"), None, None)
            .await
            .is_err());
    }
    let window_start = chrono::Utc::now() - Duration::seconds(900);
    let failures_at_lock = audits
        .count_recent_failures(user_id, window_start)
        .await
        .expect("count");
    assert_eq!(failures_at_lock, 2);
    for _ in 0..25 {
        assert!(use_case
            .login("hana@example.com", "Passw0rd!", None, None)
            .await
            .is_err());
    }
    assert_eq!(
        audits
            .count_recent_failures(user_id, window_start)
            .await
            .expect("count"),
        failures_at_lock
    );
    assert!(audits.audits.lock().await.len() >= 27);
}

#[tokio::test]
async fn session_status_ends_at_logout_while_the_access_token_stays_valid() {
    use auth_service::domain::session::SessionStatus;

    let users = crate::support::InMemoryUserRepository::default();
    let token_service = crate::support::TestTokenService::default();
    crate::support::seed_active_user(&users, "hana@example.com")
        .await
        .expect("seed");
    let use_case = LoginUseCase::build_real(
        users,
        crate::support::InMemoryAuditRepository::default(),
        crate::support::InMemoryRefreshTokenStore::default(),
        token_service.clone(),
    );
    let first = use_case
        .login("hana@example.com", "Passw0rd!", None, None)
        .await
        .expect("first login");
    let second = use_case
        .login("hana@example.com", "Passw0rd!", None, None)
        .await
        .expect("second login");
    let session_of = |token: &str| {
        token_service
            .decode_access_token(token)
            .expect("decode")
            .session_id
            .expect("access tokens name their session")
    };
    let first_session = session_of(&first.access_token);
    let second_session = session_of(&second.access_token);
    assert_ne!(first_session, second_session);

    let refreshed = use_case
        .refresh(&first.refresh_token)
        .await
        .expect("refresh");
    assert_eq!(session_of(&refreshed.access_token), first_session);
    let before_logout = use_case
        .session_status(first.user_id, first_session)
        .await
        .expect("status");
    use_case
        .logout(&refreshed.refresh_token)
        .await
        .expect("logout");

    assert_eq!(
        before_logout,
        SessionStatus::Active {
            roles: vec!["ROLE_USER".into()]
        }
    );
    assert_eq!(
        use_case
            .session_status(first.user_id, first_session)
            .await
            .expect("status"),
        SessionStatus::Ended
    );
    assert!(matches!(
        use_case
            .session_status(second.user_id, second_session)
            .await
            .expect("status"),
        SessionStatus::Active { .. }
    ));
    assert!(token_service
        .decode_access_token(&refreshed.access_token)
        .is_ok());
    assert_eq!(
        use_case
            .session_status(uuid::Uuid::new_v4(), second_session)
            .await
            .expect("status"),
        SessionStatus::Ended
    );
}
