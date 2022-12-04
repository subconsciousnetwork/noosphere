#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
    // Call out to an external module for platform-specific compilation purposes
    noosphere_cli::native::main().await?;

    Ok(())
}

#[cfg(target_arch = "wasm32")]
#[allow(unused_must_use)]
pub fn main() {
    noosphere_cli::web::main();
}
