use auth_service::adapters::out::security::admitted_argon2::AdmittedArgon2PasswordWork;
use auth_service::application::errors::AuthError;
use auth_service::application::password_work::{
    PasswordWorkKind, PasswordWorkLimits, PasswordWorkPort,
};
use auth_service::application::ports::{UserRepositoryPort, VerificationRepositoryPort};
use auth_service::application::use_cases::signup::SignupUseCase;
use auth_service::domain::user::UserStatus;
use std::sync::Arc;

fn use_case(
    users: crate::support::InMemoryUserRepository,
    verifications: crate::support::InMemoryVerificationRepository,
    password_work: Arc<AdmittedArgon2PasswordWork>,
) -> SignupUseCase<crate::support::InMemoryUserRepository> {
    let transactions = crate::support::security::InMemoryAuthTransactions::new(users.clone())
        .with_verifications(verifications);
    SignupUseCase::new(
        users,
        crate::support::security::transaction_port(transactions),
        password_work,
        std::sync::Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
        std::sync::Arc::new(
            auth_service::adapters::out::security::email_verification::SecureEmailVerification,
        ),
    )
}

#[tokio::test]
async fn signup_creates_pending_user_and_verification_code() {
    let users = crate::support::InMemoryUserRepository::default();
    let verifications = crate::support::InMemoryVerificationRepository::default();
    let use_case = use_case(
        users.clone(),
        verifications.clone(),
        Arc::new(
            AdmittedArgon2PasswordWork::new(PasswordWorkLimits::default()).expect("password work"),
        ),
    );

    let response = use_case
        .signup_email("new-user@example.com", "Passw0rd!")
        .await
        .expect("signup");

    let user = users
        .find_by_id(response.user_id)
        .await
        .expect("repo")
        .map(auth_service::domain::user::User::from)
        .expect("user");
    assert_eq!(response.status, "PENDING_VERIFICATION");
    assert_eq!(user.status, UserStatus::PendingVerification);
    let stored_code = verifications
        .stored_code("new-user@example.com")
        .await
        .expect("verification code");
    assert_ne!(stored_code, "123456");
    assert_eq!(stored_code.len(), 6);
    assert!(stored_code.chars().all(|ch| ch.is_ascii_digit()));
    assert!(verifications
        .consume_code("new-user@example.com", &stored_code)
        .await
        .expect("consume")
        .is_some());
}

#[tokio::test]
async fn verify_promotes_user_to_active() {
    let users = crate::support::InMemoryUserRepository::default();
    let verifications = crate::support::InMemoryVerificationRepository::default();
    let use_case = use_case(
        users.clone(),
        verifications.clone(),
        Arc::new(
            AdmittedArgon2PasswordWork::new(PasswordWorkLimits::default()).expect("password work"),
        ),
    );

    let signup = use_case
        .signup_email("verify-me@example.com", "Passw0rd!")
        .await
        .expect("signup");
    let code = verifications
        .stored_code("verify-me@example.com")
        .await
        .expect("verification code");
    let verify = use_case
        .verify_email("verify-me@example.com", &code)
        .await
        .expect("verify");

    assert_eq!(signup.user_id, verify.user_id);
    let user = users
        .find_by_id(verify.user_id)
        .await
        .expect("repo")
        .map(auth_service::domain::user::User::from)
        .expect("user");
    assert_eq!(user.status, UserStatus::Active);
}

#[tokio::test]
async fn equalizing_work_is_repeatable_and_charged_to_the_login_class() {
    let password_work =
        AdmittedArgon2PasswordWork::new(PasswordWorkLimits::default()).expect("password work");

    for _ in 0..3 {
        password_work
            .equalize_login("any-candidate-password")
            .await
            .expect("equalized work must complete");
    }
    assert_eq!(password_work.stats(PasswordWorkKind::Login).admitted, 3);
    assert_eq!(password_work.stats(PasswordWorkKind::Signup).admitted, 0);
    assert_eq!(password_work.stats(PasswordWorkKind::Reset).admitted, 0);
}

#[tokio::test]
async fn a_saturated_signup_class_answers_busy_before_creating_the_account() {
    let users = crate::support::InMemoryUserRepository::default();
    let verifications = crate::support::InMemoryVerificationRepository::default();
    let password_work = crate::support::small_password_work(4);
    let use_case = use_case(users.clone(), verifications, password_work.clone());

    let slot = crate::support::occupy(password_work.admission(PasswordWorkKind::Signup)).await;
    let error = use_case
        .signup_email("busy-signup@example.com", "Passw0rd!")
        .await
        .expect_err("the signup class is full");
    assert!(matches!(Some(&error), Some(AuthError::Busy)));
    assert!(users
        .find_by_email("busy-signup@example.com")
        .await
        .expect("repository")
        .map(auth_service::domain::user::User::from)
        .is_none());

    slot.release().await;
    use_case
        .signup_email("busy-signup@example.com", "Passw0rd!")
        .await
        .expect("signup succeeds once capacity returns");
}
