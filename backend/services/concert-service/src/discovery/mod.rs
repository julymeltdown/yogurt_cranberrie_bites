//! WP-03: discovery — finding people through shared catalog picks.
//!
//! This module is the concert-service half of the favorites/discovery feature:
//!
//! * [`repository`] holds the discovery query and its result types (T-802);
//! * [`http`] holds the closed parser for `GET /api/discover/people` (T-803);
//! * [`projection`] holds the writer that keeps the projection current.
//!
//! The projection writer is what makes the first two useful in production:
//! before it existed the `discovery_*` tables were read and tested but never
//! populated, so discovery returned an empty list. The mechanism and its
//! tradeoffs are recorded in `docs/mcr/integration/discovery-projection.md`.
//!
//! The profile-service half is the catalog (`profile_service::catalog`, T-801)
//! and the favorites table (`profile_favorites`, WP-02 T-104).
//!
//! The two lists the product presents — people and mutuals-intro posts — are
//! never merged. This module serves only the people list; the intro-post list is
//! a different operation with its own audience filter.
pub mod block_consumer;
pub mod blocks;
pub mod http;
pub mod projection;
pub mod repository;
pub mod worker;

pub use block_consumer::{
    BlockConsumer, BlockJournal, BlockPollReport, FriendJournal, JournalPage, PairAuthority,
    PairFailure, PairState, MAX_SIGNALS_PER_POLL,
};
pub use blocks::{apply_blocks, BlockState};
pub use http::{parse, RouteError, PATH};
pub use projection::{
    DiscoverySource, GrpcDiscoverySource, PassReport, ProjectionError, ProjectionWriter,
    SourceFailure, SourceFavorite, SourcePage, SourceProfile, MAX_PAGES_PER_PASS, PAGE_SIZE,
};
pub use worker::{ProjectionWorker, RoundFailure, RoundReport, DEFAULT_INTERVAL, ROUND_DEADLINE};
pub use repository::{
    discover, validate, Category, DiscoveryError, FavoriteItem, PeopleCard, PeoplePage,
    PeopleQuery, DEFAULT_LIMIT, MAX_LIMIT,
};
