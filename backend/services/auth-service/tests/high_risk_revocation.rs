//! High-risk revocation across the real issuer and the Gateway's session guard.
//!
//! Real PostgreSQL, the real auth use cases and gRPC handler behind the
//! registered-caller interceptor, and the Gateway's own auth client, verifier,
//! and high-risk session guard. It shows that logout and admin-role removal stop
//! high-risk actions while the access tokens involved are still valid. Requires
//! Docker and never skips.
#[path = "support/issuer.rs"]
mod issuer;
#[path = "support/mod.rs"]
mod support;

use anyhow::Context as _;
use api_gateway::adapters::out::jwt::JwtVerifier;
use api_gateway::application::dto::issuer_dto::SessionAuthorityRequestDto;
use api_gateway::application::ports::AuthClientPort;
use api_gateway::application::service::HighRiskSessionGuard;
use api_gateway::domain::high_risk::{HighRiskAction, SessionDecision};
use issuer::Issuer;
use std::sync::Arc;
use std::time::Duration;

/// The seeded account these drills follow.
const HANA: &str = "11111111-1111-1111-1111-111111111111";

#[tokio::test]
async fn logout_and_admin_role_removal_stop_high_risk_actions_before_tokens_expire(
) -> anyhow::Result<()> {
    let app = support::start_postgres_app(true).await?;
    let issuer = Issuer::serve(&app, app.config.clone()).await?;
    let verifier = JwtVerifier::from_jwks_json(
        &app.config.issuer,
        &app.config.access_audience,
        &auth_service::adapters::r#in::jwks::public_jwks(&(issuer.state.login.jwk_set()))
            .to_string(),
    )?;
    // Zero freshness: every check reaches the issuer, so each step observes
    // the database as it is at that moment.
    let client: Arc<dyn AuthClientPort> = Arc::new(issuer.client.clone());
    let guard = HighRiskSessionGuard::over_auth_client(
        client,
        Duration::ZERO,
        Arc::new(api_gateway::adapters::out::runtime::SystemGatewayRuntime),
    );
    let decide = |session: String, action: HighRiskAction| {
        let guard = guard.clone();
        async move {
            guard
                .check(HANA, &session, action.into())
                .await
                .map(|verdict| SessionDecision::from(verdict.decision))
        }
    };
    let hana: uuid::Uuid = HANA.parse()?;
    sqlx::query(
        "insert into auth_user_roles (user_id, role, granted_by) values ($1, 'ROLE_ADMIN', $1)",
    )
    .bind(hana)
    .execute(&app.pool)
    .await?;

    // An administrator signs in on two devices.
    let desk = issuer
        .state
        .login
        .login("hana@example.com", "Passw0rd!", None, None)
        .await
        .context("signing in at the desk")?;
    let laptop = issuer
        .state
        .login
        .login("hana@example.com", "Passw0rd!", None, None)
        .await
        .context("signing in on the laptop")?;
    let desk_claims = verifier.verify_access_token(&desk.access_token)?;
    let laptop_claims = verifier.verify_access_token(&laptop.access_token)?;
    assert!(desk_claims.roles.iter().any(|role| role == "ROLE_ADMIN"));
    let desk_session = desk_claims
        .session_id
        .context("desk token names a session")?;
    let laptop_session = laptop_claims
        .session_id
        .context("laptop token names a session")?;
    assert_ne!(desk_session, laptop_session);
    assert_eq!(
        decide(desk_session.clone(), HighRiskAction::Administration).await?,
        SessionDecision::Allowed
    );

    // The desk logs out.
    issuer.state.login.logout(&desk.refresh_token).await?;

    // Its access token still verifies, so ordinary requests accept it until it
    // expires; high-risk actions are refused now, and the laptop is untouched.
    verifier.verify_access_token(&desk.access_token)?;
    assert_eq!(
        decide(desk_session.clone(), HighRiskAction::Administration).await?,
        SessionDecision::SessionEnded
    );
    assert_eq!(
        decide(desk_session, HighRiskAction::AccountSecurity).await?,
        SessionDecision::SessionEnded
    );
    assert_eq!(
        decide(laptop_session.clone(), HighRiskAction::Administration).await?,
        SessionDecision::Allowed
    );

    // The admin role is removed. The laptop's token still claims it and still
    // verifies, but the role change ended that session at the issuer.
    sqlx::query("delete from auth_user_roles where user_id = $1 and role = 'ROLE_ADMIN'")
        .bind(hana)
        .execute(&app.pool)
        .await?;
    let still_claimed = verifier.verify_access_token(&laptop.access_token)?;
    assert!(still_claimed.roles.iter().any(|role| role == "ROLE_ADMIN"));
    assert_eq!(
        decide(laptop_session, HighRiskAction::Administration).await?,
        SessionDecision::SessionEnded
    );

    // Signing in again gives a live session that is no longer administrative.
    let again = issuer
        .state
        .login
        .login("hana@example.com", "Passw0rd!", None, None)
        .await
        .context("signing in after the role change")?;
    let again_session = verifier
        .verify_access_token(&again.access_token)?
        .session_id
        .context("new token names a session")?;
    assert_eq!(
        decide(again_session.clone(), HighRiskAction::Administration).await?,
        SessionDecision::RoleRevoked
    );
    assert_eq!(
        decide(again_session, HighRiskAction::AccountSecurity).await?,
        SessionDecision::Allowed
    );

    // Identifiers that are not UUIDs are refused rather than looked up.
    assert!(issuer
        .client
        .session_state(&SessionAuthorityRequestDto {
            user_id: HANA.into(),
            session_id: "not-a-session".into()
        })
        .await
        .is_err());

    issuer.stop().await?;
    app.pool.close().await;
    Ok(())
}
