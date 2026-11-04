//! Pure product-construction/dispatch-boundary checks, not a positive owner or
//! Gateway HTTP test. No authority, TLS identity or database fixture is invented.
use concert_service::{
    adapters::out::SystemClock,
    feed::product::{FeedProductConfig, FeedProductProducer},
    product_rpc::{scoped::ScopedProductProducer, Operation},
};
use uuid::Uuid;

#[test]
fn accepts_explicit_post_addresses_without_starting_an_owner() {
    for endpoint in [
        "https://127.0.0.1:50055",
        "https://post.svc.internal:443/",
        "https://[::1]:50055",
    ] {
        assert!(FeedProductConfig::new(endpoint, "concert-feed-owner-token", &[73; 32]).is_ok());
    }
}

#[test]
fn refuses_ambient_plaintext_or_retargetable_post_addresses() {
    for endpoint in [
        "",
        "post.svc.internal:50055",
        "http://127.0.0.1:50055",
        "https://post.svc.internal",
        "https://post.svc.internal:0",
        "https://post.svc.internal:65536",
        "https://user:pass@post.svc.internal:50055",
        "https://post.svc.internal:50055/read",
        "https://post.svc.internal:50055/?owner=other",
        "https://post.svc.internal:50055#fragment",
        "https://post.svc.internal:50055/#fragment",
        " https://post.svc.internal:50055",
        "https://post.svc.internal:50055\n",
    ] {
        assert!(
            FeedProductConfig::new(endpoint, "concert-feed-owner-token", &[73; 32]).is_err(),
            "accepted unsupported Post address"
        );
    }
}

#[test]
fn refuses_missing_placeholder_and_invalid_workload_credentials() {
    for credential in [
        "",
        " ",
        "token\nother",
        "local-parity-internal-service-token",
        "dev-internal-service-token",
        " token",
        "token ",
    ] {
        assert!(
            FeedProductConfig::new("https://post.svc.internal:50055", credential, &[73; 32])
                .is_err(),
            "accepted missing or unsupported credential"
        );
    }
}

#[test]
fn cursor_secret_is_explicit_and_not_a_zero_default() {
    assert!(FeedProductConfig::new(
        "https://post.svc.internal:50055",
        "concert-feed-owner-token",
        &[0; 32]
    )
    .is_err());
}

#[test]
fn producer_is_scoped_and_handles_only_the_three_read_operations() {
    fn scoped<T: ScopedProductProducer>() {}
    scoped::<FeedProductProducer<SystemClock>>();
    let post = Uuid::from_u128(17);
    for operation in [
        Operation::Feed,
        Operation::Post(post),
        Operation::Comments(post),
    ] {
        assert!(FeedProductProducer::<SystemClock>::handles(operation));
    }
    for operation in [
        Operation::ListParties,
        Operation::GetParty(post),
        Operation::CreateParty,
        Operation::InitializeUpload,
        Operation::MediaMetadata(post),
        Operation::UploadBytes(post),
        Operation::FinalizeUpload(post),
        Operation::PrivateContent(post),
        Operation::CommandStatus,
    ] {
        assert!(!FeedProductProducer::<SystemClock>::handles(operation));
    }
}
