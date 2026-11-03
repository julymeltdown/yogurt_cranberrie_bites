//! Complete caller table for every RPC this service serves.
//!
//! Each entry names the workloads with a real call path to that method, taken
//! from their outbound clients. A method missing here is denied, and the unit
//! test below fails if this table and the Proto service definition diverge.

use platform::grpc_auth::Workload;
use platform::grpc_method_policy::MethodPolicy;

const EDGE: &[Workload] = &[Workload::Gateway];
const EDGE_POST: &[Workload] = &[Workload::Gateway, Workload::Post];
const EDGE_FRIEND: &[Workload] = &[Workload::Gateway, Workload::Friend];
const FRIEND: &[Workload] = &[Workload::Friend];
const AUTH: &[Workload] = &[Workload::Auth];

pub static PROFILE_METHOD_POLICY: MethodPolicy = MethodPolicy {
    service: "profile.v1.ProfileService",
    workload: Some(Workload::Profile),
    methods: &[
        ("ReadConcertSelfProfile", EDGE),
        ("ReplayMutation", EDGE),
        ("GetMutationReceiptStatus", EDGE),
        ("GetProfile", EDGE_POST),
        ("GetProfileByUsername", EDGE),
        ("GetProfiles", EDGE_FRIEND),
        ("UpdateProfile", EDGE),
        ("UpdateProfilePrivacy", EDGE),
        ("GetProfilePrivacy", FRIEND),
        ("DeleteProfile", EDGE),
        ("SearchProfiles", EDGE),
        ("UploadAvatar", EDGE),
        ("GetAvatar", EDGE),
        ("ApplyUserDeletion", AUTH),
    ],
    actor_fields: &[
        ("ReadConcertSelfProfile", &[&[1]]),
        ("ReplayMutation", &[&[1, 1], &[1, 2], &[2, 1]]),
        ("UpdateProfile", &[&[1]]),
        ("UpdateProfilePrivacy", &[&[1], &[2]]),
        ("DeleteProfile", &[&[1], &[2]]),
        ("UploadAvatar", &[&[1]]),
    ],
};

#[cfg(test)]
mod tests {
    use super::*;
    use platform::grpc_method_policy::proto_rpc_names;
    use std::collections::BTreeSet;

    const PROTO: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../proto/profile/v1/profile.proto"
    ));
    fn declared(service: &str) -> BTreeSet<String> {
        let header = format!("service {service} {{");
        let body = PROTO
            .split_once(&header)
            .expect("declared service")
            .1
            .split_once('}')
            .expect("closed service")
            .0;
        proto_rpc_names(body).into_iter().collect()
    }

    #[test]
    fn table_covers_exactly_the_proto_service() {
        let listed: BTreeSet<String> = PROFILE_METHOD_POLICY
            .method_names()
            .map(str::to_owned)
            .collect();
        assert_eq!(listed, declared("ProfileService"));
    }

    #[test]
    fn world_profile_has_separate_complete_concert_only_policy() {
        use crate::adapters::r#in::grpc::world_identity::{METHOD, WORLD_PROFILE_POLICY};
        let listed: BTreeSet<String> = WORLD_PROFILE_POLICY
            .method_names()
            .map(str::to_owned)
            .collect();
        assert_eq!(listed, declared("ConcertWorldProfileService"));
        assert_eq!(listed, BTreeSet::from(["ReadCurrent".to_owned()]));
        assert_eq!(
            WORLD_PROFILE_POLICY.callers("ReadCurrent"),
            Some(&[Workload::Concert][..])
        );
        for workload in Workload::ALL {
            assert_eq!(
                WORLD_PROFILE_POLICY.allows(METHOD, workload),
                workload == Workload::Concert
            );
        }
        let services: BTreeSet<_> = PROTO
            .lines()
            .filter_map(|line| line.trim_start().strip_prefix("service "))
            .map(|rest| rest.split_whitespace().next().unwrap())
            .collect();
        assert_eq!(
            services,
            BTreeSet::from(["ProfileService", "ConcertWorldProfileService"])
        );
    }

    #[test]
    fn sibling_reachable_writes_are_edge_only() {
        for method in ["UploadAvatar", "ReadConcertSelfProfile"] {
            assert_eq!(
                PROFILE_METHOD_POLICY.callers(method),
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
                    !PROFILE_METHOD_POLICY
                        .allows(&format!("/profile.v1.ProfileService/{method}"), sibling),
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
                "ReadConcertSelfProfile",
                ReadConcertSelfProfileRequest { user_id: a() }.encode_to_vec(),
            ),
            (
                "UpdateProfile",
                UpdateProfileRequest {
                    user_id: a(),
                    ..Default::default()
                }
                .encode_to_vec(),
            ),
            (
                "UpdateProfilePrivacy",
                UpdateProfilePrivacyRequest {
                    user_id: a(),
                    actor_id: a(),
                    ..Default::default()
                }
                .encode_to_vec(),
            ),
            (
                "DeleteProfile",
                DeleteProfileRequest {
                    user_id: a(),
                    actor_id: a(),
                    ..Default::default()
                }
                .encode_to_vec(),
            ),
            (
                "UploadAvatar",
                UploadAvatarRequest {
                    user_id: a(),
                    ..Default::default()
                }
                .encode_to_vec(),
            ),
            (
                "ReplayMutation",
                replay(Command::UpdateProfile(UpdateProfileRequest {
                    user_id: a(),
                    ..Default::default()
                })),
            ),
            (
                "ReplayMutation",
                replay(Command::UpdateProfilePrivacy(UpdateProfilePrivacyRequest {
                    user_id: a(),
                    actor_id: a(),
                    ..Default::default()
                })),
            ),
        ];
        platform::grpc_method_policy::check_actor_samples(&PROFILE_METHOD_POLICY, &samples, actor)
            .unwrap();
    }
}
