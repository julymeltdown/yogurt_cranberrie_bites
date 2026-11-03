use crate::dto::mutation_dto::PurgeExpiredMutationOutcomesDto;
use crate::errors::ProfileResult as Result;
use crate::ports::ProfileMutationReceiptMaintenancePort;

#[derive(Clone)]
pub struct PurgeExpiredMutationOutcomesUseCase<R> {
    repository: R,
}

impl<R> PurgeExpiredMutationOutcomesUseCase<R>
where
    R: ProfileMutationReceiptMaintenancePort,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn execute(&self, request: PurgeExpiredMutationOutcomesDto) -> Result<i64> {
        self.repository
            .purge_expired_mutation_outcomes(request.batch_size.clamp(1, 5_000))
            .await
    }
}
