//! Closed client refusal checks. Positive mTLS/native lifetime is exercised by
//! concert-service's notification_target_authority artifact.
use notification_service::concert::{authority, transport, AuthorityFence, Authorization};
#[path = "../src/concert/target_client.rs"]
mod target_client;
use platform::grpc_tls::TlsMaterial;
#[tokio::test]
async fn live_target_client_refuses_plaintext_endpoint_and_invalid_tls_material() {
    let tls = TlsMaterial::from_pem(
        b"invalid cert".to_vec(),
        b"invalid key".to_vec(),
        b"invalid ca".to_vec(),
    );
    for endpoint in [
        "http://127.0.0.1:1",
        "https://127.0.0.1:1",
        "not-an-endpoint",
    ] {
        assert!(
            target_client::RootTargetClient::connect(endpoint, "notification-fixture", &tls)
                .is_err()
        );
    }
}
