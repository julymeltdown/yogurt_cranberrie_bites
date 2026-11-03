//! Loopback provider fixture: real HTTP code grant, S256 validation and signed
//! OIDC tokens. It never contacts Apple or uses operational credentials.
use axum::extract::{Form, Query, State};
use axum::http::StatusCode;
use axum::response::Html;
use axum::Json;
use base64ct::{Base64UrlUnpadded, Encoding};
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use rsa::pkcs8::EncodePrivateKey;
use rsa::traits::PublicKeyParts;
use rsa::{RsaPrivateKey, RsaPublicKey};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

struct Grant {
    state: String,
    nonce: String,
    challenge: String,
    case: String,
}

pub struct Provider {
    pub pool: PgPool,
    pub gateway_pool: PgPool,
    pub redirect: String,
    pub jwks: String,
    key: EncodingKey,
    grants: Mutex<HashMap<String, Grant>>,
    pub calls: Mutex<BTreeMap<String, usize>>,
    states: Mutex<BTreeMap<String, String>>,
    markers: Mutex<Vec<String>>,
    pub race_state: Mutex<Option<String>>,
    pub barrier: tokio::sync::Barrier,
    pub rpc_activity: Arc<super::activity::Activity>,
    pub token_activity: Arc<super::activity::Activity>,
    events: Mutex<Vec<Value>>,
    started: std::time::Instant,
}

impl Provider {
    pub fn new(pool: PgPool, gateway_pool: PgPool, redirect: String) -> anyhow::Result<Self> {
        let key = RsaPrivateKey::new(&mut rand::rngs::OsRng, 2048)?;
        let public = RsaPublicKey::from(&key);
        let jwks = json!({"keys":[{"kty":"RSA","alg":"RS256","use":"sig","kid":"provider-fixture","n":Base64UrlUnpadded::encode_string(&public.n().to_bytes_be()),"e":Base64UrlUnpadded::encode_string(&public.e().to_bytes_be())}]}).to_string();
        Ok(Self {
            pool,
            gateway_pool,
            redirect,
            jwks,
            key: EncodingKey::from_rsa_pem(key.to_pkcs8_pem(Default::default())?.as_bytes())?,
            grants: Mutex::new(HashMap::new()),
            calls: Mutex::new(BTreeMap::new()),
            states: Mutex::new(BTreeMap::new()),
            markers: Mutex::new(Vec::new()),
            race_state: Mutex::new(None),
            barrier: tokio::sync::Barrier::new(2),
            rpc_activity: Arc::default(),
            token_activity: Arc::default(),
            events: Mutex::default(),
            started: std::time::Instant::now(),
        })
    }
}

pub async fn authorize(
    State(p): State<Arc<Provider>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Html<String>, StatusCode> {
    authorize_inner(&p, params)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)
}

async fn authorize_inner(
    p: &Provider,
    params: HashMap<String, String>,
) -> anyhow::Result<Html<String>> {
    let value = |name: &str| -> anyhow::Result<String> {
        params
            .get(name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("missing fixture authorization field"))
    };
    anyhow::ensure!(value("response_mode")? == "form_post" && value("response_type")? == "code");
    anyhow::ensure!(
        value("code_challenge_method")? == "S256" && value("client_id")? == "apple-fixture-client"
    );
    anyhow::ensure!(value("scope")?
        .split_whitespace()
        .any(|scope| scope == "email"));
    anyhow::ensure!(value("redirect_uri")? == p.redirect);
    let state = value("state")?;
    uuid::Uuid::parse_str(&state)?;
    let case = value("fixture_case")?;
    anyhow::ensure!([
        "first",
        "repeat",
        "missing_cookie",
        "mismatch",
        "expired",
        "provider_mismatch",
        "race",
        "tabs",
        "timeout",
        "cancelled",
        "bad_nonce"
    ]
    .contains(&case.as_str()));
    if case == "expired" {
        sqlx::query("update oauth_authorizations set expires_at=clock_timestamp()-interval '1 second' where state=$1")
            .bind(&state).execute(&p.pool).await?;
    }
    if case == "provider_mismatch" {
        sqlx::query("update oauth_authorizations set provider_type='GOOGLE' where state=$1")
            .bind(&state)
            .execute(&p.pool)
            .await?;
    }
    if case == "race" {
        *p.race_state.lock().unwrap() = Some(state.clone());
    }
    let code = uuid::Uuid::new_v4().to_string();
    let nonce = value("nonce")?;
    let challenge = value("code_challenge")?;
    p.states.lock().unwrap().insert(case.clone(), state.clone());
    p.markers
        .lock()
        .unwrap()
        .extend([code.clone(), state.clone(), nonce.clone()]);
    p.grants.lock().unwrap().insert(
        code.clone(),
        Grant {
            state: state.clone(),
            nonce,
            challenge,
            case: case.clone(),
        },
    );
    let posted_state = if case == "mismatch" {
        uuid::Uuid::new_v4().to_string()
    } else {
        state
    };
    let credential = if case == "cancelled" {
        "<input name=\"error\" value=\"access_denied\" type=\"hidden\">".to_owned()
    } else {
        format!("<input name=\"code\" value=\"{code}\" type=\"hidden\">")
    };
    p.markers.lock().unwrap().push(posted_state.clone());
    // Every interpolated value is either a generated UUID or the fixed,
    // listener-derived callback URI; no arbitrary query text enters HTML.
    Ok(Html(format!("<!doctype html><title>Local provider fixture</title><form method=\"post\" action=\"{}\">{credential}<input name=\"state\" value=\"{posted_state}\" type=\"hidden\"><button>Authorize fixture</button></form>", p.redirect)))
}

pub async fn token(
    State(p): State<Arc<Provider>>,
    Form(form): Form<HashMap<String, String>>,
) -> Result<Json<Value>, StatusCode> {
    let _flight = p.token_activity.enter();
    token_inner(&p, form)
        .await
        .map(Json)
        .map_err(|_| StatusCode::BAD_REQUEST)
}

async fn token_inner(p: &Provider, form: HashMap<String, String>) -> anyhow::Result<Value> {
    anyhow::ensure!(form.get("grant_type").map(String::as_str) == Some("authorization_code"));
    anyhow::ensure!(form.get("client_id").map(String::as_str) == Some("apple-fixture-client"));
    anyhow::ensure!(form.get("client_secret").map(String::as_str) == Some("local-fixture-only"));
    anyhow::ensure!(form.get("redirect_uri") == Some(&p.redirect));
    let code = form
        .get("code")
        .ok_or_else(|| anyhow::anyhow!("fixture code missing"))?;
    let grant = p
        .grants
        .lock()
        .unwrap()
        .remove(code)
        .ok_or_else(|| anyhow::anyhow!("fixture code invalid or reused"))?;
    *p.calls
        .lock()
        .unwrap()
        .entry(grant.case.clone())
        .or_default() += 1;
    let verifier = form
        .get("code_verifier")
        .ok_or_else(|| anyhow::anyhow!("fixture verifier missing"))?;
    p.markers.lock().unwrap().push(verifier.clone());
    anyhow::ensure!(
        Base64UrlUnpadded::encode_string(&Sha256::digest(verifier.as_bytes())) == grant.challenge,
        "fixture PKCE mismatch"
    );
    let consumed: bool = sqlx::query_scalar(
        "select consumed_at is not null from oauth_authorizations where state=$1",
    )
    .bind(&grant.state)
    .fetch_one(&p.pool)
    .await?;
    anyhow::ensure!(consumed, "exchange began before owning PG consumed state");
    if grant.case == "timeout" {
        tokio::time::sleep(std::time::Duration::from_millis(1800)).await;
    }
    let now = chrono::Utc::now().timestamp();
    let nonce = if grant.case == "bad_nonce" {
        "incorrect-fixture-nonce"
    } else {
        &grant.nonce
    };
    let mut claims = json!({"iss":"https://apple.fixture.invalid","aud":"apple-fixture-client","sub":"same-apple-fixture-subject","iat":now,"exp":now+600,"nonce":nonce});
    if grant.case != "repeat" {
        claims["email"] = json!("apple-network-fixture@example.com");
        claims["email_verified"] = json!("true");
    }
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some("provider-fixture".into());
    let token = jsonwebtoken::encode(&header, &claims, &p.key)?;
    p.markers.lock().unwrap().push(token.clone());
    p.markers
        .lock()
        .unwrap()
        .push("local-provider-fixture-access".into());
    Ok(
        json!({"access_token":"local-provider-fixture-access","token_type":"Bearer","id_token":token}),
    )
}

/// Test-only barrier: both isolated replay browsers must carry the original cookie
/// before either real Auth request is released, making the PG consume race real.
pub async fn race_gate(
    State(p): State<Arc<Provider>>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let state = p.race_state.lock().unwrap().clone();
    let racing = request.uri().path() == "/auth/oauth/apple/callback/complete"
        && state.is_some_and(|state| {
            request
                .headers()
                .get("cookie")
                .and_then(|value| value.to_str().ok())
                .is_some_and(|cookie| {
                    cookie
                        .split(';')
                        .any(|part| part.trim() == format!("oauth_state_apple={state}"))
                })
        });
    let completing = request.uri().path() == "/auth/oauth/apple/callback/complete";
    if completing {
        p.events.lock().unwrap().push(json!({"event":"completion-arrived","racing":racing,"elapsedMs":p.started.elapsed().as_millis()}));
    }
    if racing {
        let rendezvous =
            tokio::time::timeout(std::time::Duration::from_secs(5), p.barrier.wait()).await;
        *p.race_state.lock().unwrap() = None;
        if rendezvous.is_err() {
            p.events.lock().unwrap().push(
                json!({"event":"barrier-timeout","elapsedMs":p.started.elapsed().as_millis()}),
            );
            use axum::response::IntoResponse;
            return StatusCode::GATEWAY_TIMEOUT.into_response();
        }
    }
    let response = next.run(request).await;
    if completing {
        p.events.lock().unwrap().push(json!({"event":"completion-response","status":response.status().as_u16(),"elapsedMs":p.started.elapsed().as_millis()}));
    }
    // Observe actual successful Gateway credentials in memory, so the owning
    // database scan includes issued access/refresh tokens as well as grant data.
    if response.status().is_success()
        && response
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .is_some_and(|value| value.as_bytes().starts_with(b"application/json"))
    {
        for cookie in response.headers().get_all(axum::http::header::SET_COOKIE) {
            if let Ok(cookie) = cookie.to_str() {
                if let Some(token) = cookie
                    .strip_prefix("refresh_token=")
                    .and_then(|v| v.split(';').next())
                {
                    if !token.is_empty() {
                        p.markers.lock().unwrap().push(token.to_owned());
                    }
                }
            }
        }
        let (parts, body) = response.into_parts();
        let bytes = axum::body::to_bytes(body, 32768)
            .await
            .expect("bounded fixture JSON");
        if let Ok(value) = serde_json::from_slice::<Value>(&bytes) {
            if let Some(token) = value.get("accessToken").and_then(Value::as_str) {
                if !token.is_empty() {
                    p.markers.lock().unwrap().push(token.to_owned());
                }
            }
        }
        return axum::response::Response::from_parts(parts, axum::body::Body::from(bytes));
    }
    response
}

pub async fn observe(State(p): State<Arc<Provider>>) -> Result<Json<Value>, StatusCode> {
    observe_inner(&p)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn observe_inner(p: &Provider) -> anyhow::Result<Value> {
    // Bracket every storage read with monotonic activity epochs. Zero active
    // work at the end alone cannot certify an earlier PostgreSQL snapshot.
    let before = (p.rpc_activity.snapshot(), p.token_activity.snapshot());
    let sessions: i64 = sqlx::query_scalar("select count(*) from refresh_tokens")
        .fetch_one(&p.pool)
        .await?;
    let users: i64 = sqlx::query_scalar("select count(*) from users")
        .fetch_one(&p.pool)
        .await?;
    let identities: i64 = sqlx::query_scalar("select count(*) from user_identities")
        .fetch_one(&p.pool)
        .await?;
    let journal: i64 = sqlx::query_scalar("select count(*) from gateway_request_journal")
        .fetch_one(&p.gateway_pool)
        .await?;
    let queue: i64 = sqlx::query_scalar("select count(*) from gateway_async_operation_queue")
        .fetch_one(&p.gateway_pool)
        .await?;
    let states = p.states.lock().unwrap().clone();
    let mut consumed = BTreeMap::new();
    for (case, state) in states {
        let value: bool = sqlx::query_scalar(
            "select consumed_at is not null from oauth_authorizations where state=$1",
        )
        .bind(state)
        .fetch_one(&p.pool)
        .await?;
        consumed.insert(case, value);
    }
    let tables: Vec<String> =
        sqlx::query_scalar("select tablename from pg_tables where schemaname='public'")
            .fetch_all(&p.gateway_pool)
            .await?;
    let markers = p.markers.lock().unwrap().clone();
    let mut matches = 0;
    for table in tables {
        anyhow::ensure!(table
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'));
        let data: String = sqlx::query_scalar(&format!(
            "select coalesce(jsonb_agg(to_jsonb(t))::text,'') from \"{table}\" t"
        ))
        .fetch_one(&p.gateway_pool)
        .await?;
        matches += markers
            .iter()
            .filter(|marker| data.contains(marker.as_str()))
            .count();
    }
    let after = (p.rpc_activity.snapshot(), p.token_activity.snapshot());
    let stable = before == after && before.0.quiescent() && before.1.quiescent();
    Ok(
        json!({"sessions":sessions,"users":users,"identities":identities,"gatewayJournalRows":journal,"gatewayQueueRows":queue,"credentialStorageMatches":matches,"consumed":consumed,"providerCalls":p.calls.lock().unwrap().clone(),
        "fixtureEvents":p.events.lock().unwrap().clone(),
        "observationStable":stable,
        "authCallbacksStarted":after.0.started,
        "authCallbacksFinished":after.0.finished,
        "authCallbacksActive":after.0.active,
        "providerTokensActive":after.1.active}),
    )
}

#[tokio::test]
async fn unpaired_fixture_race_times_out_before_dispatch_without_auth_effects() -> anyhow::Result<()>
{
    use axum::http::{Request, StatusCode};
    use axum::middleware::from_fn_with_state;
    use tower::ServiceExt;
    let fixture = crate::auth_runtime_fixture::Fixture::new().await?;
    let provider = Arc::new(Provider::new(
        fixture.pool.clone(),
        fixture.pool.clone(),
        "http://127.0.0.1/callback".into(),
    )?);
    let state = uuid::Uuid::new_v4().to_string();
    *provider.race_state.lock().unwrap() = Some(state.clone());
    let router = axum::Router::new()
        .route(
            "/auth/oauth/apple/callback/complete",
            axum::routing::post(|| async { StatusCode::IM_A_TEAPOT }),
        )
        .layer(from_fn_with_state(provider.clone(), race_gate));
    let response = tokio::time::timeout(
        std::time::Duration::from_secs(7),
        router.oneshot(
            Request::post("/auth/oauth/apple/callback/complete")
                .header("cookie", format!("oauth_state_apple={state}"))
                .body(axum::body::Body::empty())?,
        ),
    )
    .await??;
    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    assert!(provider.race_state.lock().unwrap().is_none());
    assert!(provider.rpc_activity.snapshot().quiescent());
    fixture.empty_effects().await?;
    fixture.pool.close().await;
    Ok(())
}
