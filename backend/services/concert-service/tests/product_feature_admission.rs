//! Actual signed Product RPC admission, not positive owner/public acceptance.
#[path = "product_feature_admission_support/mod.rs"]
mod support;
use concert_service::{
    notification_live::NotificationOperation as Notification,
    product_rpc::{
        operation,
        proto::{product_service_client::ProductServiceClient, ProductRequest, ProductResponse},
        IntroOperation as Intro, Operation,
    },
};
use platform::{actor_delegation::HEADER, grpc_auth::Workload};
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
use support::*;
use tonic::{body::Body, codegen::Bytes, Code, Request};
use uuid::Uuid;

const SET: &str = r#"{"expected_revision":0,"text":"Hello","age_bounds":{"minimum_age":null,"maximum_age_exclusive":null},"expires_at":null}"#;
const REV: &str = r#"{"expected_revision":1}"#;

#[test]
fn existing_route_families_keep_their_exact_contract_mapping() {
    use concert_domain::relationship::social::Platform;
    use concert_service::social_live::protocol::SocialOperation as Social;
    let event = Uuid::new_v4();
    let id = Uuid::new_v4();
    let user = Uuid::new_v4();
    for (method, suffix, expected) in [
        ("GET", "feed".into(), Operation::Feed),
        ("GET", format!("posts/{id}"), Operation::Post(id)),
        (
            "GET",
            format!("posts/{id}/comments"),
            Operation::Comments(id),
        ),
        ("GET", "parties".into(), Operation::ListParties),
        ("GET", format!("parties/{id}"), Operation::GetParty(id)),
        ("POST", "parties".into(), Operation::CreateParty),
        (
            "POST",
            format!("parties/{id}/applications"),
            Operation::ApplyParty(id),
        ),
        (
            "POST",
            format!("party-applications/{id}/decision"),
            Operation::DecidePartyApplication(id),
        ),
        (
            "DELETE",
            format!("party-applications/{id}?expected_revision=1"),
            Operation::CancelPartyApplication(id),
        ),
        (
            "DELETE",
            format!("parties/{id}/members/{user}?expected_revision=1&party_revision=1"),
            Operation::RemovePartyMember(id, user),
        ),
        (
            "POST",
            format!("parties/{id}/cancel"),
            Operation::CancelParty(id),
        ),
        ("POST", "media/uploads".into(), Operation::InitializeUpload),
        ("GET", format!("media/{id}"), Operation::MediaMetadata(id)),
        (
            "PUT",
            format!("media/{id}/bytes"),
            Operation::UploadBytes(id),
        ),
        (
            "POST",
            format!("media/{id}/finalize"),
            Operation::FinalizeUpload(id),
        ),
        (
            "GET",
            format!("media/{id}/content"),
            Operation::PrivateContent(id),
        ),
        ("GET", "commands".into(), Operation::CommandStatus),
    ] {
        assert_eq!(
            operation(
                event,
                method,
                &format!("/api/concert/events/{event}/{suffix}")
            )
            .unwrap(),
            expected
        );
    }
    // Use the exact pre-existing Social parser/type for all thirteen routes.
    for (method, suffix, expected) in [
        ("GET", "me/social-handles".into(), Social::Handles),
        (
            "PUT",
            "me/social-handles/INSTAGRAM".into(),
            Social::SetHandle(Platform::Instagram),
        ),
        (
            "DELETE",
            "me/social-handles/INSTAGRAM".into(),
            Social::RemoveHandle(Platform::Instagram),
        ),
        ("POST", "social-requests".into(), Social::Create),
        ("GET", "me/social-requests".into(), Social::Requests),
        ("GET", format!("social-requests/{id}"), Social::Request(id)),
        (
            "POST",
            format!("social-requests/{id}/decision"),
            Social::Decide(id),
        ),
        (
            "DELETE",
            format!("social-requests/{id}"),
            Social::Cancel(id),
        ),
        (
            "POST",
            format!("social-requests/{id}/revoke"),
            Social::Revoke(id),
        ),
        (
            "GET",
            format!("social-requests/{id}/reveal"),
            Social::Reveal(id),
        ),
        ("GET", "me/blocks".into(), Social::Blocks),
        ("PUT", format!("me/blocks/{user}"), Social::Block(user)),
        ("DELETE", format!("me/blocks/{user}"), Social::Unblock(user)),
    ] {
        assert_eq!(
            operation(
                event,
                method,
                &format!("/api/concert/events/{event}/{suffix}")
            )
            .unwrap(),
            Operation::Social(expected)
        );
    }
    for (method, suffix) in [
        ("POST", "intros"),
        ("GET", "intros/../feed"),
        ("GET", "me/intro/"),
        ("GET", "me/notifications/"),
        ("POST", "me/notifications/read-all"),
        ("PATCH", "me/intro"),
        (
            "GET",
            "me/notifications/00000000-0000-0000-0000-000000000000/read",
        ),
        ("POST", "feed"),
    ] {
        assert!(operation(
            event,
            method,
            &format!("/api/concert/events/{event}/{suffix}")
        )
        .is_err());
    }
}
fn cases() -> Vec<(ProductRequest, Operation)> {
    let intro = Uuid::new_v4();
    let notification = Uuid::new_v4();
    vec![
        (
            input("GET", "intros?limit=100&q=hello+world&cursor=Ab_9-", ""),
            Operation::Intro(Intro::List),
        ),
        (
            input("GET", &format!("intros/{intro}"), ""),
            Operation::Intro(Intro::Get(intro)),
        ),
        (input("GET", "me/intro", ""), Operation::Intro(Intro::Own)),
        (input("PUT", "me/intro", SET), Operation::Intro(Intro::Set)),
        (
            input("POST", "me/intro/publish", REV),
            Operation::Intro(Intro::Publish),
        ),
        (
            input("POST", "me/intro/close", REV),
            Operation::Intro(Intro::Close),
        ),
        (
            input(
                "GET",
                "me/notifications?limit=100&unread_only=true&cursor=Ab_9-",
                "",
            ),
            Operation::Notification(Notification::List),
        ),
        (
            input("GET", "me/notifications/unread-count", ""),
            Operation::Notification(Notification::UnreadCount),
        ),
        (
            input(
                "PATCH",
                &format!("me/notifications/{notification}/read"),
                REV,
            ),
            Operation::Notification(Notification::MarkRead(notification)),
        ),
        (
            input("PATCH", "me/notifications/read-all", "{}"),
            Operation::Notification(Notification::MarkAllRead),
        ),
    ]
}
async fn refused(request: ProductRequest) {
    let owner = Recorder::new(Mode::Refuse);
    let result = call(server(owner.clone()), wire(&request)).await;
    assert_ne!(result.0, Code::Ok, "must refuse malformed request");
    assert_eq!(
        owner.len(),
        0,
        "malformed frame reached producer: {request:?}, {result:?}"
    );
}

#[tokio::test]
async fn all_ten_frozen_operations_reach_producer_with_exact_signed_context() {
    let owner = Recorder::new(Mode::Refuse);
    let service = server(owner.clone());
    let contracts: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../contracts/domain/v1/operations.json"
    ))
    .unwrap();
    for (request, expected) in cases() {
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
            .find(|c| c["operationId"] == expected.contract_id())
            .unwrap();
        assert_eq!(
            contract["method"].as_str().unwrap().to_uppercase(),
            request.http_method
        );
        let mut path = contract["path"]
            .as_str()
            .unwrap()
            .replace("{eventId}", &request.event_id);
        match expected {
            Operation::Intro(Intro::Get(id)) => path = path.replace("{introId}", &id.to_string()),
            Operation::Notification(Notification::MarkRead(id)) => {
                path = path.replace("{notificationId}", &id.to_string())
            }
            _ => {}
        }
        assert_eq!(path, request.http_target.split('?').next().unwrap());
        let before = Instant::now();
        assert_eq!(
            call(service.clone(), wire(&request)).await,
            (Code::Unavailable, REFUSAL.into())
        );
        let admitted = owner.admitted.lock().unwrap();
        let last = admitted.last().unwrap();
        assert_eq!(last.operation, expected);
        assert_eq!(last.request, request);
        assert!(last.stop <= before + Duration::from_millis(1820));
    }
    assert_eq!(owner.len(), 10);
}

#[tokio::test]
async fn wire_boundary_values_remain_admissible_without_normalizing_signed_bytes() {
    let owner = Recorder::new(Mode::Refuse);
    let service = server(owner.clone());
    let mut inputs = vec![
        input("GET", "intros", ""),
        input("GET", "intros?limit=1", ""),
        input("GET", &format!("intros?q={}", "a".repeat(100)), ""),
        input("GET", "me/notifications", ""),
        input("GET", "me/notifications?limit=1&unread_only=false", ""),
        input("PUT", "me/intro", &SET.replace("Hello", "")),
        input("PUT", "me/intro", &SET.replace("Hello", &"a".repeat(1000))),
        input(
            "PUT",
            "me/intro",
            &SET.replace(
                "\"expected_revision\":0",
                "\"expected_revision\":9007199254740991",
            ),
        ),
        input(
            "PUT",
            "me/intro",
            &SET.replace(
                "\"expires_at\":null",
                "\"expires_at\":\"2099-01-01T00:00:00Z\"",
            ),
        ),
        input(
            "POST",
            "me/intro/publish",
            r#"{"expected_revision":9007199254740991}"#,
        ),
        input(
            "PATCH",
            &format!("me/notifications/{}/read", Uuid::new_v4()),
            r#"{"expected_revision":9007199254740991}"#,
        ),
    ];
    for (minimum, maximum) in [(14, 15), (100, 101)] {
        inputs.push(input(
            "PUT",
            "me/intro",
            &SET.replace(
                "\"minimum_age\":null",
                &format!("\"minimum_age\":{minimum}"),
            )
            .replace(
                "\"maximum_age_exclusive\":null",
                &format!("\"maximum_age_exclusive\":{maximum}"),
            ),
        ));
    }
    for r in inputs {
        assert_eq!(
            call(service.clone(), wire(&r)).await,
            (Code::Unavailable, REFUSAL.into())
        );
        assert_eq!(owner.admitted.lock().unwrap().last().unwrap().request, r);
    }
    assert_eq!(owner.len(), 13);
}

#[tokio::test]
async fn read_queries_are_closed_decoded_once_and_bodyless() {
    for suffix in ["intros", "me/notifications"] {
        for query in [
            "",
            "limit=0",
            "limit=101",
            "limit=-1",
            "limit=+1",
            "limit=1.0",
            "limit=65536",
            "limit=1&%6cimit=2",
            "limit=1&limit=2",
            "cursor=",
            "cursor=%252f",
            "cursor=%2f",
            "cursor=%00",
            "cursor=%ff",
            "cursor=%G0",
            "cursor=%",
            "limit",
            "limit=1&",
            "actor_id=x",
            "limit=1#fragment",
        ] {
            refused(input("GET", &format!("{suffix}?{query}"), "")).await;
        }
        refused(input(
            "GET",
            &format!("{suffix}?cursor={}", "a".repeat(1025)),
            "",
        ))
        .await;
    }
    for query in [
        "q=",
        "q=%0ahello",
        "q=hello%0d",
        "q=%00",
        "q=%ff",
        "q=x&q=y",
        "q=x&%71=y",
        "unread_only=true",
    ] {
        refused(input("GET", &format!("intros?{query}"), "")).await;
    }
    refused(input("GET", &format!("intros?q={}", "a".repeat(101)), "")).await;
    refused(input("GET", &format!("intros?q={}x", "+".repeat(100)), "")).await;
    for query in [
        "q=hello",
        "unreadOnly=true",
        "unread_only=True",
        "unread_only=1",
        "unread_only=true&%75nread_only=false",
    ] {
        refused(input("GET", &format!("me/notifications?{query}"), "")).await;
    }
    for (request, _) in cases().into_iter().filter(|(r, _)| r.http_method == "GET") {
        for field in 0..3 {
            let mut r = request.clone();
            match field {
                0 => r.http_body = b"{}".to_vec(),
                1 => r.content_type = "application/json".into(),
                _ => r.idempotency_key = Uuid::new_v4().to_string(),
            }
            refused(r).await;
        }
    }
    for suffix in ["me/intro", "me/notifications/unread-count"] {
        refused(input("GET", &format!("{suffix}?limit=1"), "")).await;
    }
    // Valid encoded fields keep their original signed bytes all the way through.
    for suffix in [
        "intros?%6cimit=%32&q=hello%2520world",
        "me/notifications?%6cimit=%32&unread_only=false",
    ] {
        let owner = Recorder::new(Mode::Refuse);
        assert_eq!(
            call(server(owner.clone()), wire(&input("GET", suffix, "")))
                .await
                .1,
            REFUSAL
        );
        assert_eq!(owner.len(), 1);
    }
}

#[tokio::test]
async fn intro_put_and_post_use_real_closed_typed_command_parser() {
    for body in [
        "null",
        "[]",
        "[0,\"Hello\",[null,null],null]",
        "{}",
        "{}{}",
        r#"{"expected_revision":0,"text":"Hello","age_bounds":[null,null],"expires_at":null}"#,
        r#"{"expected_revision":0,"text":"Hello","age_bounds":{},"expires_at":null}"#,
        r#"{"expected_revision":0,"text":"Hello","age_bounds":{"minimum_age":null,"maximum_age_exclusive":null}}"#,
        r#"{"expected_revision":0,"text":"Hello","age_bounds":{"minimum_age":null,"maximum_age_exclusive":null},"expires_at":null,"actor_id":null}"#,
        r#"{"expected_revision":0,"expected_revision":1,"text":"Hello","age_bounds":{"minimum_age":null,"maximum_age_exclusive":null},"expires_at":null}"#,
        r#"{"expected_revision":0,"text":"Hello","age_bounds":{"minimum_age":null,"minimum_age":null,"maximum_age_exclusive":null},"expires_at":null}"#,
    ] {
        refused(input("PUT", "me/intro", body)).await;
    }
    for (old, new) in [
        ("\"expected_revision\":0", "\"expected_revision\":-1"),
        (
            "\"expected_revision\":0",
            "\"expected_revision\":9007199254740992",
        ),
        ("\"expected_revision\":0", "\"expected_revision\":0.0"),
        ("\"Hello\"", "\"bad\\rtext\""),
        ("\"Hello\"", "\"bad\\u0000text\""),
        ("\"minimum_age\":null", "\"minimum_age\":14"),
        (
            "\"expires_at\":null",
            "\"expires_at\":\"2000-01-01T00:00:00Z\"",
        ),
        (
            "\"expires_at\":null",
            "\"expires_at\":\"2099-01-01T00:00:00+00:00\"",
        ),
    ] {
        refused(input("PUT", "me/intro", &SET.replace(old, new))).await;
    }
    refused(input(
        "PUT",
        "me/intro",
        &SET.replace("Hello", &"a".repeat(1001)),
    ))
    .await;
    for suffix in ["me/intro/publish", "me/intro/close"] {
        for body in [
            "[1]",
            "null",
            "{}",
            "{}{}",
            r#"{"expected_revision":0}"#,
            r#"{"expected_revision":-1}"#,
            r#"{"expected_revision":1.0}"#,
            r#"{"expected_revision":"1"}"#,
            r#"{"expected_revision":9007199254740992}"#,
            r#"{"expected_revision":1,"expected_revision":2}"#,
            r#"{"expected_revision":1,"user_id":"x"}"#,
        ] {
            refused(input("POST", suffix, body)).await;
        }
    }
}

#[tokio::test]
async fn notification_patch_is_a_keyed_write_with_closed_body() {
    let suffix = format!("me/notifications/{}/read", Uuid::new_v4());
    for body in [
        "{}",
        "null",
        "[1]",
        "[]",
        "{}{}",
        r#"{"expected_revision":0}"#,
        r#"{"expected_revision":-1}"#,
        r#"{"expected_revision":1.0}"#,
        r#"{"expected_revision":"1"}"#,
        r#"{"expected_revision":9007199254740992}"#,
        r#"{"expected_revision":1,"expected_revision":2}"#,
        r#"{"expected_revision":1,"payload":"x"}"#,
    ] {
        refused(input("PATCH", &suffix, body)).await;
    }
    for body in [
        "",
        "[]",
        "null",
        r#"{"expected_revision":1}"#,
        r#"{"actor_id":null}"#,
    ] {
        refused(input("PATCH", "me/notifications/read-all", body)).await;
    }
    for (request, _) in cases().into_iter().filter(|(r, _)| r.http_method != "GET") {
        for field in 0..8 {
            let mut r = request.clone();
            match field {
                0 => r.idempotency_key.clear(),
                1 => r.idempotency_key = Uuid::nil().to_string(),
                2 => r.idempotency_key = Uuid::new_v4().simple().to_string(),
                3 => r.content_type = "application/json; charset=utf-8".into(),
                4 => r.http_body.clear(),
                5 => r.http_body = vec![b' '; 16 * 1024 + 1],
                6 => r.http_target.push_str("?limit=1"),
                _ => r.content_type.clear(),
            }
            refused(r).await;
        }
    }
}

#[tokio::test]
async fn signed_identity_event_method_path_and_deadline_relabelling_never_dispatch() {
    for (request, _) in cases() {
        for field in 0..21 {
            let mut r = request.clone();
            match field {
                0 => r.user_id = Uuid::nil().to_string(),
                1 => r.event_id = Uuid::new_v4().to_string(),
                2 => r.event_id = Uuid::nil().to_string(),
                3 => r.session_family_id = Uuid::nil().to_string(),
                4 => r.request_id = Uuid::new_v4().simple().to_string(),
                5 => r.http_method = "OPTIONS".into(),
                6 => r.http_target.push('/'),
                7 => r.http_target.push_str("#fragment"),
                8 => r.http_target = r.http_target.replace("/events/", "/events/%2f"),
                9 => r.http_target.push('\n'),
                10 => r.authority_expires_unix_micros = chrono::Utc::now().timestamp_micros() - 1,
                11 => r.access_expires_unix_micros = chrono::Utc::now().timestamp_micros() - 1,
                12 => r.auth_observed_at_unix_micros = 0,
                13 => {
                    r.profile_observed_at_unix_micros =
                        chrono::Utc::now().timestamp_micros() + 3_000_000
                }
                14 => r.authority_expires_unix_micros = r.auth_observed_at_unix_micros + 2_000_001,
                15 => r.auth_observed_at_unix_micros = r.authority_expires_unix_micros - 2_000_001,
                16 => {
                    r.profile_observed_at_unix_micros = r.authority_expires_unix_micros - 2_000_001
                }
                17 => r.trusted_network_bucket = vec![0; 32],
                18 => r.trusted_network_bucket = vec![1; 31],
                19 => r.request_id = Uuid::nil().to_string(),
                _ => r.session_family_id = Uuid::new_v4().to_string().to_uppercase(),
            }
            // Both expiries must exceed two seconds to test a too-long ceiling.
            if field == 14 {
                r.access_expires_unix_micros = r.authority_expires_unix_micros;
            }
            refused(r).await;
        }
        let owner = Recorder::new(Mode::Refuse);
        assert_ne!(
            call(
                server(owner.clone()),
                signed(&request, Uuid::new_v4(), Some("1800000u"))
            )
            .await
            .0,
            Code::Ok
        );
        assert_eq!(owner.len(), 0);
    }
    for timeout in [None, Some("invalid"), Some("0u")] {
        let r = input("GET", "intros", "");
        let owner = Recorder::new(Mode::Refuse);
        call(
            server(owner.clone()),
            signed(&r, Uuid::parse_str(&r.user_id).unwrap(), timeout),
        )
        .await;
        assert_eq!(owner.len(), 0, "missing/expired original deadline renewed");
    }
}

#[tokio::test]
async fn post_signature_frame_changes_and_wrong_credentials_never_dispatch() {
    for (request, _) in cases() {
        for field in 0..9 {
            let mut changed = request.clone();
            match field {
                0 => changed.user_id = Uuid::new_v4().to_string(),
                1 => changed.event_id = Uuid::new_v4().to_string(),
                2 => changed.http_body.extend_from_slice(b" "),
                3 => changed.content_type.push('x'),
                4 => changed.idempotency_key = Uuid::new_v4().to_string(),
                5 => changed.request_id = Uuid::new_v4().to_string(),
                6 => changed.http_target.push('/'),
                7 => changed.http_method = "DELETE".into(),
                _ => changed.authority_expires_unix_micros += 1,
            }
            let mut signed = wire(&request);
            *signed.body_mut() = Body::new(http_body_util::Full::new(Bytes::from(frame(&changed))));
            let owner = Recorder::new(Mode::Refuse);
            call(server(owner.clone()), signed).await;
            assert_eq!(owner.len(), 0);
        }
    }
    for field in 0..4 {
        let mut signed = wire(&input("GET", "me/notifications", ""));
        match field {
            0 => {
                signed.headers_mut().remove(HEADER);
            }
            1 => {
                signed
                    .headers_mut()
                    .insert("x-service-token", "wrong".parse().unwrap());
            }
            2 => {
                signed
                    .headers_mut()
                    .insert("x-service-caller", "friend".parse().unwrap());
            }
            _ => {
                signed
                    .headers_mut()
                    .insert(HEADER, "invalid".parse().unwrap());
            }
        }
        let owner = Recorder::new(Mode::Refuse);
        call(server(owner.clone()), signed).await;
        assert_eq!(owner.len(), 0);
    }
}

#[tokio::test]
async fn original_short_deadline_and_four_slot_capacity_survive_feature_admission() {
    let owner = Recorder::new(Mode::Hold);
    let service = server(owner.clone());
    let mut running = vec![];
    for (request, _) in cases().into_iter().take(4) {
        running.push(tokio::spawn(call(service.clone(), wire(&request))));
    }
    bounded_enter(&owner, 4).await;
    let r = input("GET", "me/notifications", "");
    assert_eq!(call(service.clone(), wire(&r)).await.0, Code::Unavailable);
    assert_eq!(owner.len(), 4);
    owner.release.add_permits(4);
    for running in running {
        assert_eq!(running.await.unwrap().0, Code::Unavailable);
    }
    assert_eq!(owner.dropped.load(Ordering::SeqCst), 4);

    let r = input("PATCH", "me/notifications/read-all", "{}");
    let started = Instant::now();
    assert_eq!(
        call(
            service,
            signed(&r, Uuid::parse_str(&r.user_id).unwrap(), Some("40000u"))
        )
        .await
        .0,
        Code::DeadlineExceeded
    );
    assert!(started.elapsed() < Duration::from_millis(500));
    assert_eq!(
        owner.dropped.load(Ordering::SeqCst),
        5,
        "timed-out producer future dropped"
    );
    assert!(
        owner.admitted.lock().unwrap().last().unwrap().stop <= started + Duration::from_millis(50)
    );
}

#[tokio::test]
async fn feature_admission_does_not_bypass_representation_or_canonical_delivery() {
    for (status, content_type, body, expected) in [
        (200, "application/json", b"{}".as_slice(), Code::Unavailable),
        (201, "application/json", b"{}", Code::Internal),
        (200, "image/png", b"image", Code::Internal),
        (200, "application/json", b"", Code::Internal),
    ] {
        for (r, _) in cases() {
            let owner = Recorder::new(Mode::Response(ProductResponse {
                status,
                content_type: content_type.into(),
                body: body.into(),
                ..Default::default()
            }));
            assert_eq!(call(server(owner.clone()), wire(&r)).await.0, expected);
            assert_eq!(owner.len(), 1);
        }
    }
}

#[tokio::test]
#[ignore = "requires an explicitly owned, fresh private PKI; no PG/peer services"]
async fn signed_gateway_mtls_admits_all_ten_and_refuses_wrong_certificate_role(
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
    // Abort on assertion failure too; no detached listener survives fixture drop.
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
        for (r, expected) in cases() {
            let signed = wire(&r);
            let mut request = Request::new(r.clone());
            for name in ["x-service-caller", "x-service-token", HEADER] {
                request
                    .metadata_mut()
                    .insert(name, signed.headers()[name].to_str()?.parse()?);
            }
            request.set_timeout(Duration::from_millis(1800));
            let count = owner.len();
            let result = client.dispatch(request).await.unwrap_err();
            if workload == Workload::Gateway {
                assert_eq!(result.code(), Code::Unavailable);
                assert_eq!(result.message(), REFUSAL);
                assert_eq!(owner.len(), count + 1);
                let admissions = owner.admitted.lock().unwrap();
                assert_eq!(admissions.last().unwrap().operation, expected);
                assert_eq!(admissions.last().unwrap().request, r);
            } else {
                assert_eq!(result.code(), Code::Unauthenticated);
                assert_eq!(owner.len(), count);
            }
        }
    }
    stop.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(2), task).await???;
    assert_eq!(owner.len(), 10);
    Ok(())
}
