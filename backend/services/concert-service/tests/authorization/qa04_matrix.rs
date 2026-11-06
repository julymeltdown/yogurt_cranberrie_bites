#[test]
#[ignore = "actual seven-service stack and owned PostgreSQL 16 required"]
fn actual_qa04_privacy_and_authorization_matrix() {
    let bin_dir = std::env::var("MCR_QA04_BIN_DIR").expect("seven-service binary directory required");
    let report =
        std::env::var("MCR_QA04_BACKEND_REPORT").expect("new absolute QA04 report required");
    let in03_report =
        std::env::var("MCR_IN03_REPORT").expect("new absolute IN03 report required");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let result = std::process::Command::new("python3")
        .arg("-B")
        .arg(root.join("scripts/mcr/in03_social_gate.py"))
        .args(["--bin-dir", &bin_dir, "--qa04-report", &report, "--report", &in03_report])
        .current_dir(root.join("../.."))
        .output()
        .expect("QA04 matrix runner");
    assert!(
        result.status.success(),
        "QA04/IN03 run failed; payload withheld"
    );
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(report).unwrap()).unwrap();
    assert_eq!(value["result"], "PASS");
    assert_eq!(value["counts"]["cells_by_result"]["FAIL"], 0);
}
