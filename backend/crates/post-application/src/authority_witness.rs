//! Private application DTOs for immutable, short-lived owner query witnesses.
//! Constructors validate bindings and membership, not SQL provenance. Producers
//! must map admitted context and actual owned query results; no RPC accepts rows.
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

pub const MAX_WITNESS_MEMBERS: usize = 4096;
pub const MAX_RESOLVED_TARGETS: usize = 100;
// The admitted signed lifetime is at most60 seconds, with5 seconds clock skew.
const MAX_REMAINING_SECONDS: i64 = 65;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum WitnessErrorDto {
    #[error("invalid witness binding")]
    InvalidBinding,
    #[error("invalid witness membership")]
    InvalidMembership,
    #[error("immutable witness conflicts with existing stage")]
    StageConflict,
    #[error("witness missing or expired")]
    MissingOrExpired,
    #[error("witness target was not admitted")]
    NotAdmitted,
    #[error("witness storage unavailable")]
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WitnessRootDto {
    Feed,
    Search,
    Trending,
    Post,
    CreateComment,
    Like,
    Unlike,
    DeletePost,
    NotificationPosts,
    ReplayPostMutation,
}
impl WitnessRootDto {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Feed => "FEED",
            Self::Search => "SEARCH",
            Self::Trending => "TRENDING",
            Self::Post => "POST",
            Self::CreateComment => "CREATE_COMMENT",
            Self::Like => "LIKE",
            Self::Unlike => "UNLIKE",
            Self::DeletePost => "DELETE_POST",
            Self::NotificationPosts => "NOTIFICATION_POSTS",
            Self::ReplayPostMutation => "REPLAY_POST_MUTATION",
        }
    }
    pub fn parse(value: &str) -> Result<Self, WitnessErrorDto> {
        match value {
            "FEED" => Ok(Self::Feed),
            "SEARCH" => Ok(Self::Search),
            "TRENDING" => Ok(Self::Trending),
            "POST" => Ok(Self::Post),
            "CREATE_COMMENT" => Ok(Self::CreateComment),
            "LIKE" => Ok(Self::Like),
            "UNLIKE" => Ok(Self::Unlike),
            "DELETE_POST" => Ok(Self::DeletePost),
            "NOTIFICATION_POSTS" => Ok(Self::NotificationPosts),
            "REPLAY_POST_MUTATION" => Ok(Self::ReplayPostMutation),
            _ => Err(WitnessErrorDto::InvalidBinding),
        }
    }
    fn fixed(self) -> bool {
        !matches!(self, Self::Feed | Self::Search | Self::Trending)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WitnessPurposeDto {
    RankedCandidates,
    ChronologicalPage,
    SearchBatch,
    TrendingPage,
    SnapshotPage,
    FixedPosts,
}
impl WitnessPurposeDto {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RankedCandidates => "RANKED_CANDIDATES",
            Self::ChronologicalPage => "CHRONOLOGICAL_PAGE",
            Self::SearchBatch => "SEARCH_BATCH",
            Self::TrendingPage => "TRENDING_PAGE",
            Self::SnapshotPage => "SNAPSHOT_PAGE",
            Self::FixedPosts => "FIXED_POSTS",
        }
    }
    pub fn parse(value: &str) -> Result<Self, WitnessErrorDto> {
        match value {
            "RANKED_CANDIDATES" => Ok(Self::RankedCandidates),
            "CHRONOLOGICAL_PAGE" => Ok(Self::ChronologicalPage),
            "SEARCH_BATCH" => Ok(Self::SearchBatch),
            "TRENDING_PAGE" => Ok(Self::TrendingPage),
            "SNAPSHOT_PAGE" => Ok(Self::SnapshotPage),
            "FIXED_POSTS" => Ok(Self::FixedPosts),
            _ => Err(WitnessErrorDto::InvalidBinding),
        }
    }
    fn permits(self, root: WitnessRootDto) -> bool {
        match self {
            Self::RankedCandidates | Self::ChronologicalPage | Self::SnapshotPage => {
                root == WitnessRootDto::Feed
            }
            Self::SearchBatch => root == WitnessRootDto::Search,
            Self::TrendingPage => root == WitnessRootDto::Trending,
            Self::FixedPosts => root.fixed(),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct WitnessParentDto {
    digest: [u8; 32],
    request_digest: [u8; 32],
    actor_id: Uuid,
    root: WitnessRootDto,
    resources: Vec<Uuid>,
    expires_at: DateTime<Utc>,
}
impl WitnessParentDto {
    /// Only incoming adapters map this from sealed admitted authority. Its
    /// digest commits the original token; no token or content enters storage.
    pub fn from_admitted_request(
        digest: [u8; 32],
        request_digest: [u8; 32],
        actor_id: Uuid,
        root: WitnessRootDto,
        resources: Vec<Uuid>,
        expires_at: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Result<Self, WitnessErrorDto> {
        let expires_at = microseconds(expires_at)?;
        let now = microseconds(now)?;
        if digest == [0; 32]
            || request_digest == [0; 32]
            || actor_id.is_nil()
            || expires_at <= now
            || expires_at - now > chrono::Duration::seconds(MAX_REMAINING_SECONDS)
        {
            return Err(WitnessErrorDto::InvalidBinding);
        }
        let resources = unique_ids(resources, MAX_RESOLVED_TARGETS, root.fixed())?;
        if !root.fixed() && !resources.is_empty() {
            return Err(WitnessErrorDto::InvalidBinding);
        }
        if root.fixed() && root != WitnessRootDto::NotificationPosts && resources.len() != 1 {
            return Err(WitnessErrorDto::InvalidBinding);
        }
        Ok(Self {
            digest,
            request_digest,
            actor_id,
            root,
            resources,
            expires_at,
        })
    }
    pub fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
    pub fn request_digest(&self) -> &[u8; 32] {
        &self.request_digest
    }
    pub fn actor_id(&self) -> Uuid {
        self.actor_id
    }
    pub fn root(&self) -> WitnessRootDto {
        self.root
    }
    pub fn resources(&self) -> &[Uuid] {
        &self.resources
    }
    pub fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct WitnessQueryDto {
    stage_id: Uuid,
    purpose: WitnessPurposeDto,
    digest: [u8; 32],
    sequence: u32,
}
impl WitnessQueryDto {
    /// Fingerprint the actual viewer/query/cursor/limit/source/page inputs, not
    /// arbitrary post IDs. Each admitted batch receives a new immutable stage.
    pub fn new(
        stage_id: Uuid,
        purpose: WitnessPurposeDto,
        digest: [u8; 32],
        sequence: u32,
    ) -> Result<Self, WitnessErrorDto> {
        if stage_id.is_nil() || digest == [0; 32] {
            return Err(WitnessErrorDto::InvalidBinding);
        }
        Ok(Self {
            stage_id,
            purpose,
            digest,
            sequence,
        })
    }
    pub fn stage_id(&self) -> Uuid {
        self.stage_id
    }
    pub fn purpose(&self) -> WitnessPurposeDto {
        self.purpose
    }
    pub fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
    pub fn sequence(&self) -> u32 {
        self.sequence
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct WitnessMemberDto {
    post_id: Uuid,
    author_id: Uuid,
}
impl WitnessMemberDto {
    pub fn from_owned_query(post_id: Uuid, author_id: Uuid) -> Result<Self, WitnessErrorDto> {
        if post_id.is_nil() || author_id.is_nil() {
            return Err(WitnessErrorDto::InvalidMembership);
        }
        Ok(Self { post_id, author_id })
    }
    pub fn post_id(&self) -> Uuid {
        self.post_id
    }
    pub fn author_id(&self) -> Uuid {
        self.author_id
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct AuthorityWitnessDto {
    parent: WitnessParentDto,
    query: WitnessQueryDto,
    members: Vec<WitnessMemberDto>,
    missing: Vec<Uuid>,
    created_at: DateTime<Utc>,
}
impl AuthorityWitnessDto {
    /// Publish the actually admitted subset, after collector merge/truncation.
    /// Failure to publish must prevent its dependent authorization calls.
    pub fn capture(
        parent: WitnessParentDto,
        query: WitnessQueryDto,
        members: Vec<WitnessMemberDto>,
        missing: Vec<Uuid>,
        now: DateTime<Utc>,
    ) -> Result<Self, WitnessErrorDto> {
        let now = microseconds(now)?;
        if now >= parent.expires_at
            || parent.expires_at - now > chrono::Duration::seconds(MAX_REMAINING_SECONDS)
            || !query.purpose.permits(parent.root)
        {
            return Err(WitnessErrorDto::InvalidBinding);
        }
        if members.len() + missing.len() > MAX_WITNESS_MEMBERS {
            return Err(WitnessErrorDto::InvalidMembership);
        }
        let mut rows = BTreeMap::new();
        for member in members {
            if rows.insert(member.post_id, member).is_some() {
                return Err(WitnessErrorDto::InvalidMembership);
            }
        }
        let missing = unique_ids(missing, MAX_WITNESS_MEMBERS, false)?;
        if missing.iter().any(|id| rows.contains_key(id)) {
            return Err(WitnessErrorDto::InvalidMembership);
        }
        if query.purpose == WitnessPurposeDto::FixedPosts {
            let found: BTreeSet<_> = rows
                .keys()
                .copied()
                .chain(missing.iter().copied())
                .collect();
            if found != parent.resources.iter().copied().collect() {
                return Err(WitnessErrorDto::InvalidMembership);
            }
        } else if !missing.is_empty() {
            return Err(WitnessErrorDto::InvalidMembership);
        }
        Ok(Self {
            parent,
            query,
            members: rows.into_values().collect(),
            missing,
            created_at: now,
        })
    }
    pub fn parent(&self) -> &WitnessParentDto {
        &self.parent
    }
    pub fn query(&self) -> &WitnessQueryDto {
        &self.query
    }
    pub fn members(&self) -> &[WitnessMemberDto] {
        &self.members
    }
    pub fn missing(&self) -> &[Uuid] {
        &self.missing
    }
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    pub fn resolve(
        &self,
        request: &ResolveWitnessDto,
        now: DateTime<Utc>,
    ) -> Result<WitnessSubsetDto, WitnessErrorDto> {
        if now >= self.parent.expires_at
            || request.stage_id != self.query.stage_id
            || request.parent_digest != self.parent.digest
        {
            return Err(WitnessErrorDto::MissingOrExpired);
        }
        let mut members = Vec::new();
        let mut missing = Vec::new();
        for target in &request.targets {
            if request.by_author {
                if self.query.purpose == WitnessPurposeDto::FixedPosts {
                    return Err(WitnessErrorDto::NotAdmitted);
                }
                let row = self
                    .members
                    .iter()
                    .find(|row| row.author_id == *target)
                    .ok_or(WitnessErrorDto::NotAdmitted)?;
                members.push(*row);
            } else {
                if self.query.purpose != WitnessPurposeDto::FixedPosts {
                    return Err(WitnessErrorDto::NotAdmitted);
                }
                if let Some(row) = self.members.iter().find(|row| row.post_id == *target) {
                    members.push(*row);
                } else if self.missing.contains(target) {
                    missing.push(*target);
                } else {
                    return Err(WitnessErrorDto::NotAdmitted);
                }
            }
        }
        Ok(WitnessSubsetDto {
            stage_id: self.query.stage_id,
            parent_digest: self.parent.digest,
            expires_at: self.parent.expires_at,
            members,
            missing,
        })
    }
}
pub struct ResolveWitnessDto {
    stage_id: Uuid,
    parent_digest: [u8; 32],
    targets: Vec<Uuid>,
    by_author: bool,
}
impl ResolveWitnessDto {
    pub fn authors(
        stage_id: Uuid,
        parent_digest: [u8; 32],
        authors: Vec<Uuid>,
    ) -> Result<Self, WitnessErrorDto> {
        Self::new(stage_id, parent_digest, authors, true)
    }
    pub fn posts(
        stage_id: Uuid,
        parent_digest: [u8; 32],
        posts: Vec<Uuid>,
    ) -> Result<Self, WitnessErrorDto> {
        Self::new(stage_id, parent_digest, posts, false)
    }
    fn new(
        stage_id: Uuid,
        parent_digest: [u8; 32],
        targets: Vec<Uuid>,
        by_author: bool,
    ) -> Result<Self, WitnessErrorDto> {
        if stage_id.is_nil() || parent_digest == [0; 32] {
            return Err(WitnessErrorDto::InvalidBinding);
        }
        Ok(Self {
            stage_id,
            parent_digest,
            targets: unique_ids(targets, MAX_RESOLVED_TARGETS, true)?,
            by_author,
        })
    }
    pub fn stage_id(&self) -> Uuid {
        self.stage_id
    }
    pub fn parent_digest(&self) -> &[u8; 32] {
        &self.parent_digest
    }
}
pub struct WitnessSubsetDto {
    stage_id: Uuid,
    parent_digest: [u8; 32],
    expires_at: DateTime<Utc>,
    members: Vec<WitnessMemberDto>,
    missing: Vec<Uuid>,
}
impl WitnessSubsetDto {
    pub fn stage_id(&self) -> Uuid {
        self.stage_id
    }
    pub fn parent_digest(&self) -> &[u8; 32] {
        &self.parent_digest
    }
    pub fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at
    }
    pub fn members(&self) -> &[WitnessMemberDto] {
        &self.members
    }
    pub fn missing(&self) -> &[Uuid] {
        &self.missing
    }
}
fn unique_ids(
    ids: Vec<Uuid>,
    limit: usize,
    require_one: bool,
) -> Result<Vec<Uuid>, WitnessErrorDto> {
    if ids.len() > limit || (require_one && ids.is_empty()) {
        return Err(WitnessErrorDto::InvalidMembership);
    }
    let mut unique = BTreeSet::new();
    for id in ids {
        if id.is_nil() || !unique.insert(id) {
            return Err(WitnessErrorDto::InvalidMembership);
        }
    }
    Ok(unique.into_iter().collect())
}
fn microseconds(value: DateTime<Utc>) -> Result<DateTime<Utc>, WitnessErrorDto> {
    DateTime::from_timestamp_micros(value.timestamp_micros()).ok_or(WitnessErrorDto::InvalidBinding)
}

pub struct PurgeWitnessesDto {
    limit: u16,
}
impl PurgeWitnessesDto {
    pub fn new(limit: u16) -> Result<Self, WitnessErrorDto> {
        if !(1..=1000).contains(&limit) {
            return Err(WitnessErrorDto::InvalidBinding);
        }
        Ok(Self { limit })
    }
    pub fn limit(&self) -> u16 {
        self.limit
    }
}
pub struct WitnessCleanupOutcomeDto {
    removed: u64,
}
impl WitnessCleanupOutcomeDto {
    pub fn from_affected_rows(removed: u64) -> Result<Self, WitnessErrorDto> {
        if removed > 1000 {
            return Err(WitnessErrorDto::Unavailable);
        }
        Ok(Self { removed })
    }
    pub fn removed(&self) -> u64 {
        self.removed
    }
}

#[async_trait]
pub trait AuthorityWitnessRepositoryPort: Send + Sync {
    /// Same exact stage is idempotent; changed bindings/membership conflict.
    async fn publish(&self, witness: &AuthorityWitnessDto) -> Result<(), WitnessErrorDto>;
    /// Only the independently authenticated Gateway resource adapter calls this.
    async fn resolve(
        &self,
        request: &ResolveWitnessDto,
    ) -> Result<WitnessSubsetDto, WitnessErrorDto>;
    /// Resolve only recorded members and refresh authoritative lifecycle/author
    /// state. Missing or changed resources cannot be newly admitted by lookup.
    async fn resolve_current(
        &self,
        request: &ResolveWitnessDto,
    ) -> Result<WitnessSubsetDto, WitnessErrorDto>;
    /// Security-proof expiry, independent of later business retention decisions.
    async fn purge_expired(
        &self,
        request: &PurgeWitnessesDto,
    ) -> Result<WitnessCleanupOutcomeDto, WitnessErrorDto>;
}
