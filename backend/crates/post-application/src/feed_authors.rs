//! Owner-filtered following data for a chronological query, not authority for
//! another RPC. Post rows still require a separate query witness and policy read.
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Clone)]
pub struct FeedAuthorsDto {
    viewer_id: Uuid,
    author_ids: Vec<Uuid>,
}
impl FeedAuthorsDto {
    pub fn new(viewer_id: Uuid, mut author_ids: Vec<Uuid>) -> anyhow::Result<Self> {
        anyhow::ensure!(!viewer_id.is_nil(), "Feed viewer must be non-nil");
        anyhow::ensure!(
            author_ids.iter().all(|id| !id.is_nil()),
            "Feed author must be non-nil"
        );
        let mut seen = HashSet::new();
        author_ids.retain(|id| seen.insert(*id));
        Ok(Self {
            viewer_id,
            author_ids,
        })
    }
    pub fn viewer_id(&self) -> Uuid {
        self.viewer_id
    }
    pub fn author_ids(&self) -> &[Uuid] {
        &self.author_ids
    }
}
