//! BE41 bounded cleanup tick. Failed jobs require an explicit, authorized RESUME.
use crate::event_lifecycle::{FencedLifecycleAuthority, LifecycleOwners, PgEventLifecycle};
use concert_application::commands::CommandError;
#[derive(Default, Debug)]
pub struct CleanupTick {
    pub considered: usize,
    pub advanced: usize,
    pub unavailable: usize,
}
pub async fn cleanup_tick<A: FencedLifecycleAuthority, O: LifecycleOwners>(
    service: &PgEventLifecycle<A, O>,
    limit: i64,
) -> Result<CleanupTick, CommandError> {
    let jobs = service.due_jobs(limit).await?;
    let mut result = CleanupTick {
        considered: jobs.len(),
        ..Default::default()
    };
    for (actor, job) in jobs {
        match service.run_next(actor, job).await {
            Ok(_) => result.advanced += 1,
            Err(_) => result.unavailable += 1,
        }
    }
    Ok(result)
}
