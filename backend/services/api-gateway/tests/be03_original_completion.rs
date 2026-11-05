//! Two actual retained-cohort email checks, not a mock Auth implementation.
//! The allocated controller owns the database, mail, context, deadlines and cleanup.
//! Std-only allows a bounded `rustc --test` build without Cargo/target access.
use std::{env, path::PathBuf, process::{Command, Stdio}};

fn actual_case(name: &str) {
    let context = PathBuf::from(env::var_os("MCR_BE03_CONTEXT").expect("allocated context required"));
    let runner = PathBuf::from(env::var_os("MCR_BE03_RUNNER").expect("frozen runner required"));
    let python = PathBuf::from(env::var_os("MCR_BE03_PYTHON").expect("pinned Python required"));
    assert!(context.is_absolute() && runner.is_absolute() && python.is_absolute());
    let status = Command::new(python)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .env("TMPDIR", context.parent().expect("private parent"))
        .arg(runner)
        .arg("--case")
        .arg(name)
        .arg("--context")
        .arg(context)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("native case subprocess could not start");
    assert!(status.success(), "actual email case failed; use redacted case report");
}

#[test]
#[ignore = "requires a fresh finite native lease and owned Auth/mail context"]
fn expired_email_verification_does_not_activate() {
    actual_case("expired");
}

#[test]
#[ignore = "requires a fresh finite native lease and owned Auth/mail context"]
fn unverified_resend_delivers_once_and_new_verification_succeeds() {
    actual_case("resend");
}
