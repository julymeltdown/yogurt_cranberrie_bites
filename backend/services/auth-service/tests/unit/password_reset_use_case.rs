use auth_service::adapters::out::security::admitted_argon2::AdmittedArgon2PasswordWork;
use auth_service::application::password_work::{
    PasswordWorkKind, PasswordWorkLimits, PasswordWorkPort,
};
use auth_service::application::ports::RefreshTokenStorePort;
use auth_service::application::use_cases::password_reset::{
    PasswordResetLinkBaseUrl, PasswordResetUseCase,
};
use auth_service::domain::session::RefreshTokenRecord;
use chrono::{Duration, Utc};
use std::sync::Arc;

fn reset_use_case(
    users: crate::support::InMemoryUserRepository,
    reset_store: crate::support::InMemoryPasswordResetStore,
    mailer: crate::support::RecordingResetMailer,
    refresh_store: crate::support::InMemoryRefreshTokenStore,
    password_work: Arc<AdmittedArgon2PasswordWork>,
) -> PasswordResetUseCase<crate::support::InMemoryUserRepository> {
    let transactions = crate::support::security::InMemoryAuthTransactions::new(users.clone())
        .with_reset_tokens(reset_store)
        .with_mailer(mailer)
        .with_refresh_tokens(refresh_store);
    PasswordResetUseCase::new(
        users,
        crate::support::security::transaction_port(transactions),
        password_work,
        PasswordResetLinkBaseUrl::new("https://app.example.com/auth/reset").expect("reset URL"),
        Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
        Arc::new(auth_service::adapters::out::security::password_reset::SecurePasswordReset),
    )
}

#[tokio::test]
async fn unknown_reset_token_is_rejected_before_hashing_or_consumption() {
    let users = crate::support::InMemoryUserRepository::default();
    let transactions = crate::support::security::InMemoryAuthTransactions::new(users.clone());
    let calls = transactions.calls.clone();
    let password = crate::support::security::RecordingPasswordWork::default();
    let use_case = PasswordResetUseCase::new(
        users,
        crate::support::security::transaction_port(transactions),
        password.as_port(),
        PasswordResetLinkBaseUrl::new("https://app.example.com/auth/reset").expect("reset URL"),
        Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
        Arc::new(auth_service::adapters::out::security::password_reset::SecurePasswordReset),
    );

    for _ in 0..5 {
        let error = use_case
            .confirm_reset("not-a-real-token", "NewPassw0rd!")
            .await
            .expect_err("unknown token must be rejected");
        assert_eq!(error.to_string(), "Invalid or expired reset token");
    }
    assert!(password.calls().await.is_empty());
    assert_eq!(
        calls.lock().await.as_slice(),
        [
            "reset_token_exists",
            "reset_token_exists",
            "reset_token_exists",
            "reset_token_exists",
            "reset_token_exists"
        ]
    );
}

#[tokio::test]
async fn request_reset_uses_fragment_link_and_confirm_revokes_sessions() {
    let users = crate::support::InMemoryUserRepository::default();
    let reset_store = crate::support::InMemoryPasswordResetStore::default();
    let mailer = crate::support::RecordingResetMailer::default();
    let refresh_store = crate::support::InMemoryRefreshTokenStore::default();
    let user_id = crate::support::seed_active_user(&users, "reset-user@example.com")
        .await
        .expect("seed user");
    refresh_store
        .store(
            &(&RefreshTokenRecord {
                jti: "active-refresh".into(),
                token_hash: "active-refresh-hash".into(),
                user_id,
                family_id: uuid::Uuid::new_v4(),
                expires_at: Utc::now() + Duration::days(1),
            })
                .into(),
        )
        .await
        .expect("store refresh token");
    let use_case = reset_use_case(
        users,
        reset_store,
        mailer.clone(),
        refresh_store.clone(),
        Arc::new(
            AdmittedArgon2PasswordWork::new(PasswordWorkLimits::default()).expect("password work"),
        ),
    );

    let response = use_case
        .request_reset("RESET-USER@example.com")
        .await
        .expect("request reset");
    assert_eq!(response.status, "SENT");
    let sent = mailer.sent.lock().await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0, "reset-user@example.com");
    let token = sent[0]
        .1
        .split_once("#token=")
        .map(|(_, token)| token.to_owned())
        .expect("fragment token");
    assert!(!sent[0].1.contains("?token="));
    drop(sent);

    let reset = use_case
        .confirm_reset(&token, "NewPassw0rd!")
        .await
        .expect("confirm reset");
    assert_eq!(reset.status, "RESET");
    assert!(refresh_store
        .find("active-refresh")
        .await
        .expect("find refresh token")
        .is_none());
    assert_eq!(
        use_case
            .confirm_reset(&token, "AnotherPassw0rd!")
            .await
            .expect_err("one-time token")
            .to_string(),
        "Invalid or expired reset token"
    );
}

#[tokio::test]
async fn known_and_unknown_requests_keep_the_same_public_response() {
    let users = crate::support::InMemoryUserRepository::default();
    crate::support::seed_active_user(&users, "known@example.com")
        .await
        .expect("seed known account");
    let mailer = crate::support::RecordingResetMailer::default();
    let use_case = reset_use_case(
        users,
        crate::support::InMemoryPasswordResetStore::default(),
        mailer.clone(),
        crate::support::InMemoryRefreshTokenStore::default(),
        Arc::new(
            AdmittedArgon2PasswordWork::new(PasswordWorkLimits::default()).expect("password work"),
        ),
    );

    let known = use_case
        .request_reset("known@example.com")
        .await
        .expect("known request");
    let unknown = use_case
        .request_reset("missing@example.com")
        .await
        .expect("unknown request");
    assert_eq!(known.status, "SENT");
    assert_eq!(unknown.status, known.status);
    assert_eq!(mailer.sent.lock().await.len(), 1);
}

#[tokio::test]
async fn busy_reset_admission_does_not_consume_the_token() {
    use auth_service::application::errors::AuthError;

    let users = crate::support::InMemoryUserRepository::default();
    crate::support::seed_active_user(&users, "busy-reset@example.com")
        .await
        .expect("seed user");
    let reset_store = crate::support::InMemoryPasswordResetStore::default();
    let mailer = crate::support::RecordingResetMailer::default();
    let transactions = crate::support::security::InMemoryAuthTransactions::new(users.clone())
        .with_reset_tokens(reset_store)
        .with_mailer(mailer.clone());
    let calls = transactions.calls.clone();
    let password_work = crate::support::small_password_work(4);
    let use_case = PasswordResetUseCase::new(
        users,
        crate::support::security::transaction_port(transactions),
        password_work.clone(),
        PasswordResetLinkBaseUrl::new("https://app.example.com/auth/reset").expect("reset URL"),
        Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
        Arc::new(auth_service::adapters::out::security::password_reset::SecurePasswordReset),
    );
    use_case
        .request_reset("busy-reset@example.com")
        .await
        .expect("request reset");
    let token = mailer.sent.lock().await[0]
        .1
        .split_once("#token=")
        .map(|(_, token)| token.to_owned())
        .expect("fragment token");

    let slot = crate::support::occupy(password_work.admission(PasswordWorkKind::Reset)).await;
    let error = use_case
        .confirm_reset(&token, "NewPassw0rd!")
        .await
        .expect_err("the reset class is full");
    assert!(matches!(Some(&error), Some(AuthError::Busy)));
    assert!(!calls.lock().await.contains(&"reset_password"));

    slot.release().await;
    use_case
        .confirm_reset(&token, "NewPassw0rd!")
        .await
        .expect("same token succeeds after capacity returns");
    assert_eq!(
        calls
            .lock()
            .await
            .iter()
            .filter(|call| **call == "reset_password")
            .count(),
        1
    );
}

#[tokio::test]
async fn duplicate_in_flight_confirmation_is_refused_before_a_second_hash() {
    use auth_service::application::errors::AuthError;

    let users = crate::support::InMemoryUserRepository::default();
    crate::support::seed_active_user(&users, "concurrent-reset@example.com")
        .await
        .expect("seed user");
    let mailer = crate::support::RecordingResetMailer::default();
    let password_work = Arc::new(
        AdmittedArgon2PasswordWork::new(PasswordWorkLimits {
            workers: 3,
            login_queue: 0,
            signup_workers: 1,
            signup_queue: 0,
            reset_workers: 1,
            reset_queue: 2,
        })
        .expect("password work"),
    );
    let use_case = reset_use_case(
        users,
        crate::support::InMemoryPasswordResetStore::default(),
        mailer.clone(),
        crate::support::InMemoryRefreshTokenStore::default(),
        password_work.clone(),
    );
    use_case
        .request_reset("concurrent-reset@example.com")
        .await
        .expect("request reset");
    let token = mailer.sent.lock().await[0]
        .1
        .split_once("#token=")
        .map(|(_, token)| token.to_owned())
        .expect("fragment token");

    let slot = crate::support::occupy(password_work.admission(PasswordWorkKind::Reset)).await;
    let first_use_case = use_case.clone();
    let first_token = token.clone();
    let first = tokio::spawn(async move {
        first_use_case
            .confirm_reset(&first_token, "NewPassw0rd!")
            .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while password_work.stats(PasswordWorkKind::Reset).admitted < 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("first reset hash entered the admitted queue");

    let error = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        use_case.confirm_reset(&token, "OtherPassw0rd!"),
    )
    .await
    .expect("duplicate is refused before queueing")
    .expect_err("token is already in flight");
    assert!(matches!(Some(&error), Some(AuthError::Busy)));
    assert_eq!(password_work.stats(PasswordWorkKind::Reset).admitted, 2);

    slot.release().await;
    first.await.expect("task").expect("first confirmation");
}

struct ControlledResetClock(std::sync::Mutex<chrono::DateTime<Utc>>);
impl auth_service::application::auth_runtime::AuthRuntimePort for ControlledResetClock {
    fn now(&self) -> chrono::DateTime<Utc> {
        *self.0.lock().unwrap()
    }
    fn new_id(&self) -> uuid::Uuid {
        uuid::Uuid::nil()
    }
}
struct ControlledResetCrypto {
    fail: bool,
    calls: std::sync::atomic::AtomicUsize,
}
impl auth_service::application::password_reset_crypto::PasswordResetCryptoPort
    for ControlledResetCrypto
{
    fn generate(
        &self,
    ) -> auth_service::application::errors::AuthResult<
        auth_service::application::dto::password_reset_material_dto::PasswordResetMaterialDto,
    > {
        let result: anyhow::Result<_> = (|| {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            anyhow::ensure!(!self.fail, "controlled entropy failure");
            Ok(
            auth_service::application::dto::password_reset_material_dto::PasswordResetMaterialDto {
                token: "controlled-reset-secret".into(),
                fingerprint: self.fingerprint("controlled-reset-secret"),
            },
        )
        })();
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }
    fn fingerprint(&self, token: &str) -> String {
        format!("controlled-fingerprint:{token}")
    }
}
struct ExpiringResetPasswordWork {
    clock: Arc<ControlledResetClock>,
    expiry: chrono::DateTime<Utc>,
}
#[async_trait::async_trait]
impl PasswordWorkPort for ExpiringResetPasswordWork {
    async fn verify_login(
        &self,
        _: &str,
        _: &str,
    ) -> auth_service::application::errors::AuthResult<bool> {
        let result: anyhow::Result<_> = async { anyhow::bail!("unexpected login") }.await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }
    async fn equalize_login(&self, _: &str) -> auth_service::application::errors::AuthResult<()> {
        let result: anyhow::Result<_> = async { anyhow::bail!("unexpected login") }.await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }
    async fn hash_for_signup(
        &self,
        _: &str,
    ) -> auth_service::application::errors::AuthResult<String> {
        let result: anyhow::Result<_> = async { anyhow::bail!("unexpected signup") }.await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }
    async fn hash_for_reset(
        &self,
        _: &str,
    ) -> auth_service::application::errors::AuthResult<String> {
        let result: anyhow::Result<_> = async {
            *self.clock.0.lock().unwrap() = self.expiry;
            Ok("replacement-must-not-commit".into())
        }
        .await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }
    fn stats(
        &self,
        _: PasswordWorkKind,
    ) -> auth_service::application::password_work::PasswordWorkStats {
        Default::default()
    }
}

#[tokio::test]
async fn reset_material_failure_queues_no_token_or_mail_and_unknown_user_spends_no_entropy() {
    let users = crate::support::InMemoryUserRepository::default();
    crate::support::seed_active_user(&users, "crypto-reset@example.com")
        .await
        .unwrap();
    let transactions = crate::support::security::InMemoryAuthTransactions::new(users.clone());
    let calls = transactions.calls.clone();
    let crypto = Arc::new(ControlledResetCrypto {
        fail: true,
        calls: Default::default(),
    });
    let now = "2070-01-01T00:00:00Z".parse().unwrap();
    let use_case = PasswordResetUseCase::new(
        users,
        crate::support::security::transaction_port(transactions),
        crate::support::security::RecordingPasswordWork::default().as_port(),
        PasswordResetLinkBaseUrl::new("https://app.example.com/auth/reset").unwrap(),
        Arc::new(ControlledResetClock(std::sync::Mutex::new(now))),
        crypto.clone(),
    );
    assert_eq!(
        use_case
            .request_reset("unknown@example.com")
            .await
            .unwrap()
            .status,
        "SENT"
    );
    assert_eq!(crypto.calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert!(use_case
        .request_reset("crypto-reset@example.com")
        .await
        .is_err());
    assert_eq!(crypto.calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert!(calls.lock().await.is_empty());
}

#[tokio::test]
async fn reset_expiry_during_password_work_uses_fresh_confirmation_time() {
    use auth_service::application::ports::{PasswordResetStorePort, UserRepositoryPort};
    let users = crate::support::InMemoryUserRepository::default();
    let user_id = crate::support::seed_active_user(&users, "clock-reset@example.com")
        .await
        .unwrap();
    let user = users
        .find_by_id(user_id)
        .await
        .unwrap()
        .map(auth_service::domain::user::User::from)
        .unwrap();
    let store = crate::support::InMemoryPasswordResetStore::default();
    let transactions = crate::support::security::InMemoryAuthTransactions::new(users.clone())
        .with_reset_tokens(store.clone());
    let now: chrono::DateTime<Utc> = "2070-01-01T00:00:00Z".parse().unwrap();
    let expiry = now + Duration::hours(1);
    let clock = Arc::new(ControlledResetClock(std::sync::Mutex::new(now)));
    let crypto = Arc::new(ControlledResetCrypto {
        fail: false,
        calls: Default::default(),
    });
    let use_case = PasswordResetUseCase::new(
        users.clone(),
        crate::support::security::transaction_port(transactions),
        Arc::new(ExpiringResetPasswordWork {
            clock: clock.clone(),
            expiry,
        }),
        PasswordResetLinkBaseUrl::new("https://app.example.com/auth/reset").unwrap(),
        clock,
        crypto,
    );
    use_case
        .request_reset("clock-reset@example.com")
        .await
        .unwrap();
    assert!(store
        .exists(
            "controlled-fingerprint:controlled-reset-secret",
            expiry - Duration::microseconds(1)
        )
        .await
        .unwrap());
    let error = use_case
        .confirm_reset("controlled-reset-secret", "NewPassw0rd!")
        .await
        .unwrap_err();
    assert_eq!(error.to_string(), "Invalid or expired reset token");
    assert_eq!(
        users
            .find_by_id(user.id)
            .await
            .unwrap()
            .map(auth_service::domain::user::User::from)
            .unwrap()
            .password_hash,
        user.password_hash
    );
}
