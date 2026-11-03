#[path = "unit/config.rs"]
mod config;
#[path = "unit/dto_mapping.rs"]
mod dto_mapping;
#[path = "unit/http_routes.rs"]
mod http_routes;
#[path = "unit/mutation_receipt_retention_worker.rs"]
mod mutation_receipt_retention_worker;
#[path = "unit/profile_use_cases.rs"]
mod profile_use_cases;
#[path = "unit/retention_purge_worker.rs"]
mod retention_purge_worker;
#[path = "unit/upload_avatar_use_case.rs"]
mod upload_avatar_use_case;
#[path = "unit/user_deletion.rs"]
mod user_deletion;

// Shared fixtures, including valid image signatures used by avatar tests.
#[path = "support/mod.rs"]
mod support;

#[path = "unit/profile_runtime.rs"]
mod profile_runtime;
