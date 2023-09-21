/// Wait for the specified number of seconds, yielding to the async runtime
/// rather than block. Uses [tokio::time::sleep], or [gloo_timers::future::TimeoutFuture]
/// on `wasm32` targets.
pub async fn wait(seconds: u64) {
    #[cfg(target_arch = "wasm32")]
    {
        gloo_timers::future::TimeoutFuture::new((seconds / 1000) as u32).await
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        tokio::time::sleep(std::time::Duration::from_secs(seconds)).await
    }
}
