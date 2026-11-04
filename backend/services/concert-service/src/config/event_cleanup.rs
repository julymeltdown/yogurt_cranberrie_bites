//! Root opts into scheduling explicitly and supplies all real owning bindings.
use concert_application::event_cleanup::MAX_BATCH;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventCleanupConfig {
    pub batch_limit: u16,
}
impl EventCleanupConfig {
    /// None means disabled. An enabled worker requires an explicit bounded limit;
    /// there is no guessed retention, role, database or successful owner default.
    pub fn from_values(enabled: Option<&str>, limit: Option<&str>) -> anyhow::Result<Option<Self>> {
        match enabled {
            None | Some("false") => {
                anyhow::ensure!(
                    limit.is_none(),
                    "disabled cleanup must not specify a batch limit"
                );
                Ok(None)
            }
            Some("true") => {
                let batch_limit: u16 = limit
                    .ok_or_else(|| anyhow::anyhow!("cleanup batch limit required"))?
                    .parse()
                    .map_err(|_| anyhow::anyhow!("invalid cleanup batch limit"))?;
                anyhow::ensure!(
                    (1..=MAX_BATCH).contains(&batch_limit),
                    "invalid cleanup batch limit"
                );
                Ok(Some(Self { batch_limit }))
            }
            _ => anyhow::bail!("invalid cleanup enablement"),
        }
    }
}
