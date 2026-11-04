// Authoring core: e34b90dd8616bc2c51a66d1acaf3521d4dd142c7; source-only draft, compile/runtime NOT_RUN.
//! Actual SDK acquisition uncertainty. No successful projection is produced.
use super::profile_read_fault::{Mode, State};
use super::*;
pub(super) struct UncertainProducer {
    pub reader: Arc<concert_service::adapters::out::public_profile::PublicProfileReader>,
    pub subject: Uuid,
    pub fault: Arc<State>,
    pub captured: Mutex<Option<Arc<OwnerGuardScope>>>,
}
#[async_trait::async_trait]
impl ScopedProductProducer for UncertainProducer {
    async fn dispatch(
        &self,
        _: &VerifiedProduct,
        scope: Arc<OwnerGuardScope>,
        _: proto::ProductRequest,
    ) -> Result<ProductRepresentation, Status> {
        let subject = scope.public_profile_subject(self.subject).await?;
        // Pin the ACTUAL SDK future. It persists/retains its Profile intent before
        // the transport wrapper can signal ReadCurrent arrival.
        let mut read = Box::pin(self.reader.read(&subject));
        let error = if self.fault.mode == Mode::BeforeDispatch {
            tokio::select! {
                biased;
                _ = self.fault.entered.notified() => {
                    // This explicit drop is the CR05 fault: cancellation of the
                    // real Root read future, not an Unavailable response surrogate.
                    drop(read);
                    self.fault.read_future_dropped.fetch_add(1, Ordering::SeqCst);
                    Status::cancelled("CR05 actual SDK read future cancelled")
                }
                _ = &mut read => {
                    drop(read);
                    Status::internal("CR05 read completed without cancellation handshake")
                }
            }
        } else {
            match read.await {
                Err(_) => Status::unavailable("CR02 uncertain actual Profile acquisition"),
                Ok(_) => Status::internal("required Profile read fault missing"),
            }
        };
        *self
            .captured
            .lock()
            .map_err(|_| Status::internal("scope capture"))? = Some(scope);
        // Cancelled and Unavailable both take the real closed 503 path. Assertions
        // on the transport counters distinguish the two physical fault sequences.
        Err(error)
    }
}
