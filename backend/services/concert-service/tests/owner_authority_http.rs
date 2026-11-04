//! Actual public Gateway router, current Auth/Profile RPCs and real owner guards.
//! Input tokens come only from the owned controller's real login, over stdin.
#[path = "owner_authority_support/capture.rs"]
mod capture;
#[path = "owner_authority_support/environment.rs"]
mod environment;
#[path = "owner_authority_support/fixture.rs"]
mod fixture;
#[path = "owner_authority_support/http.rs"]
mod http;
#[path = "owner_authority_support/owners.rs"]
mod owners;
