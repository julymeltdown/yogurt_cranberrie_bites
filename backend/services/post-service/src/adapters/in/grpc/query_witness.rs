//! Maps sealed handler authority and actual application query observations.
//! Raw headers cannot construct the parent; no producer API accepts RPC rows.
use super::server::proto;
use async_trait::async_trait;
use chrono::Utc;
use platform::actor_delegation::exchange::VerifiedParentAuthority;
use post_application::authority_witness::{
    AuthorityWitnessDto, AuthorityWitnessRepositoryPort, WitnessErrorDto, WitnessParentDto,
    WitnessQueryDto, WitnessRootDto,
};
use post_application::query_witness::{
    OwnedQueryInputDto, OwnedQueryObservationDto, OwnedQueryWitnessProducerPort,
    PublishedWitnessStageDto,
};
use prost::Message;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct AdmittedQueryWitnessProducer {
    repository: Arc<dyn AuthorityWitnessRepositoryPort>,
}
impl AdmittedQueryWitnessProducer {
    pub fn new(repository: Arc<dyn AuthorityWitnessRepositoryPort>) -> Self {
        Self { repository }
    }
}
fn uuid(value: &str) -> Result<Uuid, WitnessErrorDto> {
    let id = Uuid::parse_str(value).map_err(|_| WitnessErrorDto::InvalidBinding)?;
    if id.is_nil() {
        return Err(WitnessErrorDto::InvalidBinding);
    }
    Ok(id)
}
fn decode<T: Message + Default>(body: &[u8]) -> Result<T, WitnessErrorDto> {
    T::decode(body).map_err(|_| WitnessErrorDto::InvalidBinding)
}
fn parent_binding(
    parent: &VerifiedParentAuthority,
) -> Result<(WitnessRootDto, Vec<Uuid>), WitnessErrorDto> {
    let request = parent.request();
    let body = parent
        .message()
        .map_err(|_| WitnessErrorDto::InvalidBinding)?;
    let one = |root, value: &str| Ok((root, vec![uuid(value)?]));
    match request.method {
        "/post.v1.PostService/GetFeed" => Ok((WitnessRootDto::Feed, vec![])),
        "/post.v1.PostService/SearchPosts" => Ok((WitnessRootDto::Search, vec![])),
        "/post.v1.PostService/GetTrending" => Ok((WitnessRootDto::Trending, vec![])),
        "/post.v1.PostService/GetPost" => one(
            WitnessRootDto::Post,
            &decode::<proto::GetPostRequest>(body)?.post_id,
        ),
        "/post.v1.PostService/CreateComment" => one(
            WitnessRootDto::CreateComment,
            &decode::<proto::CreateCommentRequest>(body)?.post_id,
        ),
        "/post.v1.PostService/LikePost" | "/post.v1.PostService/UnlikePost" => {
            let root = if request.method.ends_with("/LikePost") {
                WitnessRootDto::Like
            } else {
                WitnessRootDto::Unlike
            };
            let id = if root == WitnessRootDto::Like {
                decode::<proto::LikePostRequest>(body)?.post_id
            } else {
                decode::<proto::UnlikePostRequest>(body)?.post_id
            };
            one(root, &id)
        }
        "/post.v1.PostService/DeletePost" => one(
            WitnessRootDto::DeletePost,
            &decode::<proto::DeletePostRequest>(body)?.post_id,
        ),
        "/post.v1.PostService/AuthorizeNotificationPosts" => Ok((
            WitnessRootDto::NotificationPosts,
            decode::<proto::AuthorizeNotificationPostsRequest>(body)?
                .post_ids
                .iter()
                .map(|id| uuid(id))
                .collect::<Result<_, _>>()?,
        )),
        "/post.v1.PostService/ReplayMutation" => {
            use proto::replay_mutation_request::Command;
            match decode::<proto::ReplayMutationRequest>(body)?.command {
                Some(Command::CreateComment(command)) => {
                    one(WitnessRootDto::ReplayPostMutation, &command.post_id)
                }
                Some(Command::DeletePost(command)) => {
                    one(WitnessRootDto::ReplayPostMutation, &command.post_id)
                }
                Some(Command::LikePost(command)) => {
                    one(WitnessRootDto::ReplayPostMutation, &command.post_id)
                }
                Some(Command::UnlikePost(command)) => {
                    one(WitnessRootDto::ReplayPostMutation, &command.post_id)
                }
                _ => Err(WitnessErrorDto::InvalidBinding),
            }
        }
        _ => Err(WitnessErrorDto::InvalidBinding),
    }
}
fn validate_query_input(
    parent: &VerifiedParentAuthority,
    observation: &OwnedQueryObservationDto,
    resources: &[Uuid],
) -> Result<(), WitnessErrorDto> {
    use post_domain::feed::{PostCursor, TrendingCursor, MIN_SEARCH_BATCH};
    let body = parent
        .message()
        .map_err(|_| WitnessErrorDto::InvalidBinding)?;
    let canonical = |value: &str| {
        PostCursor::parse(value)
            .map(|cursor| cursor.map(|c| c.encode()))
            .map_err(|_| WitnessErrorDto::InvalidBinding)
    };
    let normalize_limit = |limit: i32| if limit <= 0 { 20 } else { limit.min(100) };
    let valid = match observation.input() {
        OwnedQueryInputDto::Search {
            term,
            cursor,
            batch_limit,
            sequence,
        } => {
            let root = decode::<proto::SearchPostsRequest>(body)?;
            platform::search::normalize_search_term(&root.query)
                .ok()
                .as_ref()
                == Some(term)
                && *batch_limit >= MIN_SEARCH_BATCH as i32
                && *batch_limit <= normalize_limit(root.limit).max(MIN_SEARCH_BATCH as i32)
                && observation.members().len() <= *batch_limit as usize
                && (*sequence != 0 || *cursor == canonical(&root.cursor)?)
                && *sequence < post_domain::feed::SEARCH_SCAN_ROWS as u32
        }
        OwnedQueryInputDto::Chronological { cursor, limit, .. }
        | OwnedQueryInputDto::CachedChronological { cursor, limit, .. } => {
            let root = decode::<proto::FeedRequest>(body)?;
            *cursor == canonical(&root.cursor)?
                && (*limit == normalize_limit(root.limit) || *limit == root.limit.clamp(1, 100))
                && observation.members().len() <= *limit as usize
        }
        OwnedQueryInputDto::Trending { cursor, limit } => {
            let root = decode::<proto::TrendingRequest>(body)?;
            let root_cursor = TrendingCursor::parse(&root.cursor)
                .map_err(|_| WitnessErrorDto::InvalidBinding)?
                .map(|cursor| cursor.encode());
            *cursor == root_cursor
                && (*limit == normalize_limit(root.limit) || *limit == root.limit.clamp(1, 100))
                && observation.members().len() <= *limit as usize
        }
        OwnedQueryInputDto::Ranked {
            per_source_limit, ..
        } => {
            let root = decode::<proto::FeedRequest>(body)?;
            post_domain::recommendation::FeedSnapshotCursor::parse(&root.cursor)
                .map_err(|_| WitnessErrorDto::InvalidBinding)?
                .is_none()
                && (1..=500).contains(per_source_limit)
        }
        OwnedQueryInputDto::Snapshot {
            snapshot_id,
            offset,
            limit,
            page_post_ids,
        } => {
            let root = decode::<proto::FeedRequest>(body)?;
            let expected = post_domain::recommendation::FeedSnapshotCursor::parse(&root.cursor)
                .map_err(|_| WitnessErrorDto::InvalidBinding)?;
            *limit == root.limit.clamp(1, 100)
                && page_post_ids.len() <= *limit as usize
                && expected.map_or(*offset == 0, |cursor| {
                    cursor.snapshot_id == *snapshot_id && cursor.offset == *offset
                })
        }
        OwnedQueryInputDto::Fixed { post_ids } => {
            let mut ids = post_ids.clone();
            ids.sort_unstable();
            let mut expected = resources.to_vec();
            expected.sort_unstable();
            ids == expected
        }
    };
    if !valid {
        return Err(WitnessErrorDto::InvalidBinding);
    }
    Ok(())
}

#[async_trait]
impl OwnedQueryWitnessProducerPort for AdmittedQueryWitnessProducer {
    async fn observe(
        &self,
        observation: OwnedQueryObservationDto,
    ) -> Result<Option<PublishedWitnessStageDto>, WitnessErrorDto> {
        let admitted = VerifiedParentAuthority::current().ok_or(WitnessErrorDto::InvalidBinding)?;
        if admitted.actor() != observation.viewer_id() {
            return Err(WitnessErrorDto::InvalidBinding);
        }
        let now = Utc::now();
        let expires_at = chrono::DateTime::from_timestamp(admitted.expires_at(), 0)
            .ok_or(WitnessErrorDto::InvalidBinding)?;
        let (root, resources) = parent_binding(&admitted)?;
        validate_query_input(&admitted, &observation, &resources)?;
        let parent = WitnessParentDto::from_admitted_request(
            admitted.parent_digest(),
            admitted.request_digest(),
            admitted.actor(),
            root,
            resources,
            expires_at,
            now,
        )?;
        let fingerprint =
            serde_json::to_vec(&observation).map_err(|_| WitnessErrorDto::InvalidBinding)?;
        let query = WitnessQueryDto::new(
            Uuid::new_v4(),
            observation.input().purpose(),
            admitted.query_digest(observation.input().purpose().as_str(), &fingerprint),
            observation.input().sequence(),
        )?;
        let witness = AuthorityWitnessDto::capture(
            parent,
            query,
            observation.members().to_vec(),
            observation.missing().to_vec(),
            now,
        )?;
        self.repository.publish(&witness).await?;
        PublishedWitnessStageDto::after_publication(&witness, Utc::now()).map(Some)
    }
}
