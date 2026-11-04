use std::{
    future::Future,
    time::{Duration, Instant},
};
use tonic::Status;

pub const MAX_OPERATION: Duration = Duration::from_secs(5);
pub const MAX_COMPLETION: Duration = Duration::from_secs(2);
/// One original monotonic stop. No dependency or SQL statement restarts it.
#[derive(Clone, Copy)]
pub struct OperationBudget {
    stop: Instant,
}
impl OperationBudget {
    pub fn new(stop: Instant) -> Result<Self, Status> {
        let now = Instant::now();
        if stop <= now {
            return Err(Status::deadline_exceeded("Owner operation expired"));
        }
        if stop.saturating_duration_since(now) > MAX_OPERATION {
            return Err(Status::invalid_argument("Invalid owner operation budget"));
        }
        let stop = platform::call_deadline::current().map_or(stop, |parent| parent.min(stop));
        let result = Self { stop };
        result.remaining()?;
        Ok(result)
    }
    pub fn stop(self) -> Instant {
        self.stop
    }
    pub fn remaining(self) -> Result<Duration, Status> {
        let remaining = self.stop.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            Err(Status::deadline_exceeded("Owner operation expired"))
        } else {
            Ok(remaining)
        }
    }
    pub(super) async fn run<T>(
        self,
        work: impl Future<Output = Result<T, Status>>,
    ) -> Result<T, Status> {
        self.remaining()?;
        let result = platform::call_deadline::scope(self.stop, async {
            tokio::time::timeout_at(self.stop.into(), work)
                .await
                .map_err(|_| Status::deadline_exceeded("Owner operation expired"))?
        })
        .await;
        self.remaining()?;
        result
    }
    pub(super) async fn sql(self, connection: &mut sqlx::PgConnection) -> Result<(), Status> {
        let millis = self.remaining()?.as_millis();
        if millis == 0 {
            return Err(Status::deadline_exceeded("Owner operation expired"));
        }
        sqlx::query("select set_config('statement_timeout',$1,true),set_config('lock_timeout',$1,true),set_config('idle_in_transaction_session_timeout',$2,true)").bind(millis.to_string()).bind((millis+1000).to_string()).execute(connection).await.map_err(super::unavailable)?;
        self.remaining()?;
        Ok(())
    }
}
