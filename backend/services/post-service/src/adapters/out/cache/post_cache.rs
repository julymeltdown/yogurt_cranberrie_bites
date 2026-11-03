use super::{connect_manager, run_bounded_cache_operation};
use crate::application::ports::PostCachePort;
use crate::domain::post::Post;
use anyhow::Result;
use async_trait::async_trait;
use redis::aio::ConnectionManager;
use redis::Client;
use std::collections::HashMap;
use uuid::Uuid;

const POST_CACHE_TTL_SECONDS: u64 = 300;

#[derive(Clone)]
pub struct RedisPostCache {
    /// Reused like the feed cache connection: one handle for every call rather
    /// than a new connection and handshake per operation.
    connection: ConnectionManager,
}

impl RedisPostCache {
    pub fn with_connection(connection: ConnectionManager) -> Self {
        Self { connection }
    }

    pub async fn new(client: Client) -> Result<Self> {
        Ok(Self::with_connection(connect_manager(client).await?))
    }

    pub(super) fn key(post_id: Uuid) -> String {
        format!("post:{post_id}")
    }
}

#[async_trait]
impl PostCachePort for RedisPostCache {
    async fn get_posts(&self, post_ids: &[Uuid]) -> Result<HashMap<Uuid, Post>> {
        run_bounded_cache_operation("Redis post cache read", async {
            if post_ids.is_empty() {
                return Ok(HashMap::new());
            }
            let mut connection = self.connection.clone();
            let keys = post_ids.iter().copied().map(Self::key).collect::<Vec<_>>();
            let payloads: Vec<Option<String>> = redis::cmd("MGET")
                .arg(&keys)
                .query_async(&mut connection)
                .await?;
            let mut posts = HashMap::new();
            for (post_id, payload) in post_ids.iter().copied().zip(payloads) {
                if let Some(payload) = payload {
                    posts.insert(post_id, serde_json::from_str(&payload)?);
                }
            }
            Ok(posts)
        })
        .await
    }

    async fn set_post(&self, post: &Post) -> Result<()> {
        self.set_posts(std::slice::from_ref(post)).await
    }

    async fn set_posts(&self, posts: &[Post]) -> Result<()> {
        run_bounded_cache_operation("Redis post cache write", async {
            if posts.is_empty() {
                return Ok(());
            }
            let mut connection = self.connection.clone();
            let mut pipeline = redis::pipe();
            for post in posts {
                pipeline
                    .cmd("SETEX")
                    .arg(Self::key(post.id))
                    .arg(POST_CACHE_TTL_SECONDS)
                    .arg(serde_json::to_string(post)?)
                    .ignore();
            }
            let _: () = pipeline.query_async(&mut connection).await?;
            Ok(())
        })
        .await
    }

    async fn evict(&self, post_id: Uuid) -> Result<()> {
        run_bounded_cache_operation("Redis post cache eviction", async {
            let mut connection = self.connection.clone();
            let _: usize = redis::cmd("DEL")
                .arg(Self::key(post_id))
                .query_async::<usize>(&mut connection)
                .await?;
            Ok(())
        })
        .await
    }
}
