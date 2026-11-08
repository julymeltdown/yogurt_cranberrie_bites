//! T-203: provider configuration, including the production refusal.
//!
//! The donor's rule is that a production start with a placeholder client id or
//! secret refuses to start. These cases assert that rule exactly, plus the
//! redirect-URI rule the donor only half-checked: the configured redirect URI
//! must be the console-registered callback, because a mismatch makes every token
//! exchange fail in a way that looks like a provider outage.
//!
//! The environment is process-global, so the two `load` cases serialize on a
//! mutex and restore every variable they touch.
use profile_service::identity::naver_config::{
    validate_redirect_uri, NaverOAuthConfig, CALLBACK_PATH, DEFAULT_REDIRECT_URI,
    LOCAL_CLIENT_ID, LOCAL_CLIENT_SECRET,
};
use std::sync::Mutex;

static ENVIRONMENT: Mutex<()> = Mutex::new(());

/// Point the service's internal-TLS variables at generated certificates.
///
/// `AppConfig::load` validates internal gRPC TLS before the provider settings in
/// production, so a test about the provider must supply real material or it
/// would fail for the wrong reason.
fn apply_tls() -> Vec<String> {
    let names = test_support::tls::apply_env("PROFILE", "profile").expect("test TLS material");
    names.to_vec()
}

/// Standard Base64 of 32 bytes of `0x07`. A fixed value, not a secret: it only
/// has to be a well-formed key that is not the local parity key.
const STANDARD_BASE64_SEVEN: &str = "BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc=";

/// Supply the caller credentials a production start requires, so the only
/// remaining reason to refuse is the one a case is about.
fn apply_callers() -> Vec<&'static str> {
    let names = [
        "GATEWAY_SERVICE_CREDENTIAL",
        "POST_SERVICE_CREDENTIAL",
        "FRIEND_SERVICE_CREDENTIAL",
        "AUTH_SERVICE_CREDENTIAL",
        "CONCERT_SERVICE_CREDENTIAL",
    ];
    // SAFETY: the caller holds `ENVIRONMENT`.
    unsafe {
        for name in names {
            std::env::set_var(name, "production-caller-credential-for-test");
        }
        std::env::set_var(
            "PROFILE_RETENTION_PURGE_ACTOR_ID",
            "99999999-9999-9999-9999-999999999999",
        );
        // A real verifying key: the local parity key is refused in production.
        std::env::set_var("ACTOR_DELEGATION_VERIFYING_KEYS", STANDARD_BASE64_SEVEN);
    }
    names.to_vec()
}

fn clear_callers(names: &[&'static str]) {
    for name in names {
        // SAFETY: the caller holds `ENVIRONMENT`.
        unsafe { std::env::remove_var(name) };
    }
    // SAFETY: the caller holds `ENVIRONMENT`.
    unsafe {
        std::env::remove_var("PROFILE_RETENTION_PURGE_ACTOR_ID");
        std::env::remove_var("ACTOR_DELEGATION_VERIFYING_KEYS");
    }
}

fn clear_tls(names: &[String]) {
    for name in names {
        // SAFETY: the caller holds `ENVIRONMENT`.
        unsafe { std::env::remove_var(name) };
    }
}

fn config(client_id: &str, client_secret: &str, redirect_uri: &str) -> NaverOAuthConfig {
    NaverOAuthConfig::new(
        client_id,
        client_secret,
        "https://nid.naver.com/oauth2.0/authorize",
        "https://nid.naver.com/oauth2.0/token",
        "https://openapi.naver.com/v1/nid/me",
        redirect_uri,
        "",
    )
}

fn production(redirect_uri: &str) -> NaverOAuthConfig {
    config("real-client-id", "real-client-secret", redirect_uri)
}

/// A production callback. The host is a real registrable domain because the
/// platform refuses `.example`, `.test`, `.invalid` and loopback hosts in
/// production; the value is a fixture, never a live endpoint.
const PRODUCTION_REDIRECT: &str = "https://mcr-julymeltdown.example.org/api/identity/naver/callback";

/// V-12-p: a production start with the development placeholder refuses.
#[test]
fn a_production_start_with_the_local_client_id_refuses() {
    let error = config(LOCAL_CLIENT_ID, "real-secret", PRODUCTION_REDIRECT)
        .validate(true)
        .expect_err("a placeholder client id must refuse in production");
    let message = error.to_string();
    assert!(
        message.contains("NAVER_OAUTH_CLIENT_ID"),
        "the error must name the setting: {message}"
    );
    assert!(
        message.contains("placeholder"),
        "the error must say why: {message}"
    );
    // The message names the setting and the rule, never the value.
    assert!(!message.contains(LOCAL_CLIENT_ID), "{message}");
}

#[test]
fn a_production_start_with_the_local_client_secret_refuses() {
    let error = config("real-client-id", LOCAL_CLIENT_SECRET, PRODUCTION_REDIRECT)
        .validate(true)
        .expect_err("a placeholder client secret must refuse in production");
    let message = error.to_string();
    assert!(message.contains("NAVER_OAUTH_CLIENT_SECRET"), "{message}");
    assert!(!message.contains(LOCAL_CLIENT_SECRET), "{message}");
}

/// Every placeholder prefix the donor refused is still refused.
#[test]
fn every_donor_placeholder_prefix_is_refused_in_production() {
    for value in [
        "pending-naver-client-id",
        "replace-with-real-client",
        "local-naver-client",
        "LOCAL-NAVER-CLIENT",
        "  local-naver-client  ",
    ] {
        assert!(
            config(value, "real-secret", PRODUCTION_REDIRECT)
                .validate(true)
                .is_err(),
            "{value:?} must be refused in production"
        );
    }
}

/// A production start with real values succeeds.
#[test]
fn a_production_start_with_real_values_succeeds() {
    production(PRODUCTION_REDIRECT)
        .validate(true)
        .expect("real values must be accepted in production");
}

/// A development start with placeholders succeeds.
#[test]
fn a_development_start_with_placeholders_succeeds() {
    NaverOAuthConfig::local()
        .validate(false)
        .expect("a parity stack must start with placeholders");
}

/// A redirect URI that is not the registered callback refuses, in every mode.
///
/// This is the check the donor only applied as an https check. A redirect URI
/// with a different path cannot be the console registration, and NAVER would
/// reject every exchange with `invalid_request`.
#[test]
fn a_redirect_uri_that_is_not_the_registered_callback_refuses() {
    for value in [
        "https://mcr-julymeltdown.example.org/naver-callback",
        "https://mcr-julymeltdown.example.org/api/identity/naver/callback/",
        "https://mcr-julymeltdown.example.org/api/identity/naver/callback?next=/x",
        "https://mcr-julymeltdown.example.org/api/identity/naver/callback#fragment",
        "https://mcr-julymeltdown.example.org/api/identity/google/callback",
        "https://mcr-julymeltdown.example.org/",
        "not a url",
    ] {
        let error = production(value)
            .validate(true)
            .expect_err("a redirect URI must be the registered callback");
        assert!(
            error.to_string().contains("NAVER_OAUTH_REDIRECT_URI"),
            "{value:?}: {error}"
        );
        // The same rule applies outside production: a parity stack that
        // configures a wrong callback has the same broken flow.
        assert!(
            config(LOCAL_CLIENT_ID, LOCAL_CLIENT_SECRET, value)
                .validate(false)
                .is_err(),
            "{value:?} must also be refused outside production"
        );
    }
    // The default development redirect URI is the registered path.
    assert!(validate_redirect_uri(DEFAULT_REDIRECT_URI).is_ok());
    assert!(DEFAULT_REDIRECT_URI.ends_with(CALLBACK_PATH));
}

/// A production start requires https for every provider endpoint.
#[test]
fn a_production_start_requires_https_endpoints() {
    for (name, value) in [
        ("authorize", "http://nid.naver.com/oauth2.0/authorize"),
        ("token", "http://nid.naver.com/oauth2.0/token"),
        ("profile", "http://openapi.naver.com/v1/nid/me"),
    ] {
        let mut settings = production(PRODUCTION_REDIRECT);
        match name {
            "authorize" => settings.authorize_url = value.into(),
            "token" => settings.token_url = value.into(),
            _ => settings.profile_url = value.into(),
        }
        assert!(
            settings.validate(true).is_err(),
            "{name} must require https in production"
        );
        // Outside production the same URL is accepted, so a local fake provider
        // can be used without weakening the production rule.
        assert!(
            settings.validate(false).is_ok(),
            "{name} must be allowed outside production"
        );
    }
}

/// An empty client id or secret refuses in both modes.
#[test]
fn an_empty_credential_refuses_in_both_modes() {
    for production_mode in [true, false] {
        assert!(
            config("", "real-secret", PRODUCTION_REDIRECT)
                .validate(production_mode)
                .is_err(),
            "an empty client id must refuse (production={production_mode})"
        );
        assert!(
            config("real-client-id", "", PRODUCTION_REDIRECT)
                .validate(production_mode)
                .is_err(),
            "an empty client secret must refuse (production={production_mode})"
        );
    }
}

/// The authorization URL carries exactly the configured values.
#[test]
fn the_authorization_url_carries_the_configured_redirect_uri_exactly() {
    let settings = production(PRODUCTION_REDIRECT);
    let url = settings.authorization_url("state-token-1");
    assert!(url.starts_with("https://nid.naver.com/oauth2.0/authorize?"), "{url}");
    assert!(url.contains("response_type=code"), "{url}");
    assert!(url.contains("client_id=real-client-id"), "{url}");
    assert!(
        url.contains(
            "redirect_uri=https%3A%2F%2Fmcr-julymeltdown.example.org%2Fapi%2Fidentity%2Fnaver%2Fcallback"
        ),
        "the redirect URI must be the configured value, percent-encoded: {url}"
    );
    assert!(url.contains("state=state-token-1"), "{url}");
    // No scope is requested when none is configured.
    assert!(!url.contains("scope="), "{url}");
    // The secret is never in an authorization URL: that URL goes to the browser.
    assert!(!url.contains("real-client-secret"), "{url}");
}

#[test]
fn configured_scopes_are_appended_only_when_present() {
    let mut settings = production(PRODUCTION_REDIRECT);
    settings.scopes = "  profile  ".into();
    assert!(
        settings.authorization_url("s").contains("&scope=profile"),
        "{}",
        settings.authorization_url("s")
    );
}

/// A state value is percent-encoded, so a token cannot break the query grammar.
#[test]
fn a_state_value_is_percent_encoded() {
    let url = production(PRODUCTION_REDIRECT).authorization_url("a b&c=d/e");
    assert!(url.contains("state=a%20b%26c%3Dd%2Fe"), "{url}");
}

/// The configuration's `Debug` renders no credential.
#[test]
fn the_configuration_renders_no_credential_in_debug() {
    let rendered = format!("{:?}", production(PRODUCTION_REDIRECT));
    assert!(!rendered.contains("real-client-id"), "{rendered}");
    assert!(!rendered.contains("real-client-secret"), "{rendered}");
    // The endpoints are not secrets and are useful in a startup log.
    assert!(rendered.contains("openapi.naver.com"), "{rendered}");
}

/// A production start with placeholders refuses to start, end to end.
///
/// This is the V-12-p case as the operator experiences it: `AppConfig::load`
/// returns an error, and the binary exits non-zero before binding a listener.
///
/// `AppConfig::load` validates the whole production configuration, and internal
/// gRPC TLS is checked before the provider settings. The TLS material is
/// therefore supplied here so the only remaining reason to refuse is the
/// placeholder credential this case is about.
#[test]
fn a_production_start_with_placeholders_refuses_to_start() {
    let _guard = ENVIRONMENT.lock().unwrap_or_else(|poison| poison.into_inner());
    let previous_mode = std::env::var("PROFILE_RUNTIME_MODE").ok();
    let previous_id = std::env::var("NAVER_OAUTH_CLIENT_ID").ok();
    let previous_secret = std::env::var("NAVER_OAUTH_CLIENT_SECRET").ok();
    let previous_redirect = std::env::var("NAVER_OAUTH_REDIRECT_URI").ok();
    let tls = apply_tls();
    let callers = apply_callers();
    // SAFETY: the environment is process-global and this test holds the only
    // lock that touches these variables.
    unsafe {
        std::env::set_var("PROFILE_RUNTIME_MODE", "production");
        std::env::set_var("NAVER_OAUTH_CLIENT_ID", LOCAL_CLIENT_ID);
        std::env::set_var("NAVER_OAUTH_CLIENT_SECRET", LOCAL_CLIENT_SECRET);
        std::env::set_var(
            "NAVER_OAUTH_REDIRECT_URI",
            PRODUCTION_REDIRECT,
        );
    }
    let result = profile_service::config::AppConfig::load();
    // Restore before asserting, so a failure cannot poison later tests.
    unsafe {
        for (name, value) in [
            ("PROFILE_RUNTIME_MODE", previous_mode),
            ("NAVER_OAUTH_CLIENT_ID", previous_id),
            ("NAVER_OAUTH_CLIENT_SECRET", previous_secret),
            ("NAVER_OAUTH_REDIRECT_URI", previous_redirect),
        ] {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
    }
    clear_tls(&tls);
    clear_callers(&callers);
    let error = match result {
        Ok(_) => panic!("a production start with placeholders must refuse"),
        Err(error) => error,
    };
    let message = format!("{error:#}");
    assert!(
        message.contains("NAVER_OAUTH_CLIENT_ID") && message.contains("placeholder"),
        "{message}"
    );
    assert!(!message.contains(LOCAL_CLIENT_SECRET), "{message}");
}

/// A development start with placeholders starts.
#[test]
fn a_development_start_with_placeholders_starts() {
    let _guard = ENVIRONMENT.lock().unwrap_or_else(|poison| poison.into_inner());
    let previous_mode = std::env::var("PROFILE_RUNTIME_MODE").ok();
    let previous_id = std::env::var("NAVER_OAUTH_CLIENT_ID").ok();
    let previous_secret = std::env::var("NAVER_OAUTH_CLIENT_SECRET").ok();
    unsafe {
        std::env::set_var("PROFILE_RUNTIME_MODE", "parity");
        std::env::remove_var("NAVER_OAUTH_CLIENT_ID");
        std::env::remove_var("NAVER_OAUTH_CLIENT_SECRET");
    }
    let result = profile_service::config::AppConfig::load();
    unsafe {
        for (name, value) in [
            ("PROFILE_RUNTIME_MODE", previous_mode),
            ("NAVER_OAUTH_CLIENT_ID", previous_id),
            ("NAVER_OAUTH_CLIENT_SECRET", previous_secret),
        ] {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
    }
    let config = result.expect("a parity start with placeholders must succeed");
    assert_eq!(config.naver_oauth.client_id, LOCAL_CLIENT_ID);
    assert_eq!(config.naver_oauth.redirect_uri, DEFAULT_REDIRECT_URI);
    // The subject salt is present in parity too, so the hash is stable.
    assert!(config.identity_subject_salt.len() >= 32);
}

/// A production start without the subject salt refuses.
#[test]
fn a_production_start_without_the_subject_salt_refuses() {
    let _guard = ENVIRONMENT.lock().unwrap_or_else(|poison| poison.into_inner());
    let names = [
        "PROFILE_RUNTIME_MODE",
        "NAVER_OAUTH_CLIENT_ID",
        "NAVER_OAUTH_CLIENT_SECRET",
        "NAVER_OAUTH_REDIRECT_URI",
        "PROFILE_IDENTITY_SUBJECT_SALT",
    ];
    let previous: Vec<_> = names.iter().map(|name| (*name, std::env::var(name).ok())).collect();
    let tls = apply_tls();
    let callers = apply_callers();
    unsafe {
        std::env::set_var("PROFILE_RUNTIME_MODE", "production");
        std::env::set_var("NAVER_OAUTH_CLIENT_ID", "real-client-id");
        std::env::set_var("NAVER_OAUTH_CLIENT_SECRET", "real-client-secret");
        std::env::set_var("NAVER_OAUTH_REDIRECT_URI", PRODUCTION_REDIRECT);
        std::env::remove_var("PROFILE_IDENTITY_SUBJECT_SALT");
    }
    let result = profile_service::config::AppConfig::load();
    unsafe {
        for (name, value) in &previous {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
    }
    clear_tls(&tls);
    clear_callers(&callers);
    let error = match result {
        Ok(_) => panic!("a production start without a subject salt must refuse"),
        Err(error) => error,
    };
    assert!(
        format!("{error:#}").contains("PROFILE_IDENTITY_SUBJECT_SALT"),
        "{error:#}"
    );
}
