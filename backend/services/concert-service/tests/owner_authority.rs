//! Actual canonical consumer/owner qualification. Only a private frozen controller
//! may run native cases; ignored cases are never acceptance in a library build.
#[path = "owner_authority_support/capture.rs"]
mod capture;
#[path = "owner_authority_support/environment.rs"]
mod environment;
#[path = "owner_authority_support/fixture.rs"]
mod fixture;
#[path = "owner_authority_support/native.rs"]
mod native;
#[path = "owner_authority_support/owners.rs"]
mod owners;
