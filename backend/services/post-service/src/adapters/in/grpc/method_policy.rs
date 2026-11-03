//! Complete caller table for every RPC this service serves.
//!
//! Each entry names the workloads with a real call path to that method, taken
//! from their outbound clients. A method missing here is denied, and the unit
//! test below fails if this table and the Proto service definition diverge.

use platform::grpc_auth::Workload;
use platform::grpc_method_policy::MethodPolicy;

const EDGE: &[Workload] = &[Workload::Gateway];
const AUTH: &[Workload] = &[Workload::Auth];

pub static POST_METHOD_POLICY: MethodPolicy = MethodPolicy {
    service: "post.v1.PostService",
    workload: Some(Workload::Post),
    methods: &[
        ("AuthorizeNotificationPosts", EDGE),
        ("ReplayMutation", EDGE),
        ("GetMutationReceiptStatus", EDGE),
        ("CreatePost", EDGE),
        ("DeletePost", EDGE),
        ("CreateComment", EDGE),
        ("DeleteComment", EDGE),
        ("GetFeed", EDGE),
        ("RecordFeedRecommendationEvent", EDGE),
        ("GetAuthorPosts", EDGE),
        ("GetAuthorPostCounts", EDGE),
        ("SearchPosts", EDGE),
        ("GetPost", EDGE),
        ("LikePost", EDGE),
        ("UnlikePost", EDGE),
        ("GetTrending", EDGE),
        ("ApplyUserDeletion", AUTH),
        ("ApplyContentModeration", EDGE),
        ("ResolveDeadLetterReplay", EDGE),
    ],
    actor_fields: &[
        ("AuthorizeNotificationPosts", &[&[1]]),
        (
            "ReplayMutation",
            &[&[1, 1], &[2, 2], &[3, 2], &[4, 2], &[5, 2]],
        ),
        ("CreatePost", &[&[1]]),
        ("DeletePost", &[&[2]]),
        ("CreateComment", &[&[2]]),
        ("DeleteComment", &[&[2]]),
        ("RecordFeedRecommendationEvent", &[&[4], &[6]]),
        ("LikePost", &[&[2]]),
        ("UnlikePost", &[&[2]]),
        ("ApplyContentModeration", &[&[4]]),
        ("ResolveDeadLetterReplay", &[&[3]]),
        ("GetFeed", &[&[1]]),
        ("GetAuthorPosts", &[&[4]]),
        ("SearchPosts", &[&[4]]),
        ("GetPost", &[&[2]]),
        ("GetTrending", &[&[1]]),
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
            "/../../proto/post/v1/post.proto"
        ));
        let declared: BTreeSet<String> = proto_rpc_names(proto).into_iter().collect();
        let listed: BTreeSet<String> = POST_METHOD_POLICY
            .method_names()
            .map(str::to_owned)
            .collect();
        assert_eq!(listed, declared);
    }

    #[test]
    fn sibling_reachable_writes_are_edge_only() {
        for method in ["DeleteComment", "RecordFeedRecommendationEvent"] {
            assert_eq!(
                POST_METHOD_POLICY.callers(method),
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
                    !POST_METHOD_POLICY.allows(&format!("/post.v1.PostService/{method}"), sibling),
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
                "AuthorizeNotificationPosts",
                AuthorizeNotificationPostsRequest {
                    viewer_id: a(),
                    ..Default::default()
                }
                .encode_to_vec(),
            ),
            (
                "CreatePost",
                CreatePostRequest {
                    author_id: a(),
                    ..Default::default()
                }
                .encode_to_vec(),
            ),
            (
                "DeletePost",
                DeletePostRequest {
                    actor_id: a(),
                    ..Default::default()
                }
                .encode_to_vec(),
            ),
            (
                "CreateComment",
                CreateCommentRequest {
                    author_id: a(),
                    ..Default::default()
                }
                .encode_to_vec(),
            ),
            (
                "DeleteComment",
                DeleteCommentRequest {
                    actor_id: a(),
                    ..Default::default()
                }
                .encode_to_vec(),
            ),
            (
                "RecordFeedRecommendationEvent",
                RecordFeedRecommendationEventRequest {
                    viewer_id: a(),
                    actor_id: a(),
                    ..Default::default()
                }
                .encode_to_vec(),
            ),
            (
                "LikePost",
                LikePostRequest {
                    user_id: a(),
                    ..Default::default()
                }
                .encode_to_vec(),
            ),
            (
                "UnlikePost",
                UnlikePostRequest {
                    user_id: a(),
                    ..Default::default()
                }
                .encode_to_vec(),
            ),
            (
                "ApplyContentModeration",
                ApplyContentModerationRequest {
                    actor_id: a(),
                    ..Default::default()
                }
                .encode_to_vec(),
            ),
            (
                "ResolveDeadLetterReplay",
                ResolveDeadLetterReplayRequest {
                    actor_id: a(),
                    ..Default::default()
                }
                .encode_to_vec(),
            ),
            (
                "ReplayMutation",
                replay(Command::CreatePost(CreatePostRequest {
                    author_id: a(),
                    ..Default::default()
                })),
            ),
            (
                "ReplayMutation",
                replay(Command::CreateComment(CreateCommentRequest {
                    author_id: a(),
                    ..Default::default()
                })),
            ),
            (
                "ReplayMutation",
                replay(Command::DeletePost(DeletePostRequest {
                    actor_id: a(),
                    ..Default::default()
                })),
            ),
            (
                "ReplayMutation",
                replay(Command::LikePost(LikePostRequest {
                    user_id: a(),
                    ..Default::default()
                })),
            ),
            (
                "ReplayMutation",
                replay(Command::UnlikePost(UnlikePostRequest {
                    user_id: a(),
                    ..Default::default()
                })),
            ),
            (
                "GetFeed",
                FeedRequest {
                    user_id: a(),
                    ..Default::default()
                }
                .encode_to_vec(),
            ),
            (
                "GetAuthorPosts",
                AuthorPostsRequest {
                    viewer_id: a(),
                    ..Default::default()
                }
                .encode_to_vec(),
            ),
            (
                "SearchPosts",
                SearchPostsRequest {
                    viewer_id: a(),
                    ..Default::default()
                }
                .encode_to_vec(),
            ),
            (
                "GetPost",
                GetPostRequest {
                    viewer_id: a(),
                    ..Default::default()
                }
                .encode_to_vec(),
            ),
            (
                "GetTrending",
                TrendingRequest {
                    viewer_id: a(),
                    ..Default::default()
                }
                .encode_to_vec(),
            ),
        ];
        platform::grpc_method_policy::check_actor_samples(&POST_METHOD_POLICY, &samples, actor)
            .unwrap();
    }
}
