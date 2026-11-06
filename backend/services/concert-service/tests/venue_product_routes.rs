use concert_service::product_rpc::operation;
use uuid::Uuid;

#[test]
fn every_mounted_venue_request_is_classified_instead_of_unsupported() {
    let event = Uuid::new_v4();
    let id = Uuid::new_v4();
    let base = format!("/api/concert/events/{event}");
    for (method, tail, contract) in [
        (
            "GET",
            "venue/location".to_owned(),
            "getVenueLocationSnapshot",
        ),
        (
            "PUT",
            "venue/location/sharing".to_owned(),
            "setVenueLocationSharing",
        ),
        (
            "PUT",
            "venue/location/fix".to_owned(),
            "publishVenueLocationFix",
        ),
        (
            "GET",
            format!("venue/location/commands/{id}?operation=publishVenueLocationFix"),
            "getVenueLocationCommand",
        ),
        (
            "GET",
            "venue/landmarks/lm-main/notes".to_owned(),
            "listVenuePlaceNotes",
        ),
        (
            "POST",
            "venue/landmarks/lm-main/notes".to_owned(),
            "createVenuePlaceNote",
        ),
        (
            "GET",
            format!("venue/landmarks/lm-main/notes/{id}"),
            "getVenuePlaceNote",
        ),
        (
            "GET",
            format!("venue/landmarks/lm-main/notes/{id}/replies"),
            "listVenuePlaceReplies",
        ),
        (
            "POST",
            format!("venue/landmarks/lm-main/notes/{id}/replies"),
            "createVenuePlaceReply",
        ),
        (
            "DELETE",
            format!("venue/landmarks/lm-main/notes/{id}?expected_revision=1"),
            "withdrawVenuePlaceNote",
        ),
        (
            "GET",
            format!("venue-meeting/context?selectedFriendId={id}"),
            "resolveVenueMeetingContext",
        ),
        (
            "GET",
            format!("venue-meeting/messages?selectedFriendId={id}"),
            "listVenueMeetingMessages",
        ),
    ] {
        let target = format!("{base}/{tail}");
        let operation = operation(event, method, &target)
            .unwrap_or_else(|status| panic!("{method} {tail} refused: {status:?}"));
        assert_eq!(operation.contract_id(), contract, "{method} {tail}");
    }
}

#[test]
fn unmounted_and_malformed_venue_targets_stay_closed() {
    let event = Uuid::new_v4();
    let base = format!("/api/concert/events/{event}");
    for (method, tail) in [
        ("GET", "venue/unknown".to_owned()),
        ("GET", "venue/location/requests".to_owned()),
        ("GET", "venue/landmarks/bad%2Fid/notes".to_owned()),
        (
            "DELETE",
            "venue/landmarks/lm-main/notes/not-a-uuid".to_owned(),
        ),
    ] {
        assert!(operation(event, method, &format!("{base}/{tail}")).is_err());
    }
}
