#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RelationshipPolicy {
    pub blocked_by_viewer: bool,
    pub blocked_viewer: bool,
    pub muted_by_viewer: bool,
    pub following: bool,
}

impl RelationshipPolicy {
    pub fn is_blocked(self) -> bool {
        self.blocked_by_viewer || self.blocked_viewer
    }
}
