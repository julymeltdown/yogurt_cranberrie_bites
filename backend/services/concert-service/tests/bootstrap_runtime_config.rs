use concert_service::config::bootstrap_runtime::{settings, RELEASE_SETTINGS};
use std::collections::HashMap;

fn resolve(
    values: &[(&str, &str)],
) -> anyhow::Result<Option<(std::net::SocketAddr, [std::path::PathBuf; 6], bool)>> {
    let mut values: HashMap<&str, &str> = values.iter().copied().collect();
    for name in RELEASE_SETTINGS {
        values.entry(name).or_insert("/explicit/reviewed.json");
    }
    settings(|name| Ok(values.get(name).map(|v| v.to_string())))
}
#[test]
fn bootstrap_listener_requires_an_explicit_safe_opt_in() {
    assert!(resolve(&[]).unwrap().is_none());
    for address in ["", "bad", "127.0.0.1:50056", "0.0.0.0:0"] {
        assert!(resolve(&[("CONCERT_GRPC_ADDR", address)]).is_err());
    }
    assert!(
        resolve(&[("CONCERT_GRPC_ADDR", "0.0.0.0:50056")])
            .unwrap()
            .unwrap()
            .2
    );
    assert!(resolve(&[
        ("CONCERT_GRPC_ADDR", "0.0.0.0:50056"),
        ("CONCERT_RUNTIME_MODE", "local")
    ])
    .is_err());
    assert!(
        !resolve(&[
            ("CONCERT_GRPC_ADDR", "127.0.0.1:0"),
            ("CONCERT_RUNTIME_MODE", "local")
        ])
        .unwrap()
        .unwrap()
        .2
    );
    assert!(resolve(&[
        ("CONCERT_GRPC_ADDR", "127.0.0.1:0"),
        ("CONCERT_RUNTIME_MODE", "demo")
    ])
    .is_err());
}
#[test]
fn every_product_file_is_required_without_echoing_values() {
    for absent in RELEASE_SETTINGS {
        let error = settings(|name| {
            Ok(match name {
                "CONCERT_GRPC_ADDR" => Some("0.0.0.0:50056".into()),
                name if RELEASE_SETTINGS.contains(&name) && name != absent => {
                    Some("private-path".into())
                }
                _ => None,
            })
        })
        .err()
        .expect("missing product file refused");
        assert!(error.to_string().contains(absent));
        assert!(!error.to_string().contains("private-path"));
    }
}
