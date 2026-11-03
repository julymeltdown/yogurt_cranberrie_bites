//! Wire regressions compare explicit facade records with the existing serialized
//! domain contract. Repository records remain independent and unserialized.
use super::facade_dto::*;
use crate::dto::infrastructure_dto as repository;
use chrono::{TimeZone, Utc};
use serde::Serialize;
use serde_json::json;

fn same_wire(old: &impl Serialize, new: &impl Serialize) {
    assert_eq!(
        serde_json::to_vec(old).unwrap(),
        serde_json::to_vec(new).unwrap()
    );
}

#[test]
fn governance_response_records_preserve_names_nulls_order_and_timestamp_precision() {
    let version = gateway_domain::contract::RouteContractVersion {
        version: "v2-東京".into(),
        status: "deprecated".into(),
        deprecation_date: Some(Utc.with_ymd_and_hms(2030, 2, 3, 4, 5, 6).unwrap()),
        response_schema_ref: "private-schema-value".into(),
    };
    let mut nullable = version.clone();
    nullable.deprecation_date = None;
    let contract = gateway_domain::contract::RouteContract {
        id: "private-contract-id".into(),
        route_key: "GET /東京".into(),
        owner: "owner".into(),
        sla_target: None,
        status: "draft".into(),
        description: Some("说明\nline".into()),
        versions: vec![version.clone(), nullable.clone()],
    };
    let dto = RouteContractResponseDto::from(contract.clone());
    same_wire(&contract, &dto);
    assert_eq!(
        serde_json::to_value(&dto).unwrap(),
        json!({
            "id":"private-contract-id", "routeKey":"GET /東京", "owner":"owner", "slaTarget":null,
            "status":"draft", "description":"说明\nline", "versions":[
                {"version":"v2-東京","status":"deprecated","deprecationDate":"2030-02-03T04:05:06Z","responseSchemaRef":"private-schema-value"},
                {"version":"v2-東京","status":"deprecated","deprecationDate":null,"responseSchemaRef":"private-schema-value"}
            ]
        })
    );
    assert!(!format!("{dto:?}").contains("private-contract-id"));
    assert!(!format!("{dto:?}").contains("private-schema-value"));
    let transfer = repository::RouteContractDto::from(dto.clone());
    assert_eq!(RouteContractResponseDto::from(transfer), dto);
    assert_eq!(gateway_domain::contract::RouteContract::from(dto), contract);
    for value in [version, nullable] {
        let dto = RouteContractVersionResponseDto::from(value.clone());
        same_wire(&value, &dto);
        let transfer = repository::RouteContractVersionDto::from(dto.clone());
        assert_eq!(RouteContractVersionResponseDto::from(transfer), dto);
        assert_eq!(
            gateway_domain::contract::RouteContractVersion::from(dto),
            value
        );
    }
    let precise = gateway_domain::contract::RouteContractVersion {
        version: "precise".into(),
        status: "active".into(),
        deprecation_date: Some(
            chrono::DateTime::parse_from_rfc3339("2030-02-03T04:05:06.123456789Z")
                .unwrap()
                .with_timezone(&Utc),
        ),
        response_schema_ref: "schema".into(),
    };
    let dto = RouteContractVersionResponseDto::from(precise.clone());
    same_wire(&precise, &dto);
    assert_eq!(
        serde_json::to_value(dto).unwrap()["deprecationDate"],
        "2030-02-03T04:05:06.123456789Z"
    );
}

#[test]
fn policy_client_rollout_and_summary_wire_and_repository_transfers_remain_equal() {
    let policy = gateway_domain::policy::PolicySet {
        id: "policy-1".into(),
        name: "private-policy-name".into(),
        rate_limit: Some("12/minute".into()),
        quota: None,
        timeout_ms: Some(i32::MAX),
        retry_policy: Some("bounded".into()),
        circuit_breaker_policy: None,
        cache_policy: Some("private-cache-policy".into()),
    };
    let dto = PolicySetResponseDto::from(policy.clone());
    same_wire(&policy, &dto);
    assert_eq!(serde_json::to_value(&dto).unwrap()["timeoutMs"], i32::MAX);
    assert!(serde_json::to_value(&dto).unwrap()["quota"].is_null());
    assert!(!format!("{dto:?}").contains("private-policy-name"));
    assert_eq!(
        PolicySetResponseDto::from(repository::PolicySetDto::from(dto.clone())),
        dto
    );
    assert_eq!(gateway_domain::policy::PolicySet::from(dto), policy);
    let client = gateway_domain::client::ClientProfile {
        id: "private-client-id".into(),
        client_type: "desktop".into(),
        allowed_contracts: vec!["second".into(), "first".into()],
        policy_set_id: None,
    };
    let dto = ClientProfileResponseDto::from(client.clone());
    same_wire(&client, &dto);
    assert_eq!(
        serde_json::to_value(&dto).unwrap()["allowedContracts"],
        json!(["second", "first"])
    );
    assert!(!format!("{dto:?}").contains("private-client-id"));
    assert_eq!(
        ClientProfileResponseDto::from(repository::ClientProfileDto::from(dto.clone())),
        dto
    );
    assert_eq!(gateway_domain::client::ClientProfile::from(dto), client);
    let rollout = gateway_domain::policy::RolloutPolicy {
        id: "rollout-1".into(),
        contract_id: "private-rollout-contract".into(),
        version: "v2".into(),
        strategy: "canary".into(),
        status: "active".into(),
        percent: Some(100),
    };
    let dto = RolloutPolicyResponseDto::from(rollout.clone());
    same_wire(&rollout, &dto);
    assert!(!format!("{dto:?}").contains("private-rollout-contract"));
    assert_eq!(
        RolloutPolicyResponseDto::from(repository::RolloutPolicyDto::from(dto.clone())),
        dto
    );
    assert_eq!(gateway_domain::policy::RolloutPolicy::from(dto), rollout);
    for rate in [0.0, 0.25, -0.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let value = gateway_domain::telemetry::TelemetrySummary {
            total_requests: i64::MAX,
            error_rate: rate,
            p95_latency_ms: i64::MAX,
        };
        let dto = TelemetrySummaryResponseDto::from(value.clone());
        same_wire(&value, &dto);
        let restored =
            TelemetrySummaryResponseDto::from(repository::TelemetrySummaryDto::from(dto.clone()));
        assert_eq!(restored.total_requests, dto.total_requests);
        assert_eq!(restored.p95_latency_ms, dto.p95_latency_ms);
        assert_eq!(restored.error_rate.to_bits(), dto.error_rate.to_bits());
        let restored = gateway_domain::telemetry::TelemetrySummary::from(dto);
        assert_eq!(restored.error_rate.to_bits(), rate.to_bits());
    }
}

#[test]
fn provider_and_session_selection_transfers_preserve_all_domain_meanings() {
    use gateway_domain::IdentityProvider as DomainProvider;
    for provider in [
        DomainProvider::Email,
        DomainProvider::Google,
        DomainProvider::Apple,
        DomainProvider::X,
    ] {
        let dto = IdentityProviderDto::from(provider);
        same_wire(&provider, &dto);
        assert_eq!(DomainProvider::from(dto), provider);
    }
    use gateway_domain::high_risk::{
        HighRiskAction as DomainAction, SessionDecision as DomainDecision, SessionState,
    };
    for action in [DomainAction::Administration, DomainAction::AccountSecurity] {
        let dto = HighRiskActionDto::from(action);
        assert_eq!(dto.as_str(), action.as_str());
        assert_eq!(DomainAction::from(dto), action);
        for state in [
            SessionState::Ended,
            SessionState::Active { roles: vec![] },
            SessionState::Active {
                roles: vec!["ROLE_ADMIN".into()],
            },
        ] {
            let before = state.permits(action);
            let transferred = SessionDecisionDto::from(state.permits(dto.into()));
            assert_eq!(DomainDecision::from(transferred), before);
        }
    }
    for decision in [
        DomainDecision::Allowed,
        DomainDecision::SessionEnded,
        DomainDecision::RoleRevoked,
    ] {
        assert_eq!(
            DomainDecision::from(SessionDecisionDto::from(decision)),
            decision
        );
    }
}
