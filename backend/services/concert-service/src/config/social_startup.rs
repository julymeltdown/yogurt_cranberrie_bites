//! Explicit Social startup inputs. No key defaults, claims or separate owners.
use super::social_runtime::SocialRuntimeConfig;
use crate::adapters::out::notification::delivery::NotificationDeliveryClient;
use crate::social_live::keys::{KeyMaterial, ManagedSocialKeys};
use platform::grpc_tls::TlsMaterial;
use serde::Deserialize;
use std::{collections::BTreeSet, sync::Arc};

pub struct SocialStartupSettings {
    pub config: SocialRuntimeConfig,
    pub notification: Arc<NotificationDeliveryClient>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Key {
    id: String,
    secret_hex: String,
}
fn secret(raw: &str) -> anyhow::Result<[u8; 32]> {
    anyhow::ensure!(
        raw.len() == 64
            && raw
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "Social keys require 32-byte lowercase hexadecimal secrets"
    );
    let mut key = [0; 32];
    for (i, pair) in raw.as_bytes().chunks_exact(2).enumerate() {
        let digit = |b| if b <= b'9' { b - b'0' } else { b - b'a' + 10 };
        key[i] = digit(pair[0]) * 16 + digit(pair[1]);
    }
    anyhow::ensure!(key != [0; 32], "Social keys must be nonzero");
    Ok(key)
}
fn required(
    read: &mut impl FnMut(&str) -> anyhow::Result<Option<String>>,
    name: &str,
) -> anyhow::Result<String> {
    read(name)?
        .filter(|v| !v.is_empty())
        .ok_or_else(|| anyhow::anyhow!("{name} is required for live Social"))
}
fn keys(
    raw: &str,
    seen: &mut BTreeSet<[u8; 32]>,
) -> anyhow::Result<(KeyMaterial, Vec<KeyMaterial>)> {
    anyhow::ensure!(
        raw.len() <= 16 * 1024,
        "Social key configuration is too large"
    );
    let parsed: Vec<Key> = serde_json::from_str(raw)
        .map_err(|_| anyhow::anyhow!("Social keys require a closed JSON array"))?;
    anyhow::ensure!(
        (1..=32).contains(&parsed.len()),
        "Social keys require one active and at most 31 retained versions"
    );
    let mut material = Vec::new();
    for key in parsed {
        let bytes = secret(&key.secret_hex)?;
        anyhow::ensure!(
            seen.insert(bytes),
            "Social handle, command and cursor secrets must be independent"
        );
        material.push(KeyMaterial::new(key.id, bytes.to_vec()));
    }
    let active = material.remove(0);
    Ok((active, material))
}
impl SocialStartupSettings {
    /// Default/off never reads optional secrets. Live requires the original
    /// shared core, existing World socket and actual Notification delivery path.
    pub fn from_values(
        mut read: impl FnMut(&str) -> anyhow::Result<Option<String>>,
        shared_intro: bool,
        world: bool,
        inbox_and_target: bool,
        tls: Option<&TlsMaterial>,
    ) -> anyhow::Result<Option<Self>> {
        match read("CONCERT_SOCIAL_MODE")?.as_deref() {
            None | Some("off") => return Ok(None),
            Some("live") => (),
            _ => anyhow::bail!("CONCERT_SOCIAL_MODE must be off or live"),
        }
        anyhow::ensure!(shared_intro && world && inbox_and_target, "Live Social requires shared Intro/Participant, World and live Notification inbox/target composition");
        let tls = tls.ok_or_else(|| anyhow::anyhow!("Live Social requires mTLS"))?;
        tls.server_config()?;
        let cursor = secret(&required(&mut read, "CONCERT_SOCIAL_CURSOR_KEY_HEX")?)?;
        let mut seen = BTreeSet::from([cursor]);
        let (handle, old_handles) = keys(
            &required(&mut read, "CONCERT_SOCIAL_HANDLE_KEYS_JSON")?,
            &mut seen,
        )?;
        let (command, old_commands) = keys(
            &required(&mut read, "CONCERT_SOCIAL_COMMAND_KEYS_JSON")?,
            &mut seen,
        )?;
        let material = ManagedSocialKeys::new(handle, old_handles, command, old_commands, &cursor)?;
        let raw_ttl = required(&mut read, "CONCERT_SOCIAL_REQUEST_TTL_SECONDS")?;
        let ttl: i64 = raw_ttl
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid Social request TTL"))?;
        anyhow::ensure!(
            raw_ttl == ttl.to_string(),
            "Social request TTL must be canonical integer seconds"
        );
        let issuer = read("CONCERT_SOCIAL_ACCEPTED_CLAIM_ISSUER")?;
        let revision = read("CONCERT_SOCIAL_ACCEPTED_CLAIM_REVISION")?;
        let accepted = match (&issuer, &revision) {
            (None, None) => None,
            (Some(i), Some(r)) => Some((i.as_str(), r.as_str())),
            _ => {
                anyhow::bail!("Social accepted claim issuer and revision must be supplied together")
            }
        };
        let config = SocialRuntimeConfig::new(material, accepted, ttl)?;
        // These are the same explicitly configured endpoint and Concert
        // workload credential used by the shared Intro/Participant owners.
        let endpoint = required(&mut read, "CONCERT_NOTIFICATION_GRPC_ENDPOINT")?;
        let uri: axum::http::Uri = endpoint
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid Social Notification endpoint"))?;
        anyhow::ensure!(!endpoint.contains('#') && uri.scheme_str() == Some("https") && uri.host().is_some_and(|h| !h.is_empty()) && uri.port_u16().is_some_and(|p| p > 0) && uri.authority().is_some_and(|a| !a.as_str().contains('@')) && uri.path_and_query().is_none_or(|p| p.as_str() == "/"), "Social Notification endpoint requires explicit HTTPS host/port without credentials, path, query or fragment");
        let token = required(&mut read, "CONCERT_SERVICE_CREDENTIAL")?;
        let token = platform::config::resolve_service_credential(
            Some(token),
            "CONCERT_SERVICE_CREDENTIAL",
        )?
        .ok_or_else(|| anyhow::anyhow!("Explicit Concert credential is required"))?;
        let notification = Arc::new(NotificationDeliveryClient::connect(&endpoint, &token, tls)?);
        Ok(Some(Self {
            config,
            notification,
        }))
    }
    pub fn load(
        shared_intro: bool,
        world: bool,
        inbox_and_target: bool,
        tls: Option<&TlsMaterial>,
    ) -> anyhow::Result<Option<Self>> {
        Self::from_values(
            |name| match std::env::var(name) {
                Ok(value) => Ok(Some(value)),
                Err(std::env::VarError::NotPresent) => Ok(None),
                Err(_) => anyhow::bail!("Invalid Social configuration encoding"),
            },
            shared_intro,
            world,
            inbox_and_target,
            tls,
        )
    }
}
