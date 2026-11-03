use crate::application::ports::ResetMailerPort;
use async_trait::async_trait;
use serde::Serialize;
use std::time::Duration;

#[derive(Clone, Default)]
pub struct LoggingResetMailer;

#[async_trait]
impl ResetMailerPort for LoggingResetMailer {
    async fn send_reset_link(
        &self,
        _email: &str,
        _link: &str,
    ) -> crate::application::errors::AuthResult<()> {
        let result: anyhow::Result<_> = async {
            tracing::info!("password reset link generated");
            Ok(())
        }
        .await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }
}

#[derive(Clone)]
pub struct HttpResetMailer {
    client: reqwest::Client,
    endpoint: String,
    bearer_token: String,
}

impl HttpResetMailer {
    pub fn new(endpoint: String, bearer_token: String) -> anyhow::Result<Self> {
        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()?,
            endpoint,
            bearer_token,
        })
    }
}

#[async_trait]
impl ResetMailerPort for HttpResetMailer {
    async fn send_reset_link(
        &self,
        email: &str,
        link: &str,
    ) -> crate::application::errors::AuthResult<()> {
        let result: anyhow::Result<_> = async {
            self.client
                .post(&self.endpoint)
                .bearer_auth(&self.bearer_token)
                .json(&PasswordResetMailRequest {
                    email,
                    reset_link: link,
                })
                .send()
                .await?
                .error_for_status()?;
            Ok(())
        }
        .await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }
}

#[derive(Clone)]
pub enum ConfiguredResetMailer {
    Logging(LoggingResetMailer),
    Http(HttpResetMailer),
}

#[async_trait]
impl ResetMailerPort for ConfiguredResetMailer {
    async fn send_reset_link(
        &self,
        email: &str,
        link: &str,
    ) -> crate::application::errors::AuthResult<()> {
        let result = async {
            match self {
                Self::Logging(mailer) => mailer.send_reset_link(email, link).await,
                Self::Http(mailer) => mailer.send_reset_link(email, link).await,
            }
        }
        .await;
        result
    }
}

#[derive(Serialize)]
struct PasswordResetMailRequest<'a> {
    email: &'a str,
    reset_link: &'a str,
}
