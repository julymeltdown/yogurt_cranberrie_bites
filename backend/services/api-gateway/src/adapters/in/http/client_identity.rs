use anyhow::Context;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, HeaderValue, Request};
use axum::middleware::Next;
use axum::response::Response;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::collections::HashSet;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use uuid::Uuid;

const MAX_FORWARDED_HOPS: usize = 32;
const MAX_TRUSTED_PROXIES: usize = 128;
const MAX_TRUSTED_PROXY_CONFIG_BYTES: usize = 8 * 1024;
const MAX_FORWARDED_HEADER_BYTES: usize = 4 * 1024;

/// Transport-derived address, minted only by the normalizer after resolving the
/// actual socket peer and explicitly trusted proxies. A header is not this type.
#[derive(Clone, Copy)]
pub struct VerifiedClientAddress(IpAddr);
impl VerifiedClientAddress {
    pub fn address(self) -> IpAddr {
        self.0
    }
}

/// Create a stable pseudonymous rate-limit key without persisting account PII.
/// Normalization matches the Auth service: trim surrounding whitespace and
/// lowercase ASCII characters before HMAC-SHA256.
pub fn keyed_admission_identity(secret: &[u8], identifier: &str) -> String {
    let normalized = identifier.trim().to_ascii_lowercase();
    keyed_admission_value(
        secret,
        b"port-rust/gateway/admission/identity/v1\0",
        normalized.as_bytes(),
    )
}

/// Create a pseudonymous key from a verified JWT subject. Subjects are opaque
/// and case-sensitive, so unlike email identifiers they are not normalized.
pub fn keyed_admission_principal(secret: &[u8], subject: &str) -> String {
    keyed_admission_value(
        secret,
        b"port-rust/gateway/admission/principal/v1\0",
        subject.as_bytes(),
    )
}

fn keyed_admission_value(secret: &[u8], domain: &[u8], value: &[u8]) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).expect("HMAC accepts arbitrary key sizes");
    mac.update(domain);
    mac.update(value);
    mac.finalize()
        .into_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Exact proxy IPs, not host names or implicitly trusted private networks.
#[derive(Clone, Debug, Default)]
pub struct TrustedProxies {
    ips: Arc<HashSet<IpAddr>>,
}

impl TrustedProxies {
    pub fn from_env() -> anyhow::Result<Self> {
        match std::env::var("GATEWAY_TRUSTED_PROXY_IPS") {
            Ok(value) => Self::parse(&value),
            Err(std::env::VarError::NotPresent) => Ok(Self::default()),
            Err(error) => Err(error).context("invalid GATEWAY_TRUSTED_PROXY_IPS"),
        }
    }

    pub fn parse(value: &str) -> anyhow::Result<Self> {
        if value.len() > MAX_TRUSTED_PROXY_CONFIG_BYTES {
            anyhow::bail!("GATEWAY_TRUSTED_PROXY_IPS exceeds the byte limit");
        }
        if value.trim().is_empty() {
            return Ok(Self::default());
        }
        let mut ips = HashSet::new();
        for (index, value) in value.split(',').enumerate() {
            if index >= MAX_TRUSTED_PROXIES {
                anyhow::bail!("GATEWAY_TRUSTED_PROXY_IPS contains too many entries");
            }
            let ip = value
                .trim()
                .parse::<IpAddr>()
                .context("GATEWAY_TRUSTED_PROXY_IPS must contain comma-separated IP addresses")?;
            ips.insert(canonical_ip(ip));
        }
        Ok(Self { ips: Arc::new(ips) })
    }

    fn resolve(&self, peer: IpAddr, headers: &HeaderMap) -> IpAddr {
        let peer = canonical_ip(peer);
        if !self.ips.contains(&peer) {
            return peer;
        }
        let mut chain = Vec::new();
        let mut header_bytes = 0usize;
        for value in headers.get_all("x-forwarded-for").iter() {
            header_bytes = header_bytes.saturating_add(value.as_bytes().len());
            if header_bytes > MAX_FORWARDED_HEADER_BYTES {
                return peer;
            }
            let Ok(value) = value.to_str() else {
                return peer;
            };
            for value in value.split(',') {
                if chain.len() >= MAX_FORWARDED_HOPS {
                    return peer;
                }
                let Ok(ip) = value.trim().parse::<IpAddr>() else {
                    return peer;
                };
                chain.push(canonical_ip(ip));
            }
        }
        // A correctly configured proxy appends the address it observed. Walk
        // from that trusted end; never select an attacker-supplied leftmost hop.
        chain
            .into_iter()
            .rev()
            .find(|ip| !self.ips.contains(ip))
            .unwrap_or(peer)
    }
}

/// Exposes the production forwarded-header resolver only to the isolated
/// coverage-guided fuzz workspace; normal Gateway builds do not include it.
#[cfg(feature = "fuzzing")]
#[doc(hidden)]
pub fn fuzz_resolve_forwarded_for(
    peer: IpAddr,
    proxies: &TrustedProxies,
    headers: &HeaderMap,
) -> IpAddr {
    proxies.resolve(peer, headers)
}

fn canonical_ip(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(ip) => ip
            .to_ipv4_mapped()
            .map(IpAddr::V4)
            .unwrap_or(IpAddr::V6(ip)),
        ip => ip,
    }
}

/// Normalize client and request identity before all application middleware.
/// Without transport ConnectInfo, discard forwarded identity and fail closed
/// into the shared unknown-client bucket rather than accepting a supplied key.
pub async fn normalize_forwarded_for(mut request: Request<Body>, next: Next) -> Response {
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip());
    let proxies = request
        .extensions()
        .get::<TrustedProxies>()
        .cloned()
        .unwrap_or_default();
    let client = peer.map(|peer| proxies.resolve(peer, request.headers()));
    request.extensions_mut().remove::<VerifiedClientAddress>();
    if let Some(client) = client {
        request
            .extensions_mut()
            .insert(VerifiedClientAddress(client));
    }
    request.headers_mut().remove("x-forwarded-for");
    if let Some(client) = client {
        let value = HeaderValue::from_str(&client.to_string()).expect("canonical IP header");
        request.headers_mut().insert("x-forwarded-for", value);
    }

    // This layer is outside the deadline: even a timeout before telemetry can
    // finish must preserve the request's correlation ID in the response.
    let correlation_id = request
        .headers()
        .get("x-correlation-id")
        .filter(|value| {
            value.as_bytes().len() <= 128
                && value.to_str().is_ok_and(|value| !value.trim().is_empty())
        })
        .cloned()
        .unwrap_or_else(|| {
            HeaderValue::from_str(&Uuid::new_v4().to_string()).expect("UUID header")
        });
    request
        .headers_mut()
        .insert("x-correlation-id", correlation_id.clone());
    // Run the rest of the chain inside the correlation scope so an error response
    // built deep in a handler can report the same identifier as the header. The
    // value is a bounded ASCII header, so it is already safe to log.
    let correlation_value = correlation_id.to_str().unwrap_or_default().to_owned();
    let mut response = super::CORRELATION_ID
        .scope(correlation_value, next.run(request))
        .await;
    response
        .headers_mut()
        .insert("x-correlation-id", correlation_id);
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::routing::get;
    use axum::Router;
    use http_body_util::BodyExt;
    use proptest::prelude::*;
    use std::net::Ipv4Addr;
    use tower::ServiceExt;

    fn headers(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", value.parse().unwrap());
        headers
    }

    #[tokio::test]
    async fn typed_address_requires_actual_peer_and_uses_the_production_proxy_resolver() {
        for (peer, proxies, forwarded, expected) in [
            (None, "", "198.51.100.8", "unverified"),
            (Some("192.0.2.10:1234"), "", "198.51.100.8", "192.0.2.10"),
            (
                Some("192.0.2.1:1234"),
                "192.0.2.1,192.0.2.2",
                "203.0.113.99,198.51.100.8,192.0.2.2",
                "198.51.100.8",
            ),
        ] {
            let app = Router::new()
                .route(
                    "/",
                    get(|request: Request<Body>| async move {
                        request
                            .extensions()
                            .get::<VerifiedClientAddress>()
                            .map(|address| address.address().to_string())
                            .unwrap_or_else(|| "unverified".into())
                    }),
                )
                .layer(axum::middleware::from_fn(normalize_forwarded_for));
            let mut request = Request::builder()
                .uri("/")
                .header("x-forwarded-for", forwarded)
                .body(Body::empty())
                .unwrap();
            request
                .extensions_mut()
                .insert(TrustedProxies::parse(proxies).unwrap());
            if let Some(peer) = peer {
                request
                    .extensions_mut()
                    .insert(ConnectInfo(peer.parse::<SocketAddr>().unwrap()));
            }
            let body = app
                .oneshot(request)
                .await
                .unwrap()
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes();
            assert_eq!(body.as_ref(), expected.as_bytes());
        }
    }

    #[test]
    fn untrusted_peer_cannot_choose_its_rate_limit_identity() {
        let peer = "192.0.2.10".parse().unwrap();
        assert_eq!(
            TrustedProxies::default().resolve(peer, &headers("198.51.100.1")),
            peer,
        );
    }

    #[test]
    fn trusted_chain_uses_rightmost_untrusted_hop() {
        let proxies = TrustedProxies::parse("192.0.2.1,192.0.2.2").unwrap();
        let actual = proxies.resolve(
            "192.0.2.1".parse().unwrap(),
            &headers("203.0.113.99,198.51.100.8,192.0.2.2"),
        );
        assert_eq!(actual, "198.51.100.8".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn malformed_or_oversized_chains_fall_back_to_peer() {
        let proxies = TrustedProxies::parse("192.0.2.1").unwrap();
        let peer = "192.0.2.1".parse().unwrap();
        for value in ["garbage".to_owned(), vec!["198.51.100.1"; 33].join(",")] {
            assert_eq!(proxies.resolve(peer, &headers(&value)), peer);
        }
        let padded = format!(
            "{}198.51.100.8{}",
            " ".repeat(MAX_FORWARDED_HEADER_BYTES / 2),
            " ".repeat(MAX_FORWARDED_HEADER_BYTES / 2 + 1)
        );
        assert_eq!(proxies.resolve(peer, &headers(&padded)), peer);
    }

    #[test]
    fn multiple_header_lines_preserve_trusted_end_order() {
        let proxies = TrustedProxies::parse("192.0.2.1").unwrap();
        let mut headers = headers("203.0.113.99");
        headers.append("x-forwarded-for", "198.51.100.8".parse().unwrap());
        assert_eq!(
            proxies.resolve("192.0.2.1".parse().unwrap(), &headers),
            "198.51.100.8".parse::<IpAddr>().unwrap(),
        );
    }

    #[test]
    fn ipv4_mapped_addresses_share_the_same_identity() {
        assert_eq!(
            canonical_ip("::ffff:192.0.2.1".parse().unwrap()),
            "192.0.2.1".parse::<IpAddr>().unwrap(),
        );
    }

    #[test]
    fn keyed_admission_identity_matches_auth_normalization_without_exposing_email() {
        let key = b"unit-test-key-with-at-least-32-bytes";
        let digest = keyed_admission_identity(key, "  USER@Example.COM ");
        assert_eq!(digest, keyed_admission_identity(key, "user@example.com"));
        assert_eq!(digest.len(), 64);
        assert!(!digest.contains("user"));
        assert_ne!(
            digest,
            keyed_admission_identity(b"another-key-with-at-least-32-bytes", "user@example.com")
        );
    }

    #[test]
    fn keyed_principal_is_stable_opaque_and_domain_separated() {
        let key = b"unit-test-key-with-at-least-32-bytes";
        let subject = "User:ABC-123";
        let digest = keyed_admission_principal(key, subject);
        assert_eq!(digest, keyed_admission_principal(key, subject));
        assert_eq!(digest.len(), 64);
        assert!(!digest.contains(subject));
        assert_ne!(
            digest,
            keyed_admission_principal(b"another-key-with-at-least-32-bytes", subject)
        );
        assert_ne!(
            digest,
            keyed_admission_identity(key, subject),
            "principal and anonymous identity scopes use distinct HMAC domains"
        );
        assert_ne!(
            keyed_admission_principal(key, subject),
            keyed_admission_principal(key, &subject.to_ascii_uppercase()),
            "JWT subjects are opaque and case-sensitive"
        );
    }

    #[test]
    fn invalid_proxy_configuration_is_rejected() {
        for value in ["*", "localhost", "192.0.2.0/24", "192.0.2.1,"] {
            assert!(TrustedProxies::parse(value).is_err());
        }
        assert!(TrustedProxies::parse(&" ".repeat(MAX_TRUSTED_PROXY_CONFIG_BYTES + 1)).is_err());
    }

    proptest! {
        #![proptest_config(ProptestConfig {
            failure_persistence: Some(Box::new(proptest::test_runner::FileFailurePersistence::WithSource("proptest-regressions"))),
            ..ProptestConfig::default()
        })]

        #[test]
        fn trusted_proxy_configuration_parser_is_bounded(input in prop::collection::vec(any::<u8>(), 0..=MAX_TRUSTED_PROXY_CONFIG_BYTES + 1024)) {
            let candidate = String::from_utf8_lossy(&input);
            let parsed = TrustedProxies::parse(&candidate);
            if candidate.len() > MAX_TRUSTED_PROXY_CONFIG_BYTES {
                prop_assert!(parsed.is_err());
            }
            if let Ok(proxies) = parsed {
                prop_assert!(proxies.ips.len() <= MAX_TRUSTED_PROXIES);
            }
        }

        #[test]
        fn trusted_forwarded_chains_resolve_right_to_left_and_fail_closed_after_the_hop_limit(hops in prop::collection::vec(any::<[u8; 4]>(), 0..=MAX_FORWARDED_HOPS + 8)) {
            let peer = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1));
            let proxies = TrustedProxies::parse(&peer.to_string()).expect("valid proxy");
            let chain = hops.iter().copied().map(Ipv4Addr::from).map(IpAddr::V4).collect::<Vec<_>>();
            let mut request_headers = HeaderMap::new();
            if !chain.is_empty() {
                let encoded = chain.iter().map(ToString::to_string).collect::<Vec<_>>().join(",");
                request_headers.insert("x-forwarded-for", HeaderValue::from_str(&encoded).expect("valid header"));
            }
            let expected = if chain.len() > MAX_FORWARDED_HOPS {
                peer
            } else {
                chain.iter().rev().copied().find(|ip| *ip != peer).unwrap_or(peer)
            };
            prop_assert_eq!(proxies.resolve(peer, &request_headers), expected);
        }
    }

    #[tokio::test]
    async fn normalization_strips_spoofed_headers_without_connection_info() {
        let app = Router::new()
            .route(
                "/",
                get(|headers: HeaderMap| async move {
                    headers
                        .get("x-forwarded-for")
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or("unknown")
                        .to_owned()
                }),
            )
            .layer(axum::middleware::from_fn(normalize_forwarded_for));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header("x-forwarded-for", "198.51.100.1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(Uuid::parse_str(response.headers()["x-correlation-id"].to_str().unwrap()).is_ok());
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(body.as_ref(), b"unknown");
    }

    #[tokio::test]
    async fn direct_connections_use_the_socket_ip_and_preserve_correlation() {
        let app = Router::new()
            .route(
                "/",
                get(|headers: HeaderMap| async move {
                    headers["x-forwarded-for"].to_str().unwrap().to_owned()
                }),
            )
            .layer(axum::middleware::from_fn(normalize_forwarded_for));
        let mut request = Request::builder()
            .uri("/")
            .header("x-forwarded-for", "198.51.100.1")
            .header("x-correlation-id", "test-request")
            .body(Body::empty())
            .unwrap();
        request.extensions_mut().insert(ConnectInfo(
            "192.0.2.10:1234".parse::<SocketAddr>().unwrap(),
        ));
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.headers()["x-correlation-id"], "test-request");
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(body.as_ref(), b"192.0.2.10");
    }
}
