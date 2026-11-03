//! The merge decision must be conservative in exactly the right places.
//!
//! Each test here pins one clause of the rule that decides whether a candidate
//! can reach the protected branch. The valuable cases are the ones where a
//! plausible implementation would answer "mergeable" and be wrong.

use chrono::{Duration, Utc};
use gate_report::{
    sign, verify, DocumentedException, GateReport, GateStatus, RunEnvironment, SignedGateReport,
    SCHEMA_VERSION,
};
use std::collections::BTreeMap;

fn gates(entries: &[(&str, GateStatus)]) -> BTreeMap<String, GateStatus> {
    entries
        .iter()
        .map(|(name, status)| ((*name).to_owned(), status.clone()))
        .collect()
}

fn report(gates: BTreeMap<String, GateStatus>, exceptions: Vec<DocumentedException>) -> GateReport {
    GateReport {
        schema_version: SCHEMA_VERSION,
        candidate_sha: "candidate".to_owned(),
        base_sha: "base".to_owned(),
        rust_toolchain: "rustc 1.88.0".to_owned(),
        lockfile_sha256: "0".repeat(64),
        environment: RunEnvironment {
            platform: "test-os".to_owned(),
            arch: "test-arch".to_owned(),
            cpu_count: Some(8),
            memory_bytes: Some(16 * 1024 * 1024 * 1024),
        },
        durations_ms: BTreeMap::new(),
        gates,
        exceptions,
        verified_at: Utc::now(),
    }
}

fn exception(gate: &str, expires_in_hours: i64) -> DocumentedException {
    DocumentedException {
        gate: gate.to_owned(),
        reason: "tracked in an incident ticket".to_owned(),
        owner: "platform".to_owned(),
        expires_at: Utc::now() + Duration::hours(expires_in_hours),
    }
}

fn run_verify(path: &std::path::Path, extra: &[&str]) -> std::process::Output {
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_gate-report"));
    command.arg("verify").arg("--report").arg(path);
    command.args(extra);
    command
        .env("GATE_REPORT_KEY", "secret-key")
        .output()
        .expect("run the verifier")
}

fn write_report(name: &str, signed: &SignedGateReport) -> std::path::PathBuf {
    let directory =
        std::env::temp_dir().join(format!("gate-report-{}-{}", std::process::id(), name));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let path = directory.join("report.json");
    std::fs::write(&path, serde_json::to_string(signed).expect("serialize")).expect("write");
    path
}

#[test]
fn a_fully_green_report_is_mergeable() {
    let report = report(
        gates(&[
            ("fmt", GateStatus::Pass),
            ("clippy", GateStatus::Pass),
            ("tests", GateStatus::Pass),
        ]),
        Vec::new(),
    );
    assert!(report.is_mergeable());
    assert!(report.unsatisfied_gates().is_empty());
}

#[test]
fn a_failing_gate_blocks_the_merge() {
    let report = report(
        gates(&[
            ("fmt", GateStatus::Pass),
            (
                "tests",
                GateStatus::Fail {
                    detail: "1 failed".to_owned(),
                },
            ),
        ]),
        Vec::new(),
    );
    assert!(!report.is_mergeable());
    assert_eq!(report.unsatisfied_gates(), vec!["tests"]);
}

#[test]
fn a_skipped_gate_is_not_a_pass() {
    // The specific mistake this guards against: "tests were written but not
    // executed" arriving as green. A gate that never ran produced no evidence,
    // so it must block.
    let report = report(
        gates(&[
            ("fmt", GateStatus::Pass),
            (
                "integration",
                GateStatus::NotRun {
                    reason: "docker unavailable".to_owned(),
                },
            ),
        ]),
        Vec::new(),
    );
    assert!(!report.is_mergeable());
    assert_eq!(report.unsatisfied_gates(), vec!["integration"]);
}

#[test]
fn a_blocked_gate_is_never_waivable_by_an_exception() {
    // A blocked gate is not a failure to waive; it is an absence of evidence.
    // Allowing an exception here would let an unrun gate be papered over.
    let report = report(
        gates(&[(
            "supply-chain",
            GateStatus::Blocked {
                reason: "no network".to_owned(),
            },
        )]),
        vec![exception("supply-chain", 24)],
    );
    assert!(!report.is_mergeable());
}

#[test]
fn an_unexpired_exception_makes_a_failure_mergeable() {
    let report = report(
        gates(&[(
            "coverage",
            GateStatus::Fail {
                detail: "73% against a 75% floor".to_owned(),
            },
        )]),
        vec![exception("coverage", 24)],
    );
    assert!(report.is_mergeable());
}

#[test]
fn an_expired_exception_no_longer_makes_a_failure_mergeable() {
    // Without this, a waiver with a deadline would be an unbounded waiver.
    let report = report(
        gates(&[(
            "coverage",
            GateStatus::Fail {
                detail: "73% against a 75% floor".to_owned(),
            },
        )]),
        vec![exception("coverage", -1)],
    );
    assert!(!report.is_mergeable());
    assert_eq!(report.expired_exceptions(Utc::now()).len(), 1);
}

#[test]
fn an_exception_for_one_gate_does_not_cover_another() {
    let report = report(
        gates(&[
            (
                "coverage",
                GateStatus::Fail {
                    detail: "below floor".to_owned(),
                },
            ),
            (
                "clippy",
                GateStatus::Fail {
                    detail: "2 warnings".to_owned(),
                },
            ),
        ]),
        vec![exception("coverage", 24)],
    );
    assert!(!report.is_mergeable());
    // The excused gate is not listed: the message must name what actually
    // blocks the merge, or a reader has to re-derive the rule by hand.
    assert_eq!(report.unsatisfied_gates(), vec!["clippy"]);
}

#[test]
fn the_emitter_refuses_a_report_that_verifies_nothing() {
    // An empty gate map would otherwise satisfy "every gate passed".
    let emitted = std::process::Command::new(env!("CARGO_BIN_EXE_gate-report"))
        .args([
            "emit",
            "--candidate-sha",
            "abc",
            "--base-sha",
            "def",
            "--gates",
            "{}",
        ])
        .env("GATE_REPORT_KEY", "secret-key")
        .output()
        .expect("run the emitter");
    assert!(
        !emitted.status.success(),
        "emitting an empty gate set must fail rather than assert nothing"
    );
    assert!(
        String::from_utf8_lossy(&emitted.stderr).contains("verifies nothing"),
        "unexpected error: {}",
        String::from_utf8_lossy(&emitted.stderr)
    );
}

#[test]
fn a_signature_is_bound_to_the_report_content() {
    let original = report(gates(&[("tests", GateStatus::Pass)]), Vec::new());
    let mut signed = sign(original, b"secret-key", "2026-q3").expect("sign");
    verify(&signed, b"secret-key").expect("the signature must verify");

    // Altering any verified field must invalidate the seal, otherwise the
    // signature would attest to nothing that matters.
    signed.report.candidate_sha = "someone-elses-commit".to_owned();
    assert!(
        verify(&signed, b"secret-key").is_err(),
        "editing a signed report must not go unnoticed"
    );
}

#[test]
fn a_signature_does_not_verify_under_a_different_key() {
    let signed = sign(
        report(gates(&[("tests", GateStatus::Pass)]), Vec::new()),
        b"the-real-key",
        "2026-q3",
    )
    .expect("sign");
    assert!(
        verify(&signed, b"a-different-key").is_err(),
        "a candidate revision must not be able to mint its own passing result"
    );
}

#[test]
fn an_empty_signing_key_is_rejected() {
    let signed = sign(
        report(gates(&[("tests", GateStatus::Pass)]), Vec::new()),
        b"",
        "2026-q3",
    );
    assert!(
        signed.is_err(),
        "an empty key must not be accepted as a seal"
    );
}

#[test]
fn a_signed_report_round_trips_through_json() {
    // The report is written by one job and read by another, so the serialized
    // form is the interface. If it did not round-trip, verification would be
    // checking a different value than the one that was signed.
    let signed = sign(
        report(
            gates(&[
                ("tests", GateStatus::Pass),
                (
                    "coverage",
                    GateStatus::Fail {
                        detail: "below floor".to_owned(),
                    },
                ),
            ]),
            vec![exception("coverage", 24)],
        ),
        b"secret-key",
        "2026-q3",
    )
    .expect("sign");

    let encoded = serde_json::to_string(&signed).expect("serialize");
    let decoded: SignedGateReport = serde_json::from_str(&encoded).expect("deserialize");
    verify(&decoded, b"secret-key").expect("the round-tripped signature must verify");
    assert_eq!(decoded.report, signed.report);
}

#[test]
fn the_verifier_refuses_a_failing_candidate() {
    // The exit code is the interface the merge job actually consumes.
    let signed = sign(
        report(
            gates(&[(
                "tests",
                GateStatus::Fail {
                    detail: "1 failed".to_owned(),
                },
            )]),
            Vec::new(),
        ),
        b"secret-key",
        "2026-q3",
    )
    .expect("sign");
    let path = write_report("failing", &signed);

    let refused = run_verify(&path, &[]);
    assert!(
        !refused.status.success(),
        "a failing gate must block the merge"
    );
    std::fs::remove_dir_all(path.parent().expect("parent")).ok();
}

#[test]
fn the_verifier_accepts_a_green_candidate() {
    let signed = sign(
        report(gates(&[("tests", GateStatus::Pass)]), Vec::new()),
        b"secret-key",
        "2026-q3",
    )
    .expect("sign");
    let path = write_report("green", &signed);

    let accepted = run_verify(&path, &[]);
    assert!(
        accepted.status.success(),
        "a green report must allow the merge: {}",
        String::from_utf8_lossy(&accepted.stderr)
    );
    assert!(String::from_utf8_lossy(&accepted.stdout).contains("MERGEABLE"));
    std::fs::remove_dir_all(path.parent().expect("parent")).ok();
}

#[test]
fn the_verifier_rejects_a_forged_signature() {
    let signed = sign(
        report(gates(&[("tests", GateStatus::Pass)]), Vec::new()),
        b"the-real-key",
        "2026-q3",
    )
    .expect("sign");
    let path = write_report("forged", &signed);

    // The runner holds a different key than the publisher, so it cannot
    // validate a report signed elsewhere.
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_gate-report"));
    command
        .arg("verify")
        .arg("--report")
        .arg(&path)
        .env("GATE_REPORT_KEY", "a-forged-key");
    let output = command.output().expect("run the verifier");
    assert!(
        !output.status.success(),
        "a report signed by another key must not be accepted"
    );
    std::fs::remove_dir_all(path.parent().expect("parent")).ok();
}

#[test]
fn the_verifier_rejects_a_report_for_a_different_candidate() {
    let signed = sign(
        report(gates(&[("tests", GateStatus::Pass)]), Vec::new()),
        b"secret-key",
        "2026-q3",
    )
    .expect("sign");
    let path = write_report("mismatch", &signed);

    let output = run_verify(&path, &["--expect-candidate-sha", "a-different-commit"]);
    assert!(
        !output.status.success(),
        "a green report for another revision must not merge this one"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("not the candidate under review"),
        "unexpected error: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::remove_dir_all(path.parent().expect("parent")).ok();
}

#[test]
fn the_verifier_rejects_a_report_based_on_a_stale_base() {
    let signed = sign(
        report(gates(&[("tests", GateStatus::Pass)]), Vec::new()),
        b"secret-key",
        "2026-q3",
    )
    .expect("sign");
    let path = write_report("stale-base", &signed);

    let output = run_verify(&path, &["--expect-base-sha", "new-base"]);
    assert!(
        !output.status.success(),
        "a green report based on an old protected-branch revision must be refused"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("not the current base (new-base)"),
        "unexpected error: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::remove_dir_all(path.parent().expect("parent")).ok();
}

#[test]
fn the_verifier_requires_an_explicit_opt_out_when_the_key_is_absent() {
    // Otherwise a missing key would look identical to a checked signature.
    let signed = sign(
        report(gates(&[("tests", GateStatus::Pass)]), Vec::new()),
        b"secret-key",
        "2026-q3",
    )
    .expect("sign");
    let path = write_report("nokey", &signed);

    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_gate-report"));
    command.arg("verify").arg("--report").arg(&path);
    command.env_remove("GATE_REPORT_KEY");
    let output = command.output().expect("run the verifier");
    assert!(
        !output.status.success(),
        "verifying without a key must fail loudly rather than silently skip"
    );
    std::fs::remove_dir_all(path.parent().expect("parent")).ok();
}

#[test]
fn a_report_in_an_unknown_schema_version_is_refused() {
    // A reader that trusted a future version would ignore whatever that version
    // added and answer on the strength of the fields it happened to recognize.
    let mut report = report(gates(&[("tests", GateStatus::Pass)]), Vec::new());
    report.schema_version = SCHEMA_VERSION + 1;
    let error = report
        .ensure_supported_schema()
        .expect_err("a newer schema must be refused");
    assert!(
        error.to_string().contains("schema version"),
        "the refusal must name the schema version: {error}"
    );
}

#[test]
fn the_current_schema_version_is_accepted() {
    let report = report(gates(&[("tests", GateStatus::Pass)]), Vec::new());
    report.ensure_supported_schema().expect("current schema");
}

#[test]
fn the_emitter_records_the_environment_and_each_gate_duration() {
    // The manifest is what a release reader uses to judge whether one gate result
    // is comparable to another, so an absent host would make two runs silently
    // incomparable.
    let directory = std::env::temp_dir().join(format!("gate-report-env-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let report_path = directory.join("report.json");
    let durations_path = directory.join("durations.json");
    std::fs::write(&durations_path, "{\"tests\":1234}").expect("write durations");

    let emitted = std::process::Command::new(env!("CARGO_BIN_EXE_gate-report"))
        .args([
            "emit",
            "--candidate-sha",
            "abc",
            "--base-sha",
            "def",
            "--gates",
            "{\"tests\":{\"status\":\"pass\"}}",
            "--durations",
            &format!("@{}", durations_path.display()),
            "--output",
            &report_path.to_string_lossy(),
        ])
        .env("GATE_REPORT_KEY", "secret-key")
        .output()
        .expect("run the emitter");
    assert!(
        emitted.status.success(),
        "emitting a complete report must succeed: {}",
        String::from_utf8_lossy(&emitted.stderr)
    );

    let raw = std::fs::read_to_string(&report_path).expect("read the report");
    let signed: SignedGateReport = serde_json::from_str(&raw).expect("parse the report");
    verify(&signed, b"secret-key").expect("the report must verify");

    let report = &signed.report;
    assert_eq!(report.schema_version, SCHEMA_VERSION);
    assert!(!report.environment.platform.is_empty());
    assert!(!report.environment.arch.is_empty());
    assert_eq!(report.durations_ms.get("tests"), Some(&1234));

    // The recorded environment has to describe this machine, not a placeholder.
    if let Some(cpu_count) = report.environment.cpu_count {
        assert!(cpu_count >= 1, "a machine has at least one logical CPU");
    }

    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn the_verifier_refuses_a_report_in_an_unknown_schema_version() {
    let mut report = report(gates(&[("tests", GateStatus::Pass)]), Vec::new());
    report.schema_version = SCHEMA_VERSION + 1;
    let signed = sign(report, b"secret-key", "2026-q3").expect("sign");
    let path = write_report("schema", &signed);

    let output = run_verify(&path, &[]);
    assert!(
        !output.status.success(),
        "a report this build cannot read must not be accepted"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("schema version"),
        "the refusal must name the schema version: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::remove_dir_all(path.parent().expect("parent")).ok();
}
