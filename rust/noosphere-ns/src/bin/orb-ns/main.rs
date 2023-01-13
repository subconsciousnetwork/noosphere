#[cfg(target_arch = "wasm32")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
mod cli;
#[cfg(not(target_arch = "wasm32"))]
mod runner;
#[cfg(not(target_arch = "wasm32"))]
mod utils;

#[cfg(not(target_arch = "wasm32"))]
mod inner {
    pub use crate::cli::{CLICommand, CLI};
    pub use crate::runner::{run, RunnerConfig};
    pub use anyhow::{anyhow, Result};
    pub use clap::Parser;
    pub use noosphere::key::{InsecureKeyStorage, KeyStorage};
    pub use tokio;
}

#[cfg(not(target_arch = "wasm32"))]
use inner::*;

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() -> Result<()> {
    let key_storage = InsecureKeyStorage::new(&utils::get_keys_dir()?)?;

    match CLI::parse().command {
        command @ CLICommand::Run { .. } => {
            let config = RunnerConfig::try_from_command(&key_storage, command).await?;
            utils::run_until_abort(async move { run(config).await }).await?;
            Ok(())
        }
        CLICommand::KeyGen { key } => {
            if key_storage.require_key(&key).await.is_ok() {
                println!("Key \"{}\" already exists in `~/.noosphere/keys/`.", &key);
            } else {
                key_storage.create_key(&key).await?;
                println!("Key \"{}\" created in `~/.noosphere/keys/`.", &key);
            }
            Ok(())
        }
    }
}
