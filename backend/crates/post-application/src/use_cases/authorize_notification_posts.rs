use super::PostApplicationService;
use crate::authority_witness::WitnessMemberDto;
use crate::dto::post_dto::{AuthorizeNotificationPostsDto, AuthorizedNotificationPostsDto};
use crate::query_witness::{OwnedQueryInputDto, OwnedQueryObservationDto};
use anyhow::Result;
use std::collections::HashSet;
use std::sync::Arc;

#[derive(Clone)]
pub struct AuthorizeNotificationPostsUseCase {
    service: Arc<PostApplicationService>,
}

impl AuthorizeNotificationPostsUseCase {
    pub fn new(service: Arc<PostApplicationService>) -> Self {
        Self { service }
    }

    pub async fn execute(
        &self,
        query: AuthorizeNotificationPostsDto,
    ) -> Result<AuthorizedNotificationPostsDto> {
        anyhow::ensure!(
            query.post_ids.len() <= 100,
            "Notification authorization exceeds its bound"
        );
        // Empty input needs no resource-dependent authorization call.
        if query.post_ids.is_empty() {
            return Ok(AuthorizedNotificationPostsDto {
                visible_post_ids: vec![],
            });
        }
        let access = self
            .service
            .posts
            .fetch_post_access_snapshots(&query.post_ids)
            .await?;
        let members = access
            .iter()
            .map(|(id, post)| WitnessMemberDto::from_owned_query(*id, post.author_id))
            .collect::<Result<Vec<_>, _>>()?;
        let missing = query
            .post_ids
            .iter()
            .filter(|id| !access.contains_key(id))
            .copied()
            .collect();
        let stage = self
            .service
            .query_witnesses
            .observe(OwnedQueryObservationDto::new(
                query.viewer_id,
                OwnedQueryInputDto::Fixed {
                    post_ids: query.post_ids.clone(),
                },
                members,
                missing,
            )?)
            .await?;
        let authors: Vec<_> = access
            .values()
            .map(|post| post.author_id)
            .filter(|author| *author != query.viewer_id)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let policies = if authors.is_empty() {
            Default::default()
        } else {
            self.service
                .friend_client
                .batch_relationship_policies(query.viewer_id, &authors, stage.as_ref())
                .await?
        };
        anyhow::ensure!(
            authors.iter().all(|id| policies.contains_key(id)),
            "Incomplete notification authorization policies"
        );
        let visible_post_ids = query
            .post_ids
            .into_iter()
            .filter(|id| {
                let Some(post) = access.get(id) else {
                    return false;
                };
                if post.author_id == query.viewer_id {
                    return matches!(post.visibility.as_str(), "PUBLIC" | "FRIENDS" | "PRIVATE");
                }
                let Some(policy) = policies.get(&post.author_id) else {
                    return false;
                };
                !policy.is_blocked()
                    && !policy.muted_by_viewer
                    && match post.visibility.as_str() {
                        "PUBLIC" => true,
                        "FRIENDS" => policy.following,
                        _ => false,
                    }
            })
            .collect();
        Ok(AuthorizedNotificationPostsDto { visible_post_ids })
    }
}
