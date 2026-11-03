#[path = "support/public_profile_privacy.rs"]
mod public_profile_privacy;

#[path = "unit/dto_mapping.rs"]
mod dto_mapping;

#[path = "unit/config.rs"]
mod config;

#[path = "unit/facade_helpers.rs"]
mod facade_helpers;

#[path = "unit/follow_use_cases.rs"]
mod follow_use_cases;

#[path = "unit/grpc_server.rs"]
mod grpc_server;

#[path = "unit/repository_errors.rs"]
mod repository_errors;

#[path = "unit/people_recommendation.rs"]
mod people_recommendation;
#[path = "unit/recommendation_domain.rs"]
mod recommendation_domain;
#[path = "unit/recommendation_event.rs"]
mod recommendation_event;
#[path = "unit/relationship_use_cases.rs"]
mod relationship_use_cases;
#[path = "unit/user_deletion.rs"]
mod user_deletion;

#[path = "unit/recommendation_operations.rs"]
mod recommendation_operations;

#[path = "unit/http_metrics.rs"]
mod http_metrics;
