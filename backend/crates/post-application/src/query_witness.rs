//! Internal owned-query observations. They are not public request fields or
//! grants. The incoming adapter must bind them to sealed admitted authority.
use crate::authority_witness::{
    AuthorityWitnessDto, WitnessErrorDto, WitnessMemberDto, WitnessPurposeDto,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize)]
pub enum OwnedQueryInputDto {
    Chronological {
        cursor: Option<String>,
        limit: i32,
        author_ids: Vec<Uuid>,
    },
    CachedChronological {
        cursor: Option<String>,
        limit: i32,
        page_post_ids: Vec<Uuid>,
    },
    Search {
        term: String,
        cursor: Option<String>,
        batch_limit: i32,
        sequence: u32,
    },
    Trending {
        cursor: Option<String>,
        limit: i32,
    },
    Ranked {
        per_source_limit: i32,
        following_ids: Vec<Uuid>,
        failed_sources: Vec<String>,
        collected_at: DateTime<Utc>,
    },
    Snapshot {
        snapshot_id: Uuid,
        offset: u32,
        limit: i32,
        page_post_ids: Vec<Uuid>,
    },
    Fixed {
        post_ids: Vec<Uuid>,
    },
}
impl OwnedQueryInputDto {
    pub fn purpose(&self) -> WitnessPurposeDto {
        match self {
            Self::Chronological { .. } | Self::CachedChronological { .. } => {
                WitnessPurposeDto::ChronologicalPage
            }
            Self::Search { .. } => WitnessPurposeDto::SearchBatch,
            Self::Trending { .. } => WitnessPurposeDto::TrendingPage,
            Self::Ranked { .. } => WitnessPurposeDto::RankedCandidates,
            Self::Snapshot { .. } => WitnessPurposeDto::SnapshotPage,
            Self::Fixed { .. } => WitnessPurposeDto::FixedPosts,
        }
    }
    pub fn sequence(&self) -> u32 {
        match self {
            Self::Search { sequence, .. } => *sequence,
            _ => 0,
        }
    }
}

// Never Debug: query text stays transient and is stored only as a fingerprint.
#[derive(Serialize)]
pub struct OwnedQueryObservationDto {
    viewer_id: Uuid,
    input: OwnedQueryInputDto,
    #[serde(skip)]
    members: Vec<WitnessMemberDto>,
    #[serde(skip)]
    missing: Vec<Uuid>,
}
impl OwnedQueryObservationDto {
    pub fn new(
        viewer_id: Uuid,
        input: OwnedQueryInputDto,
        members: Vec<WitnessMemberDto>,
        missing: Vec<Uuid>,
    ) -> Result<Self, WitnessErrorDto> {
        if viewer_id.is_nil() {
            return Err(WitnessErrorDto::InvalidBinding);
        }
        let admitted_ids = match &input {
            OwnedQueryInputDto::CachedChronological { page_post_ids, .. }
            | OwnedQueryInputDto::Snapshot { page_post_ids, .. } => Some(page_post_ids),
            _ => None,
        };
        if admitted_ids.is_some_and(|ids| members.iter().any(|row| !ids.contains(&row.post_id()))) {
            return Err(WitnessErrorDto::InvalidMembership);
        }
        Ok(Self {
            viewer_id,
            input,
            members,
            missing,
        })
    }
    pub fn viewer_id(&self) -> Uuid {
        self.viewer_id
    }
    pub fn input(&self) -> &OwnedQueryInputDto {
        &self.input
    }
    pub fn members(&self) -> &[WitnessMemberDto] {
        &self.members
    }
    pub fn missing(&self) -> &[Uuid] {
        &self.missing
    }
}

/// An explicit locator returned only after publication. It cannot replace the
/// parent's proof or the independently resolved owner evidence at exchange.
#[derive(Clone)]
pub struct PublishedWitnessStageDto {
    stage_id: Uuid,
    parent_digest: [u8; 32],
    actor_id: Uuid,
    expires_at: DateTime<Utc>,
}
impl PublishedWitnessStageDto {
    pub fn after_publication(
        witness: &AuthorityWitnessDto,
        now: DateTime<Utc>,
    ) -> Result<Self, WitnessErrorDto> {
        if now >= witness.parent().expires_at() {
            return Err(WitnessErrorDto::MissingOrExpired);
        }
        Ok(Self {
            stage_id: witness.query().stage_id(),
            parent_digest: *witness.parent().digest(),
            actor_id: witness.parent().actor_id(),
            expires_at: witness.parent().expires_at(),
        })
    }
    pub fn stage_id(&self) -> Uuid {
        self.stage_id
    }
    pub fn parent_digest(&self) -> &[u8; 32] {
        &self.parent_digest
    }
    pub fn actor_id(&self) -> Uuid {
        self.actor_id
    }
    pub fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at
    }
}

#[async_trait]
pub trait OwnedQueryWitnessProducerPort: Send + Sync {
    /// None explicitly means no forwarding grant. Interactive adapters must
    /// reject missing admission or publication, never substitute system access.
    async fn observe(
        &self,
        observation: OwnedQueryObservationDto,
    ) -> Result<Option<PublishedWitnessStageDto>, WitnessErrorDto>;
}
