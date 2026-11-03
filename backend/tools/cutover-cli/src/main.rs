use cutover_cli::{evaluate, CutoverInput, CutoverStatus};

fn main() -> anyhow::Result<()> {
    let arg = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("input json required"))?;
    let input: CutoverInput = serde_json::from_str(&arg)?;
    match evaluate(&input) {
        CutoverStatus::Approved => println!("APPROVED"),
        CutoverStatus::Blocked(reason) => println!("BLOCKED: {reason}"),
    }
    Ok(())
}
