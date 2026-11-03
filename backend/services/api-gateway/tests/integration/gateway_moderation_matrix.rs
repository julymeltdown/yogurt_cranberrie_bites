use crate::support::start_gateway_app;
use api_gateway::application::dto::moderation_dto::{
    CreateModerationDecisionRequest, CreateReportRequest,
};
use chrono::Utc;
use uuid::Uuid;

const REPORTER_ID: &str = "11111111-1111-1111-1111-111111111111";
const OTHER_REPORTER_ID: &str = "22222222-2222-2222-2222-222222222222";
const TARGET_ID: &str = "33333333-3333-3333-3333-333333333333";
const ADMIN_ID: &str = "44444444-4444-4444-4444-444444444444";

#[tokio::test]
async fn reports_decisions_and_restrictions_are_idempotent_and_reporter_scoped() {
    let app = start_gateway_app(false).await.expect("gateway app");
    let service = &app.state.moderation_service;
    let request = CreateReportRequest {
        target_type: "POST".into(),
        target_id: TARGET_ID.into(),
        category: "SPAM".into(),
        reason: "Repeated unsolicited advertising".into(),
    };

    let first = service
        .create_report(REPORTER_ID.into(), request.clone())
        .await
        .expect("first report");
    let duplicate = service
        .create_report(REPORTER_ID.into(), request)
        .await
        .expect("duplicate report");
    service
        .create_report(
            OTHER_REPORTER_ID.into(),
            CreateReportRequest {
                target_type: "POST".into(),
                target_id: TARGET_ID.into(),
                category: "SPAM".into(),
                reason: "Independent reporter".into(),
            },
        )
        .await
        .expect("other reporter");

    assert_eq!(first.id, duplicate.id);
    let own = service
        .list_my_reports(REPORTER_ID.into(), 100)
        .await
        .expect("own reports");
    assert_eq!(own.reports.len(), 1);
    let report_count: i64 = sqlx::query_scalar("select count(*) from moderation_reports")
        .fetch_one(&app.pool)
        .await
        .expect("report count");
    assert_eq!(report_count, 2);

    let user_report = service
        .create_report(
            REPORTER_ID.into(),
            CreateReportRequest {
                target_type: "USER".into(),
                target_id: TARGET_ID.into(),
                category: "HARASSMENT".into(),
                reason: "Repeated abusive behavior".into(),
            },
        )
        .await
        .expect("user report");
    let decision_request = CreateModerationDecisionRequest {
        decision_type: "RESTRICT_USER".into(),
        note: "Temporary mutation restriction".into(),
        restriction_expires_at: Some(Utc::now() + chrono::Duration::hours(1)),
    };
    let first_decision = service
        .decide(
            user_report.id.clone(),
            ADMIN_ID.into(),
            decision_request.clone(),
        )
        .await
        .expect("first decision");
    let duplicate_decision = service
        .decide(user_report.id, ADMIN_ID.into(), decision_request)
        .await
        .expect("duplicate decision");

    assert_eq!(first_decision.decision.id, duplicate_decision.decision.id);
    assert!(service
        .is_user_restricted(TARGET_ID)
        .await
        .expect("restricted"));
    let decision_count: i64 =
        sqlx::query_scalar("select count(*) from moderation_decisions where report_id = $1")
            .bind(Uuid::parse_str(&first_decision.report.id).unwrap())
            .fetch_one(&app.pool)
            .await
            .expect("decision count");
    assert_eq!(decision_count, 1);
    app.shutdown().await.expect("app shutdown");
}
