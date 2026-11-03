//! Existing direct application fixtures have no admitted transport context.
//! This fixture grants no forwarding authority and stores no witness database.
use async_trait::async_trait;
use post_service::application::authority_witness::WitnessErrorDto;
use post_service::application::query_witness::{
    OwnedQueryObservationDto, OwnedQueryWitnessProducerPort, PublishedWitnessStageDto,
};
pub struct UnforwardedQueryFixture;
#[async_trait]
impl OwnedQueryWitnessProducerPort for UnforwardedQueryFixture {
    async fn observe(
        &self,
        _observation: OwnedQueryObservationDto,
    ) -> Result<Option<PublishedWitnessStageDto>, WitnessErrorDto> {
        Ok(None)
    }
}
