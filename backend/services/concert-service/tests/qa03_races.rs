//! QA03 real PostgreSQL races. Run only through the owned QA03 controller.
//! The database adapters are production implementations; the isolated ACL is
//! explicitly test authority, not canonical Auth/Friend or public HTTP evidence.
#[path = "qa03_races_support/gifts.rs"]
mod gifts;
#[path = "qa03_races_support/party.rs"]
mod party;
#[path = "qa03_races_support/mod.rs"]
mod support;
#[path = "qa03_races_support/wall.rs"]
mod wall;
