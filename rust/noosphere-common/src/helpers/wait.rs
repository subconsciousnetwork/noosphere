use std::time::Duration;

/// Wait for the specified number of seconds; uses [tokio::time::sleep], so this
/// will yield to the async runtime rather than block until the sleep time is
/// elapsed.
pub async fn wait(seconds: u64) {
    tokio::time::sleep(Duration::from_secs(seconds)).await;
}
