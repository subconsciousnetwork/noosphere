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
    pub use noosphere_core::tracing::initialize_tracing;
    pub use tokio;
    pub use tracing::*;
}

#[cfg(not(target_arch = "wasm32"))]
use inner::*;

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    initialize_tracing(None);

    let key_storage = InsecureKeyStorage::new(&utils::get_keys_dir()?)?;
    cli::process_args(&key_storage)
        .await
        .map_err(|s| anyhow!(s))
}
