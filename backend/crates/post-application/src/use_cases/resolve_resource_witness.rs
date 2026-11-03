use crate::authority_witness::{
    AuthorityWitnessRepositoryPort, ResolveWitnessDto, WitnessErrorDto, WitnessSubsetDto,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct ResolveResourceWitnessUseCase {
    repository: Arc<dyn AuthorityWitnessRepositoryPort>,
}
impl ResolveResourceWitnessUseCase {
    pub fn new(repository: Arc<dyn AuthorityWitnessRepositoryPort>) -> Self {
        Self { repository }
    }
    pub async fn execute(
        &self,
        request: &ResolveWitnessDto,
    ) -> Result<WitnessSubsetDto, WitnessErrorDto> {
        self.repository.resolve_current(request).await
    }
}
