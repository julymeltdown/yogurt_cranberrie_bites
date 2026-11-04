//! Real signed RPC admission only; the recorder never grants owner authority.
#[path = "product_feature_admission_support/mod.rs"]
mod support;
use concert_service::product_rpc::{
    operation,
    proto::{product_service_client::ProductServiceClient, ProductRequest, ProductResponse},
    AgeOperation as Age, Operation,
};
use platform::{actor_delegation::HEADER, grpc_auth::Workload};
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
use support::*;
use tonic::{body::Body, codegen::Bytes, Code, Request};
use uuid::Uuid;

const START: &str = r#"{"return_target":"AGE_PRIVACY"}"#;
fn callback(state: &str, response: &str) -> String {
    serde_json::json!({
        "verification_id": Uuid::new_v4().to_string(),
        "state_token": state,
        "provider_response_token": response,
    })
    .to_string()
}
fn cases() -> Vec<(ProductRequest, Age)> {
    vec![
        (input("GET", "me/age-status", ""), Age::GetStatus),
        (input("POST", "me/age-verifications", START), Age::Start),
        (
            input(
                "POST",
                "me/age-verifications/callback",
                &callback(&"a".repeat(64), "opaque-response"),
            ),
            Age::Callback,
        ),
    ]
}
async fn refused(request: ProductRequest) {
    let owner = Recorder::new(Mode::Refuse);
    assert_ne!(
        call(server(owner.clone()), wire(&request)).await.0,
        Code::Ok
    );
    assert_eq!(owner.len(), 0, "invalid frame reached owner");
}

#[tokio::test]
async fn three_frozen_operations_reach_owner_with_original_signed_context() {
    let contracts: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../contracts/domain/v1/operations.json"
    ))
    .unwrap();
    let owner = Recorder::new(Mode::Refuse);
    let service = server(owner.clone());
    for (request, age) in cases() {
        let expected = Operation::Age(age);
        assert_eq!(
            operation(
                Uuid::parse_str(&request.event_id).unwrap(),
                &request.http_method,
                &request.http_target
            )
            .unwrap(),
            expected
        );
        let contract = contracts
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["operationId"] == age.id())
            .unwrap();
        assert_eq!(
            contract["method"].as_str().unwrap().to_uppercase(),
            request.http_method
        );
        assert_eq!(
            contract["path"]
                .as_str()
                .unwrap()
                .replace("{eventId}", &request.event_id),
            request.http_target
        );
        assert_eq!(contract["idempotency_key"].as_bool(), Some(age.mutation()));
        let before = Instant::now();
        assert_eq!(
            call(service.clone(), wire(&request)).await,
            (Code::Unavailable, REFUSAL.into())
        );
        let admissions = owner.admitted.lock().unwrap();
        let last = admissions.last().unwrap();
        assert_eq!(last.operation, expected);
        assert_eq!(last.request, request);
        assert!(last.stop <= before + Duration::from_millis(1820));
    }
    assert_eq!(owner.len(), 3);
}

#[tokio::test]
async fn exact_method_path_and_absent_query_are_required() {
    for (request, _) in cases() {
        for method in [
            "GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS", "get", "post",
        ] {
            if method != request.http_method {
                let mut changed = request.clone();
                changed.http_method = method.into();
                refused(changed).await;
            }
        }
        for suffix in [
            "?",
            "?unknown=1",
            "?state=anything",
            "/",
            "#fragment",
            "%3f",
            "\n",
        ] {
            let mut changed = request.clone();
            changed.http_target.push_str(suffix);
            refused(changed).await;
        }
        for field in 0..3 {
            let mut changed = request.clone();
            changed.http_target = match field {
                0 => changed.http_target.replace("/me/", "/users/"),
                1 => changed.http_target.replace("/age-", "/%61ge-"),
                _ => changed.http_target.replace("/me/", "/me/../me/"),
            };
            refused(changed).await;
        }
    }
    for suffix in [
        "me/age-verifications/erase",
        "me/age-status/erase",
        "me/age-claims",
    ] {
        refused(input("DELETE", suffix, "")).await;
    }
}

#[tokio::test]
async fn status_is_bodyless_and_commands_require_canonical_keys_and_json() {
    for (request, age) in cases() {
        let count = if age.mutation() { 9 } else { 3 };
        for field in 0..count {
            let mut changed = request.clone();
            if age == Age::GetStatus {
                match field {
                    0 => changed.http_body = b"{}".to_vec(),
                    1 => changed.content_type = "application/json".into(),
                    _ => changed.idempotency_key = Uuid::new_v4().to_string(),
                }
            } else {
                match field {
                    0 => changed.idempotency_key.clear(),
                    1 => changed.idempotency_key = Uuid::nil().to_string(),
                    2 => changed.idempotency_key = Uuid::new_v4().simple().to_string(),
                    3 => changed.idempotency_key = Uuid::new_v4().to_string().to_uppercase(),
                    4 => changed.content_type.clear(),
                    5 => changed.content_type = "application/json; charset=utf-8".into(),
                    6 => changed.content_type = "text/plain".into(),
                    7 => changed.http_body.clear(),
                    _ => changed.http_body = vec![b' '; 8193],
                }
            }
            refused(changed).await;
        }
    }
}

#[tokio::test]
async fn start_body_is_a_closed_object_with_one_fixed_return_target() {
    for body in [
        "null",
        "[]",
        r#"["AGE_PRIVACY"]"#,
        "{}",
        "{}{}",
        r#"{"return_target":null}"#,
        r#"{"return_target":1}"#,
        r#"{"return_target":"age_privacy"}"#,
        r#"{"return_target":"https://example.com"}"#,
        r#"{"return_target":"AGE_PRIVACY","actor_id":null}"#,
        r#"{"return_target":"AGE_PRIVACY","return_target":"AGE_PRIVACY"}"#,
        r#"{"return_target":"AGE_PRIVACY","return_\u0074arget":"AGE_PRIVACY"}"#,
    ] {
        refused(input("POST", "me/age-verifications", body)).await;
    }
}

#[tokio::test]
async fn callback_rejects_nonobjects_duplicates_unknown_fields_and_token_overflow() {
    let valid = callback("state", "response");
    for body in ["null", "[]", "{}", "{}{}", r#"["id","state","response"]"#] {
        refused(input("POST", "me/age-verifications/callback", body)).await;
    }
    for field in ["verification_id", "state_token", "provider_response_token"] {
        for replacement in [
            serde_json::Value::Null,
            serde_json::json!(1),
            serde_json::json!(""),
        ] {
            let mut body: serde_json::Value = serde_json::from_str(&valid).unwrap();
            body[field] = replacement;
            refused(input(
                "POST",
                "me/age-verifications/callback",
                &body.to_string(),
            ))
            .await;
        }
        let mut body: serde_json::Value = serde_json::from_str(&valid).unwrap();
        body.as_object_mut().unwrap().remove(field);
        refused(input(
            "POST",
            "me/age-verifications/callback",
            &body.to_string(),
        ))
        .await;
        let duplicate = format!("{},\"{field}\":\"x\"}}", &valid[..valid.len() - 1]);
        refused(input("POST", "me/age-verifications/callback", &duplicate)).await;
    }
    for id in [
        Uuid::nil().to_string(),
        Uuid::new_v4().simple().to_string(),
        Uuid::new_v4().to_string().to_uppercase(),
    ] {
        let mut body: serde_json::Value = serde_json::from_str(&valid).unwrap();
        body["verification_id"] = id.into();
        refused(input(
            "POST",
            "me/age-verifications/callback",
            &body.to_string(),
        ))
        .await;
    }
    let unknown = format!("{},\"birthyear\":\"2000\"}}", &valid[..valid.len() - 1]);
    refused(input("POST", "me/age-verifications/callback", &unknown)).await;
    for (state, response) in [
        ("a".repeat(513), "a".into()),
        ("a".into(), "a".repeat(4097)),
        ("state\n".into(), "response".into()),
        ("state".into(), "response\0".into()),
    ] {
        refused(input(
            "POST",
            "me/age-verifications/callback",
            &callback(&state, &response),
        ))
        .await;
    }
    let encoded_duplicate = format!(
        "{},\"state_\\u0074oken\":\"x\"}}",
        &valid[..valid.len() - 1]
    );
    refused(input(
        "POST",
        "me/age-verifications/callback",
        &encoded_duplicate,
    ))
    .await;
    let mut invalid_utf8 = input("POST", "me/age-verifications/callback", &valid);
    invalid_utf8.http_body.push(255);
    refused(invalid_utf8).await;
}

#[tokio::test]
async fn frozen_token_boundaries_and_signed_bytes_are_preserved_without_proof_acceptance() {
    let owner = Recorder::new(Mode::Refuse);
    let service = server(owner.clone());
    for body in [
        callback("a", "a"),
        callback(&"a".repeat(512), &"a".repeat(4096)),
        callback(&"é".repeat(512), "opaque"),
        format!("{}{}", START, " ".repeat(8192 - START.len())),
    ] {
        let suffix = if body.contains("return_target") {
            "me/age-verifications"
        } else {
            "me/age-verifications/callback"
        };
        let request = input("POST", suffix, &body);
        assert_eq!(
            call(service.clone(), wire(&request)).await,
            (Code::Unavailable, REFUSAL.into())
        );
        assert_eq!(
            owner.admitted.lock().unwrap().last().unwrap().request,
            request
        );
    }
    assert_eq!(owner.len(), 4);
}

#[tokio::test]
async fn signed_identity_parent_and_frame_tampering_never_acquire_owner() {
    for (request, _) in cases() {
        for field in 0..7 {
            let mut changed = request.clone();
            match field {
                0 => changed.user_id = Uuid::new_v4().to_string(),
                1 => changed.event_id = Uuid::new_v4().to_string(),
                2 => changed.session_family_id = Uuid::nil().to_string(),
                3 => changed.request_id = Uuid::nil().to_string(),
                4 => {
                    changed.authority_expires_unix_micros =
                        chrono::Utc::now().timestamp_micros() - 1
                }
                5 => changed.profile_observed_at_unix_micros = 0,
                _ => changed.trusted_network_bucket = vec![0; 32],
            }
            let owner = Recorder::new(Mode::Refuse);
            let actor = Uuid::parse_str(&request.user_id).unwrap();
            call(
                server(owner.clone()),
                signed(&changed, actor, Some("1800000u")),
            )
            .await;
            assert_eq!(owner.len(), 0);
        }
        for field in 0..5 {
            let mut changed = request.clone();
            match field {
                0 => changed.http_body.extend_from_slice(b" "),
                1 => changed.http_target.push('/'),
                2 => changed.http_method = "DELETE".into(),
                3 => changed.idempotency_key = Uuid::new_v4().to_string(),
                _ => changed.content_type.push('x'),
            }
            let mut signed = wire(&request);
            *signed.body_mut() = Body::new(http_body_util::Full::new(Bytes::from(frame(&changed))));
            let owner = Recorder::new(Mode::Refuse);
            call(server(owner.clone()), signed).await;
            assert_eq!(owner.len(), 0);
        }
    }
}

#[tokio::test]
async fn original_short_deadline_cancels_age_producer() {
    let owner = Recorder::new(Mode::Hold);
    let request = input("POST", "me/age-verifications", START);
    let started = Instant::now();
    assert_eq!(
        call(
            server(owner.clone()),
            signed(
                &request,
                Uuid::parse_str(&request.user_id).unwrap(),
                Some("40000u")
            )
        )
        .await
        .0,
        Code::DeadlineExceeded
    );
    assert_eq!(owner.len(), 1);
    assert_eq!(owner.dropped.load(Ordering::SeqCst), 1);
    assert!(started.elapsed() < Duration::from_millis(500));
}

#[tokio::test]
async fn even_provider_unavailable_and_success_dtos_require_real_delivery_authority() {
    for (status, body) in [
        (200, r#"{"status":"VERIFIED"}"#),
        (
            200,
            r#"{"status":"PROVIDER_UNAVAILABLE","verification_available":false}"#,
        ),
        (503, r#"{"details":{"reason":"PROVIDER_UNAVAILABLE"}}"#),
    ] {
        for (request, _) in cases() {
            let owner = Recorder::new(Mode::Response(ProductResponse {
                status,
                content_type: "application/json".into(),
                body: body.as_bytes().to_vec(),
                ..Default::default()
            }));
            let result = call(server(owner.clone()), wire(&request)).await;
            assert_eq!(
                result,
                (
                    Code::Unavailable,
                    "Canonical product delivery scope required".into()
                )
            );
            assert_eq!(owner.len(), 1);
        }
    }
}

#[tokio::test]
#[ignore = "requires the existing explicitly owned fresh CF03 test PKI; no PG"]
async fn signed_gateway_mtls_admits_three_age_operations_and_refuses_wrong_role(
) -> anyhow::Result<()> {
    let owner = Recorder::new(Mode::Refuse);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("https://{}", listener.local_addr()?);
    let incoming = futures::stream::unfold(listener, |l| async {
        Some((l.accept().await.map(|(s, _)| s), l))
    });
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let service = tonic::transport::Server::builder()
        .layer(platform::call_deadline::CallDeadlineLayer)
        .tls_config(tls(Workload::Concert)?.server_config()?)?
        .add_service(server(owner.clone()));
    let task = tokio::spawn(service.serve_with_incoming_shutdown(incoming, async {
        let _ = stopped.await;
    }));
    struct Abort(tokio::task::AbortHandle);
    impl Drop for Abort {
        fn drop(&mut self) {
            self.0.abort();
        }
    }
    let _abort = Abort(task.abort_handle());
    for workload in [Workload::Gateway, Workload::Friend] {
        let channel = tls(workload)?.connect(&endpoint, Workload::Concert).await?;
        let mut client = ProductServiceClient::new(channel);
        for (request, age) in cases() {
            let signed = wire(&request);
            let mut wire_request = Request::new(request.clone());
            for name in ["x-service-caller", "x-service-token", HEADER] {
                wire_request
                    .metadata_mut()
                    .insert(name, signed.headers()[name].to_str()?.parse()?);
            }
            wire_request.set_timeout(Duration::from_millis(1800));
            let count = owner.len();
            let result = client.dispatch(wire_request).await.unwrap_err();
            if workload == Workload::Gateway {
                assert_eq!(result.code(), Code::Unavailable);
                assert_eq!(result.message(), REFUSAL);
                assert_eq!(owner.len(), count + 1);
                let admissions = owner.admitted.lock().unwrap();
                assert_eq!(admissions.last().unwrap().operation, Operation::Age(age));
                assert_eq!(admissions.last().unwrap().request, request);
            } else {
                assert_eq!(result.code(), Code::Unauthenticated);
                assert_eq!(owner.len(), count);
            }
        }
    }
    stop.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(2), task).await???;
    assert_eq!(owner.len(), 3);
    Ok(())
}
