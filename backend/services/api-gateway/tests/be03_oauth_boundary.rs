//! Native entry points for the allocated, mounted OAuth boundary controller.
//! These do not qualify the disabled 404 topology or external-provider sign-in.
use std::{env, path::PathBuf, process::{Command, Stdio}};

fn allocated_case(case: &str) {
    let context = PathBuf::from(env::var_os("MCR_BE03_OAUTH_CONTEXT").expect("fresh allocated context"));
    let runner = PathBuf::from(env::var_os("MCR_BE03_OAUTH_RUNNER").expect("pinned boundary runner"));
    let python = PathBuf::from(env::var_os("MCR_BE03_OAUTH_PYTHON").expect("pinned interpreter"));
    assert!(context.is_absolute() && runner.is_absolute() && python.is_absolute());
    let status = Command::new(python)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .env("TMPDIR", context.parent().expect("private allocated parent"))
        .arg(runner)
        .arg("--context").arg(context)
        .arg("--case").arg(case)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status().expect("boundary subprocess start");
    assert!(status.success(), "boundary failed; consult the redacted allocated report");
}

#[test]
#[ignore = "requires qualified mounted candidate, fresh guards and SQL-read custody"]
fn mounted_oauth_invalid_state_never_exchanges() {
    allocated_case("invalid-state");
}

#[test]
#[ignore = "requires frozen handoff profile and fresh allocated controller"]
fn mounted_oauth_denial_returns_to_the_bound_local_target() {
    allocated_case("denial-return");
}

#[test]
#[ignore = "requires actual owned provider issuance/exchange and fresh native guards"]
fn mounted_oauth_success_uses_cookie_then_canonical_refresh() {
    allocated_case("success-handoff");
}
