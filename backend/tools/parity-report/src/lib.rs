use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiffCategory {
    None,
    FormattingOnly,
    Behavioral,
    Critical,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceMode {
    FixtureContract,
    LegacyShadow,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ParityRecord {
    pub scenario_id: String,
    pub evidence_mode: EvidenceMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff_category: Option<DiffCategory>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub database_state_match: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dataset_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legacy_revision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rust_revision: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParitySummary {
    pub total: usize,
    pub fixture_checks: usize,
    pub legacy_comparisons: usize,
    pub unqualified_evidence: usize,
    pub critical: usize,
    pub behavioral: usize,
    pub database_mismatches: usize,
}

pub fn summarize(records: &[ParityRecord]) -> ParitySummary {
    let mut summary = ParitySummary::default();
    for record in records {
        summary.total += 1;
        match record.evidence_mode {
            EvidenceMode::FixtureContract => summary.fixture_checks += 1,
            EvidenceMode::LegacyShadow if has_legacy_provenance(record) => {
                summary.legacy_comparisons += 1;
                match record.diff_category {
                    Some(DiffCategory::Critical) => summary.critical += 1,
                    Some(DiffCategory::Behavioral) => summary.behavioral += 1,
                    _ => {}
                }
                if record.database_state_match == Some(false) {
                    summary.database_mismatches += 1;
                }
            }
            EvidenceMode::LegacyShadow => summary.unqualified_evidence += 1,
        }
    }
    summary
}

fn has_legacy_provenance(record: &ParityRecord) -> bool {
    record.diff_category.is_some()
        && record.database_state_match.is_some()
        && record
            .run_id
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        && record
            .dataset_id
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        && valid_revision(record.legacy_revision.as_deref())
        && valid_revision(record.rust_revision.as_deref())
}

fn valid_revision(revision: Option<&str>) -> bool {
    revision.is_some_and(|revision| {
        revision.len() == 40 && revision.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

#[cfg(test)]
mod tests {
    use super::{summarize, DiffCategory, EvidenceMode, ParityRecord};

    fn legacy_record(diff_category: DiffCategory, database_state_match: bool) -> ParityRecord {
        ParityRecord {
            scenario_id: "test/legacy-comparison".into(),
            evidence_mode: EvidenceMode::LegacyShadow,
            diff_category: Some(diff_category),
            database_state_match: Some(database_state_match),
            run_id: Some("run-2026-09-28-01".into()),
            dataset_id: Some("dataset-v1".into()),
            legacy_revision: Some("0123456789abcdef0123456789abcdef01234567".into()),
            rust_revision: Some("89abcdef0123456789abcdef0123456789abcdef".into()),
        }
    }

    #[test]
    fn fixture_contract_checks_are_not_counted_as_legacy_parity() {
        let fixture = ParityRecord {
            scenario_id: "gateway/fixture-contract".into(),
            evidence_mode: EvidenceMode::FixtureContract,
            diff_category: None,
            database_state_match: None,
            run_id: None,
            dataset_id: None,
            legacy_revision: None,
            rust_revision: None,
        };

        let summary = summarize(&[fixture]);

        assert_eq!(summary.total, 1);
        assert_eq!(summary.fixture_checks, 1);
        assert_eq!(summary.legacy_comparisons, 0);
        assert_eq!(summary.unqualified_evidence, 0);
        assert_eq!(summary.database_mismatches, 0);
    }

    #[test]
    fn legacy_comparison_without_full_provenance_is_unqualified() {
        let mut record = legacy_record(DiffCategory::None, true);
        record.legacy_revision = Some("not-a-git-sha".into());

        let summary = summarize(&[record]);

        assert_eq!(summary.legacy_comparisons, 0);
        assert_eq!(summary.unqualified_evidence, 1);
    }

    #[test]
    fn qualified_legacy_comparisons_contribute_diffs_and_database_mismatches() {
        let records = [
            legacy_record(DiffCategory::Critical, false),
            legacy_record(DiffCategory::Behavioral, true),
        ];

        let summary = summarize(&records);

        assert_eq!(summary.legacy_comparisons, 2);
        assert_eq!(summary.unqualified_evidence, 0);
        assert_eq!(summary.critical, 1);
        assert_eq!(summary.behavioral, 1);
        assert_eq!(summary.database_mismatches, 1);
    }
}
