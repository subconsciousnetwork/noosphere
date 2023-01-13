#[cfg(target_arch = "wasm32")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub mod cli;

#[cfg(not(target_arch = "wasm32"))]
mod runner;

#[cfg(not(target_arch = "wasm32"))]
mod utils;

#[cfg(not(target_arch = "wasm32"))]
mod inner {
    pub use crate::cli;
    pub use anyhow::{anyhow, Result};
    pub use noosphere::key::{InsecureKeyStorage, KeyStorage};
    pub use tokio;
    pub use tracing::*;
    pub use tracing_subscriber::{fmt, prelude::*, EnvFilter};
}

#[cfg(not(target_arch = "wasm32"))]
use inner::*;

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let key_storage = InsecureKeyStorage::new(&utils::get_keys_dir()?)?;
    cli::process_args(&key_storage)
        .await
        .map_err(|s| anyhow!(s))
}
