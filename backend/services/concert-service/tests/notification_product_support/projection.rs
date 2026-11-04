// Private pure projection tests included by notification_live::product. Test
// values are deliberately synthetic; no owner or target-policy authority claim.
use serde_json::{json, Value};
fn item(event: Uuid) -> proto::Notification {
    proto::Notification {
        notification_id: Uuid::new_v4().to_string(),
        event_id: event.to_string(),
        kind: 6,
        target: Some(proto::Target {
            target_type: 6,
            target_id: Uuid::new_v4().to_string(),
        }),
        created_at_unix_micros: Utc::now().timestamp_micros() - 10_000,
        read_at_unix_micros: None,
        revision: 1,
    }
}
fn reply(output: proto::execute_response::Output) -> proto::ExecuteResponse {
    proto::ExecuteResponse {
        child_id: Uuid::new_v4().to_string(),
        binding_digest: vec![9; 32],
        expires_unix_micros: Utc::now().timestamp_micros() + 1_000_000,
        output: Some(output),
    }
}
fn schema(value: &Value, name: &str) {
    fn expand(v: Value) -> Value {
        match v {
            Value::Object(mut m) => {
                if let Some(Value::String(r)) = m.remove("$ref") {
                    let (file, pointer) = r
                        .strip_prefix("https://contracts.mcr.invalid/")
                        .unwrap()
                        .split_once('#')
                        .unwrap();
                    assert!(!file.contains(".."));
                    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("../../../contracts")
                        .join(file);
                    let doc: Value = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
                    return expand(doc.pointer(pointer).unwrap().clone());
                }
                m.remove("$id");
                Value::Object(m.into_iter().map(|(k, v)| (k, expand(v))).collect())
            }
            Value::Array(v) => Value::Array(v.into_iter().map(expand).collect()),
            v => v,
        }
    }
    let contract: Value = serde_json::from_str(include_str!(
        "../../../../../contracts/domain/v1/feed.schema.json"
    ))
    .unwrap();
    let s = expand(contract["$defs"][name].clone());
    let validator = jsonschema::options()
        .should_validate_formats(true)
        .build(&s)
        .unwrap();
    assert!(validator.is_valid(value), "{name}: {value}");
}
#[test]
fn all_four_outputs_validate_against_frozen_c04_without_private_envelopes() {
    use proto::execute_response::Output;
    let actor = Uuid::new_v4();
    let event = Uuid::new_v4();
    let mut n = item(event);
    let id = canonical(&n.notification_id).unwrap();
    for (op, out, name) in [
        (
            NotificationOperation::List,
            Output::List(proto::NotificationList {
                items: vec![n.clone()],
                next_cursor: None,
            }),
            "NotificationList",
        ),
        (
            NotificationOperation::UnreadCount,
            Output::Count(proto::UnreadCount {
                actor_id: actor.to_string(),
                event_id: event.to_string(),
                unread_total: 500,
                as_of_unix_micros: Utc::now().timestamp_micros(),
            }),
            "NotificationUnreadCount",
        ),
        (
            NotificationOperation::MarkRead(id),
            {
                n.read_at_unix_micros = Some(Utc::now().timestamp_micros());
                n.revision = 2;
                Output::MarkRead(n.clone())
            },
            "Notification",
        ),
        (
            NotificationOperation::MarkAllRead,
            Output::MarkAll(proto::ReadAllResult {
                read_count: 400,
                read_at_unix_micros: Utc::now().timestamp_micros(),
            }),
            "ReadAllResult",
        ),
    ] {
        let raw = reply(out);
        let private_id = raw.child_id.clone();
        let result = project(op, actor, event, Some((20, false)), raw).unwrap();
        assert_eq!(result.status, 200);
        assert_eq!(result.content_type, "application/json");
        let v: Value = serde_json::from_slice(&result.body).unwrap();
        schema(&v, name);
        let text = String::from_utf8(result.body).unwrap();
        assert!(!text.contains(&private_id));
        for forbidden in [
            "binding_digest",
            "completion_secret",
            "authority_digest",
            "root_view",
            "payload",
            "summary",
            "source_actor_id",
            "delivery",
        ] {
            assert!(!text.contains(forbidden));
        }
    }
}
#[test]
fn foreign_malformed_unknown_kind_target_revision_and_times_are_refused() {
    let event = Uuid::new_v4();
    let original = item(event);
    let now = Utc::now().timestamp_micros();
    for variant in 0..13 {
        let mut n = original.clone();
        match variant {
            0 => n.event_id = Uuid::new_v4().to_string(),
            1 => n.notification_id = Uuid::nil().to_string(),
            2 => n.kind = 0,
            3 => n.kind = 77,
            4 => n.target.as_mut().unwrap().target_type = 0,
            5 => n.target.as_mut().unwrap().target_id = "arbitrary".into(),
            6 => n.revision = 2,
            7 => n.created_at_unix_micros = 0,
            8 => n.created_at_unix_micros = now + 2_000_000,
            9 => {
                n.read_at_unix_micros = Some(now);
                n.revision = 1
            }
            10 => {
                n.read_at_unix_micros = Some(n.created_at_unix_micros - 1);
                n.revision = 2
            }
            11 => n.notification_id = Uuid::new_v4().simple().to_string(),
            _ => {
                n.read_at_unix_micros = Some(now + 2_000_000);
                n.revision = 2
            }
        }
        assert!(
            notification(n, event, now + 1_000_000).is_err(),
            "variant {variant}"
        );
    }
    for kind in 1..=6 {
        let mut n = original.clone();
        n.kind = kind;
        assert!(notification(n, event, now + 1_000_000).is_ok());
    }
    for tag in 1..=8 {
        let mut n = original.clone();
        n.target.as_mut().unwrap().target_type = tag;
        assert!(notification(n, event, now + 1_000_000).is_ok());
    }
    let mut stale = original;
    stale.target = None;
    assert!(notification(stale, event, now + 1_000_000).is_ok());
}
#[test]
fn output_operation_page_filter_count_and_mutation_identity_are_bound() {
    use proto::execute_response::Output;
    let actor = Uuid::new_v4();
    let event = Uuid::new_v4();
    let n = item(event);
    let cases = [
        (
            NotificationOperation::UnreadCount,
            Output::List(proto::NotificationList {
                items: vec![],
                next_cursor: None,
            }),
        ),
        (
            NotificationOperation::List,
            Output::List(proto::NotificationList {
                items: vec![n.clone(), n.clone()],
                next_cursor: None,
            }),
        ),
        (
            NotificationOperation::List,
            Output::List(proto::NotificationList {
                items: vec![],
                next_cursor: Some("/secret".into()),
            }),
        ),
        (
            NotificationOperation::MarkRead(Uuid::new_v4()),
            Output::MarkRead(n.clone()),
        ),
        (
            NotificationOperation::MarkRead(canonical(&n.notification_id).unwrap()),
            Output::MarkRead(n.clone()),
        ),
        (
            NotificationOperation::MarkAllRead,
            Output::MarkAll(proto::ReadAllResult {
                read_count: -1,
                read_at_unix_micros: Utc::now().timestamp_micros(),
            }),
        ),
    ];
    for (op, out) in cases {
        assert!(project(op, actor, event, Some((1, false)), reply(out)).is_err());
    }
    let mut read = n.clone();
    read.read_at_unix_micros = Some(Utc::now().timestamp_micros());
    read.revision = 2;
    assert!(project(
        NotificationOperation::List,
        actor,
        event,
        Some((20, true)),
        reply(Output::List(proto::NotificationList {
            items: vec![read],
            next_cursor: None
        }))
    )
    .is_err());
    for variant in 0..5 {
        let mut c = proto::UnreadCount {
            actor_id: actor.to_string(),
            event_id: event.to_string(),
            unread_total: 1,
            as_of_unix_micros: Utc::now().timestamp_micros(),
        };
        match variant {
            0 => c.actor_id = Uuid::new_v4().to_string(),
            1 => c.event_id = Uuid::new_v4().to_string(),
            2 => c.unread_total = -1,
            3 => c.unread_total = concert_domain::MAX_REVISION + 1,
            _ => c.as_of_unix_micros = 0,
        };
        assert!(project(
            NotificationOperation::UnreadCount,
            actor,
            event,
            None,
            reply(Output::Count(c))
        )
        .is_err());
    }
}
#[test]
fn mutation_conflicts_remain_409_and_diagnostics_never_escape() {
    let request = Uuid::new_v4();
    for (error, status, reason) in [
        (
            Status::aborted("internal revision detail"),
            409,
            "VERSION_CONFLICT",
        ),
        (
            Status::already_exists("private key detail"),
            409,
            "IDEMPOTENCY_CONFLICT",
        ),
        (
            Status::failed_precondition("Notification idempotency expired"),
            409,
            "IDEMPOTENCY_EXPIRED",
        ),
        (
            Status::failed_precondition("unknown internal failure"),
            503,
            "DEPENDENCY_UNAVAILABLE",
        ),
        (
            Status::unavailable("private database detail"),
            503,
            "DEPENDENCY_UNAVAILABLE",
        ),
        (
            Status::permission_denied("private target detail"),
            403,
            "PERMISSION_DENIED",
        ),
    ] {
        let result =
            error_representation(NotificationOperation::MarkAllRead, &error, request).unwrap();
        assert_eq!(result.status, status);
        let v: Value = serde_json::from_slice(&result.body).unwrap();
        assert_eq!(v["details"], json!({"reason":reason}));
        assert_eq!(v["traceId"], request.to_string());
        assert!(!String::from_utf8(result.body)
            .unwrap()
            .contains(error.message()));
    }
    assert_eq!(
        error_representation(
            NotificationOperation::List,
            &Status::aborted("private"),
            request
        )
        .unwrap()
        .status,
        503
    );
}
