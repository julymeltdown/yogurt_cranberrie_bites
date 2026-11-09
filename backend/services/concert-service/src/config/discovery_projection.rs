//! Discovery projection worker startup configuration.
//!
//! The worker is **opt-in**, like every other background worker in this service:
//! it is enabled by an explicit environment variable rather than by default, so
//! a deployment that has not provisioned the profile service's discovery-source
//! endpoint does not start a worker that would fail every round.
//!
//! Configuration creates no connection, no worker and no background task. It
//! validates the endpoint shape and returns settings; composition decides what to
//! run.
use std::time::Duration;

/// The environment variable that enables the worker.
pub const MODE_VARIABLE: &str = "CONCERT_DISCOVERY_PROJECTION_MODE";
/// The profile service's discovery-source endpoint.
pub const ENDPOINT_VARIABLE: &str = "CONCERT_DISCOVERY_SOURCE_GRPC_ENDPOINT";
/// The interval between rounds, in milliseconds.
pub const INTERVAL_VARIABLE: &str = "CONCERT_DISCOVERY_PROJECTION_INTERVAL_MS";

/// The default interval when the variable is unset.
pub const DEFAULT_INTERVAL_MS: u64 = 30_000;
/// The shortest permitted interval. A tighter loop would poll the profile
/// service faster than the projection can meaningfully change.
pub const MIN_INTERVAL_MS: u64 = 1_000;
/// The longest permitted interval. Beyond this, discovery would lag far enough
/// behind a favorite that the feature feels broken.
pub const MAX_INTERVAL_MS: u64 = 600_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveryProjectionSettings {
    endpoint: String,
    interval: Duration,
}

impl DiscoveryProjectionSettings {
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
    pub fn interval(&self) -> Duration {
        self.interval
    }

    /// Load from the environment. `None` when the worker is off, which is the
    /// default.
    pub fn load() -> anyhow::Result<Option<Self>> {
        Self::from_lookup(platform::config::optional_env)
    }

    /// The same rules over an explicit lookup, so they can be tested without
    /// touching the process environment.
    pub fn from_lookup(
        lookup: impl Fn(&str) -> Option<String>,
    ) -> anyhow::Result<Option<Self>> {
        match lookup(MODE_VARIABLE).as_deref() {
            None | Some("off") => return Ok(None),
            Some("live") => {}
            Some(_) => anyhow::bail!("{MODE_VARIABLE} must be off or live"),
        }
        let endpoint = lookup(ENDPOINT_VARIABLE)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| anyhow::anyhow!("{ENDPOINT_VARIABLE} is required for live discovery projection"))?;
        // The endpoint must be an explicit HTTPS host and port with no
        // credentials, query or path — the same rule every other owner endpoint
        // in this service uses.
        let uri: tonic::codegen::http::Uri = endpoint
            .parse()
            .map_err(|_| anyhow::anyhow!("{ENDPOINT_VARIABLE} must be an absolute URI"))?;
        anyhow::ensure!(
            uri.scheme_str() == Some("https")
                && uri.host().is_some_and(|host| !host.is_empty())
                && uri.port_u16().is_some_and(|port| port > 0)
                && uri.authority().is_some_and(|authority| !authority.as_str().contains('@'))
                && uri.path_and_query().is_none_or(|path| path.as_str() == "/"),
            "{ENDPOINT_VARIABLE} requires explicit HTTPS host/port without credentials, query or path"
        );
        let interval_ms = match lookup(INTERVAL_VARIABLE) {
            None => DEFAULT_INTERVAL_MS,
            Some(value) => value
                .parse::<u64>()
                .map_err(|_| anyhow::anyhow!("{INTERVAL_VARIABLE} must be an integer"))?,
        };
        anyhow::ensure!(
            (MIN_INTERVAL_MS..=MAX_INTERVAL_MS).contains(&interval_ms),
            "{INTERVAL_VARIABLE} must be between {MIN_INTERVAL_MS} and {MAX_INTERVAL_MS}"
        );
        Ok(Some(Self {
            endpoint,
            interval: Duration::from_millis(interval_ms),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn lookup(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: BTreeMap<String, String> = pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect();
        move |key: &str| map.get(key).cloned()
    }

    const ENDPOINT: &str = "https://profile.example:443";

    #[test]
    fn the_worker_is_off_by_default() {
        assert_eq!(DiscoveryProjectionSettings::from_lookup(lookup(&[])).unwrap(), None);
        assert_eq!(
            DiscoveryProjectionSettings::from_lookup(lookup(&[(MODE_VARIABLE, "off")])).unwrap(),
            None
        );
    }

    #[test]
    fn an_unknown_mode_is_refused() {
        for mode in ["on", "true", "LIVE", ""] {
            assert!(
                DiscoveryProjectionSettings::from_lookup(lookup(&[(MODE_VARIABLE, mode)])).is_err(),
                "{mode}"
            );
        }
    }

    #[test]
    fn live_requires_an_endpoint() {
        assert!(
            DiscoveryProjectionSettings::from_lookup(lookup(&[(MODE_VARIABLE, "live")])).is_err()
        );
        // An empty endpoint is not an endpoint.
        assert!(DiscoveryProjectionSettings::from_lookup(lookup(&[
            (MODE_VARIABLE, "live"),
            (ENDPOINT_VARIABLE, "  ")
        ]))
        .is_err());
    }

    #[test]
    fn the_endpoint_must_be_an_explicit_https_host_and_port() {
        for endpoint in [
            "http://profile.example:443",
            "https://profile.example",
            "https://user:pass@profile.example:443",
            "https://profile.example:443/path",
            "https://profile.example:443?query=1",
            "not a uri",
        ] {
            assert!(
                DiscoveryProjectionSettings::from_lookup(lookup(&[
                    (MODE_VARIABLE, "live"),
                    (ENDPOINT_VARIABLE, endpoint)
                ]))
                .is_err(),
                "{endpoint}"
            );
        }
        // A bare host with an explicit port and no path is accepted.
        let settings = DiscoveryProjectionSettings::from_lookup(lookup(&[
            (MODE_VARIABLE, "live"),
            (ENDPOINT_VARIABLE, ENDPOINT),
        ]))
        .unwrap()
        .expect("configured");
        assert_eq!(settings.endpoint(), ENDPOINT);
        assert_eq!(settings.interval(), Duration::from_millis(DEFAULT_INTERVAL_MS));
    }

    #[test]
    fn the_interval_is_bounded() {
        for value in ["0", "999", "600001", "abc", "-1"] {
            assert!(
                DiscoveryProjectionSettings::from_lookup(lookup(&[
                    (MODE_VARIABLE, "live"),
                    (ENDPOINT_VARIABLE, ENDPOINT),
                    (INTERVAL_VARIABLE, value)
                ]))
                .is_err(),
                "{value}"
            );
        }
        for (value, expected) in [
            ("1000", 1_000u64),
            ("30000", 30_000),
            ("600000", 600_000),
        ] {
            let settings = DiscoveryProjectionSettings::from_lookup(lookup(&[
                (MODE_VARIABLE, "live"),
                (ENDPOINT_VARIABLE, ENDPOINT),
                (INTERVAL_VARIABLE, value),
            ]))
            .unwrap()
            .expect("configured");
            assert_eq!(settings.interval(), Duration::from_millis(expected));
        }
    }
}
