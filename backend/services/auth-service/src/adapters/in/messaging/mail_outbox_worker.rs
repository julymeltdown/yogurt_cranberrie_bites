use chrono::{DateTime, Utc};
use platform::trace_context::{self, TraceContext};
use reqwest::Url;
use serde_json::{json, Value};
use sqlx::{PgPool, Row};
use std::time::Duration;
use tracing::{field, Instrument, Span};
use uuid::Uuid;

#[derive(Clone)]
pub struct MailDestination {
    endpoint: Url,
    bearer_token: String,
}

impl MailDestination {
    pub fn new(endpoint: &str, bearer_token: &str, production: bool) -> anyhow::Result<Self> {
        let endpoint = Url::parse(endpoint)?;
        anyhow::ensure!(
            matches!(endpoint.scheme(), "http" | "https")
                && endpoint.host_str().is_some()
                && endpoint.username().is_empty()
                && endpoint.password().is_none()
                && endpoint.fragment().is_none(),
            "Invalid mail delivery endpoint"
        );
        anyhow::ensure!(
            !production || endpoint.scheme() == "https",
            "Mail delivery requires HTTPS in production"
        );
        anyhow::ensure!(
            !bearer_token.trim().is_empty(),
            "Mail delivery bearer token is required"
        );
        Ok(Self {
            endpoint,
            bearer_token: bearer_token.to_owned(),
        })
    }
}

#[derive(Clone)]
pub struct EmailOutboxWorker {
    pool: PgPool,
    client: reqwest::Client,
    verification: Option<MailDestination>,
    reset: Option<MailDestination>,
}

struct MailOutboxClaim {
    id: Uuid,
    kind: String,
    payload: Option<String>,
    expires_at: DateTime<Utc>,
    attempts: i32,
    origin_traceparent: Option<String>,
}

impl EmailOutboxWorker {
    pub fn from_env(pool: PgPool, production: bool) -> anyhow::Result<Self> {
        Self::new(
            pool,
            configured_destination("AUTH_VERIFICATION_MAILER", production)?,
            configured_destination("AUTH_PASSWORD_RESET_MAILER", production)?,
        )
    }

    pub fn new(
        pool: PgPool,
        verification: Option<MailDestination>,
        reset: Option<MailDestination>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            pool,
            verification,
            reset,
            client: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(3))
                .timeout(Duration::from_secs(10))
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
        })
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let shutdown = platform::shutdown::global();
        let mut interval = tokio::time::interval(Duration::from_secs(2));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        while shutdown.tick(&mut interval).await {
            if self.run_once().await.is_err() {
                tracing::error!(
                    code = "MAIL_OUTBOX_CYCLE_FAILED",
                    "mail delivery cycle failed"
                );
            }
        }
        Ok(())
    }

    pub async fn run_once(&self) -> anyhow::Result<usize> {
        self.cleanup().await?;
        let mut kinds = Vec::new();
        if self.verification.is_some() {
            kinds.push("VERIFY_EMAIL");
        }
        if self.reset.is_some() {
            kinds.push("PASSWORD_RESET");
        }
        if kinds.is_empty() {
            return Ok(0);
        }
        let lease_id = Uuid::new_v4();
        let rows = sqlx::query(
            r#"
            with due as (
                select o.id
                from auth_mail_outbox o join users u on u.id = o.user_id
                where o.kind = any($1::text[])
                  and o.delivered_at is null and o.dead_lettered_at is null
                  and o.next_attempt_at <= now() and o.expires_at > now()
                  and o.attempts < 8
                  and (o.leased_until is null or o.leased_until <= now())
                  and u.deleted_at is null
                  and ((o.kind = 'VERIFY_EMAIL' and u.status = 'PENDING_VERIFICATION')
                    or (o.kind = 'PASSWORD_RESET' and u.status = 'ACTIVE'))
                order by o.next_attempt_at, o.created_at, o.id
                limit 2 for update of o skip locked
            )
            update auth_mail_outbox o
            set lease_id = $2, leased_until = now() + interval '2 minutes',
                attempts = attempts + 1
            from due where o.id = due.id
            returning o.id, o.kind, o.payload_json, o.expires_at, o.attempts,
                o.origin_traceparent
            "#,
        )
        .bind(kinds)
        .bind(lease_id)
        .fetch_all(&self.pool)
        .await?;
        let mut delivered = 0;
        for row in rows {
            let claim = MailOutboxClaim {
                id: row.try_get("id")?,
                kind: row.try_get("kind")?,
                payload: row.try_get("payload_json")?,
                expires_at: row.try_get("expires_at")?,
                attempts: row.try_get("attempts")?,
                origin_traceparent: row.try_get("origin_traceparent")?,
            };
            let mut context = claim
                .origin_traceparent
                .as_deref()
                .and_then(TraceContext::child_from_traceparent);
            let span = tracing::info_span!(
                target: "auth_service::adapters::in::messaging::mail_outbox_worker",
                parent: None,
                "auth_mail_delivery",
                mail_kind = mail_kind_label(&claim.kind),
                trace_id = field::Empty,
                span_id = field::Empty,
                parent_span_id = field::Empty,
                sampled = field::Empty,
                outcome = field::Empty,
            );
            if let Some(context) = context.as_mut() {
                context.bind_to_span(&span);
                context.record_on_span(&span);
            }
            let processing = self
                .process_claimed_mail(claim, lease_id, span.clone())
                .instrument(span.clone());
            let result = match context {
                Some(context) => trace_context::scope(context, processing).await,
                None => processing.await,
            };
            match result {
                Ok(count) => delivered += count,
                Err(error) => {
                    span.record("outcome", "dependency_error");
                    return Err(error);
                }
            }
        }
        Ok(delivered)
    }

    async fn process_claimed_mail(
        &self,
        claim: MailOutboxClaim,
        lease_id: Uuid,
        span: Span,
    ) -> anyhow::Result<usize> {
        let eligible: bool = sqlx::query_scalar(
            r#"
            select exists (
                select 1 from auth_mail_outbox o join users u on u.id = o.user_id
                where o.id = $1 and o.lease_id = $2
                  and o.dead_lettered_at is null and o.delivered_at is null
                  and o.expires_at > clock_timestamp() and u.deleted_at is null
                  and ((o.kind = 'VERIFY_EMAIL' and u.status = 'PENDING_VERIFICATION')
                    or (o.kind = 'PASSWORD_RESET' and u.status = 'ACTIVE'))
            )
            "#,
        )
        .bind(claim.id)
        .bind(lease_id)
        .fetch_one(&self.pool)
        .await?;
        if !eligible {
            span.record("outcome", "ineligible");
            return Ok(0);
        }
        let result = match claim.payload {
            Some(payload) => {
                self.deliver(claim.id, &claim.kind, &payload, claim.expires_at)
                    .await
            }
            None => Err("INVALID_PAYLOAD"),
        };
        let error_code = match result {
            Ok(()) => {
                let updated = sqlx::query(
                    r#"
                    update auth_mail_outbox
                    set delivered_at = now(), payload_json = null, origin_traceparent = null,
                        lease_id = null, leased_until = null, last_error_code = null
                    where id = $1 and lease_id = $2 and dead_lettered_at is null
                    "#,
                )
                .bind(claim.id)
                .bind(lease_id)
                .execute(&self.pool)
                .await?
                .rows_affected();
                if updated == 0 {
                    span.record("outcome", "lease_lost");
                } else {
                    span.record("outcome", "delivered");
                    tracing::debug!("mail delivery completed");
                }
                return Ok(updated as usize);
            }
            Err(code) => code,
        };
        let terminal = claim.attempts >= 8 || error_code == "INVALID_PAYLOAD";
        sqlx::query(
            r#"
            update auth_mail_outbox
            set lease_id = null, leased_until = null,
                next_attempt_at = now() + ($3 * interval '1 second'),
                last_error_code = $4,
                dead_lettered_at = case when $5 then now() else null end,
                payload_json = case when $5 then null else payload_json end,
                origin_traceparent = case when $5 then null else origin_traceparent end
            where id = $1 and lease_id = $2 and dead_lettered_at is null
            "#,
        )
        .bind(claim.id)
        .bind(lease_id)
        .bind(retry_delay_seconds(claim.attempts, claim.id))
        .bind(error_code)
        .bind(terminal)
        .execute(&self.pool)
        .await?;
        span.record(
            "outcome",
            if terminal {
                "dead_lettered"
            } else {
                "retry_scheduled"
            },
        );
        tracing::warn!(code = error_code, terminal, "mail delivery attempt failed");
        Ok(0)
    }

    async fn deliver(
        &self,
        id: Uuid,
        kind: &str,
        payload: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<(), &'static str> {
        let payload: Value = serde_json::from_str(payload).map_err(|_| "INVALID_PAYLOAD")?;
        let email = payload
            .get("email")
            .and_then(Value::as_str)
            .ok_or("INVALID_PAYLOAD")?;
        let (destination, body) = match kind {
            "VERIFY_EMAIL" => {
                let code = payload
                    .get("code")
                    .and_then(Value::as_str)
                    .ok_or("INVALID_PAYLOAD")?;
                (
                    self.verification.as_ref(),
                    json!({
                        "email": email, "verification_code": code, "expires_at": expires_at
                    }),
                )
            }
            "PASSWORD_RESET" => {
                let link = payload
                    .get("link")
                    .and_then(Value::as_str)
                    .ok_or("INVALID_PAYLOAD")?;
                (
                    self.reset.as_ref(),
                    json!({"email": email, "reset_link": link}),
                )
            }
            _ => return Err("INVALID_PAYLOAD"),
        };
        let destination = destination.ok_or("DESTINATION_UNAVAILABLE")?;
        let response = self
            .client
            .post(destination.endpoint.clone())
            .bearer_auth(&destination.bearer_token)
            .header("Idempotency-Key", id.to_string())
            .json(&body)
            .send()
            .await
            .map_err(|_| "DELIVERY_TRANSPORT_ERROR")?;
        if !response.status().is_success() {
            return Err("DELIVERY_HTTP_ERROR");
        }
        Ok(())
    }

    async fn cleanup(&self) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            update auth_mail_outbox set payload_json = null, origin_traceparent = null,
                dead_lettered_at = now(), lease_id = null, leased_until = null,
                last_error_code = 'EXPIRED_OR_CANCELLED'
            where id in (
                select o.id from auth_mail_outbox o join users u on u.id = o.user_id
                where o.delivered_at is null and o.dead_lettered_at is null
                  and (o.expires_at <= now() or u.deleted_at is not null
                    or (o.kind = 'VERIFY_EMAIL' and u.status <> 'PENDING_VERIFICATION')
                    or (o.kind = 'PASSWORD_RESET' and u.status <> 'ACTIVE')
                    or (o.attempts >= 8 and o.leased_until <= now()))
                order by o.created_at limit 500
            )
            "#,
        )
        .execute(&self.pool)
        .await?;
        sqlx::query(
            r#"
            delete from auth_mail_outbox where id in (
                select id from auth_mail_outbox
                where created_at < now() - interval '7 days'
                  and (delivered_at is not null or dead_lettered_at is not null)
                order by created_at limit 500
            )
            "#,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

fn configured_destination(
    prefix: &str,
    production: bool,
) -> anyhow::Result<Option<MailDestination>> {
    let endpoint = platform::config::optional_env(&format!("{prefix}_ENDPOINT"));
    let token = platform::config::optional_env(&format!("{prefix}_BEARER_TOKEN"));
    match (endpoint, token) {
        (Some(endpoint), Some(token)) => {
            Ok(Some(MailDestination::new(&endpoint, &token, production)?))
        }
        (None, None) if !production => Ok(None),
        _ => anyhow::bail!("{prefix}_ENDPOINT and {prefix}_BEARER_TOKEN are required together"),
    }
}

fn retry_delay_seconds(attempts: i32, id: Uuid) -> i64 {
    let exponent = attempts.clamp(1, 8) as u32;
    (5_i64 * 2_i64.pow(exponent)).min(900) + i64::from(id.as_bytes()[0] % 7)
}

fn mail_kind_label(kind: &str) -> &'static str {
    match kind {
        "VERIFY_EMAIL" => "verification",
        "PASSWORD_RESET" => "password_reset",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_insecure_or_credential_bearing_production_destinations() {
        assert!(MailDestination::new("http://mail.internal/send", "token", true).is_err());
        assert!(
            MailDestination::new("https://user:pass@mail.internal/send", "token", true).is_err()
        );
        assert!(MailDestination::new("https://mail.internal/send", "", true).is_err());
        assert!(MailDestination::new("https://mail.internal/send", "token", true).is_ok());
    }

    #[test]
    fn retry_delays_are_bounded_and_increase() {
        let id = Uuid::nil();
        assert!(retry_delay_seconds(1, id) < retry_delay_seconds(2, id));
        assert!(retry_delay_seconds(i32::MAX, id) <= 906);
        assert!(retry_delay_seconds(i32::MIN, id) >= 10);
    }

    #[test]
    fn mail_kind_trace_label_is_fixed_and_bounded() {
        assert_eq!(mail_kind_label("VERIFY_EMAIL"), "verification");
        assert_eq!(mail_kind_label("PASSWORD_RESET"), "password_reset");
        assert_eq!(mail_kind_label("unexpected-user-value"), "unknown");
    }
}
