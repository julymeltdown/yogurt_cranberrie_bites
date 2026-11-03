use crate::dto::profile_dto::{PurgeExpiredProfilesDto, PurgeExpiredProfilesResponseDto};
use crate::errors::{ProfileError, ProfileResult as Result};
use crate::ports::ProfileRepositoryPort;
use crate::profile_runtime::ProfileRuntimePort;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct PurgeExpiredProfilesUseCase<R> {
    repository: R,
    runtime: Arc<dyn ProfileRuntimePort>,
}

impl<R> PurgeExpiredProfilesUseCase<R>
where
    R: ProfileRepositoryPort,
{
    pub fn new(repository: R, runtime: Arc<dyn ProfileRuntimePort>) -> Self {
        Self {
            repository,
            runtime,
        }
    }

    pub async fn execute(
        &self,
        request: PurgeExpiredProfilesDto,
    ) -> Result<PurgeExpiredProfilesResponseDto> {
        let actor_id = parse_uuid(&request.actor_id, "Actor ID is required")?;
        let purged = self
            .repository
            .purge_expired_profiles(
                self.runtime.now(),
                actor_id,
                optional_text(request.request_id),
                optional_text(request.ip_address),
                optional_text(request.user_agent),
                request.dry_run,
            )
            .await?;

        Ok(PurgeExpiredProfilesResponseDto {
            purged_count: purged.len() as i64,
        })
    }
}

fn parse_uuid(value: &str, error: &'static str) -> Result<Uuid> {
    if value.trim().is_empty() {
        return Err(ProfileError::InvalidInput(error));
    }
    Uuid::parse_str(value.trim()).map_err(|_| ProfileError::InvalidInput(error))
}

fn optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}
