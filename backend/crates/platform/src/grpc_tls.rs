//! Internal gRPC transport security: mutual TLS material and peer identity.
//!
//! Before this module existed the internal trust boundary rested on a bearer
//! secret (`x-service-token`) sent over a plaintext HTTP/2 connection. Anyone who
//! could read the network, or read one service's credential file, could act as
//! that workload for every method it was allowed to call. Authentication without
//! confidentiality is half a boundary, so this module supplies the other half.
//!
//! Design rules:
//!
//! * Production never silently falls back to plaintext. Missing or unreadable
//!   material is a startup error, not a downgrade.
//! * A peer identity comes from the validated certificate handshake and its
//!   Subject Alternative Name, never from a request header or metadata field.
//! * The peer name to verify is passed explicitly at each call site rather than
//!   stored on the material, because one process talks to several peers and a
//!   server does not verify its own peer by name at all.
//! * The bearer registry in `grpc_auth` remains as defence in depth. Two
//!   independent checks are cheaper than one that can be misconfigured.

use crate::edge::EdgeBudget;
use crate::grpc_auth::Workload;
use anyhow::Context;
use std::path::Path;
use std::time::Duration;
use tonic::transport::{
    Certificate, Channel, ClientTlsConfig, Endpoint, Identity, ServerTlsConfig,
};

/// Deadline applied to each outbound internal request.
///
/// A dependency that stalls must not hold a worker indefinitely. Without a
/// per-request deadline one slow peer stalls the caller's request path and
/// whichever worker is running it, which is how one degraded dependency becomes
/// a platform-wide stall.
pub const DEFAULT_REQUEST_DEADLINE: Duration = Duration::from_secs(3);

/// Time budget for establishing an internal connection.
pub const DEFAULT_CONNECT_DEADLINE: Duration = Duration::from_secs(2);

/// URI prefix used in peer certificate SANs to name an internal workload.
///
/// A URI SAN is used for identity rather than a DNS name because workloads are
/// roles, not DNS-resolvable hosts. Encoding the role in a URI keeps the mapping
/// explicit and testable.
pub const WORKLOAD_URI_PREFIX: &str = "spiffe://internal/";

/// DNS suffix used for the server-name half of a workload certificate.
///
/// rustls verifies the *server name* against DNS or IP SANs and never against a
/// URI SAN, so a workload certificate carries two SANs: a URI SAN for identity
/// and a DNS SAN so a client can verify the peer it dialled.
pub const WORKLOAD_DNS_SUFFIX: &str = ".svc.internal";

/// TLS material for one workload.
///
/// The same material serves both directions: a process presents it as its client
/// identity on outbound calls and as its server identity on its listener.
#[derive(Clone)]
pub struct TlsMaterial {
    cert_pem: Vec<u8>,
    key_pem: Vec<u8>,
    ca_pem: Vec<u8>,
}

impl std::fmt::Debug for TlsMaterial {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never render key material, even in a debug log or a panic message.
        formatter
            .debug_struct("TlsMaterial")
            .field("cert_pem_bytes", &self.cert_pem.len())
            .field("key_pem_bytes", &"<redacted>")
            .field("ca_pem_bytes", &self.ca_pem.len())
            .finish()
    }
}

impl TlsMaterial {
    /// Load material from three files. Every file is required.
    pub fn load(
        cert_path: impl AsRef<Path>,
        key_path: impl AsRef<Path>,
        ca_path: impl AsRef<Path>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            cert_pem: read_required(cert_path.as_ref(), "certificate")?,
            key_pem: read_required(key_path.as_ref(), "private key")?,
            ca_pem: read_required(ca_path.as_ref(), "CA bundle")?,
        })
    }

    /// Build material from in-memory PEM.
    ///
    /// Used by tests and by deployments that inject secrets through the
    /// environment rather than a file mount.
    pub fn from_pem(
        cert_pem: impl Into<Vec<u8>>,
        key_pem: impl Into<Vec<u8>>,
        ca_pem: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            cert_pem: cert_pem.into(),
            key_pem: key_pem.into(),
            ca_pem: ca_pem.into(),
        }
    }

    /// Server-side configuration requiring a client certificate from the CA.
    ///
    /// Client authentication is deliberately never optional: an optional client
    /// certificate is an unauthenticated client, which is the condition this
    /// module exists to remove.
    pub fn server_config(&self) -> anyhow::Result<ServerTlsConfig> {
        Ok(ServerTlsConfig::new()
            .identity(Identity::from_pem(&self.cert_pem, &self.key_pem))
            .client_ca_root(Certificate::from_pem(&self.ca_pem)))
    }

    /// Client-side configuration that presents this identity and verifies `peer`.
    pub fn client_config_for_peer(&self, peer: Workload) -> anyhow::Result<ClientTlsConfig> {
        Ok(ClientTlsConfig::new()
            .ca_certificate(Certificate::from_pem(&self.ca_pem))
            .identity(Identity::from_pem(&self.cert_pem, &self.key_pem))
            .domain_name(workload_dns_name(peer)))
    }

    /// Open a mutually authenticated channel to `peer` at `endpoint`.
    ///
    /// Centralizing this keeps every outbound client on the same configuration;
    /// a client that built its own channel could silently omit mutual TLS.
    pub async fn connect(&self, endpoint: &str, peer: Workload) -> anyhow::Result<Channel> {
        self.endpoint(endpoint, peer)?
            .connect()
            .await
            .with_context(|| {
                format!(
                    "mutual TLS to {} service failed; check certificate validity and CA trust",
                    peer.as_str()
                )
            })
    }

    /// Build the endpoint for `peer` without connecting yet.
    ///
    /// Useful for a caller that needs to set a connect timeout or use lazy
    /// connection while still requiring mutual TLS.
    pub fn endpoint(&self, endpoint: &str, peer: Workload) -> anyhow::Result<Endpoint> {
        let endpoint = Endpoint::from_shared(endpoint.to_owned()).map_err(|error| {
            anyhow::anyhow!("invalid internal gRPC endpoint {endpoint}: {error}")
        })?;
        with_deadlines(endpoint)
            .tls_config(self.client_config_for_peer(peer)?)
            .context("internal gRPC client TLS configuration is invalid")
    }
}

/// Build a channel to `peer` that connects on first use.
///
/// `connect_with_optional_tls` waits for the peer, so a service that owns several
/// outbound clients cannot start until every dependency is reachable. That makes an
/// unrelated dependency outage into a startup failure: a restart during a dependency
/// blip leaves the service down and unready rather than serving what it can. It also
/// couples startup order to the deployment, which is the coupling readiness is meant
/// to replace.
///
/// A lazy channel still validates configuration eagerly. `endpoint_with_optional_tls`
/// parses the URI and resolves TLS material here, so a malformed endpoint or missing
/// certificate still fails fast -- only reachability is deferred.
///
/// The caller must therefore report connectivity separately, which is what a
/// readiness probe is for; this function does not make an unreachable peer harmless,
/// it moves the failure from startup to the request that needs the peer.
pub fn lazy_channel_with_optional_tls(
    material: Option<&TlsMaterial>,
    endpoint: &str,
    peer: Workload,
) -> anyhow::Result<Channel> {
    Ok(endpoint_with_optional_tls(material, endpoint, peer)?.connect_lazy())
}

/// Build an endpoint to `peer` with a declared edge budget applied.
///
/// Same as [endpoint_with_optional_tls] but taking the deadlines from the edge
/// table, so the number is the one the contract names rather than a shared
/// default.
pub fn endpoint_for_edge_with_optional_tls(
    material: Option<&TlsMaterial>,
    endpoint: &str,
    peer: Workload,
    budget: EdgeBudget,
) -> anyhow::Result<Endpoint> {
    let parsed = Endpoint::from_shared(endpoint.to_owned())
        .map_err(|error| anyhow::anyhow!("invalid internal gRPC endpoint {endpoint}: {error}"))?;
    let endpoint = with_edge_deadlines(parsed, budget);
    match material {
        Some(material) => endpoint
            .tls_config(material.client_config_for_peer(peer)?)
            .context("internal gRPC client TLS configuration is invalid"),
        None => {
            tracing::warn!(
                peer = peer.as_str(),
                edge = budget.edge,
                "internal gRPC call is unencrypted; only supported outside production"
            );
            Ok(endpoint)
        }
    }
}

/// Build a channel to `peer` that connects on first use, with a declared budget.
pub fn lazy_channel_for_edge_with_optional_tls(
    material: Option<&TlsMaterial>,
    endpoint: &str,
    peer: Workload,
    budget: EdgeBudget,
) -> anyhow::Result<Channel> {
    Ok(endpoint_for_edge_with_optional_tls(material, endpoint, peer, budget)?.connect_lazy())
}

/// Apply an edge's declared deadlines to an endpoint.
///
/// tonic applies the request deadline per call and takes the shorter of it and
/// any caller-supplied `grpc-timeout`, so a stricter caller budget still wins.
/// Taking the budget from the declared table rather than from a default is what
/// makes the number reviewable: a uniform default cannot say whether it was
/// chosen for a database fan-out or for a cached read.
pub fn with_edge_deadlines(endpoint: Endpoint, budget: EdgeBudget) -> Endpoint {
    endpoint
        .timeout(budget.deadline)
        .connect_timeout(budget.connect_deadline)
}

/// Apply the standard internal deadlines to an endpoint.
///
/// Kept for callers that have not moved to a declared edge yet, and for the
/// transport probes that only care that a deadline exists.
pub fn with_deadlines(endpoint: Endpoint) -> Endpoint {
    endpoint
        .timeout(DEFAULT_REQUEST_DEADLINE)
        .connect_timeout(DEFAULT_CONNECT_DEADLINE)
}

/// Build an endpoint to `peer`, adding mutual TLS only when it is configured.
///
/// Development topologies start without certificates, so `None` must produce the
/// documented plaintext development path. Production cannot reach this with
/// `None` because [crate::config::resolve_tls_material] refuses to start, which
/// keeps the permissive branch unavailable where it would matter. The warning is
/// deliberate: an unencrypted internal call should be visible in logs.
pub fn endpoint_with_optional_tls(
    material: Option<&TlsMaterial>,
    endpoint: &str,
    peer: Workload,
) -> anyhow::Result<Endpoint> {
    match material {
        Some(material) => material.endpoint(endpoint, peer),
        None => {
            tracing::warn!(
                peer = peer.as_str(),
                "internal gRPC call is unencrypted; only supported outside production"
            );
            Ok(with_deadlines(
                Endpoint::from_shared(endpoint.to_owned()).map_err(|error| {
                    anyhow::anyhow!("invalid internal gRPC endpoint {endpoint}: {error}")
                })?,
            ))
        }
    }
}

/// Connect to `peer`, using mutual TLS when material is configured.
pub async fn connect_with_optional_tls(
    material: Option<&TlsMaterial>,
    endpoint: &str,
    peer: Workload,
) -> anyhow::Result<Channel> {
    endpoint_with_optional_tls(material, endpoint, peer)?
        .connect()
        .await
        .with_context(|| format!("connecting to {} service failed", peer.as_str()))
}

fn read_required(path: &Path, description: &str) -> anyhow::Result<Vec<u8>> {
    let bytes = std::fs::read(path).with_context(|| {
        format!(
            "internal gRPC TLS {description} is unreadable at {}",
            path.display()
        )
    })?;
    anyhow::ensure!(
        !bytes.is_empty(),
        "internal gRPC TLS {description} at {} is empty",
        path.display()
    );
    Ok(bytes)
}

/// Map a peer certificate to exactly one workload, using its URI SAN.
///
/// Returns `None` when the certificate carries no workload URI, which a caller
/// must treat as an unauthenticated peer rather than an unknown-but-allowed one.
pub fn workload_from_peer_certificate(der: &[u8]) -> Option<Workload> {
    if !is_complete_x509_certificate_der(der) {
        return None;
    }
    let (remainder, certificate) = x509_parser::parse_x509_certificate(der).ok()?;
    if !remainder.is_empty() {
        return None;
    }
    let extension = certificate.subject_alternative_name().ok()??;
    let mut identities = extension
        .value
        .general_names
        .iter()
        .filter_map(|name| match name {
            x509_parser::extensions::GeneralName::URI(uri)
                if uri.starts_with(WORKLOAD_URI_PREFIX) =>
            {
                Some(*uri)
            }
            _ => None,
        });
    let workload = workload_from_uri(identities.next()?)?;
    // An ambiguous leaf must not gain authority from whichever SAN came first.
    identities.next().is_none().then_some(workload)
}

/// Require a single DER Certificate with a completely consumed outer SEQUENCE
/// and exactly its three defined top-level fields. `x509-parser` parses the
/// known fields but ignores unparsed bytes inside that outer SEQUENCE, so
/// checking only its returned remainder would accept some malformed inputs.
fn is_complete_x509_certificate_der(der: &[u8]) -> bool {
    const MAX_DER_NESTING_DEPTH: usize = 16;

    let Ok((content, header)) = x509_parser::der_parser::der::der_read_element_header(der) else {
        return false;
    };
    if !header.is_universal() || header.tag().0 != 16 || !header.is_constructed() {
        return false;
    }
    let x509_parser::der_parser::ber::Length::Definite(content_length) = header.length() else {
        return false;
    };
    if content_length != content.len() {
        return false;
    }
    count_der_elements(content, 0, MAX_DER_NESTING_DEPTH) == Some(3)
}

/// Count a constructed value's complete DER children, rejecting hidden bytes,
/// indefinite lengths, end-of-content markers, and excessive nesting.
fn count_der_elements(input: &[u8], depth: usize, max_depth: usize) -> Option<usize> {
    if depth > max_depth {
        return None;
    }

    let mut remaining = input;
    let mut count = 0usize;
    while !remaining.is_empty() {
        let (content, header) =
            x509_parser::der_parser::der::der_read_element_header(remaining).ok()?;
        if header.is_universal() && header.tag().0 == 0 {
            return None;
        }
        let x509_parser::der_parser::ber::Length::Definite(content_length) = header.length() else {
            return None;
        };
        let value = content.get(..content_length)?;
        remaining = content.get(content_length..)?;
        if header.is_constructed() {
            count_der_elements(value, depth + 1, max_depth)?;
        }
        count = count.checked_add(1)?;
    }
    Some(count)
}

/// Parse a workload out of a certificate URI SAN value.
///
/// The prefix is required and the remainder must be an exact known workload, so a
/// certificate cannot claim a role by embedding it in a longer string.
pub fn workload_from_uri(uri: &str) -> Option<Workload> {
    Workload::parse(uri.strip_prefix(WORKLOAD_URI_PREFIX)?)
}

/// Build the URI SAN value that names one workload's identity.
pub fn workload_uri(workload: Workload) -> String {
    format!("{WORKLOAD_URI_PREFIX}{}", workload.as_str())
}

/// Build the DNS SAN value used to verify one workload acting as a server.
pub fn workload_dns_name(workload: Workload) -> String {
    format!("{}{WORKLOAD_DNS_SUFFIX}", workload.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workload_uri_round_trips_for_every_workload() {
        for workload in Workload::ALL {
            let uri = workload_uri(workload);
            assert_eq!(workload_from_uri(&uri), Some(workload), "uri {uri}");
        }
    }

    #[test]
    fn workload_uri_requires_an_exact_known_role() {
        // A prefix without a role is not a workload.
        assert_eq!(workload_from_uri(WORKLOAD_URI_PREFIX), None);
        // An unknown role is rejected rather than accepted as a new identity.
        assert_eq!(workload_from_uri("spiffe://internal/billing"), None);
        // A role hidden in a longer path must not match a known role.
        assert_eq!(workload_from_uri("spiffe://internal/post/extra"), None);
        // A different trust domain must not be accepted.
        assert_eq!(workload_from_uri("spiffe://other/post"), None);
        // Case matters: the wire identifier is lowercase.
        assert_eq!(workload_from_uri("spiffe://internal/Post"), None);
    }

    #[test]
    fn workload_identity_and_server_name_are_distinct() {
        for workload in Workload::ALL {
            assert_ne!(workload_uri(workload), workload_dns_name(workload));
        }
        assert_eq!(workload_dns_name(Workload::Post), "post.svc.internal");
    }

    #[test]
    fn missing_material_is_an_error_not_a_downgrade() {
        let material = TlsMaterial::load(
            "/nonexistent/cert.pem",
            "/nonexistent/key.pem",
            "/nonexistent/ca.pem",
        );
        assert!(material.is_err(), "unreadable material must fail closed");
    }

    #[test]
    fn debug_output_never_reveals_key_bytes() {
        let material = TlsMaterial::from_pem(
            b"-----BEGIN CERTIFICATE-----".to_vec(),
            b"super-secret-key-bytes".to_vec(),
            b"-----BEGIN CERTIFICATE-----".to_vec(),
        );
        let rendered = format!("{material:?}");
        assert!(!rendered.contains("super-secret-key-bytes"));
        assert!(rendered.contains("redacted"));
    }
}
