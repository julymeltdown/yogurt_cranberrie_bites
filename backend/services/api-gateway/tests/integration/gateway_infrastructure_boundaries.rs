use api_gateway::adapters::out::repository::{
    aggregation_recipe_repository::PgAggregationRecipeRepository,
    client_profile_repository::PgClientProfileRepository,
    policy_repository::{PgPolicyRepository, PgRolloutRepository},
    route_contract_repository::PgRouteContractRepository,
    telemetry_repository::PgTelemetryRepository,
};
use api_gateway::adapters::r#in::http::map_service_error;
use api_gateway::application::dto::infrastructure_dto::{
    ClientProfileDto, PolicySetDto, RolloutPolicyDto, RouteContractDto, RouteContractVersionDto,
};
use api_gateway::application::infrastructure_error::GatewayInfrastructureError;
use api_gateway::application::ports::{
    AggregationRecipeRepositoryPort, ClientProfileRepositoryPort, PolicyRepositoryPort,
    RolloutRepositoryPort, RouteContractRepositoryPort, TelemetryRecord, TelemetryRepositoryPort,
};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use chrono::Utc;
use http_body_util::BodyExt;
use sqlx::postgres::PgPoolOptions;
use std::collections::HashMap;

fn contract(id: &str, route_key: &str) -> RouteContractDto {
    RouteContractDto {
        id: id.into(),
        route_key: route_key.into(),
        owner: "owner-canary-秘密".into(),
        sla_target: Some("99.9%".into()),
        status: "ACTIVE".into(),
        description: Some("description-canary".into()),
        versions: vec![],
    }
}
fn version() -> RouteContractVersionDto {
    RouteContractVersionDto {
        version: "v2".into(),
        status: "DEPRECATED".into(),
        deprecation_date: Some(Utc::now()),
        response_schema_ref: "schema-canary".into(),
    }
}
fn policy() -> PolicySetDto {
    PolicySetDto {
        id: "".into(),
        name: "policy-canary".into(),
        rate_limit: Some("30/min".into()),
        quota: Some("100/day".into()),
        timeout_ms: Some(500),
        retry_policy: Some("retry-0".into()),
        circuit_breaker_policy: Some("closed".into()),
        cache_policy: Some("default".into()),
    }
}
fn record(id: &str, outcome: &str, latency_ms: i64) -> TelemetryRecord {
    TelemetryRecord {
        id: id.into(),
        correlation_id: "correlation-canary".into(),
        route_key: "GET /boundary".into(),
        client_profile_id: None,
        outcome: outcome.into(),
        latency_ms,
        error_code: None,
        downstream_latencies: HashMap::from([("owner-canary".into(), 3)]),
        recorded_at: Utc::now(),
    }
}

#[tokio::test]
async fn infrastructure_transfers_round_trip_real_pg_and_preserve_missing_records(
) -> anyhow::Result<()> {
    let app = crate::support::start_gateway_app(false).await?;
    let routes = PgRouteContractRepository::new(app.pool.clone());
    let policies = PgPolicyRepository::new(app.pool.clone());
    let rollouts = PgRolloutRepository::new(app.pool.clone());
    let profiles = PgClientProfileRepository::new(app.pool.clone());
    let recipes = PgAggregationRecipeRepository::new(app.pool.clone());
    let telemetry = PgTelemetryRepository::new(app.pool.clone());
    assert!(routes.find_by_id("missing").await?.is_none());
    assert!(routes.find_version("missing", "v2").await?.is_none());
    assert!(policies.find_by_id("missing").await?.is_none());
    assert!(rollouts.find_by_id("missing").await?.is_none());
    assert!(profiles.find_by_id("missing").await?.is_none());
    assert!(recipes.find_by_route_key("missing").await?.is_none());
    let mut saved = routes.save(contract("", "GET /boundary")).await?;
    assert!(uuid::Uuid::parse_str(&saved.id).is_ok());
    let mut expected_version = version();
    // PG timestamps retain microseconds; compare the exact value PG can store.
    expected_version.deprecation_date = Some(
        chrono::DateTime::from_timestamp_micros(
            expected_version
                .deprecation_date
                .unwrap()
                .timestamp_micros(),
        )
        .unwrap(),
    );
    assert_eq!(
        routes
            .save_version(&saved.id, expected_version.clone())
            .await?,
        expected_version
    );
    assert_eq!(
        routes.find_version(&saved.id, "v2").await?,
        Some(expected_version.clone())
    );
    saved.versions.push(expected_version);
    assert_eq!(routes.find_by_id(&saved.id).await?, Some(saved.clone()));
    assert_eq!(routes.find_all().await?, vec![saved.clone()]);
    let mut updated = saved.clone();
    updated.owner = "updated-owner".into();
    assert_eq!(routes.save(updated.clone()).await?, updated);
    assert_eq!(routes.find_by_id(&saved.id).await?, Some(updated));
    let saved_policy = policies.save(policy()).await?;
    assert!(uuid::Uuid::parse_str(&saved_policy.id).is_ok());
    assert_eq!(
        policies.find_by_id(&saved_policy.id).await?,
        Some(saved_policy.clone())
    );
    assert_eq!(policies.find_all().await?, vec![saved_policy.clone()]);
    let rollout = rollouts
        .save(RolloutPolicyDto {
            id: "".into(),
            contract_id: saved.id.clone(),
            version: "v2".into(),
            strategy: "CANARY".into(),
            status: "ACTIVE".into(),
            percent: Some(25),
        })
        .await?;
    assert!(uuid::Uuid::parse_str(&rollout.id).is_ok());
    assert_eq!(
        rollouts.find_by_id(&rollout.id).await?,
        Some(rollout.clone())
    );
    let profile = profiles
        .save(ClientProfileDto {
            id: "".into(),
            client_type: "WEB".into(),
            allowed_contracts: vec![saved.id.clone(), "unicode-秘密".into()],
            policy_set_id: Some(saved_policy.id.clone()),
        })
        .await?;
    assert!(uuid::Uuid::parse_str(&profile.id).is_ok());
    assert_eq!(
        profiles.find_by_id(&profile.id).await?,
        Some(profile.clone())
    );
    assert_eq!(profiles.find_all().await?, vec![profile.clone()]);
    sqlx::query("insert into aggregation_recipes (id,route_key,downstream_calls,join_strategy,timeout_ms,cache_policy) values ($1,$2,$3,$4,$5,$6)")
        .bind("recipe-id").bind("GET /boundary")
        .bind(r#"[{"service":"owner-canary","path":"/path","fields":["id","unicode-秘密"]}]"#)
        .bind("MERGE").bind(123_i32).bind("default").execute(&app.pool).await?;
    let recipe = recipes.find_by_route_key("GET /boundary").await?.unwrap();
    assert_eq!(
        recipe.downstream_calls[0].fields,
        vec!["id", "unicode-秘密"]
    );
    assert_eq!(recipe.timeout_ms, Some(123));
    assert_eq!(recipes.find_all().await?, vec![recipe.clone()]);
    // Retain the existing malformed stored-JSON fallback; this migration does not
    // introduce a validation or repair policy for historical administrative data.
    sqlx::query(
        "update aggregation_recipes set downstream_calls = 'invalid-json' where id = 'recipe-id'",
    )
    .execute(&app.pool)
    .await?;
    assert!(recipes
        .find_by_route_key("GET /boundary")
        .await?
        .unwrap()
        .downstream_calls
        .is_empty());
    assert_eq!(telemetry.summary().await?.total_requests, 0);
    telemetry.record(record("one", "success", 10)).await?;
    telemetry.record(record("two", "FAILURE", 100)).await?;
    let summary = telemetry.summary().await?;
    assert_eq!(summary.total_requests, 2);
    assert_eq!(summary.error_rate, 0.5);
    assert_eq!(summary.p95_latency_ms, 100);
    for debug in [
        format!("{saved:?}"),
        format!("{saved_policy:?}"),
        format!("{rollout:?}"),
        format!("{profile:?}"),
        format!("{recipe:?}"),
    ] {
        assert!(!debug.contains("canary"));
        assert!(!debug.contains("秘密"));
    }
    let duplicate = routes
        .save(contract("duplicate-id", "GET /boundary"))
        .await
        .unwrap_err();
    assert_eq!(duplicate, GatewayInfrastructureError::Storage);
    assert!(std::error::Error::source(&duplicate).is_none());
    assert_eq!(routes.find_all().await?.len(), 1);
    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn all_seventeen_pg_methods_refuse_closed_storage_without_diagnostics_or_effects(
) -> anyhow::Result<()> {
    let app = crate::support::start_gateway_app(false).await?;
    let closed = PgPoolOptions::new()
        .connect_with((*app.pool.connect_options()).clone())
        .await?;
    closed.close().await;
    let routes = PgRouteContractRepository::new(closed.clone());
    let policies = PgPolicyRepository::new(closed.clone());
    let rollouts = PgRolloutRepository::new(closed.clone());
    let profiles = PgClientProfileRepository::new(closed.clone());
    let recipes = PgAggregationRecipeRepository::new(closed.clone());
    let telemetry = PgTelemetryRepository::new(closed);
    let errors = [
        routes.find_all().await.unwrap_err(),
        routes.find_by_id("secret-id").await.unwrap_err(),
        routes.find_version("secret-id", "v2").await.unwrap_err(),
        routes
            .save(contract("secret-id", "GET /canary"))
            .await
            .unwrap_err(),
        routes
            .save_version("secret-id", version())
            .await
            .unwrap_err(),
        policies.find_all().await.unwrap_err(),
        policies.find_by_id("secret-id").await.unwrap_err(),
        policies.save(policy()).await.unwrap_err(),
        rollouts.find_by_id("secret-id").await.unwrap_err(),
        rollouts
            .save(RolloutPolicyDto {
                id: "secret-id".into(),
                contract_id: "secret-id".into(),
                version: "v2".into(),
                strategy: "CANARY".into(),
                status: "ACTIVE".into(),
                percent: None,
            })
            .await
            .unwrap_err(),
        profiles.find_all().await.unwrap_err(),
        profiles.find_by_id("secret-id").await.unwrap_err(),
        profiles
            .save(ClientProfileDto {
                id: "secret-id".into(),
                client_type: "WEB".into(),
                allowed_contracts: vec!["canary".into()],
                policy_set_id: None,
            })
            .await
            .unwrap_err(),
        recipes.find_all().await.unwrap_err(),
        recipes.find_by_route_key("GET /canary").await.unwrap_err(),
        telemetry
            .record(record("secret-id", "SUCCESS", 1))
            .await
            .unwrap_err(),
        telemetry.summary().await.unwrap_err(),
    ];
    for error in errors {
        assert_eq!(error, GatewayInfrastructureError::Storage);
        assert!(std::error::Error::source(&error).is_none());
        let response = map_service_error(error).into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert!(!response
            .headers()
            .contains_key(axum::http::header::RETRY_AFTER));
        let body: serde_json::Value =
            serde_json::from_slice(&response.into_body().collect().await?.to_bytes())?;
        assert_eq!(body["message"], "Internal error");
        assert_eq!(body["code"], "GATEWAY_500");
        assert!(body["details"].is_null());
    }
    for table in [
        "route_contracts",
        "route_contract_versions",
        "policy_sets",
        "client_profiles",
        "aggregation_recipes",
        "rollouts",
        "telemetry_records",
    ] {
        let count: i64 = sqlx::query_scalar(&format!("select count(*) from {table}"))
            .fetch_one(&app.pool)
            .await?;
        assert_eq!(count, 0, "failed operation must not persist in {table}");
    }
    app.shutdown().await?;
    Ok(())
}
