//! Integration coverage for internal gRPC mutual TLS.
//!
//! These tests drive a real tonic server and client over a real TCP socket, so a
//! regression in the transport configuration fails here rather than only in
//! production. Certificate material is generated per test run with rcgen and is
//! never checked in.
//!
//! Two facts discovered while writing this file, both load-bearing:
//!
//! 1. rustls verifies the server name against DNS and IP SANs and ignores URI
//!    SANs, so a workload certificate carries both a URI SAN for identity and a
//!    DNS SAN for the name a client dials.
//! 2. A successful trust check must cross the wire with an actual RPC. Asserting
//!    on `connect()` alone cannot distinguish "authenticated" from "TCP accepted".

use platform::grpc_auth::Workload;
use platform::grpc_tls::{
    workload_dns_name, workload_from_peer_certificate, workload_from_uri, workload_uri, TlsMaterial,
};
use rcgen::{
    BasicConstraints, CertificateParams, DnType, ExtendedKeyUsagePurpose, IsCa, KeyPair,
    KeyUsagePurpose,
};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use time::{Duration as TimeDuration, OffsetDateTime};
use tokio::net::TcpListener;
use tonic::transport::Server;

/// Concrete message types for the transport probe.
///
/// The fixture registers no service, so the payload is irrelevant; only the
/// handshake and delivery are under test. `()` cannot be used because
/// `ProstCodec` needs types that implement `prost::Message`.
#[derive(Clone, PartialEq, prost::Message)]
struct ProbeRequest {}

#[derive(Clone, PartialEq, prost::Message)]
struct ProbeResponse {}

/// Spelled out because `ProstCodec` is generic over both directions and neither
/// can be inferred from an empty service.
type ProbeCodec = tonic_prost::ProstCodec<ProbeRequest, ProbeResponse>;

/// A generated certificate authority plus per-workload leaf issuance.
struct TestPki {
    ca_pem: String,
    ca_key: KeyPair,
    ca_params: CertificateParams,
    directory: PathBuf,
}

struct LeafFiles {
    cert_path: PathBuf,
    key_path: PathBuf,
    ca_path: PathBuf,
}

impl LeafFiles {
    fn material(&self) -> anyhow::Result<TlsMaterial> {
        TlsMaterial::load(&self.cert_path, &self.key_path, &self.ca_path)
    }

    fn material_trusting(&self, ca_pem: &[u8]) -> anyhow::Result<TlsMaterial> {
        Ok(TlsMaterial::from_pem(
            std::fs::read(&self.cert_path)?,
            std::fs::read(&self.key_path)?,
            ca_pem.to_vec(),
        ))
    }
}

impl TestPki {
    fn generate(name: &str) -> anyhow::Result<Self> {
        let mut ca_params = CertificateParams::new(Vec::<String>::new())?;
        ca_params
            .distinguished_name
            .push(DnType::CommonName, "internal-test-ca");
        ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        ca_params.key_usages = vec![
            KeyUsagePurpose::KeyCertSign,
            KeyUsagePurpose::CrlSign,
            KeyUsagePurpose::DigitalSignature,
        ];
        let ca_key = KeyPair::generate()?;
        let ca_cert = ca_params.self_signed(&ca_key)?;

        let directory =
            std::env::temp_dir().join(format!("platform-grpc-tls-{name}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory)?;

        Ok(Self {
            ca_pem: ca_cert.pem(),
            ca_key,
            ca_params,
            directory,
        })
    }

    /// Issue a leaf certificate carrying both halves of a workload identity.
    ///
    /// The URI SAN names the workload for authorization; the DNS SAN lets a
    /// client verify the peer it dialled. Both EKUs are present because the same
    /// material is used on both sides of the fixture, and rustls rejects a client
    /// certificate that lacks clientAuth.
    fn issue(&self, workload: Workload, file_stem: &str) -> anyhow::Result<LeafFiles> {
        self.issue_with_uris(workload, file_stem, &[workload_uri(workload)])
    }

    fn issue_with_uris(
        &self,
        workload: Workload,
        file_stem: &str,
        uris: &[String],
    ) -> anyhow::Result<LeafFiles> {
        self.issue_with_validity(workload, file_stem, uris, None)
    }

    fn issue_expired(&self, workload: Workload, file_stem: &str) -> anyhow::Result<LeafFiles> {
        let not_before = OffsetDateTime::from_unix_timestamp(1_577_836_800)?;
        let not_after = not_before + TimeDuration::days(1);
        self.issue_with_validity(
            workload,
            file_stem,
            &[workload_uri(workload)],
            Some((not_before, not_after)),
        )
    }

    fn issue_with_validity(
        &self,
        workload: Workload,
        file_stem: &str,
        uris: &[String],
        validity: Option<(OffsetDateTime, OffsetDateTime)>,
    ) -> anyhow::Result<LeafFiles> {
        let mut params = CertificateParams::new(Vec::<String>::new())?;
        if let Some((not_before, not_after)) = validity {
            params.not_before = not_before;
            params.not_after = not_after;
        }
        params
            .distinguished_name
            .push(DnType::CommonName, workload.as_str());
        params.is_ca = IsCa::NoCa;
        params.key_usages = vec![
            KeyUsagePurpose::DigitalSignature,
            KeyUsagePurpose::KeyEncipherment,
        ];
        params.extended_key_usages = vec![
            ExtendedKeyUsagePurpose::ServerAuth,
            ExtendedKeyUsagePurpose::ClientAuth,
        ];
        params.subject_alt_names = vec![rcgen::SanType::DnsName(
            workload_dns_name(workload).try_into()?,
        )];
        for uri in uris {
            params
                .subject_alt_names
                .push(rcgen::SanType::URI(uri.clone().try_into()?));
        }

        let key = KeyPair::generate()?;
        let cert = params.signed_by(
            &key,
            &rcgen::Issuer::from_params(&self.ca_params, &self.ca_key),
        )?;

        let cert_path = self.directory.join(format!("{file_stem}-cert.pem"));
        let key_path = self.directory.join(format!("{file_stem}-key.pem"));
        let ca_path = self.directory.join("ca.pem");
        std::fs::write(&cert_path, cert.pem())?;
        std::fs::write(&key_path, key.serialize_pem())?;
        std::fs::write(&ca_path, &self.ca_pem)?;

        Ok(LeafFiles {
            cert_path,
            key_path,
            ca_path,
        })
    }
}

impl Drop for TestPki {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

/// Start a TLS-only tonic server and return its address and task handle.
async fn start_tls_server(
    server_material: TlsMaterial,
) -> anyhow::Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);
    let tls_config = server_material.server_config()?;
    let task = tokio::spawn(async move {
        let _ = Server::builder()
            .tls_config(tls_config)
            .expect("server tls")
            .add_routes(tonic::service::Routes::default())
            .serve_with_incoming(incoming)
            .await;
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    Ok((address, task))
}

/// Report whether the server delivered the request to the (empty) service.
///
/// The fixture registers no service, so a delivered request is answered
/// `Unimplemented`. That status is the only outcome that proves the mutual
/// handshake succeeded *and* the request crossed the transport. A transport
/// rejection, a failed handshake, or a connection error all return `false`.
async fn reached_service(material: &TlsMaterial, address: SocketAddr, peer: Workload) -> bool {
    let Ok(channel) = material.connect(&format!("https://{address}"), peer).await else {
        return false;
    };
    let mut client = tonic::client::Grpc::new(channel);
    if client.ready().await.is_err() {
        return false;
    }
    match client
        .unary(
            tonic::Request::new(ProbeRequest {}),
            http::uri::PathAndQuery::from_static("/internal.probe.v1.Probe/Check"),
            ProbeCodec::default(),
        )
        .await
    {
        Ok(_) => true,
        Err(status) => status.code() == tonic::Code::Unimplemented,
    }
}

async fn readiness_accepts_gateway_credential(
    server_material: &TlsMaterial,
    registry: platform::grpc_auth::CallerRegistry,
    client_material: &TlsMaterial,
    credential: &str,
) -> anyhow::Result<bool> {
    use platform::edge::Edge;
    use platform::readiness::{Probe, Readiness};
    use platform::shutdown::Shutdown;
    use std::time::Duration;

    let make_readiness = || {
        Readiness::with_shutdown(
            Probe::new("database", true, Duration::from_millis(100), || async {
                true
            }),
            Shutdown::new(),
        )
    };
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("https://{}", listener.local_addr()?);
    let shutdown = Shutdown::new();
    let stop = shutdown.cancelled();
    let service = make_readiness().grpc_service(registry)?;
    let task = tokio::spawn(
        Server::builder()
            .tls_config(server_material.server_config()?)?
            .add_service(service)
            .serve_with_incoming_shutdown(
                tokio_stream::wrappers::TcpListenerStream::new(listener),
                stop,
            ),
    );

    let report = make_readiness()
        .with_dependencies(vec![Probe::grpc(
            Edge::GatewayProfile,
            &endpoint,
            credential,
            Some(client_material),
        )?])?
        .report()
        .await;

    shutdown.request();
    tokio::time::timeout(Duration::from_secs(2), task).await???;
    Ok(report.ready)
}

#[test]
fn a_certificate_carries_both_identity_and_server_name() {
    let pki = TestPki::generate("san").expect("pki");
    let leaf = pki.issue(Workload::Post, "post").expect("leaf");

    // Identity comes from the URI SAN.
    assert_eq!(
        workload_from_uri(&workload_uri(Workload::Post)),
        Some(Workload::Post)
    );
    // Server-name verification comes from the DNS SAN; the two are distinct.
    assert_eq!(workload_dns_name(Workload::Post), "post.svc.internal");
    assert_ne!(
        workload_uri(Workload::Post),
        workload_dns_name(Workload::Post)
    );

    // The written certificate really carries the URI SAN the server will parse.
    let der = read_der(&leaf.cert_path);
    assert_eq!(workload_from_peer_certificate(&der), Some(Workload::Post));
}

#[test]
fn peer_certificate_parser_rejects_trailing_der_data() {
    let pki = TestPki::generate("trailing-certificate-data").expect("pki");
    let leaf = pki.issue(Workload::Post, "post").expect("leaf");
    let der = read_der(&leaf.cert_path);
    for trailing_data in [
        &[0xff][..],
        &[0][..],
        &[0, 0, 0][..],
        &b"\n"[..],
        &b"\n\0\0\0"[..],
    ] {
        let mut certificate_with_trailing_data = der.clone();
        certificate_with_trailing_data.extend_from_slice(trailing_data);

        assert_eq!(
            workload_from_peer_certificate(&certificate_with_trailing_data),
            None,
            "identity parsing must consume the entire DER certificate"
        );
    }

    // The X.509 parser returns only bytes following the outer SEQUENCE. It can
    // otherwise ignore unparsed bytes *inside* that SEQUENCE, so exercise that
    // distinct malformed encoding as well.
    assert_eq!(der[0], 0x30, "certificate starts with a SEQUENCE");
    assert_eq!(der[1], 0x82, "test certificate uses a two-byte DER length");
    let outer_content_length = u16::from_be_bytes([der[2], der[3]]);
    assert_eq!(usize::from(outer_content_length) + 4, der.len());
    let extended_content_length = outer_content_length + 3;
    let mut certificate_with_unparsed_sequence_data = der.clone();
    certificate_with_unparsed_sequence_data[2..4]
        .copy_from_slice(&extended_content_length.to_be_bytes());
    certificate_with_unparsed_sequence_data.extend_from_slice(&[0, 0, 0]);

    assert_eq!(
        workload_from_peer_certificate(&certificate_with_unparsed_sequence_data),
        None,
        "identity parsing must reject unparsed bytes inside the certificate SEQUENCE"
    );
}

/// Decode a PEM certificate file written by rcgen.
fn read_der(path: &Path) -> Vec<u8> {
    let pem = std::fs::read(path).expect("read pem");
    let (_, block) = x509_parser::pem::parse_x509_pem(&pem).expect("parse pem");
    block.contents
}

#[tokio::test]
async fn a_trusted_client_completes_a_mutual_handshake() -> anyhow::Result<()> {
    let pki = TestPki::generate("mutual")?;
    let server_leaf = pki.issue(Workload::Profile, "profile")?;
    let client_leaf = pki.issue(Workload::Gateway, "gateway")?;
    let (address, task) = start_tls_server(server_leaf.material()?).await?;

    assert!(
        reached_service(&client_leaf.material()?, address, Workload::Profile).await,
        "a trusted client must complete the mutual handshake and reach the server"
    );

    task.abort();
    Ok(())
}

#[tokio::test]
async fn a_client_trusting_the_ca_but_presenting_no_identity_is_rejected() -> anyhow::Result<()> {
    let pki = TestPki::generate("no-client-cert")?;
    let server_leaf = pki.issue(Workload::Profile, "profile")?;
    let (address, task) = start_tls_server(server_leaf.material()?).await?;

    // Trust the CA but present no client identity. The server requires a client
    // certificate, so the handshake must fail even though the CA is trusted.
    // This is the condition F-03 described before the change existed, when every
    // internal call was plaintext and unauthenticated.
    let ca_pem = std::fs::read(&server_leaf.ca_path)?;
    let endpoint = tonic::transport::Endpoint::from_shared(format!("https://{address}"))?
        .tls_config(
            tonic::transport::ClientTlsConfig::new()
                .ca_certificate(tonic::transport::Certificate::from_pem(&ca_pem))
                .domain_name("profile.svc.internal"),
        )?;
    assert!(
        !call_endpoint(endpoint).await,
        "a client with no certificate must be refused by the transport, not served"
    );

    task.abort();
    Ok(())
}

/// Drive one probe call with a fully specified endpoint config.
async fn call_endpoint(endpoint: tonic::transport::Endpoint) -> bool {
    let Ok(channel) = endpoint.connect().await else {
        return false;
    };
    let mut client = tonic::client::Grpc::new(channel);
    if client.ready().await.is_err() {
        return false;
    }
    match client
        .unary(
            tonic::Request::new(ProbeRequest {}),
            http::uri::PathAndQuery::from_static("/internal.probe.v1.Probe/Check"),
            ProbeCodec::default(),
        )
        .await
    {
        Ok(_) => true,
        Err(status) => status.code() == tonic::Code::Unimplemented,
    }
}

#[tokio::test]
async fn a_client_from_an_untrusted_ca_cannot_complete_a_call() -> anyhow::Result<()> {
    let server_pki = TestPki::generate("server")?;
    let server_leaf = server_pki.issue(Workload::Profile, "profile")?;
    let (address, task) = start_tls_server(server_leaf.material()?).await?;

    // A different CA issues this client, so the server must reject the chain.
    let foreign_pki = TestPki::generate("foreign")?;
    let foreign_leaf = foreign_pki.issue(Workload::Gateway, "gateway")?;
    assert!(
        !reached_service(&foreign_leaf.material()?, address, Workload::Profile).await,
        "a chain from an unrelated CA must be rejected"
    );

    task.abort();
    Ok(())
}

#[tokio::test]
async fn an_expired_workload_certificate_is_rejected_and_a_renewed_one_recovers(
) -> anyhow::Result<()> {
    let pki = TestPki::generate("expired-client")?;
    let server_leaf = pki.issue(Workload::Profile, "profile")?;
    let expired_client = pki.issue_expired(Workload::Gateway, "expired-gateway")?;
    let renewed_client = pki.issue(Workload::Gateway, "renewed-gateway")?;
    let (address, task) = start_tls_server(server_leaf.material()?).await?;

    assert!(
        !reached_service(&expired_client.material()?, address, Workload::Profile).await,
        "a trusted but expired workload certificate must not complete mTLS"
    );
    assert!(
        reached_service(&renewed_client.material()?, address, Workload::Profile).await,
        "a renewed certificate from the trusted CA must restore the mTLS call"
    );

    task.abort();
    Ok(())
}

#[tokio::test]
async fn overlapping_ca_rotation_accepts_new_workloads_then_rejects_the_retired_root(
) -> anyhow::Result<()> {
    let old_pki = TestPki::generate("rotation-old")?;
    let new_pki = TestPki::generate("rotation-new")?;
    let old_server = old_pki.issue(Workload::Profile, "old-profile")?;
    let old_gateway = old_pki.issue(Workload::Gateway, "old-gateway")?;
    let new_server = new_pki.issue(Workload::Profile, "new-profile")?;
    let new_gateway = new_pki.issue(Workload::Gateway, "new-gateway")?;
    let overlapping_roots = format!("{}\n{}", old_pki.ca_pem, new_pki.ca_pem);

    // Phase 1: receivers trust both roots before any workload switches. A new
    // caller can now join the mesh while old callers remain online.
    let (old_address, old_task) =
        start_tls_server(old_server.material_trusting(overlapping_roots.as_bytes())?).await?;
    let old_gateway_during_overlap = old_gateway.material()?;
    let new_gateway_during_overlap = new_gateway.material_trusting(old_pki.ca_pem.as_bytes())?;
    assert!(
        reached_service(&old_gateway_during_overlap, old_address, Workload::Profile).await,
        "the old workload must remain accepted during CA overlap"
    );
    assert!(
        reached_service(&new_gateway_during_overlap, old_address, Workload::Profile).await,
        "a replacement workload must be accepted before the old root is retired"
    );
    old_task.abort();

    // Phase 2: after every caller has moved, the receiver presents a new leaf
    // and trusts only the new root. The old identity must fail even though its
    // certificate has not expired; the new identity must continue to work.
    let (new_address, new_task) = start_tls_server(new_server.material()?).await?;
    let retired_gateway = new_gateway.material()?;
    let old_gateway_after_retirement = old_gateway.material_trusting(new_pki.ca_pem.as_bytes())?;
    assert!(
        !reached_service(
            &old_gateway_after_retirement,
            new_address,
            Workload::Profile
        )
        .await,
        "a still-valid certificate chaining to the retired root must be rejected"
    );
    assert!(
        reached_service(&retired_gateway, new_address, Workload::Profile).await,
        "the replacement workload must remain accepted after old-root retirement"
    );
    new_task.abort();
    Ok(())
}

#[tokio::test]
async fn a_client_expecting_the_wrong_peer_name_is_rejected() -> anyhow::Result<()> {
    let pki = TestPki::generate("wrong-name")?;
    let server_leaf = pki.issue(Workload::Profile, "profile")?;
    let client_leaf = pki.issue(Workload::Gateway, "gateway")?;
    let (address, task) = start_tls_server(server_leaf.material()?).await?;

    // Trust the CA but expect the name of a different peer. Without the DNS SAN
    // this check could not exist, which is why identity and name are separate.
    let material = client_leaf.material()?;
    assert!(
        !reached_service(&material, address, Workload::Friend).await,
        "a certificate naming another peer must not be accepted"
    );

    task.abort();
    Ok(())
}

#[test]
fn a_certificate_without_a_workload_uri_names_no_workload() {
    let params = CertificateParams::new(vec!["localhost".to_owned()]).expect("params");
    let key = KeyPair::generate().expect("key");
    let cert = params.self_signed(&key).expect("self signed");
    // No URI SAN means no workload identity. A server must treat that as
    // unauthenticated rather than as an anonymous-but-allowed peer.
    assert_eq!(workload_from_peer_certificate(cert.der().as_ref()), None);
}

#[tokio::test]
async fn readiness_probes_verify_peer_name_and_bind_credentials_to_the_client_certificate(
) -> anyhow::Result<()> {
    use platform::edge::Edge;
    use platform::grpc_auth::CallerRegistry;
    use platform::readiness::{Probe, Readiness};
    use platform::shutdown::Shutdown;
    use std::time::Duration;

    let pki = TestPki::generate("readiness")?;
    let server_leaf = pki.issue(Workload::Profile, "profile")?;
    let gateway_leaf = pki.issue(Workload::Gateway, "gateway")?;
    let friend_leaf = pki.issue(Workload::Friend, "friend")?;
    let unnamed_leaf = pki.issue_with_uris(Workload::Gateway, "unnamed", &[])?;
    let ambiguous_leaf = pki.issue_with_uris(
        Workload::Gateway,
        "ambiguous",
        &[
            workload_uri(Workload::Gateway),
            workload_uri(Workload::Friend),
        ],
    )?;
    let make_readiness = || {
        Readiness::with_shutdown(
            Probe::new("database", true, Duration::from_millis(100), || async {
                true
            }),
            Shutdown::new(),
        )
    };
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("https://{}", listener.local_addr()?);
    let shutdown = Shutdown::new();
    let stop = shutdown.cancelled();
    let server = Server::builder()
        .tls_config(server_leaf.material()?.server_config()?)?
        .add_service(
            make_readiness()
                .grpc_service(CallerRegistry::new().register(Workload::Gateway, "gateway-test")?)?,
        );
    let task = tokio::spawn(server.serve_with_incoming_shutdown(
        tokio_stream::wrappers::TcpListenerStream::new(listener),
        stop,
    ));

    let report = make_readiness()
        .with_dependencies(vec![Probe::grpc(
            Edge::GatewayProfile,
            &endpoint,
            "gateway-test",
            Some(&gateway_leaf.material()?),
        )?])?
        .report()
        .await;
    assert!(
        report.ready,
        "the matching mTLS identity and credential must work"
    );

    let wrong_name = make_readiness()
        .with_dependencies(vec![Probe::grpc(
            Edge::GatewayFriend,
            &endpoint,
            "gateway-test",
            Some(&gateway_leaf.material()?),
        )?])?
        .report()
        .await;
    assert!(
        !wrong_name.ready,
        "a profile certificate cannot identify the friend peer"
    );

    let wrong_caller = make_readiness()
        .with_dependencies(vec![Probe::grpc(
            Edge::GatewayProfile,
            &endpoint,
            "gateway-test",
            Some(&friend_leaf.material()?),
        )?])?
        .report()
        .await;
    assert!(
        !wrong_caller.ready,
        "a friend certificate cannot use the gateway credential"
    );

    for leaf in [unnamed_leaf, ambiguous_leaf] {
        let report = make_readiness()
            .with_dependencies(vec![Probe::grpc(
                Edge::GatewayProfile,
                &endpoint,
                "gateway-test",
                Some(&leaf.material()?),
            )?])?
            .report()
            .await;
        assert!(
            !report.ready,
            "a missing or ambiguous workload SAN must be rejected"
        );
    }

    shutdown.request();
    tokio::time::timeout(Duration::from_secs(2), task).await???;
    Ok(())
}

#[tokio::test]
async fn previous_workload_credential_can_be_removed_after_a_live_overlap() -> anyhow::Result<()> {
    use platform::grpc_auth::CallerRegistry;

    let pki = TestPki::generate("credential-rotation")?;
    let server_leaf = pki.issue(Workload::Profile, "profile")?;
    let gateway_leaf = pki.issue(Workload::Gateway, "gateway")?;
    let server_material = server_leaf.material()?;
    let gateway_material = gateway_leaf.material()?;
    let overlapping_registry = CallerRegistry::new().register_with_previous(
        Workload::Gateway,
        "gateway-new-credential",
        Some("gateway-old-credential"),
    )?;

    assert!(
        readiness_accepts_gateway_credential(
            &server_material,
            overlapping_registry.clone(),
            &gateway_material,
            "gateway-old-credential",
        )
        .await?,
        "old senders must remain accepted while receivers are rolled"
    );
    assert!(
        readiness_accepts_gateway_credential(
            &server_material,
            overlapping_registry,
            &gateway_material,
            "gateway-new-credential",
        )
        .await?,
        "new senders must work before the old credential is retired"
    );

    let retired_registry =
        CallerRegistry::new().register(Workload::Gateway, "gateway-new-credential")?;
    assert!(
        !readiness_accepts_gateway_credential(
            &server_material,
            retired_registry.clone(),
            &gateway_material,
            "gateway-old-credential",
        )
        .await?,
        "removing the previous credential must revoke the old value"
    );
    assert!(
        readiness_accepts_gateway_credential(
            &server_material,
            retired_registry,
            &gateway_material,
            "gateway-new-credential",
        )
        .await?,
        "retiring the old credential must not interrupt updated senders"
    );
    Ok(())
}

#[tokio::test]
async fn emergency_workload_replacement_retires_both_the_compromised_leaf_and_credential(
) -> anyhow::Result<()> {
    use platform::grpc_auth::CallerRegistry;

    let compromised_pki = TestPki::generate("emergency-old")?;
    let replacement_pki = TestPki::generate("emergency-new")?;
    let compromised_profile = compromised_pki.issue(Workload::Profile, "old-profile")?;
    let replacement_profile = replacement_pki.issue(Workload::Profile, "new-profile")?;
    let compromised_gateway = compromised_pki.issue(Workload::Gateway, "old-gateway")?;
    let replacement_gateway = replacement_pki.issue(Workload::Gateway, "new-gateway")?;
    let overlapping_roots = format!("{}\n{}", compromised_pki.ca_pem, replacement_pki.ca_pem);

    // Receivers first accept both roots and both Gateway credentials. Both the
    // old sender and its replacement can make real RPCs during the rollout.
    let overlap_receiver = compromised_profile.material_trusting(overlapping_roots.as_bytes())?;
    let overlap_registry = CallerRegistry::new().register_with_previous(
        Workload::Gateway,
        "gateway-emergency-new",
        Some("gateway-compromised"),
    )?;
    let old_sender = compromised_gateway.material_trusting(overlapping_roots.as_bytes())?;
    let new_sender = replacement_gateway.material_trusting(overlapping_roots.as_bytes())?;

    assert!(
        readiness_accepts_gateway_credential(
            &overlap_receiver,
            overlap_registry.clone(),
            &old_sender,
            "gateway-compromised",
        )
        .await?,
        "the in-flight old sender must remain available during the bounded overlap"
    );
    assert!(
        readiness_accepts_gateway_credential(
            &overlap_receiver,
            overlap_registry,
            &new_sender,
            "gateway-emergency-new",
        )
        .await?,
        "the replacement sender must work before the compromised identity is retired"
    );

    // Retirement removes the compromised root and previous credential. Probe
    // each factor independently so one revoked factor cannot mask the other.
    let retired_receiver = replacement_profile.material()?;
    let retired_registry =
        CallerRegistry::new().register(Workload::Gateway, "gateway-emergency-new")?;
    let old_leaf_with_new_server_trust =
        compromised_gateway.material_trusting(replacement_pki.ca_pem.as_bytes())?;

    assert!(
        !readiness_accepts_gateway_credential(
            &retired_receiver,
            retired_registry.clone(),
            &new_sender,
            "gateway-compromised",
        )
        .await?,
        "the compromised credential must fail even when presented with the replacement leaf"
    );
    assert!(
        !readiness_accepts_gateway_credential(
            &retired_receiver,
            retired_registry.clone(),
            &old_leaf_with_new_server_trust,
            "gateway-emergency-new",
        )
        .await?,
        "the compromised leaf must fail even when presented with the replacement credential"
    );
    assert!(
        readiness_accepts_gateway_credential(
            &retired_receiver,
            retired_registry,
            &new_sender,
            "gateway-emergency-new",
        )
        .await?,
        "the replacement leaf and credential must continue to work after retirement"
    );

    Ok(())
}

/// Counts requests that reach the generated service, i.e. potential effects.
#[derive(Clone)]
struct Counted<S> {
    inner: S,
    effects: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl<S: tonic::server::NamedService> tonic::server::NamedService for Counted<S> {
    const NAME: &'static str = S::NAME;
}

impl<S, R> tower::Service<R> for Counted<S>
where
    S: tower::Service<R>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(
        &mut self,
        context: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(context)
    }

    fn call(&mut self, request: R) -> Self::Future {
        self.effects
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.inner.call(request)
    }
}

static MATRIX_POLICY: platform::grpc_method_policy::MethodPolicy =
    platform::grpc_method_policy::MethodPolicy {
        service: "grpc.health.v1.Health",
        workload: Some(Workload::Profile),
        methods: &[("Check", &[Workload::Gateway]), ("Watch", &[])],
        actor_fields: &[],
    };

/// T01.3 zero-effect matrix over real mutual TLS on TCP: only the listed
/// workload, presenting its own certificate and credential, reaches the
/// service; every other combination is rejected before any effect.
#[tokio::test]
async fn method_policy_matrix_over_mutual_tls_has_zero_effect_when_denied() -> anyhow::Result<()> {
    use platform::grpc_auth::{CallerRegistry, ServiceIdentityClientInterceptor};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tonic::Code;
    use tonic_health::pb::{health_client::HealthClient, HealthCheckRequest};

    let pki = TestPki::generate("method-policy")?;
    let server_leaf = pki.issue(Workload::Profile, "profile")?;
    let gateway_leaf = pki.issue(Workload::Gateway, "gateway")?;
    let friend_leaf = pki.issue(Workload::Friend, "friend")?;
    let registry = CallerRegistry::new()
        .register(Workload::Gateway, "gateway-matrix")?
        .register(Workload::Friend, "friend-matrix")?;
    let effects = Arc::new(AtomicUsize::new(0));
    let (_reporter, health) = tonic_health::server::health_reporter();
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(
        Server::builder()
            .tls_config(server_leaf.material()?.server_config()?)?
            .add_service(platform::grpc_method_policy::guard(
                &MATRIX_POLICY,
                Counted {
                    inner: health,
                    effects: effects.clone(),
                },
                registry,
                platform::actor_delegation::ActorDelegationVerifier::local_parity(
                    Workload::Profile,
                ),
            )?)
            .serve_with_incoming_shutdown(
                tokio_stream::wrappers::TcpListenerStream::new(listener),
                async {
                    let _ = stopped.await;
                },
            ),
    );

    struct MatrixCase<'a> {
        leaf: &'a LeafFiles,
        identity: Option<(Workload, &'static str)>,
        watch: bool,
        expected: Result<(), Code>,
        label: &'static str,
    }
    let case = |leaf, identity, watch, expected, label| MatrixCase {
        leaf,
        identity,
        watch,
        expected,
        label,
    };
    let cases = [
        case(
            &gateway_leaf,
            Some((Workload::Gateway, "gateway-matrix")),
            false,
            Ok(()),
            "allowed workload",
        ),
        case(
            &friend_leaf,
            Some((Workload::Friend, "friend-matrix")),
            false,
            Err(Code::PermissionDenied),
            "authenticated sibling not listed",
        ),
        case(
            &friend_leaf,
            Some((Workload::Gateway, "gateway-matrix")),
            false,
            Err(Code::Unauthenticated),
            "gateway credential on a friend certificate",
        ),
        case(
            &gateway_leaf,
            None,
            false,
            Err(Code::Unauthenticated),
            "missing identity",
        ),
        case(
            &gateway_leaf,
            Some((Workload::Notification, "gateway-matrix")),
            false,
            Err(Code::Unauthenticated),
            "unregistered workload",
        ),
        case(
            &gateway_leaf,
            Some((Workload::Gateway, "gateway-matrix")),
            true,
            Err(Code::PermissionDenied),
            "method with no listed caller",
        ),
    ];
    for MatrixCase {
        leaf,
        identity,
        watch,
        expected,
        label,
    } in cases
    {
        let before = effects.load(Ordering::SeqCst);
        let channel = leaf
            .material()?
            .endpoint(&format!("https://{address}"), Workload::Profile)?
            .connect()
            .await?;
        let request = HealthCheckRequest {
            service: String::new(),
        };
        let outcome = match identity {
            Some((workload, token)) => {
                let mut client = HealthClient::with_interceptor(
                    channel,
                    ServiceIdentityClientInterceptor::new(workload, token)?,
                );
                if watch {
                    client.watch(request).await.map(|_| ())
                } else {
                    client.check(request).await.map(|_| ())
                }
            }
            None => HealthClient::new(channel).check(request).await.map(|_| ()),
        };
        let outcome = outcome.map_err(|status| status.code());
        let reached = effects.load(Ordering::SeqCst) - before;
        assert_eq!(outcome, expected, "{label}");
        assert_eq!(
            reached,
            usize::from(expected.is_ok()),
            "{label}: service effects"
        );
    }

    let _ = stop.send(());
    server.await??;
    Ok(())
}
