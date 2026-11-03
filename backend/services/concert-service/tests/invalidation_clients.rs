use concert_service::adapters::out::relationship_authority::{grpc::proto, invalidation::*};
use platform::{
    grpc_auth::{CallerRegistry, Workload},
    grpc_tls::TlsMaterial,
};
use prost::Message;
use std::{
    convert::Infallible,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tonic::{Request, Response, Status};
use uuid::Uuid;

fn budget() -> ReplayBudget {
    ReplayBudget::for_duration(MAX_OPERATION, ReplayCancellation::default()).unwrap()
}
fn registry() -> CallerRegistry {
    Workload::ALL
        .into_iter()
        .fold(CallerRegistry::new(), |r, w| {
            r.register(w, &format!("private-fixture-{}", w.as_str()))
                .unwrap()
        })
}

#[tokio::test]
async fn plaintext_and_missing_credentials_cannot_construct_authenticated_source() {
    let empty = TlsMaterial::from_pem(Vec::new(), Vec::new(), Vec::new());
    for endpoint in ["http://127.0.0.1:1", "not-a-uri", "file:///private"] {
        assert!(matches!(
            InvalidationSource::new(InvalidationOwner::Auth, endpoint, "fixture", &empty),
            Err(ReadError::InvalidConfiguration)
        ));
    }
    assert!(matches!(
        InvalidationSource::new(InvalidationOwner::Friend, "https://127.0.0.1:1", "", &empty),
        Err(ReadError::InvalidConfiguration)
    ));
}

#[path = "invalidation_clients_support/native.rs"]
mod native;
#[path = "invalidation_clients_support/transport.rs"]
mod transport;
