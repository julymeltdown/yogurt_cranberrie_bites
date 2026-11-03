//! Generated internal-TLS material for tests that must start in production mode.
//!
//! Production refuses to load configuration without internal gRPC TLS material, so
//! any test exercising production validation needs real certificates. Generating
//! them here rather than disabling the requirement keeps the security property
//! under test: a test that asserts "this bad database URL is rejected" must still
//! fail for the database URL, not because certificates were missing.
//!
//! The generated authority is process-wide and deliberately never dropped: the
//! files must outlive every test that points environment variables at them.

use anyhow::Context;
use rcgen::{
    BasicConstraints, CertificateParams, DnType, ExtendedKeyUsagePurpose, IsCa, KeyPair,
    KeyUsagePurpose,
};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// URI prefix used for the workload identity half of a certificate.
const WORKLOAD_URI_PREFIX: &str = "spiffe://internal/";
/// DNS suffix used for the server-name half of a certificate.
const WORKLOAD_DNS_SUFFIX: &str = ".svc.internal";

/// A generated certificate authority, created once per test process.
pub struct TestPki {
    ca_pem: String,
    ca_key: KeyPair,
    ca_params: CertificateParams,
    directory: PathBuf,
}

/// File paths for one issued leaf certificate.
#[derive(Clone, Debug)]
pub struct LeafPaths {
    pub cert: PathBuf,
    pub key: PathBuf,
    pub ca: PathBuf,
}

impl LeafPaths {
    /// The three environment variable values a service needs.
    pub fn env_values(&self) -> [String; 3] {
        [
            self.cert.display().to_string(),
            self.key.display().to_string(),
            self.ca.display().to_string(),
        ]
    }
}

impl TestPki {
    fn generate() -> anyhow::Result<Self> {
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
            std::env::temp_dir().join(format!("test-support-pki-{}", std::process::id()));
        std::fs::create_dir_all(&directory)?;

        Ok(Self {
            ca_pem: ca_cert.pem(),
            ca_key,
            ca_params,
            directory,
        })
    }

    /// Issue a leaf carrying a workload URI SAN and a DNS SAN.
    ///
    /// Both SANs are required: rustls verifies server names against DNS or IP
    /// SANs and ignores URI SANs, so identity and name verification cannot share
    /// one SAN.
    pub fn issue(&self, role: &str, stem: &str) -> anyhow::Result<LeafPaths> {
        let mut params = CertificateParams::new(Vec::<String>::new())?;
        params.distinguished_name.push(DnType::CommonName, role);
        params.is_ca = IsCa::NoCa;
        params.key_usages = vec![
            KeyUsagePurpose::DigitalSignature,
            KeyUsagePurpose::KeyEncipherment,
        ];
        params.extended_key_usages = vec![
            ExtendedKeyUsagePurpose::ServerAuth,
            ExtendedKeyUsagePurpose::ClientAuth,
        ];
        params.subject_alt_names = vec![
            rcgen::SanType::DnsName(format!("{role}{WORKLOAD_DNS_SUFFIX}").try_into()?),
            rcgen::SanType::URI(format!("{WORKLOAD_URI_PREFIX}{role}").try_into()?),
        ];

        let key = KeyPair::generate()?;
        let cert = params.signed_by(
            &key,
            &rcgen::Issuer::from_params(&self.ca_params, &self.ca_key),
        )?;

        let cert_path = self.directory.join(format!("{stem}-cert.pem"));
        let key_path = self.directory.join(format!("{stem}-key.pem"));
        let ca_path = self.directory.join("ca.pem");
        std::fs::write(&cert_path, cert.pem())?;
        std::fs::write(&key_path, key.serialize_pem())?;
        std::fs::write(&ca_path, &self.ca_pem)?;

        Ok(LeafPaths {
            cert: cert_path,
            key: key_path,
            ca: ca_path,
        })
    }
}

fn shared_pki() -> &'static TestPki {
    static PKI: OnceLock<TestPki> = OnceLock::new();
    PKI.get_or_init(|| TestPki::generate().expect("test PKI must generate"))
}

/// Issue a leaf for `role` from the process-wide authority.
pub fn issue(role: &str) -> anyhow::Result<LeafPaths> {
    shared_pki().issue(role, role)
}

/// The environment variable names a service reads for its TLS material.
pub fn env_names(service_prefix: &str) -> [String; 3] {
    let prefix = service_prefix.trim().to_ascii_uppercase();
    [
        format!("{prefix}_GRPC_TLS_CERT_PATH"),
        format!("{prefix}_GRPC_TLS_KEY_PATH"),
        format!("{prefix}_GRPC_TLS_CA_PATH"),
    ]
}

/// Point one service's TLS environment at a generated leaf and return the names.
///
/// Returns the variable names so a caller's environment guard can clear them.
pub fn apply_env(service_prefix: &str, role: &str) -> anyhow::Result<[String; 3]> {
    let names = env_names(service_prefix);
    let leaf = issue(role).with_context(|| format!("issuing a test certificate for {role}"))?;
    let values = leaf.env_values();
    for (name, value) in names.iter().zip(values.iter()) {
        std::env::set_var(name, value);
    }
    Ok(names)
}

/// Read a path back as a string, for assertions.
pub fn path_string(path: &Path) -> anyhow::Result<String> {
    Ok(path
        .to_str()
        .context("generated certificate path must be valid UTF-8")?
        .to_owned())
}
