//! Shared fixtures for the NAVER verification tests.
//!
//! Two fixtures, both deliberately real where it matters:
//!
//! * **The database is the runner-owned PostgreSQL cluster**, reached through
//!   `PROFILE_TEST_DATABASE_URL` and migrated with the service's own migration
//!   files. Every claim assertion is made against the actual schema, including
//!   its CHECK constraints, so a test cannot pass against a shape the
//!   production database would refuse.
//! * **The provider is a fake HTTP server on loopback**. The client under test
//!   is the production `NaverProviderClient`; only the far end is fake. That is
//!   the only way to exercise the real token exchange, the real bearer fetch,
//!   the real size bound and the real error classification without calling
//!   NAVER. No test in this package makes a network request off the host.
#![allow(dead_code)]

use chrono::{DateTime, Utc};
use profile_service::identity::{
    IdentityRepository, NaverOAuthConfig, NaverProviderClient, NaverProviderEndpoints,
};
use serde_json::json;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// The subject salt every test uses. The value is irrelevant to the assertions
/// except that it is stable, which is what the production contract requires.
pub const TEST_SUBJECT_SALT: &[u8] = b"wp04b-identity-test-subject-salt-0001";

/// The database URL, or `None` when the harness was not given one.
pub fn database_url() -> Option<String> {
    std::env::var("PROFILE_TEST_DATABASE_URL")
        .ok()
        .filter(|url| !url.trim().is_empty())
}

/// Connect to the fixture database and apply every migration.
///
/// The migrations are applied through the same SQLx ledger the service uses, so
/// a test asserts against the exact schema a deployment would have.
pub async fn pool() -> anyhow::Result<sqlx::PgPool> {
    let url = database_url().ok_or_else(|| anyhow::anyhow!("PROFILE_TEST_DATABASE_URL is required"))?;
    let options: sqlx::postgres::PgConnectOptions = url.parse()?;
    anyhow::ensure!(
        options.get_username() == "mcr_owner",
        "the identity fixture must use the owned cluster"
    );
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await?;
    apply_migrations(&pool).await?;
    Ok(pool)
}

async fn apply_migrations(pool: &sqlx::PgPool) -> anyhow::Result<()> {
    // Every test binary in this package shares one fixture database. The
    // advisory lock serializes startup, and the probe makes a second
    // application a no-op.
    let mut tx = pool.begin().await?;
    sqlx::query("select pg_advisory_xact_lock(hashtextextended('wp04b-identity-migrations',0))")
        .execute(&mut *tx)
        .await?;
    let applied: bool =
        sqlx::query_scalar("select to_regclass('public.identity_claims') is not null")
            .fetch_one(&mut *tx)
            .await?;
    if applied {
        tx.commit().await?;
        return Ok(());
    }
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let mut entries: Vec<_> = std::fs::read_dir(&directory)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().is_some_and(|extension| extension == "sql")
                && !path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.ends_with(".down.sql"))
        })
        .collect();
    entries.sort();
    for path in entries {
        let sql = std::fs::read_to_string(&path)?;
        sqlx::raw_sql(&sql).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// The repository over the fixture database.
pub fn repository(pool: sqlx::PgPool) -> IdentityRepository {
    IdentityRepository::new(pool, TEST_SUBJECT_SALT.to_vec())
}

// ---------------------------------------------------------------------------
// The fake provider
// ---------------------------------------------------------------------------

/// What the fake provider answers for one profile subject.
#[derive(Clone)]
pub struct FakeProfile {
    pub id: Option<String>,
    pub gender: Option<String>,
    pub birthyear: Option<String>,
    pub birthday: Option<String>,
}

impl FakeProfile {
    /// A complete, supported profile: a 26-year-old woman on 2026-07-12.
    pub fn woman() -> Self {
        Self {
            id: Some("naver-subject-woman".into()),
            gender: Some("F".into()),
            birthyear: Some("2000".into()),
            birthday: Some("07-12".into()),
        }
    }

    pub fn man() -> Self {
        Self {
            id: Some("naver-subject-man".into()),
            gender: Some("M".into()),
            birthyear: Some("1996".into()),
            birthday: Some("03-01".into()),
        }
    }

    pub fn unspecified() -> Self {
        Self {
            id: Some("naver-subject-unspecified".into()),
            gender: Some("U".into()),
            birthyear: Some("1990".into()),
            birthday: Some("01-15".into()),
        }
    }

    pub fn with_subject(mut self, subject: &str) -> Self {
        self.id = Some(subject.to_owned());
        self
    }

    pub fn with_gender(mut self, gender: Option<&str>) -> Self {
        self.gender = gender.map(str::to_owned);
        self
    }

    pub fn with_birth(mut self, year: Option<&str>, day: Option<&str>) -> Self {
        self.birthyear = year.map(str::to_owned);
        self.birthday = day.map(str::to_owned);
        self
    }

    fn body(&self) -> String {
        json!({
            "resultcode": "00",
            "message": "success",
            "response": {
                "id": self.id,
                "email": null,
                "age": null,
                "gender": self.gender,
                "birthyear": self.birthyear,
                "birthday": self.birthday,
            }
        })
        .to_string()
    }
}

/// One recorded provider request, for the redaction assertions.
#[derive(Clone, Debug)]
pub struct RecordedRequest {
    pub method: String,
    pub path: String,
    pub authorization: Option<String>,
    pub body: String,
}

/// A fake NAVER provider on loopback.
///
/// It answers the token endpoint with a fixed access token and the profile
/// endpoint with whatever profile was last set. Tests can force a status code
/// or an oversized body to exercise the client's failure classification.
pub struct FakeProvider {
    pub token_url: String,
    pub profile_url: String,
    pub access_token: String,
    requests: Arc<Mutex<Vec<RecordedRequest>>>,
    state: Arc<Mutex<FakeProviderState>>,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<()>>,
}

#[derive(Clone)]
struct FakeProviderState {
    profile: FakeProfile,
    token_status: u16,
    profile_status: u16,
    oversized_profile: bool,
    token_body_override: Option<String>,
}

impl FakeProvider {
    pub async fn start(profile: FakeProfile) -> anyhow::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let requests = Arc::new(Mutex::new(Vec::new()));
        let state = Arc::new(Mutex::new(FakeProviderState {
            profile,
            token_status: 200,
            profile_status: 200,
            oversized_profile: false,
            token_body_override: None,
        }));
        let recorded = requests.clone();
        let shared = state.clone();
        let (stop, mut stopped) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            loop {
                let accept = tokio::select! {
                    _ = &mut stopped => break,
                    accept = listener.accept() => accept,
                };
                let Ok((mut socket, _)) = accept else { break };
                let recorded = recorded.clone();
                let shared = shared.clone();
                tokio::spawn(async move {
                    let mut buffer = vec![0u8; 16 * 1024];
                    let Ok(read) = socket.read(&mut buffer).await else {
                        return;
                    };
                    let request = String::from_utf8_lossy(&buffer[..read]).into_owned();
                    let mut lines = request.lines();
                    let request_line = lines.next().unwrap_or_default();
                    let mut parts = request_line.split_whitespace();
                    let method = parts.next().unwrap_or_default().to_owned();
                    let path = parts.next().unwrap_or_default().to_owned();
                    let authorization = lines
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .starts_with("authorization:")
                                .then(|| line["authorization:".len()..].trim().to_owned())
                        });
                    let body = request
                        .split_once("\r\n\r\n")
                        .map(|(_, body)| body.to_owned())
                        .unwrap_or_default();
                    if let Ok(mut recorded) = recorded.lock() {
                        recorded.push(RecordedRequest {
                            method: method.clone(),
                            path: path.clone(),
                            authorization,
                            body,
                        });
                    }
                    let state = shared.lock().map(|state| state.clone()).unwrap_or(FakeProviderState {
                        profile: FakeProfile::woman(),
                        token_status: 200,
                        profile_status: 200,
                        oversized_profile: false,
                        token_body_override: None,
                    });
                    let (status, body) = if path.starts_with("/token") {
                        (
                            state.token_status,
                            state
                                .token_body_override
                                .clone()
                                .unwrap_or_else(|| json!({"access_token": "fake-naver-token"}).to_string()),
                        )
                    } else if state.oversized_profile {
                        // A body above the client's bound, refused before parse.
                        (200, "x".repeat(profile_service::identity::naver_profile::MAX_PROFILE_BODY_BYTES + 1))
                    } else {
                        (state.profile_status, state.profile.body())
                    };
                    let response = format!(
                        "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = socket.write_all(response.as_bytes()).await;
                    let _ = socket.shutdown().await;
                });
            }
        });
        Ok(Self {
            token_url: format!("http://{address}/token"),
            profile_url: format!("http://{address}/profile"),
            access_token: "fake-naver-token".into(),
            requests,
            state,
            stop: Some(stop),
            task: Some(task),
        })
    }

    pub fn set_profile(&self, profile: FakeProfile) {
        if let Ok(mut state) = self.state.lock() {
            state.profile = profile;
        }
    }

    pub fn set_token_status(&self, status: u16) {
        if let Ok(mut state) = self.state.lock() {
            state.token_status = status;
        }
    }

    pub fn set_profile_status(&self, status: u16) {
        if let Ok(mut state) = self.state.lock() {
            state.profile_status = status;
        }
    }

    pub fn set_oversized_profile(&self, oversized: bool) {
        if let Ok(mut state) = self.state.lock() {
            state.oversized_profile = oversized;
        }
    }

    pub fn set_token_body(&self, body: Option<&str>) {
        if let Ok(mut state) = self.state.lock() {
            state.token_body_override = body.map(str::to_owned);
        }
    }

    /// The provider's own query string for a completed consent.
    pub fn callback_query(&self, state: &str) -> String {
        format!("code=fake-authorization-code&state={state}")
    }

    pub fn requests(&self) -> Vec<RecordedRequest> {
        self.requests.lock().map(|value| value.clone()).unwrap_or_default()
    }

    pub fn reset_requests(&self) {
        if let Ok(mut requests) = self.requests.lock() {
            requests.clear();
        }
    }

    /// The production client pointed at this fake.
    pub fn client(&self) -> anyhow::Result<NaverProviderClient> {
        NaverProviderClient::new(NaverProviderEndpoints {
            token_url: self.token_url.clone(),
            profile_url: self.profile_url.clone(),
            client_id: "test-client".into(),
            client_secret: "test-secret".into(),
            redirect_uri: "http://localhost:8080/api/identity/naver/callback".into(),
        })
    }

    /// The full configuration pointed at this fake.
    pub fn config(&self) -> NaverOAuthConfig {
        NaverOAuthConfig::new(
            "test-client",
            "test-secret",
            "https://nid.naver.com/oauth2.0/authorize",
            &self.token_url,
            &self.profile_url,
            "http://localhost:8080/api/identity/naver/callback",
            "",
        )
    }
}

impl Drop for FakeProvider {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

// ---------------------------------------------------------------------------
// Row helpers
// ---------------------------------------------------------------------------

/// Every claim row for a user, ordered oldest first.
pub async fn claims(pool: &sqlx::PgPool, user: uuid::Uuid) -> anyhow::Result<Vec<ClaimSnapshot>> {
    let rows: Vec<(String, String, Option<i16>, Option<chrono::NaiveDate>, DateTime<Utc>, DateTime<Utc>)> =
        sqlx::query_as(
            "select status, gender_bucket, age_at_verification, next_birthday_at, verified_at, expires_at \
             from identity_claims where user_id = $1 order by verified_at, claim_id",
        )
        .bind(user)
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .map(|(status, gender_bucket, age, next, verified_at, expires_at)| ClaimSnapshot {
            status,
            gender_bucket,
            age_at_verification: age,
            next_birthday_at: next,
            verified_at,
            expires_at,
        })
        .collect())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimSnapshot {
    pub status: String,
    pub gender_bucket: String,
    pub age_at_verification: Option<i16>,
    pub next_birthday_at: Option<chrono::NaiveDate>,
    pub verified_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

/// The number of claim rows for a user.
pub async fn claim_count(pool: &sqlx::PgPool, user: uuid::Uuid) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar("select count(*) from identity_claims where user_id = $1")
        .bind(user)
        .fetch_one(pool)
        .await?)
}

/// The number of verification-state rows for a user.
pub async fn verification_count(pool: &sqlx::PgPool, user: uuid::Uuid) -> anyhow::Result<i64> {
    Ok(
        sqlx::query_scalar("select count(*) from identity_verifications where user_id = $1")
            .bind(user)
            .fetch_one(pool)
            .await?,
    )
}

/// Delete every identity row for a user, so a test starts from a clean slate.
pub async fn clear_user(pool: &sqlx::PgPool, user: uuid::Uuid) -> anyhow::Result<()> {
    // The tables refuse TRUNCATE by design, so a test cleans its own user's rows
    // with a delete. That is exactly the account-deletion path's own statement.
    sqlx::query("delete from identity_verifications where user_id = $1")
        .bind(user)
        .execute(pool)
        .await?;
    sqlx::query("delete from identity_claims where user_id = $1")
        .bind(user)
        .execute(pool)
        .await?;
    Ok(())
}

/// A fresh user id per test, so parallel tests never share rows.
pub fn user() -> uuid::Uuid {
    uuid::Uuid::new_v4()
}
