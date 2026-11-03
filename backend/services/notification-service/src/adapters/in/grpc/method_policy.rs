//! Complete caller table for every RPC this service serves.
//!
//! Each entry names the workloads with a real call path to that method, taken
//! from their outbound clients. A method missing here is denied, and the unit
//! test below fails if this table and the Proto service definition diverge.

use platform::grpc_auth::Workload;
use platform::grpc_method_policy::MethodPolicy;

const POST: &[Workload] = &[Workload::Post];
const EDGE: &[Workload] = &[Workload::Gateway];
const AUTH: &[Workload] = &[Workload::Auth];

pub static NOTIFICATION_METHOD_POLICY: MethodPolicy = MethodPolicy {
    service: "notification.v1.NotificationService",
    workload: Some(Workload::Notification),
    methods: &[
        ("CreateNotification", POST),
        ("ListNotifications", EDGE),
        ("MarkNotificationRead", EDGE),
        ("MarkAllNotificationsRead", EDGE),
        ("DeleteNotification", EDGE),
        ("ApplyUserDeletion", AUTH),
    ],
    actor_fields: &[
        ("MarkNotificationRead", &[&[1]]),
        ("MarkAllNotificationsRead", &[&[1]]),
        ("DeleteNotification", &[&[1], &[3]]),
        ("ListNotifications", &[&[1]]),
    ],
};

#[cfg(test)]
mod tests {
    use super::*;
    use platform::grpc_method_policy::proto_rpc_names;
    use std::collections::BTreeSet;

    #[test]
    fn table_covers_exactly_the_proto_service() {
        let proto = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../proto/notification/v1/notification.proto"
        ));
        let declared: BTreeSet<String> = proto_rpc_names(proto).into_iter().collect();
        let listed: BTreeSet<String> = NOTIFICATION_METHOD_POLICY
            .method_names()
            .map(str::to_owned)
            .collect();
        assert_eq!(listed, declared);
    }

    #[test]
    fn sibling_reachable_writes_are_edge_only() {
        for method in ["MarkNotificationRead", "MarkAllNotificationsRead"] {
            assert_eq!(
                NOTIFICATION_METHOD_POLICY.callers(method),
                Some(&[Workload::Gateway][..]),
                "{method} must accept only the Gateway"
            );
            for sibling in [
                Workload::Auth,
                Workload::Post,
                Workload::Profile,
                Workload::Friend,
                Workload::Notification,
            ] {
                assert!(
                    !NOTIFICATION_METHOD_POLICY.allows(
                        &format!("/notification.v1.NotificationService/{method}"),
                        sibling
                    ),
                    "{method} accepted {sibling:?}"
                );
            }
        }
    }
    #[test]
    fn create_notification_accepts_only_the_post_service() {
        assert_eq!(
            NOTIFICATION_METHOD_POLICY.callers("CreateNotification"),
            Some(&[Workload::Post][..])
        );
    }

    #[test]
    fn actor_fields_name_the_actor_in_generated_requests() {
        use crate::adapters::r#in::grpc::server::proto::*;
        use prost::Message;
        let actor = uuid::Uuid::new_v4();
        let a = || actor.to_string();
        let samples: Vec<(&str, Vec<u8>)> = vec![
            (
                "MarkNotificationRead",
                MarkNotificationReadRequest {
                    user_id: a(),
                    ..Default::default()
                }
                .encode_to_vec(),
            ),
            (
                "MarkAllNotificationsRead",
                MarkAllNotificationsReadRequest { user_id: a() }.encode_to_vec(),
            ),
            (
                "DeleteNotification",
                DeleteNotificationRequest {
                    user_id: a(),
                    actor_id: a(),
                    ..Default::default()
                }
                .encode_to_vec(),
            ),
            (
                "ListNotifications",
                ListNotificationsRequest {
                    user_id: a(),
                    ..Default::default()
                }
                .encode_to_vec(),
            ),
        ];
        platform::grpc_method_policy::check_actor_samples(
            &NOTIFICATION_METHOD_POLICY,
            &samples,
            actor,
        )
        .unwrap();
    }
}
