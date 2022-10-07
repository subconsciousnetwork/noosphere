use anyhow::Result;

#[cfg(not(target_arch = "wasm32"))]
mod cli;
#[cfg(not(target_arch = "wasm32"))]
use tokio;

#[cfg(target_arch = "wasm32")]
pub fn main() -> anyhow::Result<()> {
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() -> Result<()> {
    cli::run_cli_main().await
}
