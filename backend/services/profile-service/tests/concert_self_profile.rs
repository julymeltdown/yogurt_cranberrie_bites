// Keep the owned storage/model qualification independent of shared registration.
// The real service entry is registered by the separate shared-file owner.
#[path = "../src/concert_self_profile/mod.rs"]
mod concert_self_profile;
