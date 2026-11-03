use recommendation_policy::recommendation::RecommendationContractError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuthorDeliveryMode {
    Normal,
    HighFanout,
}

impl AuthorDeliveryMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "NORMAL",
            Self::HighFanout => "HIGH_FANOUT",
        }
    }

    pub fn parse(value: &str) -> Result<Self, RecommendationContractError> {
        match value.trim().to_ascii_uppercase().as_str() {
            "NORMAL" => Ok(Self::Normal),
            "HIGH_FANOUT" => Ok(Self::HighFanout),
            _ => Err(RecommendationContractError::new(
                "Unsupported feed author delivery mode",
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FeedEngagementAction {
    Like,
    Unlike,
    Comment,
    DeleteComment,
    Impression,
    Click,
    Dwell,
    Hide,
    NotInterested,
    Mute,
    Block,
    Report,
    FastScroll,
}

impl FeedEngagementAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Like => "LIKE",
            Self::Unlike => "UNLIKE",
            Self::Comment => "COMMENT",
            Self::DeleteComment => "DELETE_COMMENT",
            Self::Impression => "IMPRESSION",
            Self::Click => "CLICK",
            Self::Dwell => "DWELL",
            Self::Hide => "HIDE",
            Self::NotInterested => "NOT_INTERESTED",
            Self::Mute => "MUTE",
            Self::Block => "BLOCK",
            Self::Report => "REPORT",
            Self::FastScroll => "FAST_SCROLL",
        }
    }

    pub fn affinity_delta(self) -> f64 {
        match self {
            Self::Like => 0.08,
            Self::Unlike => -0.08,
            Self::Comment => 0.16,
            Self::DeleteComment => -0.16,
            Self::Impression => 0.0,
            Self::Click => 0.03,
            Self::Dwell => 0.05,
            Self::Hide => -0.30,
            Self::NotInterested => -0.40,
            Self::Mute => -0.75,
            Self::Block | Self::Report => -1.0,
            Self::FastScroll => -0.10,
        }
    }

    pub fn velocity_delta(self) -> (i64, i64) {
        match self {
            Self::Like => (1, 0),
            Self::Unlike => (-1, 0),
            Self::Comment => (0, 1),
            Self::DeleteComment => (0, -1),
            _ => (0, 0),
        }
    }
}
