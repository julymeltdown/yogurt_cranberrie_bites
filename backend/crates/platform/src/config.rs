use std::env;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;

pub const LOCAL_PARITY_INTERNAL_GRPC_SERVICE_TOKEN: &str = "local-parity-internal-service-token";
const LEGACY_PLACEHOLDER_INTERNAL_GRPC_SERVICE_TOKEN: &str =
    concat!("dev", "-internal", "-service", "-token");

pub fn env_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_owned())
}

pub fn optional_env(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

pub fn parse_bool_setting(name: &str, value: &str) -> anyhow::Result<bool> {
    parse_setting(name, value)
}

pub fn parse_i32_setting(name: &str, value: &str, min: i32, max: i32) -> anyhow::Result<i32> {
    ensure_ordered_bounds(name, min, max)?;
    let parsed = parse_setting::<i32>(name, value)?;
    ensure_in_range(name, parsed, min, max)?;
    Ok(parsed)
}

pub fn parse_i64_setting(name: &str, value: &str, min: i64, max: i64) -> anyhow::Result<i64> {
    ensure_ordered_bounds(name, min, max)?;
    let parsed = parse_setting::<i64>(name, value)?;
    ensure_in_range(name, parsed, min, max)?;
    Ok(parsed)
}

pub fn parse_u32_setting(name: &str, value: &str, min: u32, max: u32) -> anyhow::Result<u32> {
    ensure_ordered_bounds(name, min, max)?;
    let parsed = parse_setting::<u32>(name, value)?;
    ensure_in_range(name, parsed, min, max)?;
    Ok(parsed)
}

pub fn parse_u64_setting(name: &str, value: &str, min: u64, max: u64) -> anyhow::Result<u64> {
    ensure_ordered_bounds(name, min, max)?;
    let parsed = parse_setting::<u64>(name, value)?;
    ensure_in_range(name, parsed, min, max)?;
    Ok(parsed)
}

pub fn require_non_empty_setting(name: &str, value: impl Into<String>) -> anyhow::Result<String> {
    let value = value.into();
    if value.trim().is_empty() {
        anyhow::bail!("{name} is required");
    }
    Ok(value)
}

pub fn require_production_setting(
    is_production: bool,
    name: &str,
    value: impl Into<String>,
) -> anyhow::Result<String> {
    let value = value.into();
    if is_production {
        require_non_empty_setting(name, value)
    } else {
        Ok(value)
    }
}

pub fn require_production_disabled(
    is_production: bool,
    name: &str,
    enabled: bool,
    reason: &str,
) -> anyhow::Result<()> {
    if is_production && enabled {
        anyhow::bail!("{name} must be disabled in production: {reason}");
    }
    Ok(())
}

pub fn require_production_safe_socket_addr(
    is_production: bool,
    name: &str,
    value: SocketAddr,
) -> anyhow::Result<()> {
    if is_production && value.ip().to_canonical().is_loopback() {
        anyhow::bail!("{name} must not bind to a loopback address in production");
    }
    Ok(())
}

pub fn require_production_safe_url(
    is_production: bool,
    name: &str,
    value: impl AsRef<str>,
) -> anyhow::Result<()> {
    if !is_production {
        return Ok(());
    }

    let value = require_non_empty_setting(name, value.as_ref())?;
    let host = extract_host(&value).ok_or_else(|| anyhow::anyhow!("{name} must include a host"))?;
    if is_unsafe_production_host(&host) {
        anyhow::bail!("{name} must not target local or placeholder host `{host}` in production");
    }
    Ok(())
}

/// Validate a PostgreSQL destination by its database role, not its raw scheme.
/// SQLx accepts other URL schemes and normalizes control characters in a scheme.
/// Neither behavior may select the generic endpoint-validation path here.
pub fn require_production_postgres_url(
    is_production: bool,
    name: &str,
    value: impl AsRef<str>,
) -> anyhow::Result<()> {
    if !is_production {
        return Ok(());
    }
    let value = require_non_empty_setting(name, value.as_ref())?;
    let url = url::Url::parse(value.trim())
        .map_err(|_| anyhow::anyhow!("{name} must be a valid PostgreSQL URL"))?;
    if !matches!(url.scheme(), "postgres" | "postgresql") {
        anyhow::bail!("{name} must use a PostgreSQL URL scheme in production");
    }
    let host = production_postgres_host(name, &url)?;
    if is_unsafe_production_host(&host) {
        anyhow::bail!("{name} must not target a local or placeholder host in production");
    }
    Ok(())
}

pub fn require_production_https_url(
    is_production: bool,
    name: &str,
    value: impl AsRef<str>,
) -> anyhow::Result<()> {
    if !is_production {
        return Ok(());
    }

    let value = value.as_ref();
    require_production_safe_url(true, name, value)?;
    if !value.trim().to_ascii_lowercase().starts_with("https://") {
        anyhow::bail!("{name} must use https in production");
    }
    Ok(())
}

pub fn require_production_safe_urls<'a>(
    is_production: bool,
    name: &str,
    values: impl IntoIterator<Item = &'a String>,
) -> anyhow::Result<()> {
    if !is_production {
        return Ok(());
    }

    let values = values.into_iter().collect::<Vec<_>>();
    if values.is_empty() {
        anyhow::bail!("{name} must contain at least one production origin");
    }

    for value in values {
        require_production_https_url(true, name, value)?;
    }
    Ok(())
}

pub fn runtime_mode_is_production(value: &str, name: &str) -> anyhow::Result<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "production" | "prod" => Ok(true),
        "parity" | "demo" | "development" | "dev" => Ok(false),
        _ => anyhow::bail!("{name} must be production or parity"),
    }
}

fn is_placeholder_token(token: &str) -> bool {
    matches!(
        token.trim(),
        LEGACY_PLACEHOLDER_INTERNAL_GRPC_SERVICE_TOKEN | LOCAL_PARITY_INTERNAL_GRPC_SERVICE_TOKEN
    )
}

/// Environment variable holding one workload's internal credential.
///
/// Credentials are named by the owning workload rather than by the receiver, so
/// the same value authenticates that workload everywhere it is accepted. A
/// service that sends calls reads its own name here; a service that receives
/// calls reads the names of the callers it trusts.
pub fn service_credential_env_name(workload: crate::grpc_auth::Workload) -> String {
    format!(
        "{}_SERVICE_CREDENTIAL",
        workload.as_str().to_ascii_uppercase()
    )
}

/// Environment variable holding the previous accepted value during one
/// per-workload credential rotation window.
pub fn previous_service_credential_env_name(workload: crate::grpc_auth::Workload) -> String {
    format!(
        "{}_SERVICE_CREDENTIAL_PREVIOUS",
        workload.as_str().to_ascii_uppercase()
    )
}

/// Resolve one caller's credential.
///
/// `None` means the credential was not configured. A placeholder or a blank
/// value is always rejected, because both would silently collapse every caller
/// into one authority. Callers decide whether absence is fatal via
/// [`require_caller_credential`].
pub fn resolve_service_credential(
    token: Option<String>,
    token_env_name: &str,
) -> anyhow::Result<Option<String>> {
    match token {
        Some(token) if token.trim().is_empty() => {
            anyhow::bail!("{token_env_name} must not be blank")
        }
        Some(token) if is_placeholder_token(&token) => {
            anyhow::bail!("{token_env_name} must not use a shared placeholder credential")
        }
        Some(token) => Ok(Some(token)),
        None => Ok(None),
    }
}

/// Resolve a credential that this service must accept in production.
pub fn require_caller_credential(
    is_production: bool,
    token: Option<String>,
    token_env_name: &str,
) -> anyhow::Result<Option<String>> {
    let resolved = resolve_service_credential(token, token_env_name)?;
    if is_production && resolved.is_none() {
        anyhow::bail!("{token_env_name} is required in production runtime mode");
    }
    Ok(resolved)
}

/// Build the set of callers this service accepts.
///
/// `accepted` lists every workload that may call this service. `required` names
/// the subset that must be configured even when unset; a production start fails
/// when a required caller has no credential. Callers that are optional and
/// unset are simply not accepted, so an unused integration cannot become an
/// open door.
pub fn build_caller_registry(
    is_production: bool,
    accepted: &[crate::grpc_auth::Workload],
    required: &[crate::grpc_auth::Workload],
) -> anyhow::Result<crate::grpc_auth::CallerRegistry> {
    build_caller_registry_with_lookup(is_production, accepted, required, optional_env)
}

fn build_caller_registry_with_lookup(
    is_production: bool,
    accepted: &[crate::grpc_auth::Workload],
    required: &[crate::grpc_auth::Workload],
    mut lookup: impl FnMut(&str) -> Option<String>,
) -> anyhow::Result<crate::grpc_auth::CallerRegistry> {
    use crate::grpc_auth::{CallerRegistry, Workload};

    for workload in required {
        anyhow::ensure!(
            accepted.contains(workload),
            "required caller {} must also be accepted",
            workload.as_str()
        );
    }

    let mut registry = CallerRegistry::new();
    for workload in accepted {
        let env_name = service_credential_env_name(*workload);
        let previous_env_name = previous_service_credential_env_name(*workload);
        let credentials = resolve_caller_credentials(
            is_production,
            required.contains(workload),
            lookup(&env_name),
            lookup(&previous_env_name),
            &env_name,
            &previous_env_name,
        )?;
        if let Some((current, previous)) = credentials {
            registry = registry.register_with_previous(*workload, &current, previous.as_deref())?;
        }
    }

    if registry.is_empty() && is_production {
        anyhow::bail!("no internal caller credentials are configured");
    }
    // Every service must accept at least one caller: a dev default keeps parity
    // topologies working without weakening the production requirement above.
    if registry.is_empty() {
        registry =
            registry.register(Workload::Gateway, LOCAL_PARITY_INTERNAL_GRPC_SERVICE_TOKEN)?;
    }
    Ok(registry)
}

fn resolve_caller_credentials(
    is_production: bool,
    required: bool,
    current: Option<String>,
    previous: Option<String>,
    current_env_name: &str,
    previous_env_name: &str,
) -> anyhow::Result<Option<(String, Option<String>)>> {
    let current = if required {
        require_caller_credential(is_production, current, current_env_name)?
    } else {
        resolve_service_credential(current, current_env_name)?
    };
    let previous = resolve_service_credential(previous, previous_env_name)?;
    match current {
        Some(current) => Ok(Some((current, previous))),
        None => {
            anyhow::ensure!(
                previous.is_none(),
                "{previous_env_name} requires {current_env_name} to be configured"
            );
            Ok(None)
        }
    }
}

#[cfg(test)]
mod caller_rotation_tests {
    use super::{build_caller_registry_with_lookup, resolve_caller_credentials};
    use crate::grpc_auth::{
        RequireRegisteredCallerInterceptor, ServiceIdentityClientInterceptor, Workload,
    };
    use std::collections::HashMap;
    use tonic::service::Interceptor;
    use tonic::Request;

    #[test]
    fn previous_credential_requires_a_current_credential() {
        let error = resolve_caller_credentials(
            false,
            false,
            None,
            Some("old-credential".to_owned()),
            "GATEWAY_SERVICE_CREDENTIAL",
            "GATEWAY_SERVICE_CREDENTIAL_PREVIOUS",
        )
        .expect_err("a previous-only credential is ambiguous");
        assert!(error
            .to_string()
            .contains("GATEWAY_SERVICE_CREDENTIAL_PREVIOUS"));
    }

    #[test]
    fn production_requires_the_current_credential_during_rotation() {
        assert!(resolve_caller_credentials(
            true,
            true,
            None,
            Some("old-credential".to_owned()),
            "GATEWAY_SERVICE_CREDENTIAL",
            "GATEWAY_SERVICE_CREDENTIAL_PREVIOUS",
        )
        .is_err());
    }

    #[test]
    fn current_and_previous_credentials_are_both_resolved() {
        let credentials = resolve_caller_credentials(
            true,
            true,
            Some("new-credential".to_owned()),
            Some("old-credential".to_owned()),
            "GATEWAY_SERVICE_CREDENTIAL",
            "GATEWAY_SERVICE_CREDENTIAL_PREVIOUS",
        )
        .expect("valid overlap")
        .expect("current credential");

        assert_eq!(credentials.0, "new-credential");
        assert_eq!(credentials.1.as_deref(), Some("old-credential"));
    }

    #[test]
    fn caller_registry_reads_the_previous_credential_for_each_workload() {
        let values = HashMap::from([
            (
                "GATEWAY_SERVICE_CREDENTIAL".to_owned(),
                "gateway-new".to_owned(),
            ),
            (
                "GATEWAY_SERVICE_CREDENTIAL_PREVIOUS".to_owned(),
                "gateway-old".to_owned(),
            ),
        ]);
        let registry = build_caller_registry_with_lookup(
            true,
            &[Workload::Gateway],
            &[Workload::Gateway],
            |name| values.get(name).cloned(),
        )
        .expect("current and previous credentials");

        for credential in ["gateway-new", "gateway-old"] {
            let mut client = ServiceIdentityClientInterceptor::new(Workload::Gateway, credential)
                .expect("client identity");
            let mut server =
                RequireRegisteredCallerInterceptor::new(registry.clone()).expect("registry");
            assert!(server
                .call(client.call(Request::new(())).expect("request"))
                .is_ok());
        }
    }
}

/// Resolve the identity this service presents on outbound internal calls.
pub fn resolve_outbound_identity(
    is_production: bool,
    workload: crate::grpc_auth::Workload,
) -> anyhow::Result<String> {
    let env_name = service_credential_env_name(workload);
    let resolved = if is_production {
        require_caller_credential(is_production, optional_env(&env_name), &env_name)?
    } else {
        resolve_service_credential(optional_env(&env_name), &env_name)?
    };
    Ok(resolved.unwrap_or_else(|| LOCAL_PARITY_INTERNAL_GRPC_SERVICE_TOKEN.to_owned()))
}

// SQLx applies decoded query routing keys after the URL authority. Production
// URLs must express their destination in that authority; ordinary connection
// parameters remain supported. Errors deliberately omit the URL/credentials.
fn production_postgres_host(name: &str, url: &url::Url) -> anyhow::Result<String> {
    if url
        .query_pairs()
        .any(|(key, _)| matches!(key.as_ref(), "host" | "hostaddr"))
    {
        anyhow::bail!(
            "{name} must not override the PostgreSQL host in query parameters in production"
        );
    }
    let host = url
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("{name} must include a host"))?;
    let host = percent_encoding::percent_decode_str(host)
        .decode_utf8()
        .map_err(|_| anyhow::anyhow!("{name} must include a valid PostgreSQL host"))?;
    if host.starts_with('/') {
        anyhow::bail!("{name} must not target a Unix socket in production");
    }
    Ok(host
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_owned())
}

fn extract_host(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    let authority = value
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(value)
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .rsplit_once('@')
        .map(|(_, host)| host)
        .unwrap_or_else(|| {
            value
                .split_once("://")
                .map(|(_, rest)| rest)
                .unwrap_or(value)
                .split(['/', '?', '#'])
                .next()
                .unwrap_or("")
        });

    if authority.starts_with('[') {
        return authority
            .find(']')
            .map(|end| authority[1..end].trim().to_owned())
            .filter(|host| !host.is_empty());
    }

    authority
        .split(':')
        .next()
        .map(str::trim)
        .filter(|host| !host.is_empty())
        .map(ToOwned::to_owned)
}

fn is_unsafe_production_host(host: &str) -> bool {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty() {
        return true;
    }
    if matches!(host.as_str(), "localhost" | "host.docker.internal") {
        return true;
    }
    if host.ends_with(".localhost")
        || host.ends_with(".test")
        || host.ends_with(".example")
        || host.ends_with(".invalid")
    {
        return true;
    }
    host.parse::<IpAddr>()
        .map(|ip| {
            let ip = ip.to_canonical();
            ip.is_loopback() || ip.is_unspecified()
        })
        .unwrap_or(false)
}

fn parse_setting<T>(name: &str, value: &str) -> anyhow::Result<T>
where
    T: FromStr,
    T::Err: std::fmt::Display,
{
    let value = value.trim();
    if value.is_empty() {
        anyhow::bail!("{name} is required");
    }
    value
        .parse()
        .map_err(|error| anyhow::anyhow!("{name} is invalid: {error}"))
}

fn ensure_ordered_bounds<T>(name: &str, min: T, max: T) -> anyhow::Result<()>
where
    T: PartialOrd + std::fmt::Display + Copy,
{
    if min > max {
        anyhow::bail!("{name} validation bounds are invalid: min {min} > max {max}");
    }
    Ok(())
}

fn ensure_in_range<T>(name: &str, value: T, min: T, max: T) -> anyhow::Result<()>
where
    T: PartialOrd + std::fmt::Display + Copy,
{
    if value < min || value > max {
        anyhow::bail!("{name} must be between {min} and {max}");
    }
    Ok(())
}

/// Environment variables naming one workload's internal TLS material.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TlsEnvNames {
    pub cert: String,
    pub key: String,
    pub ca: String,
}

/// Derive the TLS environment variable names for one service prefix.
///
/// Using the service prefix rather than a shared name lets two services on the
/// same host hold different identities, which is what makes per-workload
/// authorization meaningful at the transport layer.
pub fn tls_env_names(service_prefix: &str) -> TlsEnvNames {
    let prefix = service_prefix.trim().to_ascii_uppercase();
    TlsEnvNames {
        cert: format!("{prefix}_GRPC_TLS_CERT_PATH"),
        key: format!("{prefix}_GRPC_TLS_KEY_PATH"),
        ca: format!("{prefix}_GRPC_TLS_CA_PATH"),
    }
}

/// Resolve this service's internal TLS material.
///
/// Production requires all three files, because a missing identity would leave
/// internal calls unauthenticated. Development returns `None` so a parity
/// topology still starts without certificates, and callers must then run the
/// documented plaintext development path explicitly. There is deliberately no
/// path that falls back to plaintext *in production*.
pub fn resolve_tls_material(
    is_production: bool,
    service_prefix: &str,
) -> anyhow::Result<Option<crate::grpc_tls::TlsMaterial>> {
    let names = tls_env_names(service_prefix);
    let cert = optional_env(&names.cert);
    let key = optional_env(&names.key);
    let ca = optional_env(&names.ca);

    match (cert, key, ca) {
        (None, None, None) if is_production => {
            anyhow::bail!(
                "{} is required in production runtime mode; internal gRPC must not run unencrypted",
                names.cert
            )
        }
        (None, None, None) => Ok(None),
        (cert, key, ca) => {
            let cert = cert.ok_or_else(|| anyhow::anyhow!("{} is required", names.cert))?;
            let key = key.ok_or_else(|| anyhow::anyhow!("{} is required", names.key))?;
            let ca = ca.ok_or_else(|| anyhow::anyhow!("{} is required", names.ca))?;
            Ok(Some(crate::grpc_tls::TlsMaterial::load(cert, key, ca)?))
        }
    }
}
