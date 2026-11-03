use crate::application::ports::RouteContractRepositoryPort;
use crate::domain::contract::{RouteContract, RouteContractVersion};
use async_trait::async_trait;
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Clone)]
pub struct PgRouteContractRepository {
    pool: PgPool,
}

impl PgRouteContractRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn load_versions(
        &self,
        contract_ids: &[String],
    ) -> anyhow::Result<HashMap<String, Vec<RouteContractVersion>>> {
        if contract_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let rows = sqlx::query(
            r#"
            select contract_id, version, status, deprecation_date, response_schema_ref
            from route_contract_versions
            where contract_id = any($1)
            order by version
            "#,
        )
        .bind(contract_ids)
        .fetch_all(&self.pool)
        .await?;

        let mut grouped = HashMap::<String, Vec<RouteContractVersion>>::new();
        for row in rows {
            grouped
                .entry(row.get::<String, _>("contract_id"))
                .or_default()
                .push(RouteContractVersion {
                    version: row.get("version"),
                    status: row.get("status"),
                    deprecation_date: row.get("deprecation_date"),
                    response_schema_ref: row.get("response_schema_ref"),
                });
        }
        Ok(grouped)
    }
}

impl PgRouteContractRepository {
    async fn find_all_infrastructure_inner(&self) -> anyhow::Result<Vec<RouteContract>> {
        let rows = sqlx::query(
            r#"
            select id, route_key, owner, sla_target, status, description
            from route_contracts
            order by route_key
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let contract_ids = rows
            .iter()
            .map(|row| row.get::<String, _>("id"))
            .collect::<Vec<_>>();
        let versions = self.load_versions(&contract_ids).await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                let id = row.get::<String, _>("id");
                RouteContract {
                    id: id.clone(),
                    route_key: row.get("route_key"),
                    owner: row.get("owner"),
                    sla_target: row.get("sla_target"),
                    status: row.get("status"),
                    description: row.get("description"),
                    versions: versions.get(&id).cloned().unwrap_or_default(),
                }
            })
            .collect())
    }

    async fn find_by_id_infrastructure_inner(
        &self,
        contract_id: &str,
    ) -> anyhow::Result<Option<RouteContract>> {
        let row = sqlx::query(
            r#"
            select id, route_key, owner, sla_target, status, description
            from route_contracts
            where id = $1
            "#,
        )
        .bind(contract_id)
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = row else {
            return Ok(None);
        };

        let versions = self
            .load_versions(&[contract_id.to_owned()])
            .await?
            .remove(contract_id)
            .unwrap_or_default();

        Ok(Some(RouteContract {
            id: row.get("id"),
            route_key: row.get("route_key"),
            owner: row.get("owner"),
            sla_target: row.get("sla_target"),
            status: row.get("status"),
            description: row.get("description"),
            versions,
        }))
    }

    async fn find_version_infrastructure_inner(
        &self,
        contract_id: &str,
        version: &str,
    ) -> anyhow::Result<Option<RouteContractVersion>> {
        let row = sqlx::query(
            r#"
            select version, status, deprecation_date, response_schema_ref
            from route_contract_versions
            where contract_id = $1 and version = $2
            "#,
        )
        .bind(contract_id)
        .bind(version)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| RouteContractVersion {
            version: row.get("version"),
            status: row.get("status"),
            deprecation_date: row.get("deprecation_date"),
            response_schema_ref: row.get("response_schema_ref"),
        }))
    }

    async fn save_infrastructure_inner(
        &self,
        contract: RouteContract,
    ) -> anyhow::Result<RouteContract> {
        let id = if contract.id.trim().is_empty() {
            Uuid::new_v4().to_string()
        } else {
            contract.id.clone()
        };

        sqlx::query(
            r#"
            insert into route_contracts (id, route_key, owner, sla_target, status, description)
            values ($1, $2, $3, $4, $5, $6)
            on conflict (id) do update
            set route_key = excluded.route_key,
                owner = excluded.owner,
                sla_target = excluded.sla_target,
                status = excluded.status,
                description = excluded.description,
                updated_at = now()
            "#,
        )
        .bind(&id)
        .bind(&contract.route_key)
        .bind(&contract.owner)
        .bind(&contract.sla_target)
        .bind(&contract.status)
        .bind(&contract.description)
        .execute(&self.pool)
        .await?;

        Ok(RouteContract { id, ..contract })
    }

    async fn save_version_infrastructure_inner(
        &self,
        contract_id: &str,
        version: RouteContractVersion,
    ) -> anyhow::Result<RouteContractVersion> {
        sqlx::query(
            r#"
            insert into route_contract_versions (
                contract_id, version, status, deprecation_date, response_schema_ref
            )
            values ($1, $2, $3, $4, $5)
            on conflict (contract_id, version) do update
            set status = excluded.status,
                deprecation_date = excluded.deprecation_date,
                response_schema_ref = excluded.response_schema_ref
            "#,
        )
        .bind(contract_id)
        .bind(&version.version)
        .bind(&version.status)
        .bind(version.deprecation_date)
        .bind(&version.response_schema_ref)
        .execute(&self.pool)
        .await?;

        Ok(version)
    }
}

#[async_trait]
impl RouteContractRepositoryPort for PgRouteContractRepository {
    async fn find_all(
        &self,
    ) -> crate::application::infrastructure_error::GatewayInfrastructureResult<
        Vec<crate::application::dto::infrastructure_dto::RouteContractDto>,
    > {
        self.find_all_infrastructure_inner()
            .await
            .map(|values| values.into_iter().map(Into::into).collect())
            .map_err(crate::adapters::out::infrastructure_error::classify_infrastructure_error)
    }
    async fn find_by_id(
        &self,
        contract_id: &str,
    ) -> crate::application::infrastructure_error::GatewayInfrastructureResult<
        Option<crate::application::dto::infrastructure_dto::RouteContractDto>,
    > {
        self.find_by_id_infrastructure_inner(contract_id)
            .await
            .map(|value| value.map(Into::into))
            .map_err(crate::adapters::out::infrastructure_error::classify_infrastructure_error)
    }
    async fn find_version(
        &self,
        contract_id: &str,
        version: &str,
    ) -> crate::application::infrastructure_error::GatewayInfrastructureResult<
        Option<crate::application::dto::infrastructure_dto::RouteContractVersionDto>,
    > {
        self.find_version_infrastructure_inner(contract_id, version)
            .await
            .map(|value| value.map(Into::into))
            .map_err(crate::adapters::out::infrastructure_error::classify_infrastructure_error)
    }
    async fn save(
        &self,
        contract: crate::application::dto::infrastructure_dto::RouteContractDto,
    ) -> crate::application::infrastructure_error::GatewayInfrastructureResult<
        crate::application::dto::infrastructure_dto::RouteContractDto,
    > {
        self.save_infrastructure_inner(contract.into())
            .await
            .map(Into::into)
            .map_err(crate::adapters::out::infrastructure_error::classify_infrastructure_error)
    }
    async fn save_version(
        &self,
        contract_id: &str,
        version: crate::application::dto::infrastructure_dto::RouteContractVersionDto,
    ) -> crate::application::infrastructure_error::GatewayInfrastructureResult<
        crate::application::dto::infrastructure_dto::RouteContractVersionDto,
    > {
        self.save_version_infrastructure_inner(contract_id, version.into())
            .await
            .map(Into::into)
            .map_err(crate::adapters::out::infrastructure_error::classify_infrastructure_error)
    }
}
