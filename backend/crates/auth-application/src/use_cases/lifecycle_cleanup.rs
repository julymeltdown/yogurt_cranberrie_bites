use crate::auth_runtime::AuthRuntimePort;
use crate::dto::auth_dto::AuthLifecycleCleanupResult;
use crate::errors::AuthResult as Result;
use crate::ports::AuthLifecycleCleanupPort;
use std::sync::Arc;

#[derive(Clone)]
pub struct AuthLifecycleCleanupUseCase<R> {
    repository: R,
    runtime: Arc<dyn AuthRuntimePort>,
}

impl<R> AuthLifecycleCleanupUseCase<R>
where
    R: AuthLifecycleCleanupPort,
{
    pub fn new(repository: R, runtime: Arc<dyn AuthRuntimePort>) -> Self {
        Self {
            repository,
            runtime,
        }
    }

    pub async fn execute(&self) -> Result<AuthLifecycleCleanupResult> {
        self.repository.cleanup_expired(self.runtime.now()).await
    }
}
