use concert_service::config::inbox_runtime::InboxRuntimeSettings;

#[test]
fn independent_off_mode_does_not_require_shared_feature_owners() {
    for mode in [None, Some("off")] {
        for shared in [false, true] {
            assert!(!InboxRuntimeSettings::from_values(mode, shared)
                .unwrap()
                .enabled());
        }
    }
}

#[test]
fn live_mode_requires_the_actual_shared_startup_path() {
    assert!(InboxRuntimeSettings::from_values(Some("live"), false).is_err());
    assert!(InboxRuntimeSettings::from_values(Some("live"), true)
        .unwrap()
        .enabled());
    for mode in ["", "on", "true", "LIVE", " live", "live ", "fixture"] {
        assert!(InboxRuntimeSettings::from_values(Some(mode), true).is_err());
    }
}
