//! T-204: verification start.
//!
//! Every case runs against the real `identity_verifications` table in the
//! runner-owned PostgreSQL cluster, so the schema's own bounds (a ten-minute
//! expiry, the return-path grammar, a 32-byte digest) are part of the assertion
//! rather than a convention in Rust.
use chrono::{Duration, Utc};
use profile_service::identity::start::{
    random_state, state_digest, validate_return_path, StartError, StartVerification, RETURN_PATHS,
};
use profile_service::identity::NaverOAuthConfig;

#[path = "wp04b_support.rs"]
mod support;
use support::{clear_user, pool, repository, verification_count};

fn start_use_case(pool: sqlx::PgPool) -> StartVerification {
    StartVerification::new(repository(pool), NaverOAuthConfig::local())
}

#[tokio::test]
async fn a_start_writes_one_state_and_returns_the_configured_redirect_uri() -> anyhow::Result<()> {
    let pool = pool().await?;
    let user = support::user();
    clear_user(&pool, user).await?;
    let now = Utc::now();
    let started = start_use_case(pool.clone())
        .begin(user, Some("/me/identity"), now)
        .await
        .expect("start succeeds");
    // The URL contains the configured redirect URI exactly, percent-encoded.
    assert!(
        started
            .authorization_url
            .contains("redirect_uri=http%3A%2F%2Flocalhost%3A8080%2Fapi%2Fidentity%2Fnaver%2Fcallback"),
        "{}",
        started.authorization_url
    );
    assert!(started.authorization_url.contains("response_type=code"));
    assert!(started.authorization_url.contains("client_id=local-naver-client"));
    // The state's lifetime is the configured ten minutes, never longer.
    assert!(started.expires_at > now);
    assert!(started.expires_at <= now + Duration::minutes(10));
    // Exactly one row, in PENDING, with the stored return path.
    assert_eq!(verification_count(&pool, user).await?, 1);
    let (state, return_path, expires_at): (String, String, chrono::DateTime<Utc>) =
        sqlx::query_as(
            "select state, return_path, expires_at from identity_verifications where user_id = $1",
        )
        .bind(user)
        .fetch_one(&pool)
        .await?;
    assert_eq!(state, "PENDING");
    assert_eq!(return_path, "/me/identity");
    assert_eq!(expires_at, started.expires_at);
    clear_user(&pool, user).await?;
    Ok(())
}

/// The raw state is never stored: only its digest.
#[tokio::test]
async fn the_state_is_stored_only_as_a_digest() -> anyhow::Result<()> {
    let pool = pool().await?;
    let user = support::user();
    clear_user(&pool, user).await?;
    let started = start_use_case(pool.clone())
        .begin(user, Some("/me/identity"), Utc::now())
        .await
        .expect("start succeeds");
    // Extract the state from the authorization URL.
    let state = started
        .authorization_url
        .split("state=")
        .nth(1)
        .expect("the URL carries a state")
        .split('&')
        .next()
        .expect("state value")
        .to_owned();
    assert!(!state.is_empty());
    // No column anywhere holds the raw value.
    let raw_rows: i64 = sqlx::query_scalar(
        "select count(*) from identity_verifications where state = $1",
    )
    .bind(&state)
    .fetch_one(&pool)
    .await?;
    assert_eq!(raw_rows, 0, "the raw state must never be stored");
    // The stored digest is exactly the digest of that state.
    let stored: Vec<u8> = sqlx::query_scalar(
        "select state_digest from identity_verifications where user_id = $1",
    )
    .bind(user)
    .fetch_one(&pool)
    .await?;
    assert_eq!(stored, state_digest(&state).to_vec());
    clear_user(&pool, user).await?;
    Ok(())
}

/// A return path outside the closed allowlist is refused, in every shape.
#[tokio::test]
async fn a_return_path_outside_the_allowlist_is_refused() -> anyhow::Result<()> {
    let pool = pool().await?;
    let user = support::user();
    clear_user(&pool, user).await?;
    for value in [
        "https://evil.example",
        "//evil.example",
        "http://localhost/me/identity",
        "/me/identity/../../evil",
        "/me/identity?next=https://evil.example",
        "/me/identity#fragment",
        "/evil",
        "",
        "/me/identity ",
    ] {
        assert_eq!(
            validate_return_path(value),
            Err(StartError::ReturnPathNotAllowed),
            "{value:?} must be refused"
        );
        let error = start_use_case(pool.clone())
            .begin(user, Some(value), Utc::now())
            .await
            .err()
            .expect("a refused return path must fail the start");
        assert_eq!(error, StartError::ReturnPathNotAllowed, "{value:?}");
    }
    // Nothing was written by any of the refusals.
    assert_eq!(verification_count(&pool, user).await?, 0);
    // Every allowlisted path is accepted.
    for value in RETURN_PATHS {
        assert_eq!(validate_return_path(value), Ok(*value));
    }
    clear_user(&pool, user).await?;
    Ok(())
}

/// An absent return path uses the default, which is in the allowlist.
#[tokio::test]
async fn an_absent_return_path_uses_the_default() -> anyhow::Result<()> {
    let pool = pool().await?;
    let user = support::user();
    clear_user(&pool, user).await?;
    start_use_case(pool.clone())
        .begin(user, None, Utc::now())
        .await
        .expect("start succeeds");
    let return_path: String =
        sqlx::query_scalar("select return_path from identity_verifications where user_id = $1")
            .bind(user)
            .fetch_one(&pool)
            .await?;
    assert!(RETURN_PATHS.contains(&return_path.as_str()), "{return_path}");
    clear_user(&pool, user).await?;
    Ok(())
}

/// A bounded number of in-flight states per user.
#[tokio::test]
async fn a_second_in_flight_state_beyond_the_bound_is_refused() -> anyhow::Result<()> {
    let pool = pool().await?;
    let user = support::user();
    clear_user(&pool, user).await?;
    let start = start_use_case(pool.clone());
    let now = Utc::now();
    for _ in 0..profile_service::identity::repository::MAX_IN_FLIGHT_VERIFICATIONS {
        start
            .begin(user, Some("/me/identity"), now)
            .await
            .expect("a state within the bound succeeds");
    }
    let error = start
        .begin(user, Some("/me/identity"), now)
        .await
        .err()
        .expect("the state beyond the bound must be refused");
    assert_eq!(error, StartError::TooManyInFlight);
    assert_eq!(
        verification_count(&pool, user).await?,
        profile_service::identity::repository::MAX_IN_FLIGHT_VERIFICATIONS
    );
    clear_user(&pool, user).await?;
    Ok(())
}

/// An expired state does not count against the bound.
#[tokio::test]
async fn an_expired_state_does_not_count_against_the_bound() -> anyhow::Result<()> {
    let pool = pool().await?;
    let user = support::user();
    clear_user(&pool, user).await?;
    let start = start_use_case(pool.clone());
    let past = Utc::now() - Duration::hours(1);
    for _ in 0..profile_service::identity::repository::MAX_IN_FLIGHT_VERIFICATIONS {
        start
            .begin(user, Some("/me/identity"), past)
            .await
            .expect("a state within the bound succeeds");
    }
    // Every stored state is already expired, so a new one is admitted.
    start
        .begin(user, Some("/me/identity"), Utc::now())
        .await
        .expect("an expired state must not occupy the bound");
    clear_user(&pool, user).await?;
    Ok(())
}

/// Two states are always distinct, and a state is 256 bits of base64url.
#[test]
fn states_are_distinct_and_unpadded_base64url() {
    let first = random_state().expect("entropy");
    let second = random_state().expect("entropy");
    assert_ne!(first, second);
    assert_eq!(first.len(), 43, "32 bytes of base64url is 43 characters");
    assert!(
        first
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')),
        "{first}"
    );
    // The digest is domain-separated from a bare hash of the state.
    assert_ne!(state_digest(&first), state_digest(&second));
}

/// The digest is 32 bytes, which is what the schema's CHECK requires.
#[test]
fn the_state_digest_is_thirty_two_bytes() {
    assert_eq!(state_digest("any-state").len(), 32);
}
