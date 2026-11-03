use crate::adapters::out::security::apple_client_secret::AppleClientSecretGenerator;
use crate::application::ports::OAuthProviderPort;
use crate::domain::oauth_failure::OAuthProviderFailure;
use crate::domain::oauth_identity::OAuthUserInfo;
use crate::domain::user::IdentityProvider;
use anyhow::Context;
use async_trait::async_trait;
use base64ct::{Base64UrlUnpadded, Encoding};
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use reqwest::Url;
use serde::Deserialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// Upper bound for successful OAuth token, userinfo, and JWKS response bodies.
const MAX_OAUTH_PROVIDER_RESPONSE_BYTES: usize = 64 * 1024;
const MAX_OAUTH_PROVIDER_ERROR_BYTES: usize = 8 * 1024;
const MAX_OAUTH_PROVIDER_JWKS_KEYS: usize = 16;
const MIN_OAUTH_PROVIDER_RSA_MODULUS_BITS: usize = 2_048;
const MAX_OAUTH_PROVIDER_RSA_MODULUS_BITS: usize = 4_096;

#[derive(Clone)]
pub struct ParityOAuthProviderClient {
    google_authorize_base_uri: String,
    apple_authorize_base_uri: String,
    x_authorize_base_uri: String,
}

impl ParityOAuthProviderClient {
    pub fn new(
        google_authorize_base_uri: String,
        apple_authorize_base_uri: String,
        x_authorize_base_uri: String,
    ) -> Self {
        Self {
            google_authorize_base_uri,
            apple_authorize_base_uri,
            x_authorize_base_uri,
        }
    }

    fn authorize_base_uri(&self, provider: IdentityProvider) -> anyhow::Result<&str> {
        match provider {
            IdentityProvider::Google => Ok(self.google_authorize_base_uri.as_str()),
            IdentityProvider::Apple => Ok(self.apple_authorize_base_uri.as_str()),
            IdentityProvider::X => Ok(self.x_authorize_base_uri.as_str()),
            IdentityProvider::Email => anyhow::bail!("OAuth provider required"),
        }
    }

    fn fixture(provider: IdentityProvider, code: &str) -> Option<OAuthUserInfo> {
        let fixtures: HashMap<
            (IdentityProvider, &'static str),
            (&'static str, &'static str, bool),
        > = HashMap::from([
            (
                (IdentityProvider::Google, "google-hana-code"),
                ("google-hana-subject", "hana@example.com", true),
            ),
            (
                (IdentityProvider::Google, "google-rust-code"),
                ("google-rust-subject", "rust.oauth@example.com", true),
            ),
            (
                (IdentityProvider::Apple, "apple-hana-code"),
                ("apple-hana-subject", "hana@example.com", true),
            ),
            (
                (IdentityProvider::Apple, "apple-mira-code"),
                ("apple-mira-subject", "mira@example.com", true),
            ),
            (
                (IdentityProvider::X, "x-hana-code"),
                ("x-hana-id", "hana.x@example.com", true),
            ),
            (
                (IdentityProvider::X, "x-rust-code"),
                ("x-rust-id", "rust.x@example.com", true),
            ),
        ]);

        fixtures
            .get(&(provider, code))
            .map(|(subject, email, verified)| OAuthUserInfo {
                provider,
                subject: (*subject).to_owned(),
                email: (*email).to_owned(),
                email_verified: *verified,
            })
    }
}

#[async_trait]
impl OAuthProviderPort for ParityOAuthProviderClient {
    fn build_authorization_uri(
        &self,
        provider: crate::application::dto::repository_dto::AuthIdentityProviderDto,
        redirect_uri: &str,
        state: &str,
        code_challenge: &str,
        nonce: &str,
    ) -> crate::application::errors::AuthResult<String> {
        let provider = crate::domain::user::IdentityProvider::from(provider);

        let result: anyhow::Result<_> = (|| {
            let base = self.authorize_base_uri(provider)?;
            if provider == IdentityProvider::Apple {
                return authorization_uri(
                    base,
                    provider,
                    vec![
                        ("response_type", "code".into()),
                        ("client_id", "apple-rust-client".into()),
                        ("redirect_uri", redirect_uri.into()),
                        ("state", state.into()),
                        ("code_challenge", code_challenge.into()),
                        ("code_challenge_method", "S256".into()),
                        ("nonce", nonce.into()),
                        ("response_mode", "form_post".into()),
                    ],
                );
            }
            let nonce_param = match provider {
                IdentityProvider::Google | IdentityProvider::Apple => format!("&nonce={nonce}"),
                IdentityProvider::X => String::new(),
                IdentityProvider::Email => unreachable!(),
            };
            Ok(format!(
            "{base}?response_type=code&client_id={}&redirect_uri={redirect_uri}&state={state}&code_challenge={code_challenge}&code_challenge_method=S256{nonce_param}",
            match provider {
                IdentityProvider::Google => "google-rust-client",
                IdentityProvider::Apple => "apple-rust-client",
                IdentityProvider::X => "x-rust-client",
                IdentityProvider::Email => unreachable!(),
            }
        ))
        })();
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    async fn fetch_user_info(
        &self,
        provider: crate::application::dto::repository_dto::AuthIdentityProviderDto,
        code: &str,
        _redirect_uri: &str,
        _code_verifier: Option<&str>,
        _expected_nonce: &str,
    ) -> crate::application::errors::AuthResult<
        crate::application::dto::repository_dto::AuthOAuthProofDto,
    > {
        let provider = crate::domain::user::IdentityProvider::from(provider);
        let result: anyhow::Result<_> = async {
            Self::fixture(provider, code).ok_or_else(|| OAuthProviderFailure::code_invalid().into())
        }
        .await;
        result
            .map_err(crate::adapters::out::errors::boundary_error)
            .map(|value| value.into())
    }
}

#[derive(Clone)]
pub struct OAuthHttpProviderSettings {
    pub authorize_base_uri: String,
    pub token_uri: String,
    pub userinfo_uri: String,
    pub issuer: String,
    pub jwks_json: String,
    pub jwks_uri: Option<String>,
    pub client_id: String,
    /// A secret that cannot be rotated, used when no generator is configured.
    pub client_secret: String,
    /// Mints a signed, expiring secret on each token request.
    ///
    /// A generated secret is a JWT with an expiry, so it has to be minted when
    /// it is used rather than once at startup: a process that runs longer than
    /// its lifetime would otherwise start presenting an expired secret and fail
    /// every sign-in until it was restarted.
    pub client_secret_generator: Option<std::sync::Arc<AppleClientSecretGenerator>>,
    pub scopes: Vec<String>,
    pub token_auth_method: OAuthTokenAuthMethod,
    pub userinfo_format: OAuthUserInfoFormat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OAuthTokenAuthMethod {
    ClientSecretPost,
    BasicAuth,
}

impl OAuthHttpProviderSettings {
    /// The client secret to present on this request.
    ///
    /// A configured generator mints a fresh signed secret, so a long-running
    /// process never presents an expired one. Otherwise the configured static
    /// value is used, which is the case for providers that issue a fixed secret.
    fn resolve_client_secret(&self) -> anyhow::Result<String> {
        match self.client_secret_generator.as_ref() {
            Some(generator) => generator.generate(chrono::Utc::now()),
            None => Ok(self.client_secret.clone()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OAuthUserInfoFormat {
    Oidc,
    XApiV2,
}

/// Provider calls run only inside the Gateway to Auth OAuth callback, whose
/// caller abandons the request at that edge's deadline. Any provider request
/// still running after it does no useful work, so each request is capped at
/// that deadline even if cancellation does not reach this task. This is a
/// ceiling, not a tuned provider budget; each request also stops at the
/// caller's remaining time (see [provider_request_budget]).
pub fn provider_request_ceiling() -> std::time::Duration {
    platform::edge::Edge::GatewayAuth.budget().deadline
}

/// Time for one provider request: the caller's remaining budget when the
/// Gateway sent one, never more than [provider_request_ceiling]. Sequential
/// callback requests therefore share the caller's time instead of each taking
/// the full ceiling.
pub fn provider_request_budget() -> std::time::Duration {
    platform::call_deadline::bounded(provider_request_ceiling())
}

fn provider_http_client() -> anyhow::Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .timeout(provider_request_ceiling())
        .build()?)
}

#[derive(Clone)]
pub struct HttpOAuthProviderClient {
    google: OAuthHttpProviderSettings,
    apple: OAuthHttpProviderSettings,
    x: OAuthHttpProviderSettings,
    http: reqwest::Client,
}

impl HttpOAuthProviderClient {
    pub fn new(
        google: OAuthHttpProviderSettings,
        apple: OAuthHttpProviderSettings,
        x: OAuthHttpProviderSettings,
    ) -> anyhow::Result<Self> {
        validate_settings("google", &google)?;
        validate_settings("apple", &apple)?;
        validate_settings("x", &x)?;
        Ok(Self {
            google,
            apple,
            x,
            http: provider_http_client()?,
        })
    }

    fn settings(&self, provider: IdentityProvider) -> anyhow::Result<&OAuthHttpProviderSettings> {
        match provider {
            IdentityProvider::Google => Ok(&self.google),
            IdentityProvider::Apple => Ok(&self.apple),
            IdentityProvider::X => Ok(&self.x),
            IdentityProvider::Email => anyhow::bail!("OAuth provider required"),
        }
    }
}

// Apple requires one unambiguous form_post response mode even when a configured
// authorization base already carries a query. Preserve unrelated query pairs.
fn authorization_uri(
    base: &str,
    provider: IdentityProvider,
    params: Vec<(&str, String)>,
) -> anyhow::Result<String> {
    let mut url = Url::parse(base).context("invalid OAuth authorize URI")?;
    if provider == IdentityProvider::Apple {
        let retained: Vec<(String, String)> = url
            .query_pairs()
            .filter(|(key, _)| key != "response_mode")
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect();
        url.set_query(None);
        url.query_pairs_mut().extend_pairs(retained);
    }
    url.query_pairs_mut().extend_pairs(params);
    Ok(url.to_string())
}

#[async_trait]
impl OAuthProviderPort for HttpOAuthProviderClient {
    fn build_authorization_uri(
        &self,
        provider: crate::application::dto::repository_dto::AuthIdentityProviderDto,
        redirect_uri: &str,
        state: &str,
        code_challenge: &str,
        nonce: &str,
    ) -> crate::application::errors::AuthResult<String> {
        let provider = crate::domain::user::IdentityProvider::from(provider);

        let result: anyhow::Result<_> = (|| {
            let settings = self.settings(provider)?;
            let mut params = vec![
                ("response_type", "code".to_owned()),
                ("client_id", settings.client_id.clone()),
                ("redirect_uri", redirect_uri.to_owned()),
                ("state", state.to_owned()),
                ("code_challenge", code_challenge.to_owned()),
                ("code_challenge_method", "S256".to_owned()),
            ];
            if settings.userinfo_format == OAuthUserInfoFormat::Oidc {
                params.push(("nonce", nonce.to_owned()));
            }
            if provider == IdentityProvider::Apple {
                params.push(("response_mode", "form_post".to_owned()));
            }
            if !settings.scopes.is_empty() {
                params.push(("scope", settings.scopes.join(" ")));
            }
            authorization_uri(&settings.authorize_base_uri, provider, params)
        })();
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    async fn fetch_user_info(
        &self,
        provider: crate::application::dto::repository_dto::AuthIdentityProviderDto,
        code: &str,
        redirect_uri: &str,
        code_verifier: Option<&str>,
        expected_nonce: &str,
    ) -> crate::application::errors::AuthResult<
        crate::application::dto::repository_dto::AuthOAuthProofDto,
    > {
        let provider = crate::domain::user::IdentityProvider::from(provider);
        let result: anyhow::Result<_> = async {
            let settings = self.settings(provider)?;
            let mut form = vec![
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", redirect_uri),
            ];
            let mut token_request = self.http.post(&settings.token_uri);
            // Resolved once per request and held for the request's lifetime. A
            // generated secret is a fresh JWT, so it must not be borrowed from a
            // temporary that is dropped before the form is sent.
            let client_secret = settings.resolve_client_secret()?;
            match settings.token_auth_method {
                OAuthTokenAuthMethod::ClientSecretPost => {
                    form.push(("client_id", settings.client_id.as_str()));
                    form.push(("client_secret", client_secret.as_str()));
                }
                OAuthTokenAuthMethod::BasicAuth => {
                    token_request =
                        token_request.basic_auth(&settings.client_id, Some(&client_secret));
                }
            }
            if let Some(code_verifier) = code_verifier.filter(|value| !value.trim().is_empty()) {
                form.push(("code_verifier", code_verifier));
            }
            let token_response = token_request
                .form(&form)
                .timeout(provider_request_budget())
                .send()
                .await
                .map_err(|_| OAuthProviderFailure::token_request_failed())?;
            if !token_response.status().is_success() {
                return Err(classify_token_error(token_response).await.into());
            }
            let token_body =
                read_bounded_response_body(token_response, MAX_OAUTH_PROVIDER_RESPONSE_BYTES)
                    .await
                    .map_err(|_| OAuthProviderFailure::token_response_invalid())?;
            let token = serde_json::from_slice::<OAuthTokenResponse>(&token_body)
                .map_err(|_| OAuthProviderFailure::token_response_invalid())?;

            if token.access_token.trim().is_empty() {
                return Err(OAuthProviderFailure::token_access_token_missing().into());
            }
            let id_token_subject = match settings.userinfo_format {
                OAuthUserInfoFormat::Oidc => {
                    let id_token = token
                        .id_token
                        .as_deref()
                        .filter(|value| !value.trim().is_empty())
                        .ok_or_else(OAuthProviderFailure::id_token_invalid)?;
                    Some(validate_id_token(settings, &self.http, id_token, expected_nonce).await?)
                }
                OAuthUserInfoFormat::XApiV2 => None,
            };

            // Apple does not serve a userinfo endpoint that returns the profile, so
            // requesting one is not merely wasteful: it fails, and the flow broke
            // with it. The identity token is the authority instead, which is also the
            // safer source, because its claims are covered by a signature the client
            // can verify while a userinfo response is trusted on the strength of the
            // access token alone.
            if provider == IdentityProvider::Apple {
                let id_token = token
                    .id_token
                    .as_deref()
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(OAuthProviderFailure::id_token_invalid)?;
                return apple_user_info(settings, &self.http, id_token, expected_nonce).await;
            }

            let userinfo_response = self
                .http
                .get(&settings.userinfo_uri)
                .bearer_auth(&token.access_token)
                .timeout(provider_request_budget())
                .send()
                .await
                .map_err(|_| OAuthProviderFailure::userinfo_request_failed())?;
            if !userinfo_response.status().is_success() {
                return Err(classify_userinfo_error(userinfo_response).await.into());
            }
            let userinfo_body =
                read_bounded_response_body(userinfo_response, MAX_OAUTH_PROVIDER_RESPONSE_BYTES)
                    .await
                    .map_err(|_| OAuthProviderFailure::userinfo_response_invalid())?;
            let userinfo = serde_json::from_slice::<OAuthUserInfoResponse>(&userinfo_body)
                .map_err(|_| OAuthProviderFailure::userinfo_response_invalid())?;

            let subject = userinfo
                .subject(settings.userinfo_format)
                .ok_or_else(OAuthProviderFailure::userinfo_subject_missing)?;
            if let Some(id_token_subject) = id_token_subject.as_deref() {
                if subject != id_token_subject {
                    return Err(OAuthProviderFailure::id_token_invalid().into());
                }
            }
            let email_verified = userinfo.email_verified(settings.userinfo_format);
            let email = userinfo.email(settings.userinfo_format, subject);

            Ok(OAuthUserInfo {
                provider,
                subject: subject.to_owned(),
                email,
                email_verified,
            })
        }
        .await;
        result
            .map_err(crate::adapters::out::errors::boundary_error)
            .map(|value| value.into())
    }
}

#[derive(Deserialize)]
struct OAuthTokenResponse {
    access_token: String,
    id_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OAuthErrorResponse {
    error: Option<String>,
}

#[derive(Deserialize)]
struct OidcIdTokenClaims {
    sub: String,
    nonce: String,
    /// Present on Apple identity tokens, and the only trustworthy source of the
    /// address: Apple returns the email here rather than through a userinfo call.
    #[serde(default)]
    email: Option<String>,
    /// Apple sends this as a string ("true"/"false") rather than a JSON boolean,
    /// so it is kept as a value and interpreted explicitly.
    #[serde(default)]
    email_verified: Option<Value>,
}

#[derive(Deserialize)]
struct OAuthUserInfoResponse {
    sub: Option<String>,
    email: Option<String>,
    email_verified: Option<Value>,
    data: Option<XApiV2UserInfoData>,
}

#[derive(Debug, Deserialize)]
struct XApiV2UserInfoData {
    id: String,
    email: Option<String>,
    confirmed_email: Option<String>,
}

impl OAuthUserInfoResponse {
    fn subject(&self, format: OAuthUserInfoFormat) -> Option<&str> {
        match format {
            OAuthUserInfoFormat::Oidc => trimmed(self.sub.as_deref()),
            OAuthUserInfoFormat::XApiV2 => self
                .data
                .as_ref()
                .and_then(|data| trimmed(Some(data.id.as_str()))),
        }
    }

    fn email(&self, format: OAuthUserInfoFormat, subject: &str) -> String {
        match format {
            OAuthUserInfoFormat::Oidc => trimmed(self.email.as_deref())
                .map(str::to_owned)
                .unwrap_or_else(|| format!("{subject}@oauth.local")),
            OAuthUserInfoFormat::XApiV2 => self
                .data
                .as_ref()
                .and_then(|data| {
                    trimmed(data.confirmed_email.as_deref())
                        .or_else(|| trimmed(data.email.as_deref()))
                })
                .or_else(|| trimmed(self.email.as_deref()))
                .map(str::to_owned)
                .unwrap_or_else(|| format!("x-{subject}@oauth.local")),
        }
    }

    fn email_verified(&self, format: OAuthUserInfoFormat) -> bool {
        match format {
            OAuthUserInfoFormat::Oidc => match self.email_verified.as_ref() {
                Some(Value::Bool(value)) => *value,
                Some(Value::String(value)) => value.eq_ignore_ascii_case("true"),
                _ => false,
            },
            OAuthUserInfoFormat::XApiV2 => self
                .data
                .as_ref()
                .and_then(|data| trimmed(data.confirmed_email.as_deref()))
                .is_some(),
        }
    }
}

/// Build the identity from a verified Apple identity token.
///
/// Apple is handled separately because its identity is carried entirely by the
/// token. There is no userinfo call to make: the endpoint does not serve the
/// profile, so requesting it failed the whole flow. Reading claims from the token
/// is also the stronger choice. The signature covers those claims and the client
/// verifies it against Apple keys, whereas a userinfo response is trusted only on
/// the strength of whoever presents the access token.
async fn apple_user_info(
    settings: &OAuthHttpProviderSettings,
    http: &reqwest::Client,
    id_token: &str,
    expected_nonce: &str,
) -> anyhow::Result<OAuthUserInfo> {
    let claims = validate_id_token_claims(settings, http, id_token, expected_nonce).await?;
    // Apple only returns the address on the first authorization, so it may be
    // absent on later sign-ins. An absent address is not an error here: the
    // subject identifies the account, and the caller decides what an existing
    // account needs.
    let email = claims
        .email
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        // The domain models "no address supplied" as an empty string and the
        // sign-in and link paths already branch on that, so an absent email is
        // represented that way rather than invented or treated as a failure.
        .unwrap_or_default();
    let email_verified = if email.is_empty() {
        false
    } else {
        verified_flag(claims.email_verified.as_ref())
    };
    Ok(OAuthUserInfo {
        provider: IdentityProvider::Apple,
        subject: claims.sub,
        email,
        email_verified,
    })
}

/// Interpret an email_verified claim, which is not consistently typed.
///
/// Apple sends the string "true" rather than a JSON boolean, and a boolean-only
/// check would silently report every Apple address as unverified. Anything other
/// than an explicit affirmative is treated as unverified, so an unrecognized
/// shape cannot be mistaken for a verified address.
fn verified_flag(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Bool(value)) => *value,
        Some(Value::String(value)) => value.eq_ignore_ascii_case("true"),
        _ => false,
    }
}

fn validate_settings(provider: &str, settings: &OAuthHttpProviderSettings) -> anyhow::Result<()> {
    require_url(
        &settings.authorize_base_uri,
        &format!("{provider} authorize URI"),
    )?;
    require_url(&settings.token_uri, &format!("{provider} token URI"))?;
    require_url(&settings.userinfo_uri, &format!("{provider} userinfo URI"))?;
    if settings.client_id.trim().is_empty() {
        anyhow::bail!("{provider} OAuth client id is required");
    }
    // A generator replaces the static secret, so the empty static value is
    // expected when one is configured.
    if settings.client_secret.trim().is_empty() && settings.client_secret_generator.is_none() {
        anyhow::bail!("{provider} OAuth client secret is required");
    }
    if settings.userinfo_format == OAuthUserInfoFormat::Oidc {
        require_url(&settings.issuer, &format!("{provider} issuer"))?;
        parse_jwks(&settings.jwks_json)?;
        if let Some(jwks_uri) = settings.jwks_uri.as_deref() {
            require_url(jwks_uri, &format!("{provider} JWKS URI"))?;
        }
    }
    Ok(())
}

fn trimmed(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn require_url(value: &str, label: &str) -> anyhow::Result<()> {
    Url::parse(value)
        .with_context(|| format!("{label} is invalid"))?
        .has_host()
        .then_some(())
        .ok_or_else(|| anyhow::anyhow!("{label} must include a host"))
}

async fn validate_id_token(
    settings: &OAuthHttpProviderSettings,
    http: &reqwest::Client,
    id_token: &str,
    expected_nonce: &str,
) -> anyhow::Result<String> {
    Ok(
        validate_id_token_claims(settings, http, id_token, expected_nonce)
            .await?
            .sub,
    )
}

/// Verify an identity token and return its claims.
///
/// The signature, issuer, audience, expiry, and nonce are all checked before any
/// claim is used, so a claim read from the result is one the provider signed.
async fn validate_id_token_claims(
    settings: &OAuthHttpProviderSettings,
    http: &reqwest::Client,
    id_token: &str,
    expected_nonce: &str,
) -> anyhow::Result<OidcIdTokenClaims> {
    if id_token.trim().is_empty() || expected_nonce.trim().is_empty() {
        return Err(OAuthProviderFailure::id_token_invalid().into());
    }
    let (key, algorithm) = match id_token_decoding_key(id_token, &settings.jwks_json) {
        Ok(resolved) => resolved,
        Err(_) => async_jwks_refresh_decoding_key(http, settings, id_token)
            .await
            .map_err(|_| OAuthProviderFailure::id_token_invalid())?,
    };
    let mut validation = Validation::new(algorithm);
    validation.set_issuer(&[settings.issuer.as_str()]);
    validation.set_audience(&[settings.client_id.as_str()]);
    validation.set_required_spec_claims(&["exp", "iss", "sub", "aud", "nonce"]);

    let data = decode::<OidcIdTokenClaims>(id_token, &key, &validation)
        .map_err(|_| OAuthProviderFailure::id_token_invalid())?;
    if data.claims.nonce != expected_nonce {
        return Err(OAuthProviderFailure::id_token_invalid().into());
    }
    Ok(data.claims)
}

async fn classify_token_error(response: reqwest::Response) -> OAuthProviderFailure {
    match provider_error_code(response).await.as_deref() {
        Some("invalid_grant") => OAuthProviderFailure::token_invalid_grant(),
        Some("invalid_client") => OAuthProviderFailure::token_invalid_client(),
        Some("invalid_request") => OAuthProviderFailure::token_invalid_request(),
        _ => OAuthProviderFailure::token_http_failure(),
    }
}

async fn classify_userinfo_error(response: reqwest::Response) -> OAuthProviderFailure {
    match provider_error_code(response).await.as_deref() {
        Some("insufficient_scope") => OAuthProviderFailure::userinfo_insufficient_scope(),
        _ => OAuthProviderFailure::userinfo_http_failure(),
    }
}

async fn provider_error_code(response: reqwest::Response) -> Option<String> {
    let body = read_bounded_response_body(response, MAX_OAUTH_PROVIDER_ERROR_BYTES)
        .await
        .ok()?;
    serde_json::from_slice::<OAuthErrorResponse>(&body)
        .ok()
        .and_then(|body| body.error)
}

async fn read_bounded_response_body(
    mut response: reqwest::Response,
    max_bytes: usize,
) -> anyhow::Result<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|length| length > max_bytes as u64)
    {
        anyhow::bail!("OAuth provider response exceeded the configured size limit");
    }

    let capacity = response
        .content_length()
        .and_then(|length| usize::try_from(length).ok())
        .unwrap_or_default()
        .min(max_bytes);
    let mut body = Vec::with_capacity(capacity);
    while let Some(chunk) = response.chunk().await? {
        if chunk.len() > max_bytes.saturating_sub(body.len()) {
            anyhow::bail!("OAuth provider response exceeded the configured size limit");
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

async fn async_jwks_refresh_decoding_key(
    http: &reqwest::Client,
    settings: &OAuthHttpProviderSettings,
    id_token: &str,
) -> anyhow::Result<(DecodingKey, Algorithm)> {
    let jwks_uri = settings
        .jwks_uri
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("OAuth provider JWKS URI is not configured"))?;
    let response = http
        .get(jwks_uri)
        .timeout(provider_request_budget())
        .send()
        .await?;
    if !response.status().is_success() {
        anyhow::bail!("OAuth provider JWKS refresh failed");
    }
    let refreshed_jwks =
        read_bounded_response_body(response, MAX_OAUTH_PROVIDER_RESPONSE_BYTES).await?;
    let refreshed_jwks = std::str::from_utf8(&refreshed_jwks)
        .context("OAuth provider JWKS response was not UTF-8")?;
    id_token_decoding_key(id_token, refreshed_jwks)
}

/// Select the key that signed the token, and the algorithm it is published for.
///
/// The algorithm comes from the JWKS entry rather than being assumed. Apple signs
/// identity tokens with ES256 while other providers here use RS256, so hardcoding
/// one algorithm rejected every Apple token. The value is also what the caller must
/// validate against, because accepting a token under an algorithm its key was not
/// published for is where algorithm confusion begins.
fn id_token_decoding_key(
    id_token: &str,
    jwks_json: &str,
) -> anyhow::Result<(DecodingKey, Algorithm)> {
    let header = decode_header(id_token)?;
    let kid = header
        .kid
        .ok_or_else(|| anyhow::anyhow!("OAuth provider ID token kid is required"))?;
    let token_algorithm = header.alg;
    let mut candidates = parse_jwks(jwks_json)?
        .keys
        .into_iter()
        .filter(|key| {
            key.kid.as_deref() == Some(kid.as_str()) && key.supports_signature_verification()
        })
        .filter_map(|key| {
            let algorithm = key.algorithm().ok()?;
            (algorithm == token_algorithm).then_some((key, algorithm))
        });
    let (key, algorithm) = candidates.next().ok_or_else(|| {
        anyhow::anyhow!("OAuth provider ID token kid has no usable matching signing key")
    })?;
    anyhow::ensure!(
        candidates.next().is_none(),
        "OAuth provider ID token kid has ambiguous matching signing keys"
    );
    Ok((key.decoding_key()?, algorithm))
}

fn parse_jwks(jwks_json: &str) -> anyhow::Result<JwkSet> {
    if jwks_json.len() > MAX_OAUTH_PROVIDER_RESPONSE_BYTES {
        anyhow::bail!("OAuth provider JWKS exceeded the configured size limit");
    }
    if jwks_json.trim().is_empty() {
        anyhow::bail!("OAuth provider JWKS JSON is required");
    }
    let jwks: JwkSet = serde_json::from_str(jwks_json)?;
    if jwks.keys.len() > MAX_OAUTH_PROVIDER_JWKS_KEYS {
        anyhow::bail!("OAuth provider JWKS exceeded the configured key limit");
    }
    // A usable key must be identifiable, supported, and explicitly compatible
    // with signature verification when the JWK declares its intended use.
    if jwks.keys.iter().any(Jwk::supports_signature_verification) {
        return Ok(jwks);
    }
    anyhow::bail!("OAuth provider JWKS does not contain a usable signing key");
}

/// Maximum accepted response size exposed only to the isolated parser fuzzer.
#[cfg(feature = "fuzzing")]
#[doc(hidden)]
pub const FUZZ_MAX_OAUTH_PROVIDER_RESPONSE_BYTES: usize = MAX_OAUTH_PROVIDER_RESPONSE_BYTES;

#[cfg(feature = "fuzzing")]
#[doc(hidden)]
pub const FUZZ_MAX_OAUTH_PROVIDER_ERROR_BYTES: usize = MAX_OAUTH_PROVIDER_ERROR_BYTES;

#[cfg(feature = "fuzzing")]
#[doc(hidden)]
pub const FUZZ_MAX_OAUTH_PROVIDER_JWKS_KEYS: usize = MAX_OAUTH_PROVIDER_JWKS_KEYS;

#[cfg(feature = "fuzzing")]
#[doc(hidden)]
pub const FUZZ_PARSED_TOKEN_RESPONSE: u8 = 1;
#[cfg(feature = "fuzzing")]
#[doc(hidden)]
pub const FUZZ_PARSED_USERINFO_RESPONSE: u8 = 2;
#[cfg(feature = "fuzzing")]
#[doc(hidden)]
pub const FUZZ_PARSED_ERROR_RESPONSE: u8 = 4;
#[cfg(feature = "fuzzing")]
#[doc(hidden)]
pub const FUZZ_PARSED_JWKS: u8 = 8;
#[cfg(feature = "fuzzing")]
#[doc(hidden)]
pub const FUZZ_SELECTED_ID_TOKEN_KEY: u8 = 16;

/// Exercise bounded provider-response decoders and ID-token signing-key selection.
/// Token signatures and claims are outside this hook; it is compiled only by the
/// isolated fuzz workspace.
#[cfg(feature = "fuzzing")]
#[doc(hidden)]
pub fn fuzz_process_oauth_provider_response(input: &[u8], jwks_json: &str) -> Option<u8> {
    if input.len() > MAX_OAUTH_PROVIDER_RESPONSE_BYTES
        || jwks_json.len() > MAX_OAUTH_PROVIDER_RESPONSE_BYTES
    {
        return None;
    }

    let mut parsed = 0;
    if let Ok(token) = serde_json::from_slice::<OAuthTokenResponse>(input) {
        parsed |= FUZZ_PARSED_TOKEN_RESPONSE;
        let _ = token.access_token.trim();
        if let Some(id_token) = token.id_token.as_deref() {
            let _ = decode_header(id_token);
            if id_token_decoding_key(id_token, jwks_json).is_ok() {
                parsed |= FUZZ_SELECTED_ID_TOKEN_KEY;
            }
        }
    }

    if let Ok(userinfo) = serde_json::from_slice::<OAuthUserInfoResponse>(input) {
        parsed |= FUZZ_PARSED_USERINFO_RESPONSE;
        for format in [OAuthUserInfoFormat::Oidc, OAuthUserInfoFormat::XApiV2] {
            let subject = userinfo.subject(format).unwrap_or_default();
            let _ = userinfo.email(format, subject);
            let _ = userinfo.email_verified(format);
        }
    }

    if input.len() <= MAX_OAUTH_PROVIDER_ERROR_BYTES {
        if let Ok(error) = serde_json::from_slice::<OAuthErrorResponse>(input) {
            parsed |= FUZZ_PARSED_ERROR_RESPONSE;
            let _ = error.error;
        }
    }

    if let Ok(document) = std::str::from_utf8(input) {
        if let Ok(jwks) = parse_jwks(document) {
            parsed |= FUZZ_PARSED_JWKS;
            for key in jwks.keys {
                let _ = key.algorithm();
                let _ = key.decoding_key();
            }
        }
    }

    Some(parsed)
}

#[derive(Debug, Deserialize)]
struct JwkSet {
    keys: Vec<Jwk>,
}

#[derive(Debug, Deserialize)]
struct Jwk {
    kid: Option<String>,
    kty: String,
    alg: Option<String>,
    /// Elliptic-curve name required for EC keys.
    #[serde(default)]
    crv: Option<String>,
    /// Optional JWK use. If provided, only signature keys can verify ID tokens.
    #[serde(rename = "use", default, deserialize_with = "deserialize_jwk_use")]
    key_use: Option<String>,
    /// Optional permitted operations. If provided, verification must be allowed.
    #[serde(default, deserialize_with = "deserialize_jwk_key_ops")]
    key_ops: Option<Vec<String>>,
    /// RSA modulus and exponent.
    #[serde(default)]
    n: Option<String>,
    #[serde(default)]
    e: Option<String>,
    /// Elliptic-curve coordinates, used by Apple for ES256.
    #[serde(default)]
    x: Option<String>,
    #[serde(default)]
    y: Option<String>,
}

fn deserialize_jwk_use<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    String::deserialize(deserializer).map(Some)
}

fn deserialize_jwk_key_ops<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Vec::<String>::deserialize(deserializer).map(Some)
}

/// Whether a base64url RSA modulus is within the verifier's supported bounds.
fn usable_rsa_modulus(modulus: &str) -> bool {
    let Ok(bytes) = Base64UrlUnpadded::decode_vec(modulus) else {
        return false;
    };
    let Some(first_byte) = bytes.first() else {
        return false;
    };
    // JWA Base64urlUInt values use the minimum unsigned big-endian octet
    // sequence; accepting a leading zero creates a second encoding of n.
    if *first_byte == 0 || bytes.last().is_none_or(|byte| byte % 2 == 0) {
        return false;
    }
    let bits = bytes.len() * 8 - first_byte.leading_zeros() as usize;
    (MIN_OAUTH_PROVIDER_RSA_MODULUS_BITS..=MAX_OAUTH_PROVIDER_RSA_MODULUS_BITS).contains(&bits)
}

/// Whether the public exponent is an odd integer of at least three.
fn usable_rsa_exponent(exponent: &str) -> bool {
    let Ok(bytes) = Base64UrlUnpadded::decode_vec(exponent) else {
        return false;
    };
    let Some(first_byte) = bytes.first() else {
        return false;
    };
    if *first_byte == 0 || bytes.len() > 8 {
        return false;
    }
    let value = bytes
        .iter()
        .fold(0_u64, |value, byte| (value << 8) | u64::from(*byte));
    value >= 3 && value % 2 == 1
}

fn ec_coordinate_has_length(coordinate: &str, expected_bytes: usize) -> bool {
    Base64UrlUnpadded::decode_vec(coordinate)
        .is_ok_and(|coordinate| coordinate.len() == expected_bytes)
}

impl Jwk {
    /// Whether this JWK is allowed to verify a signature under its metadata.
    fn supports_signature_verification(&self) -> bool {
        self.kid
            .as_deref()
            .is_some_and(|kid| !kid.trim().is_empty())
            && matches!(self.kty.as_str(), "RSA" | "EC")
            && self.algorithm().is_ok()
            && self.has_usable_public_key_material()
            && self
                .key_use
                .as_deref()
                .is_none_or(|key_use| key_use == "sig")
            && self.key_ops.as_ref().is_none_or(|operations| {
                let unique_operations: HashSet<_> = operations.iter().collect();
                operations.iter().any(|op| op == "verify")
                    && unique_operations.len() == operations.len()
                    && (self.key_use.as_deref() != Some("sig")
                        || !operations.iter().any(|operation| {
                            matches!(
                                operation.as_str(),
                                "encrypt"
                                    | "decrypt"
                                    | "wrapKey"
                                    | "unwrapKey"
                                    | "deriveKey"
                                    | "deriveBits"
                            )
                        }))
            })
    }

    /// The signature algorithm this key is published for.
    ///
    /// Derived from the key type when the JWKS omits `alg`, which is allowed,
    /// rather than assumed to be RSA. EC algorithms are also bound to their
    /// required curve so a key cannot be selected under an incompatible alg.
    fn algorithm(&self) -> anyhow::Result<Algorithm> {
        match self.kty.as_str() {
            "RSA" => {
                anyhow::ensure!(
                    self.crv.is_none(),
                    "OAuth provider RSA key must not declare an EC curve"
                );
                match self.alg.as_deref() {
                    None | Some("RS256") => Ok(Algorithm::RS256),
                    Some("RS384") => Ok(Algorithm::RS384),
                    Some("RS512") => Ok(Algorithm::RS512),
                    Some(other) => {
                        anyhow::bail!("OAuth provider RSA key uses incompatible alg {other}")
                    }
                }
            }
            "EC" => match (self.crv.as_deref(), self.alg.as_deref()) {
                (Some("P-256"), None | Some("ES256")) => Ok(Algorithm::ES256),
                (Some("P-384"), None | Some("ES384")) => Ok(Algorithm::ES384),
                (Some(curve), Some(algorithm)) => anyhow::bail!(
                    "OAuth provider EC key curve {curve} is incompatible with alg {algorithm}"
                ),
                (Some(curve), None) => {
                    anyhow::bail!("OAuth provider EC key curve {curve} is unsupported")
                }
                (None, _) => anyhow::bail!("OAuth provider EC key is missing crv"),
            },
            other => anyhow::bail!("OAuth provider JWKS key type {other} is unsupported"),
        }
    }

    /// Validate the public key fields before this untrusted key can be selected.
    fn has_usable_public_key_material(&self) -> bool {
        match self.kty.as_str() {
            "RSA" => {
                let (Some(modulus), Some(exponent)) = (self.n.as_deref(), self.e.as_deref()) else {
                    return false;
                };
                usable_rsa_modulus(modulus) && usable_rsa_exponent(exponent)
            }
            "EC" => {
                let coordinate_bytes = match self.crv.as_deref() {
                    Some("P-256") => 32,
                    Some("P-384") => 48,
                    _ => return false,
                };
                self.x.as_deref().is_some_and(|coordinate| {
                    ec_coordinate_has_length(coordinate, coordinate_bytes)
                }) && self.y.as_deref().is_some_and(|coordinate| {
                    ec_coordinate_has_length(coordinate, coordinate_bytes)
                })
            }
            _ => false,
        }
    }

    /// Build the decoding key for this JWK.
    fn decoding_key(&self) -> anyhow::Result<DecodingKey> {
        match self.kty.as_str() {
            "RSA" => {
                let n = self
                    .n
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("OAuth provider RSA key is missing n"))?;
                let e = self
                    .e
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("OAuth provider RSA key is missing e"))?;
                Ok(DecodingKey::from_rsa_components(n, e)?)
            }
            "EC" => {
                let x = self
                    .x
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("OAuth provider EC key is missing x"))?;
                let y = self
                    .y
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("OAuth provider EC key is missing y"))?;
                Ok(DecodingKey::from_ec_components(x, y)?)
            }
            other => anyhow::bail!("OAuth provider JWKS key type {other} is unsupported"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        id_token_decoding_key, parse_jwks, MAX_OAUTH_PROVIDER_JWKS_KEYS,
        MAX_OAUTH_PROVIDER_RESPONSE_BYTES, MAX_OAUTH_PROVIDER_RSA_MODULUS_BITS,
        MIN_OAUTH_PROVIDER_RSA_MODULUS_BITS,
    };
    use base64ct::{Base64UrlUnpadded, Encoding};
    use jsonwebtoken::Algorithm;

    fn unsigned_test_token(algorithm: &str, kid: &str) -> String {
        let header = serde_json::json!({ "alg": algorithm, "kid": kid }).to_string();
        format!(
            "{}.e30.test-signature",
            Base64UrlUnpadded::encode_string(header.as_bytes())
        )
    }

    fn rsa_modulus(bits: usize) -> String {
        let byte_count = bits.div_ceil(8);
        let leading_bits = bits - (byte_count - 1) * 8;
        let mut bytes = vec![0; byte_count];
        bytes[0] = 1 << (leading_bits - 1);
        *bytes.last_mut().expect("non-empty modulus") = 1;
        Base64UrlUnpadded::encode_string(&bytes)
    }

    fn non_canonical_uint(value: &str) -> String {
        let mut bytes = vec![0];
        bytes.extend(Base64UrlUnpadded::decode_vec(value).expect("valid Base64urlUInt"));
        Base64UrlUnpadded::encode_string(&bytes)
    }

    fn ec_coordinate(byte_count: usize) -> String {
        Base64UrlUnpadded::encode_string(&vec![7; byte_count])
    }

    fn rsa_jwk_json(kid: &str, bits: usize, exponent: &[u8], alg: Option<&str>) -> String {
        let modulus = rsa_modulus(bits);
        let exponent = Base64UrlUnpadded::encode_string(exponent);
        let algorithm = alg.map_or_else(String::new, |alg| format!(r#""alg":"{alg}","#));
        format!(r#"{{"kid":"{kid}","kty":"RSA",{algorithm}"n":"{modulus}","e":"{exponent}"}}"#)
    }

    fn jwks_with_key_entries(entries: impl IntoIterator<Item = String>) -> String {
        format!(
            r#"{{"keys":[{}]}}"#,
            entries.into_iter().collect::<Vec<_>>().join(",")
        )
    }

    #[test]
    fn configured_jwks_accepts_the_limit_and_rejects_the_first_oversized_byte() {
        let document = jwks_with_key_entries([rsa_jwk_json("key", 2_048, &[1, 0, 1], None)]);
        let at_limit = format!(
            "{document}{}",
            " ".repeat(MAX_OAUTH_PROVIDER_RESPONSE_BYTES - document.len())
        );
        assert_eq!(at_limit.len(), MAX_OAUTH_PROVIDER_RESPONSE_BYTES);
        assert!(parse_jwks(&at_limit).is_ok());

        let over_limit = format!("{at_limit} ");
        assert_eq!(over_limit.len(), MAX_OAUTH_PROVIDER_RESPONSE_BYTES + 1);
        assert_eq!(
            parse_jwks(&over_limit).unwrap_err().to_string(),
            "OAuth provider JWKS exceeded the configured size limit"
        );
    }

    #[test]
    fn provider_jwks_accepts_sixteen_keys_and_rejects_the_seventeenth() {
        let make_jwks = |key_count: usize| {
            jwks_with_key_entries(
                (0..key_count)
                    .map(|index| rsa_jwk_json(&format!("key-{index}"), 2_048, &[1, 0, 1], None)),
            )
        };

        let at_limit = make_jwks(MAX_OAUTH_PROVIDER_JWKS_KEYS);
        assert!(parse_jwks(&at_limit).is_ok());

        let over_limit = make_jwks(MAX_OAUTH_PROVIDER_JWKS_KEYS + 1);
        assert_eq!(
            parse_jwks(&over_limit).unwrap_err().to_string(),
            "OAuth provider JWKS exceeded the configured key limit"
        );
    }

    #[test]
    fn provider_jwks_rejects_encryption_and_non_verification_keys() {
        let key = |metadata: &str| {
            format!(
                r#"{{"kid":"key","kty":"RSA",{metadata}"n":"{}","e":"AQAB"}}"#,
                rsa_modulus(2_048)
            )
        };
        let encryption_key = key(r#""use":"enc","alg":"RS256","#);
        let non_verification_key = key(r#""use":"sig","key_ops":["encrypt"],"alg":"RS256","#);
        let empty_operations = key(r#""use":"sig","key_ops":[],"alg":"RS256","#);
        let inconsistent_metadata =
            key(r#""use":"sig","key_ops":["verify","encrypt"],"alg":"RS256","#);
        let duplicate_operations =
            key(r#""use":"sig","key_ops":["verify","verify"],"alg":"RS256","#);

        for jwk in [
            encryption_key,
            non_verification_key,
            empty_operations,
            inconsistent_metadata,
            duplicate_operations,
        ] {
            let jwks = jwks_with_key_entries([jwk]);
            let error = parse_jwks(&jwks).expect_err("JWKS without a verification key must fail");
            assert_eq!(
                error.to_string(),
                "OAuth provider JWKS does not contain a usable signing key"
            );
        }

        let valid_key = rsa_jwk_json("key", 2_048, &[1, 0, 1], None);
        let key_without_use = valid_key.replace("\"n\"", "\"use\":null,\"n\"");
        let key_without_ops = valid_key.replace("\"n\"", "\"key_ops\":null,\"n\"");
        assert!(parse_jwks(&jwks_with_key_entries([key_without_use])).is_err());
        assert!(parse_jwks(&jwks_with_key_entries([key_without_ops])).is_err());
    }

    #[test]
    fn provider_jwks_enforces_rsa_strength_and_valid_public_exponents() {
        let accepted = [
            MIN_OAUTH_PROVIDER_RSA_MODULUS_BITS,
            MAX_OAUTH_PROVIDER_RSA_MODULUS_BITS,
        ];
        for bits in accepted {
            let jwks = jwks_with_key_entries([rsa_jwk_json("key", bits, &[1, 0, 1], None)]);
            assert!(
                parse_jwks(&jwks).is_ok(),
                "{bits}-bit modulus should be accepted"
            );
        }

        for bits in [
            MIN_OAUTH_PROVIDER_RSA_MODULUS_BITS - 1,
            MAX_OAUTH_PROVIDER_RSA_MODULUS_BITS + 1,
        ] {
            let jwks = jwks_with_key_entries([rsa_jwk_json("key", bits, &[1, 0, 1], None)]);
            assert!(
                parse_jwks(&jwks).is_err(),
                "{bits}-bit modulus should be rejected"
            );
        }

        for exponent in [
            &[1][..],
            &[2][..],
            &[0][..],
            &[1, 0, 1, 0, 1, 0, 1, 0, 1][..],
        ] {
            let jwks = jwks_with_key_entries([rsa_jwk_json("key", 2_048, exponent, None)]);
            assert!(
                parse_jwks(&jwks).is_err(),
                "invalid RSA exponent must be rejected"
            );
        }
    }

    #[test]
    fn provider_jwks_rejects_noncanonical_rsa_base64url_uints() {
        let modulus = rsa_modulus(2_048);
        let exponent = Base64UrlUnpadded::encode_string(&[1, 0, 1]);
        let cases = [
            (non_canonical_uint(&modulus), exponent.clone()),
            (modulus, non_canonical_uint(&exponent)),
        ];

        for (modulus, exponent) in cases {
            let jwk = format!(r#"{{"kid":"key","kty":"RSA","n":"{modulus}","e":"{exponent}"}}"#);
            assert!(
                parse_jwks(&jwks_with_key_entries([jwk])).is_err(),
                "RSA n and e must use canonical Base64urlUInt encodings"
            );
        }
    }

    #[test]
    fn provider_jwks_rejects_even_rsa_modulus() {
        let mut modulus = Base64UrlUnpadded::decode_vec(&rsa_modulus(2_048))
            .expect("generated modulus is Base64urlUInt");
        *modulus.last_mut().expect("non-empty modulus") &= !1;
        let modulus = Base64UrlUnpadded::encode_string(&modulus);
        let jwk = format!(r#"{{"kid":"key","kty":"RSA","n":"{modulus}","e":"AQAB"}}"#);

        assert!(parse_jwks(&jwks_with_key_entries([jwk])).is_err());
    }

    #[test]
    fn provider_jwks_binds_ec_algorithms_to_curve_and_coordinate_length() {
        let p256_x = ec_coordinate(32);
        let p256_y = ec_coordinate(32);
        let p256 = format!(
            r#"{{"kid":"ec-key","kty":"EC","crv":"P-256","alg":"ES256","x":"{p256_x}","y":"{p256_y}"}}"#
        );
        assert!(parse_jwks(&jwks_with_key_entries([p256.clone()])).is_ok());

        let p384_x = ec_coordinate(48);
        let p384_y = ec_coordinate(48);
        let p384 = format!(
            r#"{{"kid":"ec-key","kty":"EC","crv":"P-384","alg":"ES384","x":"{p384_x}","y":"{p384_y}"}}"#
        );
        assert!(parse_jwks(&jwks_with_key_entries([p384])).is_ok());

        let incompatible_algorithm = p256.replace("ES256", "ES384");
        let missing_curve = p256.replace("\"crv\":\"P-256\",", "");
        let wrong_coordinate_length = p256.replace(&p256_x, &ec_coordinate(31));
        for jwk in [
            incompatible_algorithm,
            missing_curve,
            wrong_coordinate_length,
        ] {
            assert!(parse_jwks(&jwks_with_key_entries([jwk])).is_err());
        }

        let wrong_key_family = rsa_jwk_json("rsa-key", 2_048, &[1, 0, 1], Some("ES256"));
        assert!(parse_jwks(&jwks_with_key_entries([wrong_key_family])).is_err());
    }

    #[test]
    fn id_token_key_selection_uses_signature_metadata_and_token_algorithm() {
        let token = unsigned_test_token("RS256", "shared-key-id");
        let ec_x = ec_coordinate(32);
        let ec_y = ec_coordinate(32);
        let encryption_key =
            r#"{"kid":"shared-key-id","kty":"RSA","use":"enc","alg":"RS256"}"#.to_owned();
        let ec_key = format!(
            r#"{{"kid":"shared-key-id","kty":"EC","crv":"P-256","use":"sig","key_ops":["verify"],"alg":"ES256","x":"{ec_x}","y":"{ec_y}"}}"#
        );
        let rsa = rsa_jwk_json("shared-key-id", 2_048, &[1, 0, 1], Some("RS256"));
        let jwks = jwks_with_key_entries([encryption_key, ec_key, rsa]);

        let (_, algorithm) = id_token_decoding_key(&token, &jwks)
            .expect("the unique RSA signature key should be selected");
        assert_eq!(algorithm, Algorithm::RS256);
    }

    #[test]
    fn id_token_key_selection_rejects_ambiguous_signing_keys() {
        let token = unsigned_test_token("RS256", "duplicate-key-id");
        let first = rsa_jwk_json("duplicate-key-id", 2_048, &[1, 0, 1], Some("RS256"));
        let second = rsa_jwk_json("duplicate-key-id", 2_049, &[1, 0, 1], Some("RS256"));
        let jwks = jwks_with_key_entries([first, second]);

        let error = match id_token_decoding_key(&token, &jwks) {
            Ok(_) => panic!("a duplicate matching signing key must be ambiguous"),
            Err(error) => error,
        };
        assert_eq!(
            error.to_string(),
            "OAuth provider ID token kid has ambiguous matching signing keys"
        );
    }
}
