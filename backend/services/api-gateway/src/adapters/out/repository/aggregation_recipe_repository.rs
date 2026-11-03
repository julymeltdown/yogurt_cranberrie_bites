use crate::application::ports::AggregationRecipeRepositoryPort;
use crate::domain::aggregation::AggregationRecipe;
use async_trait::async_trait;
use sqlx::{PgPool, Row};

#[derive(Clone)]
pub struct PgAggregationRecipeRepository {
    pool: PgPool,
}

impl PgAggregationRecipeRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl PgAggregationRecipeRepository {
    async fn find_all_infrastructure_inner(&self) -> anyhow::Result<Vec<AggregationRecipe>> {
        let rows = sqlx::query(
            r#"
            select id, route_key, downstream_calls, join_strategy, timeout_ms, cache_policy
            from aggregation_recipes
            order by route_key
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(map_recipe).collect())
    }

    async fn find_by_route_key_infrastructure_inner(
        &self,
        route_key: &str,
    ) -> anyhow::Result<Option<AggregationRecipe>> {
        let row = sqlx::query(
            r#"
            select id, route_key, downstream_calls, join_strategy, timeout_ms, cache_policy
            from aggregation_recipes
            where route_key = $1
            "#,
        )
        .bind(route_key)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(map_recipe))
    }
}

#[async_trait]
impl AggregationRecipeRepositoryPort for PgAggregationRecipeRepository {
    async fn find_all(
        &self,
    ) -> crate::application::infrastructure_error::GatewayInfrastructureResult<
        Vec<crate::application::dto::infrastructure_dto::AggregationRecipeDto>,
    > {
        self.find_all_infrastructure_inner()
            .await
            .map(|values| values.into_iter().map(Into::into).collect())
            .map_err(crate::adapters::out::infrastructure_error::classify_infrastructure_error)
    }
    async fn find_by_route_key(
        &self,
        route_key: &str,
    ) -> crate::application::infrastructure_error::GatewayInfrastructureResult<
        Option<crate::application::dto::infrastructure_dto::AggregationRecipeDto>,
    > {
        self.find_by_route_key_infrastructure_inner(route_key)
            .await
            .map(|value| value.map(Into::into))
            .map_err(crate::adapters::out::infrastructure_error::classify_infrastructure_error)
    }
}

fn map_recipe(row: sqlx::postgres::PgRow) -> AggregationRecipe {
    let downstream_calls = row.get::<String, _>("downstream_calls");
    AggregationRecipe {
        id: row.get("id"),
        route_key: row.get("route_key"),
        downstream_calls: serde_json::from_str(&downstream_calls).unwrap_or_default(),
        join_strategy: row.get("join_strategy"),
        timeout_ms: row.get("timeout_ms"),
        cache_policy: row.get("cache_policy"),
    }
}
