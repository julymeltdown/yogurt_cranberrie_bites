//! Verifiable gate results for one exact revision.
//!
//! A gate result is only useful if it can be tied to the revision it checked.
//! A report that says "green" without naming the candidate SHA and the lockfile
//! digest that were verified is unfalsifiable, and therefore worthless as merge
//! evidence: nobody can tell which tree it described.
//!
//! This crate defines that record, seals it with a key held only in CI secret
//! storage, and answers the one question a merge check needs to ask. The key
//! never lives in repository content, so a candidate revision cannot mint a
//! passing result for itself.

use anyhow::bail;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

type HmacSha256 = Hmac<Sha256>;

/// Version of the on-disk report format.
///
/// Raised to 2 when the run environment and per-gate durations were added. A
/// reader rejects any other version rather than guessing at a shape it does not
/// know, because a report that is misread is worse than one that is refused: the
/// merge decision would rest on fields the reader assumed rather than read.
pub const SCHEMA_VERSION: u32 = 2;

/// Environment variable holding the signing key.
pub const KEY_ENV: &str = "GATE_REPORT_KEY";

/// Environment variable naming the signing key, for key rotation.
pub const KEY_ID_ENV: &str = "GATE_REPORT_KEY_ID";

/// Outcome of a single gate.
///
/// The variants are deliberately not reducible to a boolean. "The check failed"
/// and "the check never ran" are different facts, and only the first can be
/// waived by a documented exception.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum GateStatus {
    /// The gate ran and passed.
    Pass,
    /// The gate ran and failed, with the observed detail.
    Fail { detail: String },
    /// The gate could not run because of a known external condition, such as a
    /// missing credential. Distinct from a failure: nothing was learned.
    Blocked { reason: String },
    /// The gate was skipped. This is never a pass.
    NotRun { reason: String },
}

impl GateStatus {
    /// A gate counts as satisfied only when it actually ran and passed.
    pub fn is_pass(&self) -> bool {
        matches!(self, GateStatus::Pass)
    }
}

/// A time-limited, owned disposition for a gate that did not pass.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentedException {
    /// Gate name this exception covers.
    pub gate: String,
    /// Why the failure is acceptable for now, in reviewable terms.
    pub reason: String,
    /// Who is accountable for removing it.
    pub owner: String,
    /// Moment after which the exception stops applying.
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

/// The verifiable record of one gate run against one revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateReport {
    pub schema_version: u32,
    /// Revision whose content was verified.
    pub candidate_sha: String,
    /// Revision the candidate is judged against, so the diff is unambiguous.
    pub base_sha: String,
    /// Compiler that produced the result, since results are toolchain-specific.
    pub rust_toolchain: String,
    /// Digest of the lockfile that was verified, so a later dependency edit
    /// invalidates this evidence.
    pub lockfile_sha256: String,
    /// The machine the gates ran on.
    ///
    /// A gate result is a fact about a machine as much as about a revision: a
    /// suite that passes on a 16-core host and exhausts the disk on a small runner
    /// is not the same evidence. Recording the host is what makes the difference
    /// visible instead of implied.
    pub environment: RunEnvironment,
    /// Wall time each gate took, keyed by gate name.
    ///
    /// Duration is what turns "the tests pass" into "the tests pass in 40 seconds",
    /// which is the difference between a suite that can run before every push and
    /// one that has quietly become too slow to be enforced. Defaults to empty so a
    /// report written before this field existed still deserializes.
    #[serde(default)]
    pub durations_ms: BTreeMap<String, u64>,
    pub gates: BTreeMap<String, GateStatus>,
    #[serde(default)]
    pub exceptions: Vec<DocumentedException>,
    pub verified_at: chrono::DateTime<chrono::Utc>,
}

/// Where a gate run happened.
///
/// Every field is optional-by-absence rather than defaulted: a host fact that
/// could not be read is recorded as missing, because a zero CPU count or a zero
/// memory size is a claim about the machine that would be false. A reader can
/// tell "not reported" from "reported as small".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunEnvironment {
    /// Operating system, as reported by the compiler that built the reporter.
    pub platform: String,
    /// CPU architecture, on the same basis.
    pub arch: String,
    /// Logical CPUs available to the process, when the platform reports it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_count: Option<u32>,
    /// Usable physical memory in bytes, when it can be read without a dependency.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_bytes: Option<u64>,
}

impl RunEnvironment {
    /// Describe the machine this process is running on.
    pub fn current() -> Self {
        Self {
            platform: std::env::consts::OS.to_owned(),
            arch: std::env::consts::ARCH.to_owned(),
            cpu_count: std::thread::available_parallelism()
                .ok()
                .map(|value| value.get() as u32),
            memory_bytes: physical_memory_bytes(),
        }
    }
}

/// Read total physical memory, or report that it could not be read.
///
/// Kept to a platform-gated command rather than a new dependency: the value is
/// descriptive metadata for a human reading the manifest, not an input to any
/// decision, so it does not justify widening the dependency set.
fn physical_memory_bytes() -> Option<u64> {
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("sysctl")
            .args(["-n", "hw.memsize"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        String::from_utf8_lossy(&output.stdout).trim().parse().ok()
    }
    #[cfg(target_os = "linux")]
    {
        let contents = std::fs::read_to_string("/proc/meminfo").ok()?;
        let line = contents
            .lines()
            .find(|line| line.starts_with("MemTotal:"))?;
        // MemTotal is reported in kibibytes.
        let kibibytes: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
        kibibytes.checked_mul(1024)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        None
    }
}

/// A report together with the proof that a key-holding job produced it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedGateReport {
    pub report: GateReport,
    /// Which key signed, so rotation does not invalidate the format.
    pub key_id: String,
    /// Hex-encoded HMAC over the canonical report bytes.
    pub signature: String,
}

impl GateReport {
    /// Refuse a report written in a format this build does not understand.
    ///
    /// A merge decision reads fields by name. A reader that accepted a future
    /// version would silently ignore whatever that version added, and would answer
    /// "mergeable" on the strength of the fields it happened to recognize. Refusing
    /// is the only safe reading of a shape that is not known.
    pub fn ensure_supported_schema(&self) -> anyhow::Result<()> {
        if self.schema_version != SCHEMA_VERSION {
            bail!(
                "the gate report uses schema version {}, but this build reads version {SCHEMA_VERSION}",
                self.schema_version
            );
        }
        Ok(())
    }

    /// Exceptions whose window has already closed.
    ///
    /// These are reported rather than ignored because an expired exception that
    /// still waived a failure would be an unbounded waiver wearing a deadline.
    pub fn expired_exceptions(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Vec<&DocumentedException> {
        self.exceptions
            .iter()
            .filter(|exception| exception.expires_at <= now)
            .collect()
    }

    /// Whether an unexpired exception covers the named gate.
    fn has_valid_exception(&self, gate: &str, now: chrono::DateTime<chrono::Utc>) -> bool {
        self.exceptions
            .iter()
            .any(|exception| exception.gate == gate && exception.expires_at > now)
    }

    /// Whether a single gate is satisfied under the merge rule.
    ///
    /// Kept as the one place that decides satisfaction, so the merge decision
    /// and any diagnostic message about it cannot disagree.
    fn is_satisfied(
        &self,
        gate: &str,
        status: &GateStatus,
        now: chrono::DateTime<chrono::Utc>,
    ) -> bool {
        match status {
            GateStatus::Pass => true,
            GateStatus::Fail { .. } => self.has_valid_exception(gate, now),
            GateStatus::Blocked { .. } | GateStatus::NotRun { .. } => false,
        }
    }

    /// Gates that are not satisfied as of the given moment, in stable order.
    ///
    /// This accounts for exceptions, so it names what actually blocks a merge.
    /// A gate waived by a valid exception is reported as satisfied, not as a
    /// failure the reader has to remember is excused.
    pub fn unsatisfied_gates_at(&self, now: chrono::DateTime<chrono::Utc>) -> Vec<&str> {
        self.gates
            .iter()
            .filter(|(name, status)| !self.is_satisfied(name, status, now))
            .map(|(name, _)| name.as_str())
            .collect()
    }

    /// Gates that are not satisfied as of now.
    pub fn unsatisfied_gates(&self) -> Vec<&str> {
        self.unsatisfied_gates_at(chrono::Utc::now())
    }

    /// The merge decision, evaluated at a caller-supplied time.
    ///
    /// The rule is intentionally asymmetric:
    ///
    /// * [GateStatus::Pass] is satisfied.
    /// * [GateStatus::Fail] is satisfied only by an unexpired, owned exception,
    ///   which is what makes a short-lived waiver legitimate.
    /// * [GateStatus::Blocked] and [GateStatus::NotRun] are never satisfied.
    ///   A check that did not execute produced no evidence, and accepting one
    ///   would let "written but not executed" launder itself into a pass.
    pub fn is_mergeable_at(&self, now: chrono::DateTime<chrono::Utc>) -> bool {
        self.unsatisfied_gates_at(now).is_empty()
    }

    /// Locate the lockfile by walking up from the working directory.
    ///
    /// A job should not have to know its own depth in the tree for its evidence
    /// to be bound to the dependency set it actually built.
    pub fn discover_lockfile(start: &std::path::Path) -> Option<std::path::PathBuf> {
        let mut directory = Some(start);
        while let Some(current) = directory {
            let candidate = current.join("Cargo.lock");
            if candidate.is_file() {
                return Some(candidate);
            }
            directory = current.parent();
        }
        None
    }

    /// The merge decision as of now.
    pub fn is_mergeable(&self) -> bool {
        self.is_mergeable_at(chrono::Utc::now())
    }

    /// Deterministic bytes covered by the signature.
    ///
    /// Serializing the typed value rather than concatenating fields by hand
    /// keeps the signed form and the stored form from drifting apart.
    pub fn canonical_bytes(&self) -> anyhow::Result<Vec<u8>> {
        Ok(serde_json::to_vec(self)?)
    }
}

fn mac(report: &GateReport, key: &[u8]) -> anyhow::Result<HmacSha256> {
    if key.is_empty() {
        bail!("the gate signing key must not be empty");
    }
    let mut mac = HmacSha256::new_from_slice(key)
        .map_err(|_| anyhow::anyhow!("the gate signing key is not usable"))?;
    mac.update(&report.canonical_bytes()?);
    Ok(mac)
}

/// Seal a report with the publishing key.
pub fn sign(report: GateReport, key: &[u8], key_id: &str) -> anyhow::Result<SignedGateReport> {
    let signature = hex_encode(&mac(&report, key)?.finalize().into_bytes());
    Ok(SignedGateReport {
        report,
        key_id: key_id.to_owned(),
        signature,
    })
}

/// Check that a signed report was produced by the holder of the signing key and
/// has not been altered since.
///
/// Comparison is constant-time: a byte-by-byte early return would leak how much
/// of a guessed signature is correct, which is enough to forge one.
pub fn verify(signed: &SignedGateReport, key: &[u8]) -> anyhow::Result<()> {
    let expected = hex_decode(&signed.signature)?;
    mac(&signed.report, key)?
        .verify_slice(&expected)
        .map_err(|_| anyhow::anyhow!("the gate report signature is not valid for this key"))
}

/// SHA-256 of a file, used to bind a report to the resolved dependency set.
pub fn sha256_file(path: &std::path::Path) -> anyhow::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher)?;
    Ok(hex_encode(&hasher.finalize()))
}

/// Lowercase hex encoding.
pub fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(nibble(byte >> 4));
        out.push(nibble(byte & 0x0f));
    }
    out
}

fn nibble(value: u8) -> char {
    match value {
        0..=9 => (b'0' + value) as char,
        _ => (b'a' + value - 10) as char,
    }
}

fn hex_decode(text: &str) -> anyhow::Result<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        bail!("a signature must have an even number of hex digits");
    }
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 2);
    for pair in bytes.chunks(2) {
        out.push((hex_digit(pair[0])? << 4) | hex_digit(pair[1])?);
    }
    Ok(out)
}

fn hex_digit(byte: u8) -> anyhow::Result<u8> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => bail!("a signature contains a non-hex character"),
    }
}
