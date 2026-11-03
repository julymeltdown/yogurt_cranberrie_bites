use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

#[tokio::test]
async fn gateway_governance_contract_covers_admin_crud_surface() {
    let app = crate::support::start_gateway_app(true).await.expect("app");
    let admin_token =
        crate::support::bearer_token("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa", &["ROLE_ADMIN"]);

    let list_contracts = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/contracts")
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(list_contracts.status(), StatusCode::OK);

    let created_contract = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/contracts")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"routeKey":"profile-summary","owner":"gateway","slaTarget":"p95<500ms","status":"draft","description":"Profile aggregate"}"#,
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(created_contract.status(), StatusCode::CREATED);
    let contract_body = created_contract
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let contract_json: serde_json::Value = serde_json::from_slice(&contract_body).expect("json");
    let contract_id = contract_json["id"].as_str().expect("id").to_owned();
    assert!(uuid::Uuid::parse_str(&contract_id).is_ok());
    assert_eq!(
        contract_json,
        serde_json::json!({
            "id": contract_id, "routeKey": "profile-summary", "owner": "gateway",
            "slaTarget": "p95<500ms", "status": "draft",
            "description": "Profile aggregate", "versions": []
        })
    );
    assert_eq!(
        serde_json::to_value(
            app.state
                .route_contract_service
                .get_contract(&contract_id)
                .await
                .unwrap()
        )
        .unwrap(),
        contract_json
    );

    let created_policy = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/policies")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"name":"burst","rateLimit":"120/min","timeoutMs":900,"retryPolicy":"retry-1","circuitBreakerPolicy":"enabled","cachePolicy":"default"}"#,
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(created_policy.status(), StatusCode::CREATED);
    let policy_body = created_policy
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let policy_json: serde_json::Value = serde_json::from_slice(&policy_body).expect("json");
    let policy_id = policy_json["id"].as_str().expect("id").to_owned();
    assert!(uuid::Uuid::parse_str(&policy_id).is_ok());
    assert_eq!(
        policy_json,
        serde_json::json!({
            "id": policy_id, "name": "burst", "rateLimit": "120/min", "quota": null,
            "timeoutMs": 900, "retryPolicy": "retry-1",
            "circuitBreakerPolicy": "enabled", "cachePolicy": "default"
        })
    );
    assert_eq!(
        serde_json::to_value(
            app.state
                .policy_service
                .get_policy(&policy_id)
                .await
                .unwrap()
        )
        .unwrap(),
        policy_json
    );

    let create_client = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/clients")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"clientType":"mobile","allowedContracts":["profile-summary"],"policySetId":"{policy_id}"}}"#
                )))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(create_client.status(), StatusCode::CREATED);
    let client_body = create_client
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let client_json: serde_json::Value = serde_json::from_slice(&client_body).expect("json");
    let client_id = client_json["id"].as_str().expect("id");
    assert!(uuid::Uuid::parse_str(client_id).is_ok());
    assert_eq!(
        client_json,
        serde_json::json!({
            "id": client_id, "clientType": "mobile", "allowedContracts": ["profile-summary"],
            "policySetId": policy_id
        })
    );
    assert_eq!(
        serde_json::to_value(
            app.state
                .client_profile_service
                .get_profile(client_id)
                .await
                .unwrap()
        )
        .unwrap(),
        client_json
    );

    let create_version = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/admin/contracts/{contract_id}/versions"))
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r##"{"version":"v2","responseSchemaRef":"#/components/schemas/ProfileSummary","status":"active"}"##,
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(create_version.status(), StatusCode::CREATED);
    let version_body = create_version
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let version_json: serde_json::Value = serde_json::from_slice(&version_body).expect("json");
    assert_eq!(
        version_json,
        serde_json::json!({
            "version": "v2", "responseSchemaRef": "#/components/schemas/ProfileSummary",
            "status": "active", "deprecationDate": null
        })
    );
    assert_eq!(
        serde_json::to_value(
            app.state
                .route_contract_service
                .get_version(&contract_id, "v2")
                .await
                .unwrap()
        )
        .unwrap(),
        version_json
    );
    let restored_contract = app
        .state
        .route_contract_service
        .get_contract(&contract_id)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(restored_contract).unwrap()["versions"],
        serde_json::json!([version_json])
    );

    let create_rollout = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/rollouts")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"contractId":"{contract_id}","version":"v2","strategy":"canary","status":"active","percent":10}}"#
                )))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(create_rollout.status(), StatusCode::CREATED);
    let rollout_body = create_rollout
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let rollout_json: serde_json::Value = serde_json::from_slice(&rollout_body).expect("json");
    let rollout_id = rollout_json["id"].as_str().expect("id");
    assert!(uuid::Uuid::parse_str(rollout_id).is_ok());
    assert_eq!(
        rollout_json,
        serde_json::json!({
            "id": rollout_id, "contractId": contract_id, "version": "v2",
            "strategy": "canary", "status": "active", "percent": 10
        })
    );
    assert_eq!(
        serde_json::to_value(
            app.state
                .policy_service
                .get_rollout(rollout_id)
                .await
                .unwrap()
        )
        .unwrap(),
        rollout_json
    );

    let telemetry = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/telemetry/summary")
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(telemetry.status(), StatusCode::OK);
    let telemetry_body = telemetry
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let telemetry_json: serde_json::Value = serde_json::from_slice(&telemetry_body).expect("json");
    assert_eq!(
        telemetry_json
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["errorRate", "p95LatencyMs", "totalRequests"]
    );
    assert!(telemetry_json["totalRequests"].as_i64().unwrap() >= 0);
    assert!((0.0..=1.0).contains(&telemetry_json["errorRate"].as_f64().unwrap()));
    assert!(telemetry_json["p95LatencyMs"].as_i64().unwrap() >= 0);
    app.shutdown().await.expect("app shutdown");
}

#[tokio::test]
async fn operation_status_is_admin_only_and_distinguishes_unknown_from_unresolved() {
    let app = crate::support::start_gateway_app(true).await.expect("app");
    let admin_token =
        crate::support::bearer_token("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa", &["ROLE_ADMIN"]);
    let user_token =
        crate::support::bearer_token("11111111-1111-1111-1111-111111111111", &["ROLE_USER"]);

    let completed = uuid::Uuid::new_v4();
    let unresolved = uuid::Uuid::new_v4();
    let mismatched = uuid::Uuid::new_v4();
    let unavailable = uuid::Uuid::new_v4();
    let non_applicable = uuid::Uuid::new_v4();
    let never_issued = uuid::Uuid::new_v4();
    let actor = uuid::Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
    let post_id = uuid::Uuid::new_v4();
    sqlx::query(
        r#"
        insert into gateway_request_journal
            (id, actor_id, action, resource_path, state, outcome, outcome_evidence, response_status, response_body, created_at, finished_at)
        values
            ($1, $6, 'POST /api/posts', '/api/posts', 'COMPLETED', 'COMMITTED', 'SUCCESS_RESPONSE', 201, '\x00'::bytea, now(), now()),
            ($2, $6, 'POST /api/posts', '/api/posts', 'UNKNOWN', 'UNKNOWN', 'LEGACY_UNRESOLVED', null, null, now(), now()),
            ($3, $6, 'DELETE /api/posts/' || $7::text, '/api/posts/' || $7::text, 'UNKNOWN', 'UNKNOWN', 'LEGACY_UNRESOLVED', null, null, now(), now()),
            ($4, $6, 'POST /api/posts', '/api/posts', 'UNKNOWN', 'UNKNOWN', 'LEGACY_UNRESOLVED', null, null, now(), now()),
            ($5, $6, 'GET /api/profile', '/api/profile', 'COMPLETED', 'COMMITTED', 'SUCCESS_RESPONSE', 200, '\x00'::bytea, now(), now())
        "#,
    )
    .bind(completed)
    .bind(unresolved)
    .bind(mismatched)
    .bind(unavailable)
    .bind(non_applicable)
    .bind(actor.to_string())
    .bind(post_id.to_string())
    .execute(&app.pool)
    .await
    .expect("seed journal rows");
    let committed_at = chrono::Utc::now();
    app.set_owner_receipt_status(
        api_gateway::application::dto::post_dto::MutationReceiptStatusDto {
            operation_id: completed,
            actor_id: actor,
            scope: "post.v1.CreatePost".into(),
            committed_at,
            replay_until: committed_at + chrono::Duration::hours(24),
            state: api_gateway::application::dto::post_dto::MutationReceiptStateDto::Replayable,
        },
    );
    app.set_owner_receipt_status(
        api_gateway::application::dto::post_dto::MutationReceiptStatusDto {
            operation_id: mismatched,
            actor_id: uuid::Uuid::new_v4(),
            scope: "post.v1.DeletePost".into(),
            committed_at,
            replay_until: committed_at + chrono::Duration::hours(24),
            state: api_gateway::application::dto::post_dto::MutationReceiptStateDto::Expired,
        },
    );

    // A signed-in non-admin must not be able to read another tenant's operation.
    let forbidden = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/admin/operations/{completed}"))
                .header("authorization", format!("Bearer {user_token}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);

    let resolved = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/admin/operations/{completed}"))
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(resolved.status(), StatusCode::OK);
    let body = resolved
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(json["state"], "COMPLETED");
    assert_eq!(json["outcome"], "COMMITTED");
    assert_eq!(json["resourcePath"], "/api/posts");
    assert_eq!(json["responseStatus"], 201);
    assert_eq!(json["replayable"], true);
    assert_eq!(json["ownerReceipt"]["lookupStatus"], "FOUND");
    assert_eq!(json["ownerReceipt"]["receiptState"], "REPLAYABLE");
    assert_eq!(json["ownerReceipt"]["scope"], "post.v1.CreatePost");
    assert_eq!(
        chrono::DateTime::parse_from_rfc3339(
            json["ownerReceipt"]["committedAt"]
                .as_str()
                .expect("owner commit timestamp")
        )
        .expect("parse owner commit timestamp")
        .with_timezone(&chrono::Utc),
        committed_at
    );
    assert!(json.get("actorId").is_none());
    assert!(json["ownerReceipt"].get("actorId").is_none());

    // An unresolved operation is a known answer, not a missing one.
    let pending = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/admin/operations/{unresolved}"))
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(pending.status(), StatusCode::OK);
    let body = pending
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(json["state"], "UNKNOWN");
    assert_eq!(json["outcome"], "UNKNOWN");
    assert_eq!(json["responseStatus"], serde_json::Value::Null);
    assert_eq!(json["replayable"], false);
    assert_eq!(json["ownerReceipt"]["lookupStatus"], "NOT_FOUND");
    assert!(json.get("actorId").is_none());

    let mismatch = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/admin/operations/{mismatched}"))
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(mismatch.status(), StatusCode::OK);
    let body = mismatch
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(json["ownerReceipt"]["lookupStatus"], "MISMATCH");
    assert_eq!(json["outcome"], "UNKNOWN");
    assert!(json["ownerReceipt"]["receiptState"].is_null());

    let non_applicable_response = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/admin/operations/{non_applicable}"))
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(non_applicable_response.status(), StatusCode::OK);
    let body = non_applicable_response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(json["ownerReceipt"]["lookupStatus"], "NOT_APPLICABLE");

    app.set_owner_receipt_lookup_available(false);
    let unavailable_response = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/admin/operations/{unavailable}"))
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(unavailable_response.status(), StatusCode::OK);
    let body = unavailable_response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(json["state"], "UNKNOWN");
    assert_eq!(json["outcome"], "UNKNOWN");
    assert_eq!(json["ownerReceipt"]["lookupStatus"], "UNAVAILABLE");

    let missing = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/admin/operations/{never_issued}"))
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);

    app.shutdown().await.expect("app shutdown");
}

#[tokio::test]
async fn caller_operation_status_is_owner_scoped_and_privacy_minimal() {
    let app = crate::support::start_gateway_app(true).await.expect("app");
    let owner_id = "11111111-1111-1111-1111-111111111111";
    let other_id = "22222222-2222-2222-2222-222222222222";
    let owner_token = crate::support::bearer_token(owner_id, &["ROLE_USER"]);
    let other_token = crate::support::bearer_token(other_id, &["ROLE_USER"]);
    let operation_id = uuid::Uuid::new_v4();
    let other_operation_id = uuid::Uuid::new_v4();
    let stored_response = b"private stored mutation response".to_vec();

    sqlx::query(
        r#"
        insert into gateway_request_journal (
            id, actor_id, action, resource_path, state, outcome, outcome_evidence, response_status,
            response_body, created_at, finished_at
        ) values
            ($1, $3, 'POST /api/posts', '/api/posts', 'COMPLETED', 'COMMITTED', 'SUCCESS_RESPONSE', 201, $5, now(), now()),
            ($2, $4, 'POST /api/posts', '/api/posts', 'UNKNOWN', 'UNKNOWN', 'LEGACY_UNRESOLVED', null, null, now(), null)
        "#,
    )
    .bind(operation_id)
    .bind(other_operation_id)
    .bind(owner_id)
    .bind(other_id)
    .bind(&stored_response)
    .execute(&app.pool)
    .await
    .expect("seed operation journal");

    let response = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/operations/{operation_id}"))
                .header("authorization", format!("Bearer {owner_token}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(json["operationId"], operation_id.to_string());
    assert_eq!(json["state"], "COMPLETED");
    assert_eq!(json["outcome"], "COMMITTED");
    assert_eq!(json["responseStatus"], 201);
    assert_eq!(json["replayable"], true);
    assert_eq!(json.as_object().expect("object").len(), 7);
    assert!(json.get("action").is_none());
    assert!(json.get("resourcePath").is_none());
    assert!(json.get("ownerReceipt").is_none());
    assert!(json.get("manualResolution").is_none());
    assert!(!body
        .windows(stored_response.len())
        .any(|window| window == stored_response));

    // Ownership is checked in the SQL lookup. An id belonging to another user
    // is deliberately indistinguishable from one that never existed.
    let forbidden_owner = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/operations/{operation_id}"))
                .header("authorization", format!("Bearer {other_token}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(forbidden_owner.status(), StatusCode::NOT_FOUND);

    let unauthenticated = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/operations/{operation_id}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

    app.shutdown().await.expect("app shutdown");
}
