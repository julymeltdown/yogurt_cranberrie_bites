//! C03-CANCEL01: real public HTTP cancellation against native owner services.
//! The private controller supplies real signup/login and a TCP reset; this host
//! adds only a deterministic gate after the real durable delivery publication.
#[path = "participant_cancellation_support/host.rs"]
mod host;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires the owned participant_cancellation_support/native.py controller"]
async fn public_read_cancellation_releases_fences_and_unknown_callback_quarantines(
) -> anyhow::Result<()> {
    host::run().await
}
