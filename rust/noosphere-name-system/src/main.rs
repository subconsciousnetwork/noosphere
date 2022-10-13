#[macro_use]
extern crate tracing;

use anyhow::Result;

#[cfg(not(target_arch = "wasm32"))]
mod cli;
#[cfg(not(target_arch = "wasm32"))]
use tokio;
#[cfg(not(target_arch = "wasm32"))]
use tracing_subscriber::prelude::*;

#[cfg(target_arch = "wasm32")]
pub fn main() -> anyhow::Result<()> {
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "noosphere_name_system=trace".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();
    cli::run_cli_main().await
}
