use crate::application::ports::{PolicyRepositoryPort, RolloutRepositoryPort};
use crate::domain::policy::{PolicySet, RolloutPolicy};
use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Clone)]
pub struct PgPolicyRepository {
    pool: PgPool,
}

impl PgPolicyRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(Clone)]
pub struct PgRolloutRepository {
    pool: PgPool,
}

impl PgRolloutRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl PgPolicyRepository {
    async fn find_all_infrastructure_inner(&self) -> anyhow::Result<Vec<PolicySet>> {
        let rows = sqlx::query(
            r#"
            select id, name, rate_limit, quota, timeout_ms, retry_policy, circuit_breaker_policy, cache_policy
            from policy_sets
            order by name
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(map_policy).collect())
    }

    async fn find_by_id_infrastructure_inner(
        &self,
        policy_id: &str,
    ) -> anyhow::Result<Option<PolicySet>> {
        let row = sqlx::query(
            r#"
            select id, name, rate_limit, quota, timeout_ms, retry_policy, circuit_breaker_policy, cache_policy
            from policy_sets
            where id = $1
            "#,
        )
        .bind(policy_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(map_policy))
    }

    async fn save_infrastructure_inner(&self, policy: PolicySet) -> anyhow::Result<PolicySet> {
        let id = if policy.id.trim().is_empty() {
            Uuid::new_v4().to_string()
        } else {
            policy.id.clone()
        };

        sqlx::query(
            r#"
            insert into policy_sets (
                id, name, rate_limit, quota, timeout_ms, retry_policy, circuit_breaker_policy, cache_policy
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8)
            on conflict (id) do update
            set name = excluded.name,
                rate_limit = excluded.rate_limit,
                quota = excluded.quota,
                timeout_ms = excluded.timeout_ms,
                retry_policy = excluded.retry_policy,
                circuit_breaker_policy = excluded.circuit_breaker_policy,
                cache_policy = excluded.cache_policy,
                updated_at = now()
            "#,
        )
        .bind(&id)
        .bind(&policy.name)
        .bind(&policy.rate_limit)
        .bind(&policy.quota)
        .bind(policy.timeout_ms)
        .bind(&policy.retry_policy)
        .bind(&policy.circuit_breaker_policy)
        .bind(&policy.cache_policy)
        .execute(&self.pool)
        .await?;

        Ok(PolicySet { id, ..policy })
    }
}

#[async_trait]
impl PolicyRepositoryPort for PgPolicyRepository {
    async fn find_all(
        &self,
    ) -> crate::application::infrastructure_error::GatewayInfrastructureResult<
        Vec<crate::application::dto::infrastructure_dto::PolicySetDto>,
    > {
        self.find_all_infrastructure_inner()
            .await
            .map(|values| values.into_iter().map(Into::into).collect())
            .map_err(crate::adapters::out::infrastructure_error::classify_infrastructure_error)
    }
    async fn find_by_id(
        &self,
        policy_id: &str,
    ) -> crate::application::infrastructure_error::GatewayInfrastructureResult<
        Option<crate::application::dto::infrastructure_dto::PolicySetDto>,
    > {
        self.find_by_id_infrastructure_inner(policy_id)
            .await
            .map(|value| value.map(Into::into))
            .map_err(crate::adapters::out::infrastructure_error::classify_infrastructure_error)
    }
    async fn save(
        &self,
        policy: crate::application::dto::infrastructure_dto::PolicySetDto,
    ) -> crate::application::infrastructure_error::GatewayInfrastructureResult<
        crate::application::dto::infrastructure_dto::PolicySetDto,
    > {
        self.save_infrastructure_inner(policy.into())
            .await
            .map(Into::into)
            .map_err(crate::adapters::out::infrastructure_error::classify_infrastructure_error)
    }
}

impl PgRolloutRepository {
    async fn find_by_id_infrastructure_inner(
        &self,
        rollout_id: &str,
    ) -> anyhow::Result<Option<RolloutPolicy>> {
        let row = sqlx::query(
            r#"
            select id, contract_id, version, strategy, status, percent
            from rollouts
            where id = $1
            "#,
        )
        .bind(rollout_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(map_rollout))
    }

    async fn save_infrastructure_inner(
        &self,
        rollout: RolloutPolicy,
    ) -> anyhow::Result<RolloutPolicy> {
        let id = if rollout.id.trim().is_empty() {
            Uuid::new_v4().to_string()
        } else {
            rollout.id.clone()
        };

        sqlx::query(
            r#"
            insert into rollouts (id, contract_id, version, strategy, status, percent)
            values ($1, $2, $3, $4, $5, $6)
            on conflict (id) do update
            set contract_id = excluded.contract_id,
                version = excluded.version,
                strategy = excluded.strategy,
                status = excluded.status,
                percent = excluded.percent
            "#,
        )
        .bind(&id)
        .bind(&rollout.contract_id)
        .bind(&rollout.version)
        .bind(&rollout.strategy)
        .bind(&rollout.status)
        .bind(rollout.percent)
        .execute(&self.pool)
        .await?;

        Ok(RolloutPolicy { id, ..rollout })
    }
}

#[async_trait]
impl RolloutRepositoryPort for PgRolloutRepository {
    async fn find_by_id(
        &self,
        rollout_id: &str,
    ) -> crate::application::infrastructure_error::GatewayInfrastructureResult<
        Option<crate::application::dto::infrastructure_dto::RolloutPolicyDto>,
    > {
        self.find_by_id_infrastructure_inner(rollout_id)
            .await
            .map(|value| value.map(Into::into))
            .map_err(crate::adapters::out::infrastructure_error::classify_infrastructure_error)
    }
    async fn save(
        &self,
        rollout: crate::application::dto::infrastructure_dto::RolloutPolicyDto,
    ) -> crate::application::infrastructure_error::GatewayInfrastructureResult<
        crate::application::dto::infrastructure_dto::RolloutPolicyDto,
    > {
        self.save_infrastructure_inner(rollout.into())
            .await
            .map(Into::into)
            .map_err(crate::adapters::out::infrastructure_error::classify_infrastructure_error)
    }
}

fn map_policy(row: sqlx::postgres::PgRow) -> PolicySet {
    PolicySet {
        id: row.get("id"),
        name: row.get("name"),
        rate_limit: row.get("rate_limit"),
        quota: row.get("quota"),
        timeout_ms: row.get("timeout_ms"),
        retry_policy: row.get("retry_policy"),
        circuit_breaker_policy: row.get("circuit_breaker_policy"),
        cache_policy: row.get("cache_policy"),
    }
}

fn map_rollout(row: sqlx::postgres::PgRow) -> RolloutPolicy {
    RolloutPolicy {
        id: row.get("id"),
        contract_id: row.get("contract_id"),
        version: row.get("version"),
        strategy: row.get("strategy"),
        status: row.get("status"),
        percent: row.get("percent"),
    }
}
