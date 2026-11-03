#[tokio::test]
async fn postgres_backed_signup_verify_login_refresh_logout_flow() {
    let app = crate::support::start_postgres_app(false)
        .await
        .expect("app");

    let signup = app
        .state
        .signup
        .signup_email("flow@example.com", "Passw0rd!")
        .await
        .expect("signup");
    let verification_code: String =
        sqlx::query_scalar("select code from email_verifications where email = $1")
            .bind("flow@example.com")
            .fetch_one(&app.pool)
            .await
            .expect("verification code");
    app.state
        .signup
        .verify_email("flow@example.com", &verification_code)
        .await
        .expect("verify");
    let login = app
        .state
        .login
        .login("flow@example.com", "Passw0rd!", None, None)
        .await
        .expect("login");
    let refreshed = app
        .state
        .login
        .refresh(&login.refresh_token)
        .await
        .expect("refresh");
    app.state
        .login
        .logout(&refreshed.refresh_token)
        .await
        .expect("logout");
    assert_eq!(signup.status, "PENDING_VERIFICATION");
    assert_eq!(refreshed.token_type, "Bearer");
}

#[tokio::test]
async fn lifecycle_cleanup_worker_deletes_expired_auth_rows_and_records_metrics() {
    let app = crate::support::start_postgres_app(false)
        .await
        .expect("app");
    let user_id = uuid::Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("uuid");
    let expired_verification_id =
        uuid::Uuid::parse_str("22222222-2222-2222-2222-222222222222").expect("uuid");
    let live_verification_id =
        uuid::Uuid::parse_str("33333333-3333-3333-3333-333333333333").expect("uuid");
    let expired_reset_id =
        uuid::Uuid::parse_str("44444444-4444-4444-4444-444444444444").expect("uuid");
    let live_reset_id =
        uuid::Uuid::parse_str("55555555-5555-5555-5555-555555555555").expect("uuid");

    sqlx::query(
        r#"
        insert into users (id, primary_email, password_hash, status, last_login_at)
        values ($1, 'cleanup@example.com', 'hash', 'ACTIVE', null)
        "#,
    )
    .bind(user_id)
    .execute(&app.pool)
    .await
    .expect("user");
    sqlx::query(
        r#"
        insert into email_verifications (id, user_id, email, code, expires_at)
        values
            ($2, $1, 'cleanup@example.com', 'expired-code', now() - interval '1 day'),
            ($3, $1, 'cleanup@example.com', 'live-code', now() + interval '1 day')
        "#,
    )
    .bind(user_id)
    .bind(expired_verification_id)
    .bind(live_verification_id)
    .execute(&app.pool)
    .await
    .expect("verifications");
    sqlx::query(
        r#"
        insert into password_reset_tokens (id, user_id, token_hash, expires_at)
        values
            ($2, $1, 'expired-reset-hash', now() - interval '1 day'),
            ($3, $1, 'live-reset-hash', now() + interval '1 day')
        "#,
    )
    .bind(user_id)
    .bind(expired_reset_id)
    .bind(live_reset_id)
    .execute(&app.pool)
    .await
    .expect("reset tokens");
    sqlx::query(
        r#"
        insert into refresh_tokens (jti, token_hash, user_id, family_id, expires_at, revoked_at)
        values
            ('expired-refresh', 'expired-refresh-hash', $1, '33333333-3333-3333-3333-333333333333', now() - interval '1 day', null),
            ('live-refresh', 'live-refresh-hash', $1, '44444444-4444-4444-4444-444444444444', now() + interval '1 day', null)
        "#,
    )
    .bind(user_id)
    .execute(&app.pool)
    .await
    .expect("refresh tokens");
    sqlx::query(
        r#"
        insert into oauth_authorizations (
            state, provider_type, redirect_uri, code_verifier, code_challenge,
            nonce, expires_at, consumed_at
        )
        values
            ('expired-oauth-state', 'GOOGLE', 'http://localhost/callback', 'verifier', 'challenge',
             'nonce', now() - interval '1 day', null),
            ('live-oauth-state', 'GOOGLE', 'http://localhost/callback', 'verifier', 'challenge',
             'nonce', now() + interval '1 day', null)
        "#,
    )
    .execute(&app.pool)
    .await
    .expect("oauth authorizations");

    let result = app
        .state
        .lifecycle_cleanup_worker
        .tick()
        .await
        .expect("cleanup worker tick");

    assert_eq!(result.deleted_email_verifications, 1);
    assert_eq!(result.deleted_password_reset_tokens, 1);
    assert_eq!(result.deleted_refresh_tokens, 1);
    assert_eq!(result.deleted_oauth_authorizations, 1);

    let expired_count: i64 = sqlx::query_scalar(
        r#"
        select
            (select count(*) from email_verifications where code = 'expired-code') +
            (select count(*) from password_reset_tokens where token_hash = 'expired-reset-hash') +
            (select count(*) from refresh_tokens where jti = 'expired-refresh') +
            (select count(*) from oauth_authorizations where state = 'expired-oauth-state')
        "#,
    )
    .fetch_one(&app.pool)
    .await
    .expect("expired count");
    assert_eq!(expired_count, 0);

    let live_count: i64 = sqlx::query_scalar(
        r#"
        select
            (select count(*) from email_verifications where code = 'live-code') +
            (select count(*) from password_reset_tokens where token_hash = 'live-reset-hash') +
            (select count(*) from refresh_tokens where jti = 'live-refresh') +
            (select count(*) from oauth_authorizations where state = 'live-oauth-state')
        "#,
    )
    .fetch_one(&app.pool)
    .await
    .expect("live count");
    assert_eq!(live_count, 4);

    let metrics = app.metrics().await.expect("metrics");
    assert!(metrics.contains("auth_lifecycle_cleanup_runs_total{outcome=\"success\"} 1"));
    assert!(metrics.contains(
        "auth_lifecycle_cleanup_deleted_records_total{record_type=\"email_verification\",outcome=\"success\"} 1"
    ));
    assert!(metrics.contains(
        "auth_lifecycle_cleanup_deleted_records_total{record_type=\"password_reset\",outcome=\"success\"} 1"
    ));
    assert!(metrics.contains(
        "auth_lifecycle_cleanup_deleted_records_total{record_type=\"refresh_token\",outcome=\"success\"} 1"
    ));
    assert!(metrics.contains(
        "auth_lifecycle_cleanup_deleted_records_total{record_type=\"oauth_authorization\",outcome=\"success\"} 1"
    ));
}
