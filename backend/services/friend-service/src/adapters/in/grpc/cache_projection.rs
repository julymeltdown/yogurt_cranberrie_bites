//! Minimal system projection. Registration requires CacheGuard plus verified
//! workload policy; this handler cannot run a query without its sealed proof.
use crate::application::ports::FollowRepositoryPort;
use crate::application::use_cases::list_followers::ListFollowersUseCase;
use crate::domain::follow_edge::FollowCursor;
use platform::actor_delegation::cache::AdmittedCache;
use platform::grpc_auth::{authorize_call, RpcAction, Workload};
use platform::grpc_method_policy::MethodPolicy;
use std::{collections::HashSet, sync::Arc};
use tonic::{Request, Response, Status};
use uuid::Uuid;

use super::fanout_projection::proto;
pub static CACHE_PROJECTION_POLICY: MethodPolicy = MethodPolicy {
    service: "authority.v1.CacheProjectionService",
    workload: Some(Workload::Friend),
    methods: &[("ListCacheFollowers", &[Workload::Post])],
    actor_fields: &[],
};
#[derive(Clone)]
pub struct CacheProjectionGrpcServer {
    query: ListFollowersUseCase<Arc<dyn FollowRepositoryPort>>,
}
impl CacheProjectionGrpcServer {
    pub fn new(repository: Arc<dyn FollowRepositoryPort>) -> Self {
        Self {
            query: ListFollowersUseCase::new(repository),
        }
    }
}
#[tonic::async_trait]
impl proto::cache_projection_service_server::CacheProjectionService for CacheProjectionGrpcServer {
    async fn list_cache_followers(
        &self,
        request: Request<proto::ListCacheFollowersRequest>,
    ) -> Result<Response<proto::ListCacheFollowersResponse>, Status> {
        if authorize_call(&request, RpcAction::Read)? != Workload::Post {
            return Err(Status::permission_denied("Post presenter required"));
        }
        let admitted = request
            .extensions()
            .get::<AdmittedCache>()
            .ok_or_else(|| Status::permission_denied("Fresh cache admission required"))?;
        let deadline = admitted
            .deadline()
            .map_err(platform::actor_delegation::cache::status)?;
        let scope = admitted.scope();
        // The guard hashes the original wire bytes; Gateway independently
        // decodes that exact body and compares every scope field before admission.
        let cursor = scope
            .cursor
            .as_deref()
            .map(FollowCursor::parse)
            .transpose()
            .map_err(|_| Status::invalid_argument("Invalid cache cursor"))?
            .flatten();
        let page = tokio::time::timeout_at(
            deadline.stop(),
            self.query.execute(scope.author_id, cursor, scope.limit),
        )
        .await
        .map_err(|_| Status::permission_denied("Authority expired during projection"))?
        .map_err(|_| Status::unavailable("Cache projection unavailable"))?;
        deadline
            .remaining(
                std::time::Duration::from_secs(60),
                deadline.expires_micros(),
            )
            .map_err(|_| Status::permission_denied("Authority expired during projection"))?;
        let invalid = || Status::unavailable("Invalid cache projection");
        let mut seen = HashSet::new();
        let mut ids = Vec::new();
        let mut last = None;
        if page.edges.len() > scope.limit as usize {
            return Err(invalid());
        }
        for edge in page.edges {
            let follower = Uuid::parse_str(&edge.follower_id).map_err(|_| invalid())?;
            let followee = Uuid::parse_str(&edge.followee_id).map_err(|_| invalid())?;
            let created = chrono::DateTime::parse_from_rfc3339(&edge.created_at)
                .map_err(|_| invalid())?
                .with_timezone(&chrono::Utc);
            let boundary = (created, follower);
            if follower.is_nil()
                || follower == scope.author_id
                || followee != scope.author_id
                || !seen.insert(follower)
                || last.is_some_and(|previous| boundary >= previous)
                || cursor.is_some_and(|previous| {
                    boundary >= (previous.created_at, previous.tie_breaker_id)
                })
            {
                return Err(invalid());
            }
            last = Some(boundary);
            ids.push(follower.to_string());
        }
        if let Some(next) = &page.next_cursor {
            let parsed = FollowCursor::parse(next)
                .map_err(|_| invalid())?
                .ok_or_else(invalid)?;
            if Some((parsed.created_at, parsed.tie_breaker_id)) != last {
                return Err(invalid());
            }
        }
        deadline
            .remaining(
                std::time::Duration::from_secs(60),
                deadline.expires_micros(),
            )
            .map_err(|_| Status::permission_denied("Authority expired before projection reply"))?;
        Ok(Response::new(proto::ListCacheFollowersResponse {
            author_id: scope.author_id.to_string(),
            follower_ids: ids,
            next_cursor: page.next_cursor,
            expires_micros: admitted.expires_micros(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_table_matches_exact_proto_service_and_denies_other_workloads() {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../proto/authority/v1/authority.proto"
        ));
        let section = source
            .split("service CacheProjectionService")
            .nth(1)
            .unwrap()
            .split('}')
            .next()
            .unwrap();
        assert_eq!(
            platform::grpc_method_policy::proto_rpc_names(section),
            vec!["ListCacheFollowers"]
        );
        assert_eq!(
            CACHE_PROJECTION_POLICY.method_names().collect::<Vec<_>>(),
            vec!["ListCacheFollowers"]
        );
        for caller in Workload::ALL {
            assert_eq!(
                CACHE_PROJECTION_POLICY.allows(
                    "/authority.v1.CacheProjectionService/ListCacheFollowers",
                    caller
                ),
                caller == Workload::Post
            );
        }
    }
}
