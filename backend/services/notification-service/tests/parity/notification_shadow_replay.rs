use notification_service::application::dto::notification_dto::NotificationCreateDto;

#[tokio::test]
async fn notification_shadow_fixture_matches_created_record_shape() {
    let app = crate::support::start_postgres_app().await.expect("app");
    let fixture: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../scripts/parity/notification-service/notification-shadow.json"),
        )
        .expect("fixture"),
    )
    .expect("json");

    let created = app
        .state
        .create_notification
        .execute(NotificationCreateDto {
            user_id: fixture["userId"].as_str().unwrap_or_default().into(),
            r#type: fixture["type"].as_str().unwrap_or_default().into(),
            payload_json: fixture["payloadJson"].as_str().unwrap_or_default().into(),
            event_id: fixture["eventId"].as_str().unwrap_or_default().into(),
            actor_id: fixture["actorId"].as_str().unwrap_or_default().into(),
            occurred_at: fixture["occurredAt"].as_str().unwrap_or_default().into(),
        })
        .await
        .expect("create");

    let created = created.notification.expect("notification");
    assert_eq!(
        created.user_id,
        fixture["userId"].as_str().unwrap_or_default()
    );
    assert_eq!(created.r#type, fixture["type"].as_str().unwrap_or_default());
    assert_eq!(
        created.payload_json,
        fixture["payloadJson"].as_str().unwrap_or_default()
    );
    app.shutdown().await.expect("shutdown parity fixture");
}
