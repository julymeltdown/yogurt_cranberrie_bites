mod fixture;
pub async fn run() -> anyhow::Result<()> {
    fixture::run().await
}
