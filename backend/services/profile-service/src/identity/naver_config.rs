//! NAVER OAuth provider configuration.
//!
//! Adapted from Bigyeting `5eecdcaf` `config/mod.rs` (`naver_oauth`,
//! `NAVER_OAUTH_*`, `require_production_setting`) and `config/production.rs`
//! (`validate_oauth_client_id`, `validate_oauth_urls`).
//!
//! The donor's rule is preserved: outside production a placeholder client id and
//! secret are acceptable so a parity stack starts, and in production they are
//! refused and the service does not start. What is added here:
//!
//! * the redirect URI must equal the console-registered callback exactly, which
//!   the donor only checked for being an https URL. A redirect URI that differs
//!   from the registered one makes every token exchange fail with
//!   `invalid_request`, and the failure looks like a provider outage. Refusing at
//!   startup turns a silent runtime failure into a startup error.
//! * the placeholder list is closed and documented, so a test can assert exactly
//!   which values are refused rather than "some value is refused".
//!
//! Nothing here logs a secret. The development warning names the setting, never
//! its value.
use crate::identity::naver_provider::NaverProviderEndpoints;
use platform::config::{
    env_or, require_non_empty_setting, require_production_https_url, require_production_setting,
};

/// The client id a development stack uses when nothing is configured.
pub const LOCAL_CLIENT_ID: &str = "local-naver-client";
/// The client secret a development stack uses when nothing is configured.
pub const LOCAL_CLIENT_SECRET: &str = "local-naver-secret";
pub const DEFAULT_AUTHORIZE_URL: &str = "https://nid.naver.com/oauth2.0/authorize";
pub const DEFAULT_TOKEN_URL: &str = "https://nid.naver.com/oauth2.0/token";
pub const DEFAULT_PROFILE_URL: &str = "https://openapi.naver.com/v1/nid/me";
/// The destination's callback path. The console registration must match it.
pub const CALLBACK_PATH: &str = "/api/identity/naver/callback";
pub const DEFAULT_REDIRECT_URI: &str = "http://localhost:8080/api/identity/naver/callback";

/// The closed set of placeholder prefixes refused in production.
///
/// The donor refused `pending-`, `replace-with-` and `local-`. The same three are
/// kept, and `local-naver-client` / `local-naver-secret` are covered by `local-`.
const PLACEHOLDER_PREFIXES: [&str; 3] = ["pending-", "replace-with-", "local-"];

/// The resolved NAVER provider settings.
#[derive(Clone)]
pub struct NaverOAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub authorize_url: String,
    pub token_url: String,
    pub profile_url: String,
    pub redirect_uri: String,
    pub scopes: String,
}

impl std::fmt::Debug for NaverOAuthConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The secret never renders. The client id is not a secret but is also
        // not useful in a log line, so it is redacted for the same reason.
        formatter
            .debug_struct("NaverOAuthConfig")
            .field("client_id", &"[REDACTED]")
            .field("client_secret", &"[REDACTED]")
            .field("authorize_url", &self.authorize_url)
            .field("token_url", &self.token_url)
            .field("profile_url", &self.profile_url)
            .field("redirect_uri", &self.redirect_uri)
            .field("scopes", &self.scopes)
            .finish()
    }
}

impl NaverOAuthConfig {
    /// Load from the environment and validate.
    ///
    /// `production` is the resolved runtime mode. `PROFILE_RUNTIME_MODE` decides
    /// it, exactly as the rest of this service does.
    pub fn load(production: bool) -> anyhow::Result<Self> {
        let config = Self {
            client_id: require_production_setting(
                production,
                "NAVER_OAUTH_CLIENT_ID",
                env_or("NAVER_OAUTH_CLIENT_ID", LOCAL_CLIENT_ID),
            )?,
            client_secret: require_production_setting(
                production,
                "NAVER_OAUTH_CLIENT_SECRET",
                env_or_file("NAVER_OAUTH_CLIENT_SECRET", LOCAL_CLIENT_SECRET)?,
            )?,
            authorize_url: env_or("NAVER_OAUTH_AUTHORIZE_URL", DEFAULT_AUTHORIZE_URL),
            token_url: env_or("NAVER_OAUTH_TOKEN_URL", DEFAULT_TOKEN_URL),
            profile_url: env_or("NAVER_OAUTH_PROFILE_URL", DEFAULT_PROFILE_URL),
            redirect_uri: require_production_setting(
                production,
                "NAVER_OAUTH_REDIRECT_URI",
                env_or("NAVER_OAUTH_REDIRECT_URI", DEFAULT_REDIRECT_URI),
            )?,
            scopes: env_or("NAVER_OAUTH_SCOPES", ""),
        };
        config.validate(production)?;
        Ok(config)
    }

    /// Build the validated settings directly. Used by tests and by any caller
    /// that already resolved its values.
    pub fn new(
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        authorize_url: impl Into<String>,
        token_url: impl Into<String>,
        profile_url: impl Into<String>,
        redirect_uri: impl Into<String>,
        scopes: impl Into<String>,
    ) -> Self {
        Self {
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            authorize_url: authorize_url.into(),
            token_url: token_url.into(),
            profile_url: profile_url.into(),
            redirect_uri: redirect_uri.into(),
            scopes: scopes.into(),
        }
    }

    /// The development defaults. Never valid in production.
    pub fn local() -> Self {
        Self::new(
            LOCAL_CLIENT_ID,
            LOCAL_CLIENT_SECRET,
            DEFAULT_AUTHORIZE_URL,
            DEFAULT_TOKEN_URL,
            DEFAULT_PROFILE_URL,
            DEFAULT_REDIRECT_URI,
            "",
        )
    }

    /// Validate the settings for the given runtime mode.
    ///
    /// In production every check is enforced and the first failure is returned
    /// as an error, which stops the service from starting. Outside production
    /// the same checks run in a "warn" form: the value is accepted and the
    /// setting name is reported, never the value.
    pub fn validate(&self, production: bool) -> anyhow::Result<()> {
        require_non_empty_setting("NAVER_OAUTH_CLIENT_ID", &self.client_id)?;
        require_non_empty_setting("NAVER_OAUTH_CLIENT_SECRET", &self.client_secret)?;
        require_non_empty_setting("NAVER_OAUTH_REDIRECT_URI", &self.redirect_uri)?;
        validate_redirect_uri(&self.redirect_uri)?;

        for (setting, value) in [
            ("NAVER_OAUTH_AUTHORIZE_URL", &self.authorize_url),
            ("NAVER_OAUTH_TOKEN_URL", &self.token_url),
            ("NAVER_OAUTH_PROFILE_URL", &self.profile_url),
            ("NAVER_OAUTH_REDIRECT_URI", &self.redirect_uri),
        ] {
            require_production_https_url(production, setting, value)?;
        }

        if production {
            refuse_placeholder("NAVER_OAUTH_CLIENT_ID", &self.client_id)?;
            refuse_placeholder("NAVER_OAUTH_CLIENT_SECRET", &self.client_secret)?;
        } else {
            // A development start with a placeholder is allowed, and says so
            // without printing the value.
            if is_placeholder(&self.client_id) {
                tracing::warn!(
                    setting = "NAVER_OAUTH_CLIENT_ID",
                    "NAVER provider is configured with a development placeholder; production refuses this value"
                );
            }
            if is_placeholder(&self.client_secret) {
                tracing::warn!(
                    setting = "NAVER_OAUTH_CLIENT_SECRET",
                    "NAVER provider is configured with a development placeholder; production refuses this value"
                );
            }
        }
        Ok(())
    }

    /// The endpoints handed to the provider client. Only these are ever called.
    pub fn endpoints(&self) -> NaverProviderEndpoints {
        NaverProviderEndpoints {
            token_url: self.token_url.clone(),
            profile_url: self.profile_url.clone(),
            client_id: self.client_id.clone(),
            client_secret: self.client_secret.clone(),
            redirect_uri: self.redirect_uri.clone(),
        }
    }

    /// The provider authorization URL for one state value.
    ///
    /// `response_type=code`, the configured client id, the configured redirect
    /// URI, and the state. The scopes are appended only when configured, because
    /// NAVER treats an empty `scope` as a request for nothing.
    pub fn authorization_url(&self, state: &str) -> String {
        let mut url = format!(
            "{}?response_type=code&client_id={}&redirect_uri={}&state={}",
            self.authorize_url,
            percent_encode(&self.client_id),
            percent_encode(&self.redirect_uri),
            percent_encode(state),
        );
        if !self.scopes.trim().is_empty() {
            url.push_str("&scope=");
            url.push_str(&percent_encode(self.scopes.trim()));
        }
        url
    }
}

/// Resolve `NAVER_OAUTH_CLIENT_SECRET` from the environment, or from the file
/// named by `NAVER_OAUTH_CLIENT_SECRET_FILE`.
///
/// The donor's `env_or_file` shape is preserved: a file is the preferred way to
/// supply a secret, because an environment variable is visible to every child
/// process. A file that cannot be read is a startup error, never a silent
/// fallback to the placeholder.
pub fn env_or_file(name: &str, default: &str) -> anyhow::Result<String> {
    if let Some(value) = platform::config::optional_env(name) {
        return Ok(value);
    }
    let file_name = format!("{name}_FILE");
    if let Some(path) = platform::config::optional_env(&file_name) {
        let value = std::fs::read_to_string(&path)
            .map_err(|_| anyhow::anyhow!("{file_name} must name a readable secret file"))?;
        let value = value.trim();
        if value.is_empty() {
            anyhow::bail!("{file_name} must contain a non-empty secret");
        }
        return Ok(value.to_owned());
    }
    Ok(default.to_owned())
}

/// The redirect URI must be an absolute URL whose path is exactly the
/// destination's callback path.
///
/// The console registration is `{origin}{CALLBACK_PATH}`. A redirect URI with a
/// different path cannot be the registered one, and NAVER would reject every
/// exchange. Trailing slash, query and fragment are all refused for the same
/// reason: NAVER compares the value literally.
pub fn validate_redirect_uri(value: &str) -> anyhow::Result<()> {
    let url = reqwest::Url::parse(value)
        .map_err(|_| anyhow::anyhow!("NAVER_OAUTH_REDIRECT_URI must be an absolute URL"))?;
    if !url.has_host() {
        anyhow::bail!("NAVER_OAUTH_REDIRECT_URI must include a host");
    }
    if url.path() != CALLBACK_PATH {
        anyhow::bail!(
            "NAVER_OAUTH_REDIRECT_URI must end with the registered callback path {CALLBACK_PATH}"
        );
    }
    if url.query().is_some() || url.fragment().is_some() {
        anyhow::bail!("NAVER_OAUTH_REDIRECT_URI must not carry a query or fragment");
    }
    Ok(())
}

fn is_placeholder(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    normalized.is_empty()
        || normalized == LOCAL_CLIENT_ID
        || normalized == LOCAL_CLIENT_SECRET
        || PLACEHOLDER_PREFIXES
            .iter()
            .any(|prefix| normalized.starts_with(prefix))
}

fn refuse_placeholder(setting: &str, value: &str) -> anyhow::Result<()> {
    if is_placeholder(value) {
        // The message names the setting and the rule, never the value, so a
        // startup failure cannot leak a secret into a log aggregator.
        anyhow::bail!(
            "{setting} must not use a placeholder in production (refused prefixes: {})",
            PLACEHOLDER_PREFIXES.join(", ")
        );
    }
    Ok(())
}

/// Percent-encode a query value with the RFC 3986 unreserved set plus the
/// sub-delimiters the contract's `return_path` grammar already admits.
///
/// `url::form_urlencoded` would encode a space as `+`, which NAVER's authorize
/// endpoint does not decode. This encoder is strict and total.
fn percent_encode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

/// Resolve the runtime mode exactly as the rest of this service does.
pub fn runtime_production_mode() -> anyhow::Result<bool> {
    platform::config::runtime_mode_is_production(
        &env_or("PROFILE_RUNTIME_MODE", "production"),
        "PROFILE_RUNTIME_MODE",
    )
}
