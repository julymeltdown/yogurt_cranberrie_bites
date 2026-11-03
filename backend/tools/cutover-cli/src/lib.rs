use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct CutoverInput {
    pub service_name: String,
    pub open_critical_diffs: usize,
    pub open_behavioral_diffs: usize,
    pub database_state_mismatches: usize,
    pub postgres_validation_complete: bool,
    pub rollback_runbook_ready: bool,
    pub contract_coverage_complete: bool,
    pub fixture_suite_complete: bool,
    pub legacy_comparison_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CutoverStatus {
    Approved,
    Blocked(String),
}

pub fn evaluate(input: &CutoverInput) -> CutoverStatus {
    if input.open_critical_diffs > 0 {
        return CutoverStatus::Blocked("critical parity diffs remain".into());
    }
    if input.open_behavioral_diffs > 0 {
        return CutoverStatus::Blocked("behavioral parity diffs remain".into());
    }
    if input.database_state_mismatches > 0 {
        return CutoverStatus::Blocked("database state mismatches remain".into());
    }
    if !input.postgres_validation_complete {
        return CutoverStatus::Blocked("postgres validation incomplete".into());
    }
    if !input.rollback_runbook_ready {
        return CutoverStatus::Blocked("rollback runbook missing".into());
    }
    if !input.contract_coverage_complete || !input.fixture_suite_complete {
        return CutoverStatus::Blocked("contract or fixture-suite coverage incomplete".into());
    }
    if !input.legacy_comparison_complete {
        return CutoverStatus::Blocked("legacy comparison evidence incomplete".into());
    }
    CutoverStatus::Approved
}

#[cfg(test)]
mod tests {
    use super::{evaluate, CutoverInput, CutoverStatus};

    fn fully_ready() -> CutoverInput {
        CutoverInput {
            service_name: "auth-service".into(),
            open_critical_diffs: 0,
            open_behavioral_diffs: 0,
            database_state_mismatches: 0,
            postgres_validation_complete: true,
            rollback_runbook_ready: true,
            contract_coverage_complete: true,
            fixture_suite_complete: true,
            legacy_comparison_complete: true,
        }
    }

    #[test]
    fn fixture_tests_without_legacy_comparison_block_cutover() {
        let mut input = fully_ready();
        input.legacy_comparison_complete = false;

        assert_eq!(
            evaluate(&input),
            CutoverStatus::Blocked("legacy comparison evidence incomplete".into())
        );
    }

    #[test]
    fn behavioral_diffs_block_cutover() {
        let mut input = fully_ready();
        input.open_behavioral_diffs = 1;

        assert_eq!(
            evaluate(&input),
            CutoverStatus::Blocked("behavioral parity diffs remain".into())
        );
    }

    #[test]
    fn database_state_mismatches_block_cutover() {
        let mut input = fully_ready();
        input.database_state_mismatches = 1;

        assert_eq!(
            evaluate(&input),
            CutoverStatus::Blocked("database state mismatches remain".into())
        );
    }

    #[test]
    fn fully_qualified_legacy_comparison_can_clear_the_evidence_gate() {
        assert_eq!(evaluate(&fully_ready()), CutoverStatus::Approved);
    }
}
