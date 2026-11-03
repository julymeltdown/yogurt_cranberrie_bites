use crate::application::ports::ClientProfileRepositoryPort;
use crate::domain::client::ClientProfile;
use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Clone)]
pub struct PgClientProfileRepository {
    pool: PgPool,
}

impl PgClientProfileRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl PgClientProfileRepository {
    async fn find_all_infrastructure_inner(&self) -> anyhow::Result<Vec<ClientProfile>> {
        let rows = sqlx::query(
            r#"
            select id, client_type, allowed_contracts, policy_set_id
            from client_profiles
            order by client_type
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(map_client_profile).collect())
    }

    async fn find_by_id_infrastructure_inner(
        &self,
        client_id: &str,
    ) -> anyhow::Result<Option<ClientProfile>> {
        let row = sqlx::query(
            r#"
            select id, client_type, allowed_contracts, policy_set_id
            from client_profiles
            where id = $1
            "#,
        )
        .bind(client_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(map_client_profile))
    }

    async fn save_infrastructure_inner(
        &self,
        profile: ClientProfile,
    ) -> anyhow::Result<ClientProfile> {
        let id = if profile.id.trim().is_empty() {
            Uuid::new_v4().to_string()
        } else {
            profile.id.clone()
        };
        let allowed_contracts = serde_json::to_string(&profile.allowed_contracts)?;

        sqlx::query(
            r#"
            insert into client_profiles (id, client_type, allowed_contracts, policy_set_id)
            values ($1, $2, $3, $4)
            on conflict (id) do update
            set client_type = excluded.client_type,
                allowed_contracts = excluded.allowed_contracts,
                policy_set_id = excluded.policy_set_id,
                updated_at = now()
            "#,
        )
        .bind(&id)
        .bind(&profile.client_type)
        .bind(allowed_contracts)
        .bind(&profile.policy_set_id)
        .execute(&self.pool)
        .await?;

        Ok(ClientProfile { id, ..profile })
    }
}

#[async_trait]
impl ClientProfileRepositoryPort for PgClientProfileRepository {
    async fn find_all(
        &self,
    ) -> crate::application::infrastructure_error::GatewayInfrastructureResult<
        Vec<crate::application::dto::infrastructure_dto::ClientProfileDto>,
    > {
        self.find_all_infrastructure_inner()
            .await
            .map(|values| values.into_iter().map(Into::into).collect())
            .map_err(crate::adapters::out::infrastructure_error::classify_infrastructure_error)
    }
    async fn find_by_id(
        &self,
        client_id: &str,
    ) -> crate::application::infrastructure_error::GatewayInfrastructureResult<
        Option<crate::application::dto::infrastructure_dto::ClientProfileDto>,
    > {
        self.find_by_id_infrastructure_inner(client_id)
            .await
            .map(|value| value.map(Into::into))
            .map_err(crate::adapters::out::infrastructure_error::classify_infrastructure_error)
    }
    async fn save(
        &self,
        profile: crate::application::dto::infrastructure_dto::ClientProfileDto,
    ) -> crate::application::infrastructure_error::GatewayInfrastructureResult<
        crate::application::dto::infrastructure_dto::ClientProfileDto,
    > {
        self.save_infrastructure_inner(profile.into())
            .await
            .map(Into::into)
            .map_err(crate::adapters::out::infrastructure_error::classify_infrastructure_error)
    }
}

fn map_client_profile(row: sqlx::postgres::PgRow) -> ClientProfile {
    let allowed_contracts = row.get::<String, _>("allowed_contracts");
    ClientProfile {
        id: row.get("id"),
        client_type: row.get("client_type"),
        allowed_contracts: serde_json::from_str(&allowed_contracts).unwrap_or_default(),
        policy_set_id: row.get("policy_set_id"),
    }
}
