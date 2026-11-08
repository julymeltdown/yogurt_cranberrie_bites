//! The NAVER OAuth provider client.
//!
//! Adapted from Bigyeting `5eecdcaf` `adapters/out/oauth_naver_provider.rs`
//! (`fetch_naver_profile`, `load_naver_profile`, `gender_bucket`),
//! `adapters/out/oauth_token_exchange.rs` (`exchange_naver_code`) and
//! `adapters/out/oauth_provider_response.rs` (`decode_oauth_response`,
//! `OAuthProviderError`).
//!
//! What is preserved:
//!
//! * the token exchange is a form POST to the configured token URL with
//!   `grant_type=authorization_code`, `code`, `redirect_uri`, `state`,
//!   `client_id` and `client_secret` (NAVER's documented shape);
//! * the profile fetch is a bearer-authenticated GET of the configured profile
//!   URL;
//! * provider failures are classified into a small closed set of kinds, and the
//!   classification never carries the response body, the code or the token.
//!
//! What changed:
//!
//! * the donor's `OAuthProviderError` carried `reqwest::Error` as a source. That
//!   error renders the request URL and, for a `Request` error, can render the
//!   request itself. Here the client error is reduced to a kind at the point it
//!   is observed, so no `reqwest` value is ever retained by a typed failure.
//! * the access token is held in [`NaverAccessToken`], which has no derived
//!   `Debug` and no `Display`. The only way to obtain the token bytes is
//!   [`NaverAccessToken::expose`], whose name makes a leak visible in review.
//! * only the configured endpoint is used. There is no caller-supplied URL
//!   anywhere in this module's public surface.
use crate::identity::naver_profile::{
    decode_profile, NaverGender, NaverProfileDecodeError, MAX_PROFILE_BODY_BYTES,
};
use serde::Deserialize;
use std::time::Duration;

/// Largest token response body this client will read.
///
/// A NAVER token response is a few hundred bytes. The donor's error-body bound
/// was 8 KiB; the same ceiling is used here for both success and failure bodies,
/// so a provider that streams an unbounded body cannot exhaust this service.
pub const MAX_TOKEN_BODY_BYTES: usize = 8 * 1024;

/// One provider request's ceiling. The callback runs inside the Gateway's
/// request deadline; this is the client's own bound so a hung provider cannot
/// hold a profile-service worker until the outer deadline fires.
pub const PROVIDER_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// A provider access token.
///
/// Deliberately not `Debug`, not `Display`, not `Clone`-by-render: nothing in
/// this service formats it. `expose` is the single, greppable accessor.
pub struct NaverAccessToken(String);

impl NaverAccessToken {
    pub fn new(value: String) -> Self {
        Self(value)
    }

    /// The bearer value. Call this only where the token is immediately placed in
    /// an authorization header; never pass the result to a formatter.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for NaverAccessToken {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("NaverAccessToken([REDACTED])")
    }
}

/// The provider profile reduced to exactly what the claim needs.
///
/// The donor called this `NaverProviderProfile`. `gender` keeps the donor's
/// `Option<GenderBucket>` shape: `None` means the provider gave no usable value,
/// and the callback must fail with `profile_incomplete`.
///
/// `Debug` is hand written and redacted: the birth date and the subject are
/// exactly the values V-12-o forbids from reaching a log.
pub struct NaverProviderProfile {
    pub subject: Option<String>,
    pub gender: Option<NaverGender>,
    pub birthyear: Option<String>,
    pub birthday: Option<String>,
}

impl std::fmt::Debug for NaverProviderProfile {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NaverProviderProfile")
            .field("subject", &self.subject.as_ref().map(|_| "[REDACTED]"))
            .field("gender", &self.gender)
            .field("birthyear", &self.birthyear.as_ref().map(|_| "[REDACTED]"))
            .field("birthday", &self.birthday.as_ref().map(|_| "[REDACTED]"))
            .finish()
    }
}

/// Which provider step failed. Carried by [`NaverProviderError`] so an operator
/// can tell a token problem from a profile problem without any provider content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NaverProviderStage {
    TokenExchange,
    ProfileFetch,
}

impl NaverProviderStage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TokenExchange => "token_exchange",
            Self::ProfileFetch => "profile_fetch",
        }
    }
}

/// How a provider request failed. Closed set; never carries provider content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NaverProviderFailure {
    /// The request could not be completed: connection, TLS, or timeout. The
    /// distinction between those is deliberately not retained, because a
    /// `reqwest::Error` renders the URL it was built from.
    Transport,
    /// The provider answered with a non-success HTTP status.
    Rejected { status: u16 },
    /// The provider answered successfully but the body was not the documented
    /// shape, or exceeded the size bound.
    InvalidResponse,
    /// The provider answered successfully but the token response carried no
    /// access token.
    MissingAccessToken,
}

/// A typed provider failure with its stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NaverProviderError {
    pub stage: NaverProviderStage,
    pub failure: NaverProviderFailure,
}

impl NaverProviderError {
    pub const fn new(stage: NaverProviderStage, failure: NaverProviderFailure) -> Self {
        Self { stage, failure }
    }

    /// A stable, non-sensitive classification string for metrics and logs. It
    /// contains no token, subject, birth date, code or provider body.
    pub fn classification(&self) -> &'static str {
        match self.failure {
            NaverProviderFailure::Transport => "transport",
            NaverProviderFailure::Rejected { .. } => "rejected",
            NaverProviderFailure::InvalidResponse => "invalid_response",
            NaverProviderFailure::MissingAccessToken => "missing_access_token",
        }
    }
}

impl std::fmt::Display for NaverProviderError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "NAVER {} failed ({})",
            self.stage.as_str(),
            self.classification()
        )
    }
}

impl std::error::Error for NaverProviderError {}

/// The exact provider endpoints this client may call.
///
/// The values come from configuration only. There is no setter, no per-call URL
/// argument, and no redirect following, so a provider response cannot move the
/// client to another origin.
#[derive(Clone, Debug)]
pub struct NaverProviderEndpoints {
    pub token_url: String,
    pub profile_url: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
}

/// The NAVER provider client.
///
/// One `reqwest::Client` is shared, with redirects disabled and a fixed timeout.
/// Every request body is read through a bounded reader, so neither a success nor
/// an error body can exceed its ceiling.
///
/// `Clone` shares the same underlying connection pool, so the runtime can hand
/// each use case a copy without opening a second pool.
#[derive(Clone)]
pub struct NaverProviderClient {
    endpoints: NaverProviderEndpoints,
    http: reqwest::Client,
}

impl NaverProviderClient {
    pub fn new(endpoints: NaverProviderEndpoints) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(PROVIDER_REQUEST_TIMEOUT)
            .connect_timeout(PROVIDER_REQUEST_TIMEOUT)
            // A provider redirect is never followed: the configured endpoint is
            // the only destination, and following would let a compromised or
            // misconfigured provider move the bearer token to another origin.
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self { endpoints, http })
    }

    /// Exchange an authorization code for an access token.
    ///
    /// The `state` is sent because NAVER's token endpoint documents it as part
    /// of the exchange; the value has already been consumed and validated by
    /// the caller, so this is a protocol requirement, not a second check.
    pub async fn exchange_authorization_code(
        &self,
        code: &str,
        state: &str,
    ) -> Result<NaverAccessToken, NaverProviderError> {
        let stage = NaverProviderStage::TokenExchange;
        let form = [
            ("grant_type", "authorization_code"),
            ("code", code),
            ("state", state),
            ("redirect_uri", self.endpoints.redirect_uri.as_str()),
            ("client_id", self.endpoints.client_id.as_str()),
            ("client_secret", self.endpoints.client_secret.as_str()),
        ];
        let response = self
            .http
            .post(&self.endpoints.token_url)
            .form(&form)
            .send()
            .await
            .map_err(|_| NaverProviderError::new(stage, NaverProviderFailure::Transport))?;
        let status = response.status();
        let body = read_bounded(response, MAX_TOKEN_BODY_BYTES)
            .await
            .map_err(|failure| NaverProviderError::new(stage, failure))?;
        if !status.is_success() {
            return Err(NaverProviderError::new(
                stage,
                NaverProviderFailure::Rejected {
                    status: status.as_u16(),
                },
            ));
        }
        let token = serde_json::from_slice::<TokenResponse>(&body)
            .map_err(|_| NaverProviderError::new(stage, NaverProviderFailure::InvalidResponse))?;
        token
            .access_token
            .filter(|value| !value.trim().is_empty())
            .map(NaverAccessToken::new)
            .ok_or_else(|| {
                NaverProviderError::new(stage, NaverProviderFailure::MissingAccessToken)
            })
    }

    /// Fetch the provider profile and reduce it to the claim inputs.
    pub async fn load_profile(
        &self,
        token: &NaverAccessToken,
    ) -> Result<NaverProviderProfile, NaverProviderError> {
        let stage = NaverProviderStage::ProfileFetch;
        let response = self
            .http
            .get(&self.endpoints.profile_url)
            .bearer_auth(token.expose())
            .send()
            .await
            .map_err(|_| NaverProviderError::new(stage, NaverProviderFailure::Transport))?;
        let status = response.status();
        // A body above the bound is the provider answering outside its
        // contract, not a transport problem: the connection succeeded and the
        // provider chose to send something this service will not parse.
        let body = read_bounded(response, MAX_PROFILE_BODY_BYTES)
            .await
            .map_err(|failure| NaverProviderError::new(stage, failure))?;
        if !status.is_success() {
            return Err(NaverProviderError::new(
                stage,
                NaverProviderFailure::Rejected {
                    status: status.as_u16(),
                },
            ));
        }
        let profile = decode_profile(&body).map_err(|error| {
            NaverProviderError::new(
                stage,
                match error {
                    NaverProfileDecodeError::TooLarge
                    | NaverProfileDecodeError::Malformed => NaverProviderFailure::InvalidResponse,
                },
            )
        })?;
        Ok(NaverProviderProfile {
            subject: profile.subject().map(str::to_owned),
            gender: profile.normalized_gender(),
            birthyear: profile.birthyear,
            birthday: profile.birthday,
        })
    }
}

/// Read a response body up to `limit` bytes.
///
/// The declared `Content-Length` is checked first, then the stream is read
/// chunk by chunk, so a provider that understates or omits the length still
/// cannot exceed the ceiling.
async fn read_bounded(
    mut response: reqwest::Response,
    limit: usize,
) -> Result<Vec<u8>, NaverProviderFailure> {
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err(NaverProviderFailure::InvalidResponse);
    }
    let capacity = response
        .content_length()
        .and_then(|length| usize::try_from(length).ok())
        .unwrap_or_default()
        .min(limit);
    let mut body = Vec::with_capacity(capacity);
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| NaverProviderFailure::Transport)?
    {
        if chunk.len() > limit.saturating_sub(body.len()) {
            return Err(NaverProviderFailure::InvalidResponse);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}
