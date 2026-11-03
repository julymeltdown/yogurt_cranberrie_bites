//! BE41 closure state. Newly authored; seven steps match C04 admin.ClosureJob.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Step {
    ReadOnly,
    FreezeMural,
    Snapshot,
    ExpireGrants,
    ExpireBeacons,
    Export,
    Cleanup,
}
impl Step {
    pub const ALL: [Self; 7] = [
        Self::ReadOnly,
        Self::FreezeMural,
        Self::Snapshot,
        Self::ExpireGrants,
        Self::ExpireBeacons,
        Self::Export,
        Self::Cleanup,
    ];
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|v| *v == self).unwrap()
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::ReadOnly => "READ_ONLY",
            Self::FreezeMural => "FREEZE_MURAL",
            Self::Snapshot => "SNAPSHOT",
            Self::ExpireGrants => "EXPIRE_GRANTS",
            Self::ExpireBeacons => "EXPIRE_BEACONS",
            Self::Export => "EXPORT",
            Self::Cleanup => "CLEANUP",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StepState {
    Pending,
    Succeeded,
    Failed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JobState {
    Pending,
    Running,
    PartialFailure,
    Completed,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Progress {
    pub name: Step,
    pub state: StepState,
}
/// Only this allowlisted projection may reach the operations HTTP adapter.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Job {
    pub job_id: Uuid,
    pub event_id: Uuid,
    pub state: JobState,
    pub revision: i64,
    pub steps: Vec<Progress>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum LifecycleError {
    #[error("Invalid lifecycle state")]
    Invalid,
    #[error("Revision changed")]
    Conflict,
    #[error("Current authority denied")]
    Denied,
    #[error("Required owner unavailable")]
    Unavailable,
    #[error("Configured retention has not elapsed")]
    NotDue,
}
impl Job {
    pub fn new(job_id: Uuid, event_id: Uuid) -> Result<Self, LifecycleError> {
        if job_id.is_nil() || event_id.is_nil() {
            return Err(LifecycleError::Invalid);
        }
        Ok(Self {
            job_id,
            event_id,
            state: JobState::Pending,
            revision: 1,
            steps: Step::ALL
                .into_iter()
                .map(|name| Progress {
                    name,
                    state: if name == Step::ReadOnly {
                        StepState::Succeeded
                    } else {
                        StepState::Pending
                    },
                })
                .collect(),
        })
    }
    pub fn validate(&self) -> Result<(), LifecycleError> {
        if self.job_id.is_nil()
            || self.event_id.is_nil()
            || !(1..=9_007_199_254_740_991).contains(&self.revision)
            || self.steps.len() != 7
            || self.steps.iter().zip(Step::ALL).any(|(p, s)| p.name != s)
            || self.steps[0].state != StepState::Succeeded
        {
            return Err(LifecycleError::Invalid);
        }
        let mut unfinished = false;
        for p in &self.steps {
            if unfinished && matches!(p.state, StepState::Succeeded | StepState::Failed) {
                return Err(LifecycleError::Invalid);
            }
            unfinished |= p.state != StepState::Succeeded;
        }
        let failed = self.steps.iter().any(|p| p.state == StepState::Failed);
        if (self.state == JobState::Completed) != !unfinished
            || (self.state == JobState::PartialFailure) != failed
        {
            return Err(LifecycleError::Invalid);
        }
        Ok(())
    }
    pub fn next(&self) -> Option<Step> {
        self.steps
            .iter()
            .find(|p| p.state != StepState::Succeeded)
            .map(|p| p.name)
    }
    pub fn record(&mut self, step: Step, applied: bool) -> Result<(), LifecycleError> {
        self.validate()?;
        if self.next() != Some(step) {
            return Err(LifecycleError::Conflict);
        }
        self.revision = self
            .revision
            .checked_add(1)
            .filter(|r| *r <= 9_007_199_254_740_991)
            .ok_or(LifecycleError::Invalid)?;
        self.steps[step.index()].state = if applied {
            StepState::Succeeded
        } else {
            StepState::Failed
        };
        self.state = if !applied {
            JobState::PartialFailure
        } else if self.next().is_none() {
            JobState::Completed
        } else {
            JobState::Running
        };
        self.validate()
    }
    pub fn resume(&mut self, expected: i64) -> Result<(), LifecycleError> {
        self.validate()?;
        if self.revision != expected {
            return Err(LifecycleError::Conflict);
        }
        if self.state != JobState::PartialFailure {
            return Err(LifecycleError::Invalid);
        }
        self.revision = self
            .revision
            .checked_add(1)
            .filter(|r| *r <= 9_007_199_254_740_991)
            .ok_or(LifecycleError::Invalid)?;
        for p in &mut self.steps {
            if p.state == StepState::Failed {
                p.state = StepState::Pending
            }
        }
        self.state = JobState::Pending;
        self.validate()
    }
}
/// Reviewed operator configuration is pinned at close. No guessed production default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Retention {
    pub policy_revision: i64,
    pub closed_at: DateTime<Utc>,
    pub cleanup_after: DateTime<Utc>,
}
impl Retention {
    pub fn validate(self) -> Result<Self, LifecycleError> {
        if !(1..=9_007_199_254_740_991).contains(&self.policy_revision)
            || self.cleanup_after < self.closed_at
        {
            Err(LifecycleError::Invalid)
        } else {
            Ok(self)
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resumes_only_failed_boundary_and_never_repeats_success() {
        let mut j = Job::new(Uuid::new_v4(), Uuid::new_v4()).unwrap();
        j.record(Step::FreezeMural, true).unwrap();
        j.record(Step::Snapshot, false).unwrap();
        let rev = j.revision;
        assert_eq!(j.resume(rev - 1), Err(LifecycleError::Conflict));
        j.resume(rev).unwrap();
        assert_eq!(j.next(), Some(Step::Snapshot));
        assert_eq!(
            j.record(Step::FreezeMural, true),
            Err(LifecycleError::Conflict)
        );
        while let Some(s) = j.next() {
            j.record(s, true).unwrap();
        }
        assert_eq!(j.state, JobState::Completed);
        j.validate().unwrap();
    }
    #[test]
    fn rejects_false_success_and_duplicate_steps() {
        let mut j = Job::new(Uuid::new_v4(), Uuid::new_v4()).unwrap();
        j.state = JobState::Completed;
        assert!(j.validate().is_err());
        j.state = JobState::Pending;
        j.steps[3].name = Step::Snapshot;
        assert!(j.validate().is_err());
    }
}
