//! QA04 actual public entry point, separate from synthetic SQL qualification.
//! The seed owner supplies seven real canonical principals via a private plan.
//! A role name never changes authority or assumes that every SELF list is 404.
#[test]
#[ignore = "actual isolated registered public runtime and seven-principal seed plan required"]
fn actual_qa04_http_authorization_matrix() {
    let plan = std::env::var("MCR_QA04_REAL_PLAN").expect("actual private seed plan required");
    let report =
        std::env::var("MCR_QA04_BACKEND_REPORT").expect("new absolute local report required");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let result = std::process::Command::new("python3")
        .arg(root.join("tests/authorization/social_matrix_support/http_matrix.py"))
        .args(["--plan", &plan, "--report", &report])
        .output()
        .expect("backend matrix runner");
    // HTTP qualification remains PARTIAL with WS/cached product lifetime open.
    assert_eq!(
        result.status.code(),
        Some(2),
        "matrix setup/assertion failed; payload withheld"
    );
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(report).unwrap()).unwrap();
    assert_eq!(
        value["HTTP"], "PASS",
        "real HTTP not qualified; payload withheld"
    );
    assert_eq!(value["result"], "PARTIAL");
    assert_eq!(value["WS"], "NOT_RUN");
    assert_eq!(value["cachedProductUI"], "NOT_RUN");
}
