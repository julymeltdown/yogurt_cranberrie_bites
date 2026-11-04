use post_service::config::concert_runtime::ReadSettings;
use std::collections::BTreeMap;

fn values() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("POST_CONCERT_READ_MODE".into(), "read".into()),
        (
            "POST_CONCERT_READ_AUTH_GRPC_ENDPOINT".into(),
            "https://auth.internal:8443".into(),
        ),
        (
            "POST_FRIEND_GRPC_ENDPOINT".into(),
            "https://friend.internal:8443".into(),
        ),
        (
            "POST_CONCERT_ROOT_GRPC_ENDPOINT".into(),
            "https://concert.internal:8443".into(),
        ),
    ])
}
#[test]
fn off_is_independent_of_unconfigured_feature_owners() {
    for mode in [None, Some("off")] {
        assert!(ReadSettings::from_values(
            |name| {
                assert_eq!(name, "POST_CONCERT_READ_MODE");
                Ok(mode.map(str::to_string))
            },
            false,
            1
        )
        .unwrap()
        .is_none());
    }
}
#[test]
fn read_requires_tls_pool_and_every_real_endpoint() {
    let config = values();
    for (tls, pool) in [(false, 2), (true, 1)] {
        assert!(
            ReadSettings::from_values(|name| Ok(config.get(name).cloned()), tls, pool).is_err()
        );
    }
    assert!(
        ReadSettings::from_values(|name| Ok(config.get(name).cloned()), true, 2)
            .unwrap()
            .is_some()
    );
    for name in config
        .keys()
        .filter(|name| name.as_str() != "POST_CONCERT_READ_MODE")
    {
        let mut absent = config.clone();
        absent.remove(name);
        assert!(ReadSettings::from_values(|name| Ok(absent.get(name).cloned()), true, 2).is_err());
        for bad in [
            "http://owner:8443",
            "https://owner",
            "https://owner:0",
            "https://user@owner:8443",
            "https://owner:8443/path",
            "https://owner:8443/?key=value",
        ] {
            let mut invalid = config.clone();
            invalid.insert(name.clone(), bad.into());
            assert!(
                ReadSettings::from_values(|name| Ok(invalid.get(name).cloned()), true, 2).is_err()
            );
        }
    }
}
