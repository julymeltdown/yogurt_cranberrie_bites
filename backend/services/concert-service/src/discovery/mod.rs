//! WP-03: discovery — finding people through shared catalog picks.
//!
//! This module is the concert-service half of the favorites/discovery feature:
//!
//! * [`repository`] holds the discovery query and its result types (T-802);
//! * [`http`] holds the closed parser for `GET /api/discover/people` (T-803).
//!
//! The profile-service half is the catalog (`profile_service::catalog`, T-801)
//! and the favorites table (`profile_favorites`, WP-02 T-104).
//!
//! The two lists the product presents — people and mutuals-intro posts — are
//! never merged. This module serves only the people list; the intro-post list is
//! a different operation with its own audience filter.
pub mod http;
pub mod repository;

pub use http::{parse, RouteError, PATH};
pub use repository::{
    discover, validate, Category, DiscoveryError, FavoriteItem, PeopleCard, PeoplePage,
    PeopleQuery, DEFAULT_LIMIT, MAX_LIMIT,
};
