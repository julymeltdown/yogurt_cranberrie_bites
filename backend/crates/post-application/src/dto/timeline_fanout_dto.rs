use chrono::{DateTime, Utc};
use post_domain::recommendation::TimelineFanoutJob;
use uuid::Uuid;

pub struct TimelineFanoutClaimRequestDto {
    pub lease_timeout: std::time::Duration,
    pub limit: i32,
    pub max_page_attempts: i32,
    pub retry_base: std::time::Duration,
    pub excluded_job_ids: Vec<Uuid>,
}

#[derive(Clone)]
pub struct TimelineFanoutClaimDto {
    pub job: TimelineFanoutJob,
    pub lease_token: Uuid,
    pub lease_expires_at: DateTime<Utc>,
    pub page_attempt_count: i32,
    pub max_page_attempts: i32,
}

pub struct TimelineFanoutPageDto {
    pub follower_ids: Vec<Uuid>,
    pub next_cursor: Option<String>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimelineFanoutOutcomeDto {
    Applied,
    Lost,
    Suspended,
}

pub struct TimelineFanoutFailureDto {
    pub message: String,
}

/// Friend paginates descending by (created_at, follower_id). Only a strictly
/// smaller boundary proves page progress; opaque string inequality does not.
pub fn validate_fanout_page_progress(
    previous: Option<&str>,
    page: &TimelineFanoutPageDto,
) -> anyhow::Result<()> {
    anyhow::ensure!(page.follower_ids.len() <= 100, "oversized follower page");
    let previous = previous.map(parse_fanout_cursor).transpose()?;
    if let Some(next) = page.next_cursor.as_deref() {
        let next = parse_fanout_cursor(next)?;
        anyhow::ensure!(
            page.follower_ids.last() == Some(&next.1),
            "follower cursor must identify the last page recipient"
        );
        anyhow::ensure!(
            previous.is_none_or(|previous| next < previous),
            "non-advancing follower cursor"
        );
    }
    Ok(())
}

fn parse_fanout_cursor(value: &str) -> anyhow::Result<(DateTime<Utc>, Uuid)> {
    anyhow::ensure!(value.len() <= 256, "oversized follower cursor");
    let (time, id) = value
        .trim()
        .split_once('|')
        .ok_or_else(|| anyhow::anyhow!("invalid follower cursor"))?;
    Ok((
        DateTime::parse_from_rfc3339(time)?.with_timezone(&Utc),
        Uuid::parse_str(id)?,
    ))
}
