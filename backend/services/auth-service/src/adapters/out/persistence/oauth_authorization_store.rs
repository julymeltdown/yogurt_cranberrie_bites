use crate::application::ports::OAuthAuthorizationStorePort;
use crate::domain::oauth_authorization::ConsumedOAuthAuthorization;
use crate::domain::user::IdentityProvider;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};

#[derive(Clone)]
pub struct PgOAuthAuthorizationStore {
    pool: PgPool,
}

impl PgOAuthAuthorizationStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl OAuthAuthorizationStorePort for PgOAuthAuthorizationStore {
    async fn save(
        &self,
        transaction: &crate::application::dto::repository_dto::AuthOAuthAuthorizationRecordDto,
    ) -> crate::application::errors::AuthResult<()> {
        let transaction = &crate::domain::oauth_authorization::OAuthAuthorizationTransaction::from(
            transaction.clone(),
        );

        let result: anyhow::Result<_> = async {
            sqlx::query(
                r#"
            insert into oauth_authorizations (
                state, provider_type, redirect_uri, code_verifier,
                code_challenge, nonce, expires_at, consumed_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, null)
            "#,
            )
            .bind(&transaction.state)
            .bind(provider_name(transaction.provider))
            .bind(&transaction.redirect_uri)
            .bind(&transaction.code_verifier)
            .bind(&transaction.code_challenge)
            .bind(&transaction.nonce)
            .bind(transaction.expires_at)
            .execute(&self.pool)
            .await?;
            Ok(())
        }
        .await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    async fn consume(
        &self,
        provider: crate::application::dto::repository_dto::AuthIdentityProviderDto,
        state: &str,
        now: DateTime<Utc>,
    ) -> crate::application::errors::AuthResult<
        Option<crate::application::dto::repository_dto::AuthConsumedOAuthAuthorizationDto>,
    > {
        let provider = crate::domain::user::IdentityProvider::from(provider);
        let result: anyhow::Result<_> = async {
            let row = sqlx::query(
                r#"
            update oauth_authorizations
            set consumed_at = $3
            where provider_type = $1
              and state = $2
              and consumed_at is null
              and expires_at > $3
            returning provider_type, state, redirect_uri, code_verifier, nonce
            "#,
            )
            .bind(provider_name(provider))
            .bind(state)
            .bind(now)
            .fetch_optional(&self.pool)
            .await?;

            Ok(row.map(|row| ConsumedOAuthAuthorization {
                provider: parse_provider(row.get("provider_type")),
                state: row.get("state"),
                redirect_uri: row.get("redirect_uri"),
                code_verifier: row.get("code_verifier"),
                nonce: row.get("nonce"),
            }))
        }
        .await;
        result
            .map_err(crate::adapters::out::errors::boundary_error)
            .map(|value| value.map(Into::into))
    }
}

fn provider_name(provider: IdentityProvider) -> &'static str {
    match provider {
        IdentityProvider::Email => "EMAIL",
        IdentityProvider::Google => "GOOGLE",
        IdentityProvider::Apple => "APPLE",
        IdentityProvider::X => "X",
    }
}

fn parse_provider(value: String) -> IdentityProvider {
    match value.as_str() {
        "GOOGLE" => IdentityProvider::Google,
        "APPLE" => IdentityProvider::Apple,
        "X" => IdentityProvider::X,
        _ => IdentityProvider::Email,
    }
}
