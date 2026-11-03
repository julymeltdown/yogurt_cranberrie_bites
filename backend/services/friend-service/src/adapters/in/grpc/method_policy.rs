//! Complete caller table for every RPC this service serves.
//!
//! Each entry names the workloads with a real call path to that method, taken
//! from their outbound clients. A method missing here is denied, and the unit
//! test below fails if this table and the Proto service definition diverge.

use platform::grpc_auth::Workload;
use platform::grpc_method_policy::{ForwardedMethod, MethodPolicy};

const EDGE: &[Workload] = &[Workload::Gateway];
const EDGE_POST: &[Workload] = &[Workload::Gateway, Workload::Post];
const AUTH: &[Workload] = &[Workload::Auth];

pub static FRIEND_METHOD_POLICY: MethodPolicy = MethodPolicy {
    service: "friend.v1.FriendService",
    workload: Some(Workload::Friend),
    methods: &[
        ("ReplayMutation", EDGE),
        ("GetMutationReceiptStatus", EDGE),
        ("FollowUser", EDGE),
        ("UnfollowUser", EDGE),
        ("ListFollowers", EDGE_POST),
        ("ListFollowing", EDGE_POST),
        ("GetFollowCounts", EDGE_POST),
        ("GetFollowStats", EDGE),
        ("CheckFollow", EDGE),
        ("AcceptFollowRequest", EDGE),
        ("RejectFollowRequest", EDGE),
        ("CancelFollowRequest", EDGE),
        ("ListIncomingFollowRequests", EDGE),
        ("ListOutgoingFollowRequests", EDGE),
        ("BlockUser", EDGE),
        ("UnblockUser", EDGE),
        ("MuteUser", EDGE),
        ("UnmuteUser", EDGE),
        ("GetRelationship", EDGE_POST),
        ("BatchGetRelationships", EDGE_POST),
        ("GetPeopleRecommendations", EDGE),
        ("RecordPeopleRecommendationEvent", EDGE),
        ("ApplyUserDeletion", AUTH),
    ],
    actor_fields: &[
        ("ReplayMutation", &[&[1, 1], &[2, 1]]),
        ("FollowUser", &[&[1]]),
        ("UnfollowUser", &[&[1]]),
        ("AcceptFollowRequest", &[&[2]]),
        ("RejectFollowRequest", &[&[2]]),
        ("CancelFollowRequest", &[&[2]]),
        ("BlockUser", &[&[1]]),
        ("UnblockUser", &[&[1]]),
        ("MuteUser", &[&[1]]),
        ("UnmuteUser", &[&[1]]),
        ("RecordPeopleRecommendationEvent", &[&[4], &[7]]),
        ("ListIncomingFollowRequests", &[&[1]]),
        ("ListOutgoingFollowRequests", &[&[1]]),
        ("GetPeopleRecommendations", &[&[1]]),
    ],
};

/// Post reads that act for the admitted viewer. Post must present a
/// Gateway-issued one-hop child bound to this method, the exact body and the
/// viewer in field 1, so a Post credential alone cannot enumerate another
/// user's graph. Post's background workers use only methods outside this list.
pub static FRIEND_FORWARDING: &[ForwardedMethod] = &[
    ForwardedMethod {
        method: "ListFollowing",
        caller: Workload::Post,
        actor_fields: &[&[1]],
    },
    ForwardedMethod {
        method: "BatchGetRelationships",
        caller: Workload::Post,
        actor_fields: &[&[1]],
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use platform::grpc_method_policy::proto_rpc_names;
    use std::collections::BTreeSet;

    #[test]
    fn post_discovery_reads_require_gateway_issued_child_authority() {
        let forwarded: Vec<_> = FRIEND_FORWARDING
            .iter()
            .map(|entry| (entry.method, entry.caller, entry.actor_fields))
            .collect();
        assert_eq!(
            forwarded,
            vec![
                ("ListFollowing", Workload::Post, &[&[1u32][..]][..]),
                ("BatchGetRelationships", Workload::Post, &[&[1u32][..]][..]),
            ]
        );
        for entry in FRIEND_FORWARDING {
            assert!(FRIEND_METHOD_POLICY.allows(
                &format!("/friend.v1.FriendService/{}", entry.method),
                entry.caller
            ));
        }
    }

    #[test]
    fn table_covers_exactly_the_proto_service() {
        let proto = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../proto/friend/v1/friend.proto"
        ));
        let declared: BTreeSet<String> = proto_rpc_names(proto).into_iter().collect();
        let listed: BTreeSet<String> = FRIEND_METHOD_POLICY
            .method_names()
            .map(str::to_owned)
            .collect();
        assert_eq!(listed, declared);
    }

    #[test]
    fn sibling_reachable_writes_are_edge_only() {
        for method in [
            "AcceptFollowRequest",
            "RejectFollowRequest",
            "CancelFollowRequest",
            "BlockUser",
            "UnblockUser",
            "MuteUser",
            "UnmuteUser",
            "RecordPeopleRecommendationEvent",
        ] {
            assert_eq!(
                FRIEND_METHOD_POLICY.callers(method),
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
                    !FRIEND_METHOD_POLICY
                        .allows(&format!("/friend.v1.FriendService/{method}"), sibling),
                    "{method} accepted {sibling:?}"
                );
            }
        }
    }

    #[test]
    fn actor_fields_name_the_actor_in_generated_requests() {
        use crate::adapters::r#in::grpc::server::proto::*;
        use prost::Message;
        let actor = uuid::Uuid::new_v4();
        let a = || actor.to_string();
        use replay_mutation_request::Command;
        let replay = |command| {
            ReplayMutationRequest {
                command: Some(command),
            }
            .encode_to_vec()
        };
        let samples: Vec<(&str, Vec<u8>)> = vec![
            (
                "FollowUser",
                FollowUserRequest {
                    follower_id: a(),
                    ..Default::default()
                }
                .encode_to_vec(),
            ),
            (
                "UnfollowUser",
                UnfollowUserRequest {
                    follower_id: a(),
                    ..Default::default()
                }
                .encode_to_vec(),
            ),
            (
                "AcceptFollowRequest",
                FollowRequestActionRequest {
                    actor_id: a(),
                    ..Default::default()
                }
                .encode_to_vec(),
            ),
            (
                "RejectFollowRequest",
                FollowRequestActionRequest {
                    actor_id: a(),
                    ..Default::default()
                }
                .encode_to_vec(),
            ),
            (
                "CancelFollowRequest",
                FollowRequestActionRequest {
                    actor_id: a(),
                    ..Default::default()
                }
                .encode_to_vec(),
            ),
            (
                "BlockUser",
                RelationshipRequest {
                    viewer_id: a(),
                    ..Default::default()
                }
                .encode_to_vec(),
            ),
            (
                "UnblockUser",
                RelationshipRequest {
                    viewer_id: a(),
                    ..Default::default()
                }
                .encode_to_vec(),
            ),
            (
                "MuteUser",
                RelationshipRequest {
                    viewer_id: a(),
                    ..Default::default()
                }
                .encode_to_vec(),
            ),
            (
                "UnmuteUser",
                RelationshipRequest {
                    viewer_id: a(),
                    ..Default::default()
                }
                .encode_to_vec(),
            ),
            (
                "RecordPeopleRecommendationEvent",
                RecordRecommendationEventRequest {
                    viewer_id: a(),
                    actor_id: a(),
                    ..Default::default()
                }
                .encode_to_vec(),
            ),
            (
                "ReplayMutation",
                replay(Command::FollowUser(FollowUserRequest {
                    follower_id: a(),
                    ..Default::default()
                })),
            ),
            (
                "ReplayMutation",
                replay(Command::UnfollowUser(UnfollowUserRequest {
                    follower_id: a(),
                    ..Default::default()
                })),
            ),
            (
                "ListIncomingFollowRequests",
                ListFollowRequestsRequest {
                    user_id: a(),
                    ..Default::default()
                }
                .encode_to_vec(),
            ),
            (
                "ListOutgoingFollowRequests",
                ListFollowRequestsRequest {
                    user_id: a(),
                    ..Default::default()
                }
                .encode_to_vec(),
            ),
            (
                "GetPeopleRecommendations",
                GetPeopleRecommendationsRequest {
                    viewer_id: a(),
                    ..Default::default()
                }
                .encode_to_vec(),
            ),
        ];
        platform::grpc_method_policy::check_actor_samples(&FRIEND_METHOD_POLICY, &samples, actor)
            .unwrap();
    }
}
