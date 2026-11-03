use crate::dto::aggregation_dto::AggregationResponse;
use crate::ports::{AggregationRecipeRepositoryPort, ResponseCachePort};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct AggregationService {
    repository: Arc<dyn AggregationRecipeRepositoryPort>,
    cache: Arc<dyn ResponseCachePort>,
    cache_ttl: Duration,
}

impl AggregationService {
    pub fn new(
        repository: Arc<dyn AggregationRecipeRepositoryPort>,
        cache: Arc<dyn ResponseCachePort>,
        cache_ttl: Duration,
    ) -> Self {
        Self {
            repository,
            cache,
            cache_ttl,
        }
    }

    pub async fn aggregate(
        &self,
        route_key: String,
    ) -> crate::service_error::GatewayServiceResult<AggregationResponse> {
        let cache_key = format!("aggregation:{route_key}");
        if let Some(payload) = self.cache.get(&cache_key).await? {
            return Ok(AggregationResponse {
                route_key,
                cached: true,
                payload: payload.value,
            });
        }

        let recipe = self
            .repository
            .find_by_route_key(&route_key)
            .await?
            .ok_or(crate::service_error::GatewayServiceError::AggregationRecipeMissing)?;
        let recipe = gateway_domain::aggregation::AggregationRecipe::from(recipe);
        let payload = json!({
            "routeKey": recipe.route_key,
            "joinStrategy": recipe.join_strategy,
            "downstreams": recipe.downstream_calls,
        });
        self.cache
            .put(&cache_key, payload.clone().into(), self.cache_ttl)
            .await?;
        Ok(AggregationResponse {
            route_key,
            cached: false,
            payload,
        })
    }
}
