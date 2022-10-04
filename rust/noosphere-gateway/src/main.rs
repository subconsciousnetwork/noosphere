#[macro_use]
extern crate tracing;

#[cfg(target_arch = "wasm32")]
pub fn main() -> anyhow::Result<()> {
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
mod gateway;

#[cfg(not(target_arch = "wasm32"))]
mod inner {
    use std::net::TcpListener;

    use crate::gateway::{
        commands, environment::GatewayRoot, tracing::initialize_tracing, Cli, Command,
    };
    use anyhow::Result;
    use clap::Parser;

    pub async fn main() -> Result<()> {
        initialize_tracing();

        let args = Cli::parse();

        let cwd = std::env::current_dir()?;

        match args.command {
            Command::Initialize { owner_did } => {
                commands::initialize(&cwd, &owner_did).await?;
            }
            Command::Serve {
                interface,
                port,
                cors_origin,
            } => {
                let listener = TcpListener::bind(&(interface, port))?;
                let root = GatewayRoot::at_path(&cwd);
                let config = root.to_config();
                let storage_provider = root.to_storage_provider()?;

                commands::serve(listener, storage_provider, config, cors_origin.as_ref()).await?;
            }
        };
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    inner::main().await
}
