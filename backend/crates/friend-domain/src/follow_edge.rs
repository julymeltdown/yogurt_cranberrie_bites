use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use std::fmt::{Debug, Formatter};
use uuid::Uuid;

const MAX_FOLLOW_CURSOR_LENGTH: usize = 128;

#[derive(Clone, PartialEq, Eq)]
pub struct FollowEdge {
    pub follower_id: Uuid,
    pub followee_id: Uuid,
    pub created_at: DateTime<Utc>,
}

impl Debug for FollowEdge {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FollowEdge")
            .field("follower_id", &"[REDACTED]")
            .field("followee_id", &"[REDACTED]")
            .field("created_at", &"[REDACTED]")
            .finish()
    }
}

impl FollowEdge {
    pub fn new(follower_id: Uuid, followee_id: Uuid, created_at: DateTime<Utc>) -> Result<Self> {
        if follower_id == followee_id {
            bail!("Cannot follow yourself");
        }

        Ok(Self {
            follower_id,
            followee_id,
            created_at,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FollowCursor {
    pub created_at: DateTime<Utc>,
    pub tie_breaker_id: Uuid,
}

impl FollowCursor {
    pub fn new(created_at: DateTime<Utc>, tie_breaker_id: Uuid) -> Self {
        Self {
            created_at,
            tie_breaker_id,
        }
    }

    pub fn parse(value: &str) -> Result<Option<Self>> {
        // Check the raw input before trimming so an oversized whitespace-only
        // cursor cannot bypass the bound by normalizing to an empty value.
        if value.len() > MAX_FOLLOW_CURSOR_LENGTH {
            bail!("Invalid cursor");
        }
        let value = value.trim();
        if value.is_empty() {
            return Ok(None);
        }
        let (created_at, tie_breaker_id) = value
            .split_once('|')
            .ok_or_else(|| anyhow::anyhow!("Invalid cursor"))?;
        let created_at = DateTime::parse_from_rfc3339(created_at)
            .map_err(|_| anyhow::anyhow!("Invalid cursor"))?
            .with_timezone(&Utc);
        let tie_breaker_id =
            Uuid::parse_str(tie_breaker_id).map_err(|_| anyhow::anyhow!("Invalid cursor"))?;
        Ok(Some(Self::new(created_at, tie_breaker_id)))
    }

    pub fn encode(self) -> String {
        format!("{}|{}", self.created_at.to_rfc3339(), self.tie_breaker_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follow_edge_debug_redacts_relationship_identity() {
        let follower_id = Uuid::new_v4();
        let followee_id = Uuid::new_v4();
        let edge = FollowEdge::new(follower_id, followee_id, Utc::now()).unwrap();

        let rendered = format!("{edge:?}");
        assert!(!rendered.contains(&follower_id.to_string()), "{rendered}");
        assert!(!rendered.contains(&followee_id.to_string()), "{rendered}");
        assert!(rendered.contains("[REDACTED]"), "{rendered}");
    }
}
