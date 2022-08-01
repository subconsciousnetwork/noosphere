#[macro_use]
extern crate tracing;

#[cfg(target_arch = "wasm32")]
mod inner {
    pub async fn main() -> anyhow::Result<()> {
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod gateway;

#[cfg(not(target_arch = "wasm32"))]
mod inner {
    use std::net::{SocketAddr, TcpListener};

    use crate::gateway::{
        commands,
        environment::{GatewayConfig, GatewayRoot},
        tracing::initialize_tracing,
        Cli, Command,
    };
    use anyhow::Result;
    use clap::Parser;
    use noosphere_storage::native::{NativeStorageInit, NativeStorageProvider};

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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    inner::main().await
}
