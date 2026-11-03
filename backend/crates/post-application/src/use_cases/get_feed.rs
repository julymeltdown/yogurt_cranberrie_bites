use super::get_ranked_feed::RankedFeedService;
use super::PostApplicationService;
use crate::dto::post_dto::FeedDto;
use anyhow::Result;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct GetFeedUseCase {
    service: Arc<PostApplicationService>,
    ranked: Option<Arc<RankedFeedService>>,
}

impl GetFeedUseCase {
    pub fn new(service: Arc<PostApplicationService>) -> Self {
        Self {
            service,
            ranked: None,
        }
    }

    pub fn with_ranked(
        service: Arc<PostApplicationService>,
        ranked: Arc<RankedFeedService>,
    ) -> Self {
        Self {
            service,
            ranked: Some(ranked),
        }
    }

    pub async fn execute(
        &self,
        user_id: Uuid,
        cursor: Option<&str>,
        limit: i32,
    ) -> Result<FeedDto> {
        self.execute_with_context(user_id, cursor, limit, None, None)
            .await
    }

    pub async fn execute_with_context(
        &self,
        user_id: Uuid,
        cursor: Option<&str>,
        limit: i32,
        request_id: Option<&str>,
        session_id: Option<Uuid>,
    ) -> Result<FeedDto> {
        match self.ranked.as_ref() {
            Some(ranked) => {
                ranked
                    .execute(user_id, cursor, limit, request_id, session_id)
                    .await
            }
            None => self.service.get_feed(user_id, cursor, limit).await,
        }
    }
}
