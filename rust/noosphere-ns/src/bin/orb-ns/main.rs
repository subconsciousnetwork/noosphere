#[cfg(target_arch = "wasm32")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
mod cli;
#[cfg(not(target_arch = "wasm32"))]
mod cli_address;
#[cfg(not(target_arch = "wasm32"))]
mod runner;
#[cfg(not(target_arch = "wasm32"))]
mod utils;

#[cfg(not(target_arch = "wasm32"))]
mod inner {
    pub use crate::cli::{CLICommand, CLIPeers, CLIRecords, CLI};
    pub use crate::runner::{run, RunnerConfig};
    pub use anyhow::{anyhow, Result};
    pub use clap::Parser;
    pub use noosphere::key::{InsecureKeyStorage, KeyStorage};
    pub use noosphere_ns::server::HTTPClient;
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
    use noosphere_ns::NameSystemClient;

    let key_storage = InsecureKeyStorage::new(&utils::get_keys_dir()?)?;

    match CLI::parse().command {
        command @ CLICommand::Run { .. } => {
            let config = RunnerConfig::try_from_command(&key_storage, command).await?;
            utils::run_until_abort(async move { run(config).await }).await?;
            Ok(())
        }
        CLICommand::KeyGen { key } => {
            if key_storage.require_key(&key).await.is_ok() {
                info!("Key \"{}\" already exists in `~/.noosphere/keys/`.", &key);
            } else {
                key_storage.create_key(&key).await?;
                info!("Key \"{}\" created in `~/.noosphere/keys/`.", &key);
            }
            Ok(())
        }
        CLICommand::Status { api_url } => {
            let client = HTTPClient::new(api_url).await?;
            let info = client.network_info().await?;
            info!("{:#?}", info);
            Ok(())
        }
        CLICommand::Records(CLIRecords::Get { identity, api_url }) => {
            let client = HTTPClient::new(api_url).await?;
            let maybe_record = client.get_record(&identity).await?;
            if let Some(record) = maybe_record {
                info!("{}", record.try_to_string()?);
            } else {
                info!("No record found.");
            }
            Ok(())
        }
        CLICommand::Records(CLIRecords::Put { record, api_url }) => {
            let client = HTTPClient::new(api_url).await?;
            client.put_record(record).await?;
            info!("success");
            Ok(())
        }
        CLICommand::Peers(CLIPeers::Ls { api_url }) => {
            let client = HTTPClient::new(api_url).await?;
            let peers = client.peers().await?;
            info!("{:#?}", peers);
            Ok(())
        }
        CLICommand::Peers(CLIPeers::Add { peer, api_url }) => {
            let client = HTTPClient::new(api_url).await?;
            client.add_peers(vec![peer]).await?;
            info!("success");
            Ok(())
        }
    }
}
