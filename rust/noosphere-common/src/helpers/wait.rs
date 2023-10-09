use instant::Duration;

/// Wait for the specified number of seconds; uses [tokio::time::sleep], so this
/// will yield to the async runtime rather than block until the sleep time is
/// elapsed.
pub async fn wait(seconds: u64) {
    #[cfg(not(target_arch = "wasm32"))]
    tokio::time::sleep(Duration::from_secs(seconds)).await;
    #[cfg(target_arch = "wasm32")]
    gloo_timers::future::sleep(Duration::from_secs(seconds)).await
}
