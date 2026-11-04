use concert_service::{
    config::product_runtime::{production_owners, ProductSettings},
    product_rpc::{operation, Operation, POLICY},
};
use platform::grpc_auth::Workload;
use uuid::Uuid;

#[test]
fn product_modes_require_explicit_supported_values_and_guarded_composition() {
    assert_eq!(
        ProductSettings::from_values(None, None, false).unwrap(),
        ProductSettings::default()
    );
    assert!(ProductSettings::from_values(Some("read"), None, false).is_err());
    assert!(ProductSettings::from_values(None, Some("uploader"), false).is_err());
    for value in ["", "true", "on", "read-write", "fixture", " read", "READ"] {
        assert!(ProductSettings::from_values(Some(value), None, true).is_err());
    }
    for value in ["", "true", "public", "linked", "fixture", "UPLOADER"] {
        assert!(ProductSettings::from_values(None, Some(value), true).is_err());
    }
    let feed = ProductSettings::from_values(Some("read"), Some("off"), true).unwrap();
    assert!(feed.public_feed_read && !feed.owner_media);
    let media = ProductSettings::from_values(Some("off"), Some("uploader"), true).unwrap();
    assert!(!media.public_feed_read && media.owner_media);
}

#[test]
fn unsupported_production_opt_in_does_not_return_an_unavailable_listener() {
    assert!(production_owners(ProductSettings::default())
        .unwrap()
        .is_none());
    for settings in [
        ProductSettings {
            public_feed_read: true,
            owner_media: false,
        },
        ProductSettings {
            public_feed_read: false,
            owner_media: true,
        },
        ProductSettings {
            public_feed_read: true,
            owner_media: true,
        },
    ] {
        assert!(production_owners(settings).is_err());
    }
}

#[test]
fn only_gateway_may_dispatch_with_a_signed_field_one_actor() {
    let path = "/concert.product.v1.ProductService/Dispatch";
    for caller in Workload::ALL {
        assert_eq!(POLICY.allows(path, caller), caller == Workload::Gateway);
    }
    assert_eq!(POLICY.actor_fields_for(path), Some(&[&[1][..]][..]));
    assert!(!POLICY.allows(
        "/concert.product.v1.ProductService/WriteFeed",
        Workload::Gateway
    ));
    assert_eq!(
        platform::grpc_method_policy::proto_rpc_names(include_str!(
            "../../../proto/concert/v1/product.proto"
        )),
        ["Dispatch", "CompleteResponse"]
    );
}

#[test]
fn closed_routes_reject_cross_event_forwarding_and_noncanonical_media_ids() {
    let event = Uuid::new_v4();
    let media = Uuid::new_v4();
    let prefix = format!("/api/concert/events/{event}");
    assert_eq!(
        operation(event, "GET", &format!("{prefix}/feed?limit=10")).unwrap(),
        Operation::Feed
    );
    assert_eq!(
        operation(event, "GET", &format!("{prefix}/media/{media}/content")).unwrap(),
        Operation::PrivateContent(media)
    );
    assert_eq!(
        operation(event, "POST", &format!("{prefix}/media/uploads")).unwrap(),
        Operation::InitializeUpload
    );
    for (method, target) in [
        ("POST", format!("{prefix}/feed")),
        ("GET", "/api/feed".into()),
        (
            "GET",
            format!("/api/concert/events/{}/feed", Uuid::new_v4()),
        ),
        (
            "GET",
            format!(
                "{prefix}/media/{}/content",
                media.to_string().to_uppercase()
            ),
        ),
        (
            "GET",
            format!("{prefix}/media/{media}/content?audience=public"),
        ),
        ("PUT", format!("{prefix}/media/{media}/bytes?purpose=POST")),
        ("GET", format!("{prefix}/media/{media}/../feed")),
        ("GET", format!("{prefix}/feed\n")),
    ] {
        assert!(
            operation(event, method, &target).is_err(),
            "{method} {target}"
        );
    }
}
