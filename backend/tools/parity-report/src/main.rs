use parity_report::{summarize, ParityRecord};

fn main() -> anyhow::Result<()> {
    let arg = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("input json required"))?;
    let records: Vec<ParityRecord> = serde_json::from_str(&arg)?;
    let summary = summarize(&records);
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "total": summary.total,
            "fixtureChecks": summary.fixture_checks,
            "legacyComparisons": summary.legacy_comparisons,
            "unqualifiedEvidence": summary.unqualified_evidence,
            "critical": summary.critical,
            "behavioral": summary.behavioral,
            "databaseMismatches": summary.database_mismatches
        }))?
    );
    Ok(())
}
