//! Complete caller table for every RPC this service serves.
//!
//! Each entry names the workloads with a real call path to that method, taken
//! from their outbound clients. A method missing here is denied, and the unit
//! test below fails if this table and the Proto service definition diverge.

use platform::grpc_auth::Workload;
use platform::grpc_method_policy::MethodPolicy;

const EDGE: &[Workload] = &[Workload::Gateway];

pub static AUTH_METHOD_POLICY: MethodPolicy = MethodPolicy {
    service: "auth.v1.AuthService",
    workload: Some(Workload::Auth),
    methods: &[
        ("SignupEmail", EDGE),
        ("VerifyEmail", EDGE),
        ("CheckEmail", EDGE),
        ("ResendVerification", EDGE),
        ("LoginEmail", EDGE),
        ("RefreshTokens", EDGE),
        ("Logout", EDGE),
        ("RequestPasswordReset", EDGE),
        ("ConfirmPasswordReset", EDGE),
        ("AuthorizeOAuth", EDGE),
        ("OAuthCallback", EDGE),
        ("LinkOAuthProvider", EDGE),
        ("DeleteUser", EDGE),
        ("GetUserDeletion", EDGE),
        ("GetUserDeletionBySagaId", EDGE),
        ("ListDeadLetteredUserDeletions", EDGE),
        ("RetryUserDeletion", EDGE),
        ("GetSigningKeys", EDGE),
        ("CheckSession", EDGE),
        ("ReadConcertAdmission", EDGE),
    ],
    actor_fields: &[
        ("LinkOAuthProvider", &[&[2]]),
        ("DeleteUser", &[&[1], &[2]]),
        ("RetryUserDeletion", &[&[2]]),
        ("GetUserDeletion", &[&[1]]),
        ("ReadConcertAdmission", &[&[1]]),
    ],
};

#[cfg(test)]
mod tests {
    use super::*;
    use platform::grpc_method_policy::proto_rpc_names;
    use std::collections::BTreeSet;

    #[test]
    fn concert_admission_is_gateway_only_and_actor_bound() {
        let path = "/auth.v1.AuthService/ReadConcertAdmission";
        for caller in Workload::ALL {
            assert_eq!(
                AUTH_METHOD_POLICY.allows(path, caller),
                caller == Workload::Gateway
            );
        }
        assert_eq!(
            AUTH_METHOD_POLICY.actor_fields_for(path),
            Some(&[&[1_u32][..]][..])
        );
    }

    #[test]
    fn table_covers_exactly_the_proto_service() {
        let proto = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../proto/auth/v1/auth.proto"
        ));
        let declared: BTreeSet<String> = proto_rpc_names(proto).into_iter().collect();
        let listed: BTreeSet<String> = AUTH_METHOD_POLICY
            .method_names()
            .map(str::to_owned)
            .collect();
        assert_eq!(listed, declared);
    }

    #[test]
    fn actor_fields_name_the_actor_in_generated_requests() {
        use crate::adapters::r#in::grpc::server::proto::*;
        use prost::Message;
        let actor = uuid::Uuid::new_v4();
        let a = || actor.to_string();
        let samples: Vec<(&str, Vec<u8>)> = vec![
            (
                "LinkOAuthProvider",
                LinkOAuthProviderRequest {
                    user_id: a(),
                    ..Default::default()
                }
                .encode_to_vec(),
            ),
            (
                "DeleteUser",
                DeleteUserRequest {
                    user_id: a(),
                    actor_id: a(),
                    ..Default::default()
                }
                .encode_to_vec(),
            ),
            (
                "RetryUserDeletion",
                RetryUserDeletionRequest {
                    actor_id: a(),
                    ..Default::default()
                }
                .encode_to_vec(),
            ),
            (
                "GetUserDeletion",
                GetUserDeletionRequest { user_id: a() }.encode_to_vec(),
            ),
            (
                "ReadConcertAdmission",
                ReadConcertAdmissionRequest {
                    user_id: a(),
                    ..Default::default()
                }
                .encode_to_vec(),
            ),
        ];
        platform::grpc_method_policy::check_actor_samples(&AUTH_METHOD_POLICY, &samples, actor)
            .unwrap();
    }
}
