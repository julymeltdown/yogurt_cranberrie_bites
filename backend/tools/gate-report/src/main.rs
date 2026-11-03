//! Command line interface for producing and checking gate reports.
//!
//! Two jobs, deliberately kept apart:
//!
//! * The emit mode runs where the signing key is available, and writes the
//!   sealed record of what was verified.
//! * The verify mode runs where only the verification key is available, and
//!   decides whether a candidate may merge.
//!
//! The merge check must not need the signing key, or every job that can read a
//! pull request could also mint a passing result.

use anyhow::{bail, Context};
use gate_report::{
    self, GateReport, GateStatus, SignedGateReport, KEY_ENV, KEY_ID_ENV, SCHEMA_VERSION,
};
use std::collections::BTreeMap;
use std::process::ExitCode;

const USAGE: &str = "\
usage:
  gate-report emit   --candidate-sha <sha> --base-sha <sha> --gates <json|@file> [--durations <json|@file>] [--output <path>]
  gate-report verify --report <path> [--expect-candidate-sha <sha>] [--expect-base-sha <sha>] [--allow-missing-key]

emit reads GATE_REPORT_KEY (and optional GATE_REPORT_KEY_ID) from the
environment and writes a signed report, recording the host it ran on and the
duration of each gate. verify decides mergeability and exits non-zero when the
candidate must not merge.
";

fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::from(2)
        }
    }
}

fn run(args: Vec<String>) -> anyhow::Result<ExitCode> {
    match args.first().map(String::as_str) {
        Some("emit") => emit(&args[1..]),
        Some("verify") => verify(&args[1..]),
        _ => {
            eprint!("{USAGE}");
            Ok(ExitCode::from(2))
        }
    }
}

/// Minimal flag reader. A present flag must carry a value, so a typo fails
/// instead of silently defaulting a field the report is supposed to name.
fn flag(args: &[String], name: &str) -> anyhow::Result<Option<String>> {
    match args.iter().position(|arg| arg == name) {
        Some(index) => args
            .get(index + 1)
            .filter(|value| !value.starts_with("--"))
            .cloned()
            .map(Some)
            .with_context(|| format!("{name} requires a value")),
        None => Ok(None),
    }
}

fn required_flag(args: &[String], name: &str) -> anyhow::Result<String> {
    flag(args, name)?.with_context(|| format!("{name} is required"))
}

fn has_flag(args: &[String], name: &str) -> bool {
    args.iter().any(|arg| arg == name)
}

fn ensure_expected_revision(
    kind: &str,
    actual: &str,
    expected: Option<String>,
) -> anyhow::Result<()> {
    if let Some(expected) = expected {
        if actual != expected {
            if kind == "candidate" {
                bail!("the report verifies {actual}, not the candidate under review ({expected})");
            }
            bail!("the report is based on {actual}, not the current {kind} ({expected})");
        }
    }
    Ok(())
}

/// Read gate statuses from inline JSON or, with a leading at sign, from a file.
fn read_gates(value: &str) -> anyhow::Result<BTreeMap<String, GateStatus>> {
    let raw = match value.strip_prefix('@') {
        Some(path) => std::fs::read_to_string(path)
            .with_context(|| format!("reading gate results from {path}"))?,
        None => value.to_owned(),
    };
    let gates: BTreeMap<String, GateStatus> =
        serde_json::from_str(&raw).context("parsing the gate results")?;
    if gates.is_empty() {
        bail!("a report with no gates verifies nothing");
    }
    Ok(gates)
}

/// Run the current compiler version, so a result can be tied to a toolchain.
fn toolchain() -> String {
    std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_else(|| "unknown".to_owned())
}

fn emit(args: &[String]) -> anyhow::Result<ExitCode> {
    let key = std::env::var(KEY_ENV).unwrap_or_default();
    if key.is_empty() {
        bail!(
            "{KEY_ENV} is not set; the report must be signed by a key outside repository content"
        );
    }
    let key_id = std::env::var(KEY_ID_ENV).unwrap_or_else(|_| "default".to_owned());

    // Validate the substance of the report before anything about the
    // environment is touched. A report that asserts nothing should fail for
    // that reason, not for how the job happened to be launched.
    let gates = read_gates(&required_flag(args, "--gates")?)?;

    let working_directory = std::env::current_dir().context("determining the working directory")?;
    let lockfile = GateReport::discover_lockfile(&working_directory)
        .context("locating Cargo.lock from the working directory")?;
    let lockfile_sha256 = gate_report::sha256_file(&lockfile)
        .with_context(|| format!("hashing {}", lockfile.display()))?;

    let report = GateReport {
        schema_version: SCHEMA_VERSION,
        candidate_sha: required_flag(args, "--candidate-sha")?,
        base_sha: required_flag(args, "--base-sha")?,
        rust_toolchain: toolchain(),
        lockfile_sha256,
        environment: gate_report::RunEnvironment::current(),
        durations_ms: read_durations(args)?,
        gates,
        exceptions: read_exceptions(args)?,
        verified_at: chrono::Utc::now(),
    };

    let signed = gate_report::sign(report, key.as_bytes(), &key_id)?;
    let encoded = serde_json::to_string_pretty(&signed)?;
    match flag(args, "--output")? {
        Some(path) => {
            std::fs::write(&path, format!("{encoded}\n"))
                .with_context(|| format!("writing the report to {path}"))?;
            eprintln!(
                "wrote a signed gate report for {} to {path}",
                signed.report.candidate_sha
            );
        }
        None => println!("{encoded}"),
    }
    Ok(ExitCode::SUCCESS)
}

/// Read per-gate wall times, in the same inline-or-file form as the gates.
///
/// Optional on purpose: the outcome is what the merge rule consumes, and a report
/// that recorded only outcomes is still valid evidence. A malformed durations
/// payload is an error rather than a silent empty map, because quietly dropping
/// it would produce a report that looks complete and is not.
fn read_durations(args: &[String]) -> anyhow::Result<BTreeMap<String, u64>> {
    match flag(args, "--durations")? {
        Some(value) => {
            let raw = match value.strip_prefix('@') {
                Some(path) => std::fs::read_to_string(path)
                    .with_context(|| format!("reading gate durations from {path}"))?,
                None => value,
            };
            serde_json::from_str(&raw).context("parsing the gate durations")
        }
        None => Ok(BTreeMap::new()),
    }
}

fn read_exceptions(args: &[String]) -> anyhow::Result<Vec<gate_report::DocumentedException>> {
    match flag(args, "--exceptions")? {
        Some(value) => {
            let raw = match value.strip_prefix('@') {
                Some(path) => std::fs::read_to_string(path)
                    .with_context(|| format!("reading exceptions from {path}"))?,
                None => value,
            };
            serde_json::from_str(&raw).context("parsing the documented exceptions")
        }
        None => Ok(Vec::new()),
    }
}

fn verify(args: &[String]) -> anyhow::Result<ExitCode> {
    let path = required_flag(args, "--report")?;
    let raw = std::fs::read_to_string(&path).with_context(|| format!("reading {path}"))?;
    let signed: SignedGateReport = serde_json::from_str(&raw).context("parsing the gate report")?;

    let key = std::env::var(KEY_ENV).unwrap_or_default();
    if key.is_empty() {
        // A caller that legitimately cannot see the key must say so explicitly,
        // otherwise an unverified report would look the same as a checked one.
        if !has_flag(args, "--allow-missing-key") {
            bail!("{KEY_ENV} is not set; pass --allow-missing-key to check without verifying");
        }
        eprintln!("warning: {KEY_ENV} is not set, so the report signature was not checked");
    } else {
        gate_report::verify(&signed, key.as_bytes())?;
        eprintln!("signature: valid (key id {})", signed.key_id);
    }

    ensure_expected_revision(
        "candidate",
        &signed.report.candidate_sha,
        flag(args, "--expect-candidate-sha")?,
    )?;
    ensure_expected_revision(
        "base",
        &signed.report.base_sha,
        flag(args, "--expect-base-sha")?,
    )?;

    let report = &signed.report;
    // Check the format before trusting any field in it.
    report.ensure_supported_schema()?;
    eprintln!(
        "candidate={} base={} toolchain={} lockfile={}",
        report.candidate_sha,
        report.base_sha,
        report.rust_toolchain,
        &report.lockfile_sha256[..12.min(report.lockfile_sha256.len())],
    );
    let environment = &report.environment;
    eprintln!(
        "environment={}/{} cpus={} memory_bytes={}",
        environment.platform,
        environment.arch,
        environment
            .cpu_count
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".to_owned()),
        environment
            .memory_bytes
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".to_owned()),
    );
    for (name, status) in &report.gates {
        let rendered = match status {
            GateStatus::Pass => "PASS".to_owned(),
            GateStatus::Fail { detail } => format!("FAIL: {detail}"),
            GateStatus::Blocked { reason } => format!("BLOCKED: {reason}"),
            GateStatus::NotRun { reason } => format!("NOT-RUN: {reason}"),
        };
        match report.durations_ms.get(name) {
            Some(milliseconds) => eprintln!("  {name:24} {rendered} ({milliseconds} ms)"),
            None => eprintln!("  {name:24} {rendered}"),
        }
    }

    let now = chrono::Utc::now();
    for exception in report.expired_exceptions(now) {
        eprintln!(
            "  exception for {} expired at {} (owner {})",
            exception.gate, exception.expires_at, exception.owner
        );
    }

    if report.is_mergeable_at(now) {
        println!("MERGEABLE candidate={}", report.candidate_sha);
        return Ok(ExitCode::SUCCESS);
    }

    let unsatisfied = report.unsatisfied_gates_at(now);
    bail!(
        "NOT MERGEABLE; gates not satisfied: {}",
        unsatisfied.join(", ")
    );
}

#[cfg(test)]
mod tests {
    use super::ensure_expected_revision;

    #[test]
    fn report_revision_must_match_when_an_expected_sha_is_given() {
        assert!(ensure_expected_revision("base", "base-sha", Some("base-sha".into())).is_ok());

        let error = ensure_expected_revision("base", "old-base", Some("current-base".into()))
            .expect_err("a stale base must be rejected");
        assert!(error.to_string().contains("current base (current-base)"));
    }

    #[test]
    fn report_revision_check_is_optional_without_an_expected_sha() {
        assert!(ensure_expected_revision("candidate", "candidate-sha", None).is_ok());
    }
}
