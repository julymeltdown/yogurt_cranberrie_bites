#[test]
fn notification_dto_is_distinct_from_domain_module() {
    let dto = notification_service::application::dto::notification_dto::NotificationDto {
        id: "notification-1".into(),
        user_id: "user-1".into(),
        r#type: "SYSTEM".into(),
        payload_json: "{}".into(),
        created_at: "2026-01-01T00:00:00Z".into(),
        read_at: String::new(),
        event_id: "event-1".into(),
        actor_id: String::new(),
        occurred_at: "2026-01-01T00:00:00Z".into(),
    };
    assert_eq!(dto.user_id, "user-1");
}
