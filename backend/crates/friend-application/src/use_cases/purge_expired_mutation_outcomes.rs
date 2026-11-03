use crate::dto::mutation_dto::PurgeExpiredMutationOutcomesDto;
use crate::ports::MutationReceiptMaintenancePort;
use anyhow::Result;

#[derive(Clone)]
pub struct PurgeExpiredMutationOutcomesUseCase<R> {
    repository: R,
}

impl<R> PurgeExpiredMutationOutcomesUseCase<R>
where
    R: MutationReceiptMaintenancePort,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn execute(&self, request: PurgeExpiredMutationOutcomesDto) -> Result<i64> {
        let batch_size = request.batch_size.clamp(1, 5_000);
        self.repository.purge_expired_outcomes(batch_size).await
    }
}
