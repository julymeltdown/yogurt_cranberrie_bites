//! REPORT-STARTUP-01 pure configuration and transport-only dispatch tests.
//! The unexported owned leaf is included directly until Root mounts its export.
//! Recording refusals never certify canonical authority or database effects.
pub use concert_service::{config, operations_live, product_rpc};
#[path = "../src/config/report_runtime.rs"]
mod report_runtime;
#[path = "product_feature_admission_support/mod.rs"]
mod support;

use product_rpc::{proto, ProductOwner, ProductRpcService, POLICY};
use report_runtime::{ReportOverlay, ReportStartupSettings};
use serde_json::json;
use std::{collections::BTreeMap, sync::Arc};
use uuid::Uuid;

fn settings() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("CONCERT_REPORT_MODE".into(), "user".into()),
        ("CONCERT_REPORT_CURSOR_KEY_HEX".into(), "11".repeat(32)),
        (
            "CONCERT_REPORT_RECEIPT_KEYS_JSON".into(),
            json!([
                {"id":"current","secret_hex":"22".repeat(32)},
                {"id":"retained","secret_hex":"33".repeat(32)}
            ])
            .to_string(),
        ),
    ])
}
fn parse(values: &BTreeMap<String, String>) -> anyhow::Result<Option<ReportStartupSettings>> {
    ReportStartupSettings::from_lookup(|key| Ok(values.get(key).cloned()), true, true)
}

#[test]
fn default_and_off_do_not_read_secrets_or_require_other_feature_settings() {
    for mode in [None, Some("off")] {
        let mut reads = Vec::new();
        let parsed = ReportStartupSettings::from_lookup(
            |key| {
                reads.push(key.to_owned());
                anyhow::ensure!(key == "CONCERT_REPORT_MODE", "Unexpected secret read");
                Ok(mode.map(str::to_owned))
            },
            false,
            false,
        )
        .unwrap();
        assert!(parsed.is_none());
        assert_eq!(reads, ["CONCERT_REPORT_MODE"]);
    }
}

#[test]
fn only_exact_user_mode_with_guarded_grpc_and_shared_canonical_is_available() {
    let values = settings();
    assert!(parse(&values).unwrap().is_some());
    for (grpc, shared) in [(false, false), (false, true), (true, false)] {
        let mut reads = Vec::new();
        assert!(ReportStartupSettings::from_lookup(
            |key| {
                reads.push(key.to_owned());
                Ok(values.get(key).cloned())
            },
            grpc,
            shared
        )
        .is_err());
        assert_eq!(reads, ["CONCERT_REPORT_MODE"]);
    }
    for mode in [
        "", "live", "on", "true", "USER", " user", "user ", "admin", "media",
    ] {
        let mut bad = values.clone();
        bad.insert("CONCERT_REPORT_MODE".into(), mode.into());
        assert!(parse(&bad).is_err());
    }
}

#[test]
fn every_private_input_is_explicit_and_reader_errors_are_redacted() {
    let values = settings();
    for name in [
        "CONCERT_REPORT_CURSOR_KEY_HEX",
        "CONCERT_REPORT_RECEIPT_KEYS_JSON",
    ] {
        for value in [None, Some(String::new())] {
            let mut bad = values.clone();
            bad.remove(name);
            if let Some(value) = value {
                bad.insert(name.into(), value);
            }
            assert!(parse(&bad).is_err());
        }
    }
    let error = ReportStartupSettings::from_lookup(
        |key| {
            if key == "CONCERT_REPORT_MODE" {
                Ok(Some("user".into()))
            } else {
                anyhow::bail!("private-reader-canary")
            }
        },
        true,
        true,
    )
    .err()
    .expect("Reader must fail closed");
    assert!(!format!("{error:#}").contains("private-reader-canary"));
    assert!(ReportStartupSettings::from_lookup(
        |_| anyhow::bail!("private-mode-canary"),
        false,
        false
    )
    .is_err());
}

#[test]
fn cursor_requires_exact_nonzero_lowercase_private_material() {
    for raw in [
        String::new(),
        "00".repeat(32),
        "AA".repeat(32),
        "g1".repeat(32),
        "12".repeat(31),
        "12".repeat(33),
        "é".repeat(32),
        format!(" {}", "12".repeat(32)),
    ] {
        let mut bad = settings();
        bad.insert("CONCERT_REPORT_CURSOR_KEY_HEX".into(), raw);
        assert!(parse(&bad).is_err());
    }
}

#[test]
fn managed_receipt_shape_versions_and_material_are_closed() {
    for raw in [
        "null".into(),
        "{}".into(),
        "[]".into(),
        json!([{"id":"key","secret_hex":"22".repeat(32),"enabled":true}]).to_string(),
        json!([{"id":"key"}]).to_string(),
        json!([{"id":false,"secret_hex":"22".repeat(32)}]).to_string(),
        json!([{"id":"same","secret_hex":"22".repeat(32)},{"id":"same","secret_hex":"33".repeat(32)}]).to_string(),
        json!([{"id":"first","secret_hex":"22".repeat(32)},{"id":"next","secret_hex":"22".repeat(32)}]).to_string(),
        json!([{"id":"key","secret_hex":"11".repeat(32)}]).to_string(),
        format!("[{{\"id\":\"first\",\"id\":\"second\",\"secret_hex\":\"{}\"}}]", "22".repeat(32)),
        format!("[{{\"id\":\"key\",\"secret_hex\":\"{}\",\"secret_hex\":\"{}\"}}]", "22".repeat(32), "33".repeat(32)),
    ] {
        let mut bad = settings();
        bad.insert("CONCERT_REPORT_RECEIPT_KEYS_JSON".into(), raw);
        assert!(parse(&bad).is_err());
    }
    for id in [
        "".into(),
        "invalid.version".into(),
        "private/key".into(),
        " ".into(),
        "a".repeat(33),
    ] {
        let mut bad = settings();
        bad.insert(
            "CONCERT_REPORT_RECEIPT_KEYS_JSON".into(),
            json!([{"id":id,"secret_hex":"22".repeat(32)}]).to_string(),
        );
        let error = parse(&bad).err().expect("Invalid key ID must fail");
        assert!(!format!("{error:#}").contains(&"22".repeat(32)));
    }
    for secret in [
        "00".repeat(32),
        "AB".repeat(32),
        "22".repeat(31),
        "22".repeat(33),
    ] {
        let mut bad = settings();
        bad.insert(
            "CONCERT_REPORT_RECEIPT_KEYS_JSON".into(),
            json!([{"id":"key","secret_hex":secret}]).to_string(),
        );
        assert!(parse(&bad).is_err());
    }
}

#[test]
fn rotation_is_bounded_and_cursor_is_independent_of_all_retained_versions() {
    for count in [1, 32, 33] {
        let versions: Vec<_> = (1..=count)
            .map(|n| json!({"id":format!("version-{n}"),"secret_hex":format!("{n:064x}")}))
            .collect();
        let mut values = settings();
        values.insert(
            "CONCERT_REPORT_RECEIPT_KEYS_JSON".into(),
            json!(versions).to_string(),
        );
        assert_eq!(parse(&values).is_ok(), count <= 32);
    }
    let mut bad = settings();
    bad.insert(
        "CONCERT_REPORT_RECEIPT_KEYS_JSON".into(),
        json!([
            {"id":"current","secret_hex":"22".repeat(32)},
            {"id":"retained","secret_hex":"11".repeat(32)}
        ])
        .to_string(),
    );
    assert!(parse(&bad).is_err());
    bad.insert(
        "CONCERT_REPORT_RECEIPT_KEYS_JSON".into(),
        " ".repeat(16 * 1024 + 1),
    );
    assert!(parse(&bad).is_err());
}

#[tokio::test]
async fn original_create_and_own_reports_use_report_owner_and_others_delegate_unchanged() {
    use platform::{
        actor_delegation::ActorDelegationVerifier,
        grpc_auth::{CallerRegistry, Workload},
        grpc_method_policy::guard,
    };
    let base = support::Recorder::new(support::Mode::Refuse);
    let reports = support::Recorder::new(support::Mode::Refuse);
    let overlay: Arc<dyn ProductOwner> =
        Arc::new(ReportOverlay::new(base.clone(), reports.clone()));
    let mut inputs = vec![support::input("POST", "reports", &json!({
        "target_type":"USER","target_id":Uuid::new_v4(),"reason":"SPAM", "text":"original signed report", "evidence_media_ids":[]
    }).to_string())];
    inputs.push(support::input("GET", "me/reports?state=OPEN&limit=1", ""));
    for suffix in ["feed", "me/age-status", "me/blocks"] {
        inputs.push(support::input("GET", suffix, ""));
    }
    for input in &inputs {
        let service = guard(
            &POLICY,
            proto::product_service_server::ProductServiceServer::new(ProductRpcService::new(
                overlay.clone(),
            )),
            CallerRegistry::new()
                .register(Workload::Gateway, "gateway-feature-fixture")
                .unwrap(),
            ActorDelegationVerifier::new(Workload::Concert, &[support::signer().public_key()])
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            support::call(service, support::wire(input)).await,
            (tonic::Code::Unavailable, support::REFUSAL.into())
        );
    }
    assert_eq!(reports.len(), 2);
    assert_eq!(base.len(), 3);
    assert_eq!(reports.admitted.lock().unwrap()[0].request, inputs[0]);
    assert_eq!(reports.admitted.lock().unwrap()[1].request, inputs[1]);
    let admitted = base.admitted.lock().unwrap();
    for (actual, expected) in admitted.iter().zip(&inputs[2..]) {
        assert_eq!(&actual.request, expected);
    }
}
