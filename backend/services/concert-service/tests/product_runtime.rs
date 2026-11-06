use concert_service::{
    config::product_runtime::{production_owners, ProductSettings},
    product_rpc::{operation, Operation, POLICY},
};
use platform::grpc_auth::Workload;
use uuid::Uuid;

#[test]
fn actual_source_bound_operator_candidate_parses_without_enabling_capabilities() {
    use concert_service::bootstrap::{BootstrapFeatures, ProductReleaseBundle};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap();
    let bundle = root.join("backend/releases/operator-bundle/quality-v6");
    let spatial = root.join("backend/releases/reference-scene-v2/quality-v6");
    let avatar = root.join("contracts/fixtures/assets/selected-static/avatar-manifest.json");
    let themes = [
        bundle.join("theme-revenge.json"),
        bundle.join("theme-parade.json"),
        bundle.join("theme-danger.json"),
    ];
    let construct = |resources: &std::path::Path| {
        ProductReleaseBundle::from_source_bound_files(
            &bundle.join("bootstrap.json"),
            &avatar,
            &spatial.join("map-manifest.json"),
            [&themes[0], &themes[1], &themes[2]],
            BootstrapFeatures::default(),
            resources,
            &spatial.join("source-binding.json"),
        )
    };
    let release = construct(&root.join("frontend/public")).unwrap();
    let document: serde_json::Value =
        serde_json::from_slice(release.release().bootstrap_json()).unwrap();
    assert!(document["capabilities"]
        .as_object()
        .unwrap()
        .values()
        .all(|value| value["available"] == false));
    assert_eq!(
        document["avatar_manifest"]["representation"],
        "whole_character"
    );
    assert_eq!(document["map_manifest"]["map_revision"], 4);
    assert_eq!(document["theme_manifests"].as_array().unwrap().len(), 3);
    // A valid metadata-only bundle cannot replace actual source-byte admission.
    assert!(construct(&root.join("contracts/fixtures/assets")).is_err());
}

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

#[test]
fn every_party_and_gift_contract_path_resolves_to_a_known_operation() {
    use concert_service::gift_live::GiftOperation;
    let event = Uuid::new_v4();
    let party = Uuid::new_v4();
    let application = Uuid::new_v4();
    let user = Uuid::new_v4();
    let giveaway = Uuid::new_v4();
    let reservation = Uuid::new_v4();
    let prefix = format!("/api/concert/events/{event}");
    let cases = vec![
        ("GET", format!("{prefix}/parties"), Operation::ListParties),
        ("POST", format!("{prefix}/parties"), Operation::CreateParty),
        ("GET", format!("{prefix}/parties/{party}"), Operation::GetParty(party)),
        ("PATCH", format!("{prefix}/parties/{party}"), Operation::UpdateParty(party)),
        ("POST", format!("{prefix}/parties/{party}/cancel"), Operation::CancelParty(party)),
        ("POST", format!("{prefix}/parties/{party}/owner"), Operation::TransferParty(party)),
        ("GET", format!("{prefix}/parties/{party}/applications"), Operation::ListPartyApplications(party)),
        ("POST", format!("{prefix}/parties/{party}/applications"), Operation::ApplyParty(party)),
        ("GET", format!("{prefix}/party-applications/{application}"), Operation::GetPartyApplication(application)),
        ("POST", format!("{prefix}/party-applications/{application}/decision"), Operation::DecidePartyApplication(application)),
        ("DELETE", format!("{prefix}/party-applications/{application}?expected_revision=2"), Operation::CancelPartyApplication(application)),
        ("GET", format!("{prefix}/parties/{party}/members"), Operation::ListPartyMembers(party)),
        ("DELETE", format!("{prefix}/parties/{party}/members/{user}?expected_revision=1&party_revision=3"), Operation::RemovePartyMember(party, user)),
        ("GET", format!("{prefix}/parties/{party}/rally"), Operation::GetPartyRally(party)),
        ("PUT", format!("{prefix}/parties/{party}/rally/me"), Operation::SetPartyRally(party)),
        ("GET", format!("{prefix}/me/parties"), Operation::PartyActivity),
        ("GET", format!("{prefix}/giveaways"), Operation::Gift(GiftOperation::List)),
        ("GET", format!("{prefix}/giveaways/{giveaway}"), Operation::Gift(GiftOperation::Detail)),
        ("POST", format!("{prefix}/giveaways"), Operation::Gift(GiftOperation::Create)),
        ("PATCH", format!("{prefix}/giveaways/{giveaway}"), Operation::Gift(GiftOperation::Update)),
        ("GET", format!("{prefix}/giveaways/{giveaway}/management"), Operation::Gift(GiftOperation::Management)),
        ("PUT", format!("{prefix}/giveaways/{giveaway}/pickup-meeting"), Operation::Gift(GiftOperation::SetMeeting)),
        ("GET", format!("{prefix}/me/giveaways"), Operation::Gift(GiftOperation::Activity)),
        ("POST", format!("{prefix}/giveaways/{giveaway}/reservations"), Operation::Gift(GiftOperation::Reserve)),
        ("GET", format!("{prefix}/reservations/{reservation}"), Operation::Gift(GiftOperation::Reservation)),
        ("DELETE", format!("{prefix}/reservations/{reservation}?expected_revision=1"), Operation::Gift(GiftOperation::Cancel)),
        ("GET", format!("{prefix}/reservations/{reservation}/pickup"), Operation::Gift(GiftOperation::Pickup)),
        ("POST", format!("{prefix}/reservations/{reservation}/pickup-code"), Operation::Gift(GiftOperation::IssueCode)),
        ("POST", format!("{prefix}/reservations/{reservation}/claim"), Operation::Gift(GiftOperation::Claim)),
    ];
    for (method, target, expected) in cases {
        assert_eq!(operation(event, method, &target).unwrap(), expected, "{method} {target}");
    }
}

#[test]
fn party_and_gift_route_classification_rejects_wrong_methods_and_noncanonical_ids() {
    use concert_service::gift_live::GiftOperation;
    let event = Uuid::new_v4();
    let party = Uuid::new_v4();
    let giveaway = Uuid::new_v4();
    let reservation = Uuid::new_v4();
    let prefix = format!("/api/concert/events/{event}");
    for (method, target) in [
        ("GET", format!("{prefix}/giveaways/{giveaway}/reservations")),
        ("POST", format!("{prefix}/giveaways/{giveaway}/management")),
        ("GET", format!("{prefix}/giveaways/{}", giveaway.to_string().to_uppercase())),
        ("GET", format!("{prefix}/reservations/{}", Uuid::nil())),
        ("GET", format!("{prefix}/parties/{party}/owner")),
        ("PUT", format!("{prefix}/parties/{party}/applications")),
        ("PATCH", format!("{prefix}/reservations/{reservation}")),
    ] {
        assert!(
            operation(event, method, &target).is_err(),
            "{method} {target}"
        );
    }
    assert!(GiftOperation::route(event, "GET", &format!("{prefix}/me/giveaways")).is_ok());
}

#[test]
fn five_party_mutation_routes_bind_exact_c04_targets_and_queries() {
    let event = Uuid::new_v4();
    let party = Uuid::new_v4();
    let application = Uuid::new_v4();
    let user = Uuid::new_v4();
    let prefix = format!("/api/concert/events/{event}");
    assert_eq!(
        operation(
            event,
            "POST",
            &format!("{prefix}/parties/{party}/applications")
        )
        .unwrap(),
        Operation::ApplyParty(party)
    );
    assert_eq!(
        operation(
            event,
            "POST",
            &format!("{prefix}/party-applications/{application}/decision")
        )
        .unwrap(),
        Operation::DecidePartyApplication(application)
    );
    assert_eq!(
        operation(
            event,
            "DELETE",
            &format!("{prefix}/party-applications/{application}?expected_revision=2")
        )
        .unwrap(),
        Operation::CancelPartyApplication(application)
    );
    assert_eq!(
        operation(
            event,
            "DELETE",
            &format!(
                "{prefix}/parties/{party}/members/{user}?expected_revision=1&party_revision=3"
            )
        )
        .unwrap(),
        Operation::RemovePartyMember(party, user)
    );
    assert_eq!(
        operation(event, "POST", &format!("{prefix}/parties/{party}/cancel")).unwrap(),
        Operation::CancelParty(party)
    );
    for (method, target) in [
        ("POST", format!("{prefix}/parties/{party}/applications?extra=true")),
        ("DELETE", format!("{prefix}/party-applications/{application}")),
        (
            "DELETE",
            format!("{prefix}/party-applications/{application}?expected_revision=0"),
        ),
        (
            "DELETE",
            format!(
                "{prefix}/party-applications/{application}?expected_revision=2&expected_revision=3"
            ),
        ),
        (
            "DELETE",
            format!("{prefix}/parties/{party}/members/{user}?expected_revision=1"),
        ),
        (
            "DELETE",
            format!(
                "{prefix}/parties/{party}/members/{user}?expected_revision=1&party_revision=3&extra=1"
            ),
        ),
        (
            "DELETE",
            format!("{prefix}/parties/{party}/members/{user}?expected_revision=1&party_revision=03"),
        ),
        (
            "POST",
            format!("{prefix}/party-applications/{application}/decision?extra=true"),
        ),
    ] {
        assert!(
            operation(event, method, &target).is_err(),
            "{method} {target}"
        );
    }
}
