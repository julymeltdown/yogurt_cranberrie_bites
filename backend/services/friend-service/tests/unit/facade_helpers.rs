#[test]
fn use_case_facade_describes_friend_use_cases() {
    let description = friend_service::application::use_cases::describe();

    assert_eq!(description, "friend use cases");
}

#[test]
fn application_http_adapter_exposes_no_metrics_route() {
    let route_count = friend_service::adapters::r#in::http::route_count();

    assert_eq!(route_count, 0);
}
