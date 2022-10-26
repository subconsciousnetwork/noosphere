mod commands;
mod utils;
use anyhow::Result;
use clap::{Parser, Subcommand};
use noosphere_cli::native::workspace::Workspace;
use std::net::SocketAddr;

const BOOTSTRAP_PEERS: [&str; 1] = ["/ip4/134.122.20.28/tcp/6666"];

#[derive(Debug, Parser)]
#[clap(name = "orb-ns")]
pub struct CLI {
    #[clap(subcommand)]
    pub command: CLICommand,
}

#[derive(Debug, Subcommand)]
pub enum CLICommand {
    Daemon {
        /// The name of the Noosphere keypair to use stored in `~/.noosphere/keys/`.
        #[clap(short, long)]
        keyname: String,
        /// The path to a TOML file of key/value pairs to maintain on the DHT.
        #[clap(long)]
        address_book: Option<std::path::PathBuf>,
        /// The listening address of this node. Must be in SocketAddr form, e.g. `127.0.0.1:6000`
        #[clap(short, long)]
        address: SocketAddr,
        /// Bootstrap peers. Must be in Multiaddr form, e.g.
        /// `/ip4/123.23.1.4/tcp/4000/p2p/{PEER_ID}`
        /// `/dnsaddr/bootstrap.subconscious.network/p2p/{PEER_ID}`
        #[clap(short, long)]
        bootstrap: Option<Vec<String>>,
        #[clap(long)]
        no_bootstrap: bool,
    },

    Query {
        #[clap(subcommand)]
        command: QueryCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum QueryCommand {
    Get {
        /// The name of the key to query in the DHT.
        name: String,
        /// The name of the Noosphere keypair to use stored in `~/.noosphere/keys/`.
        #[clap(short, long)]
        keyname: String,
        /// Bootstrap peers. Must be in Multiaddr form, e.g.
        /// `/ip4/123.23.1.4/tcp/4000/p2p/{PEER_ID}`
        /// `/dnsaddr/bootstrap.subconscious.network/p2p/{PEER_ID}`
        #[clap(short, long)]
        bootstrap: Option<Vec<String>>,
        #[clap(long)]
        no_bootstrap: bool,
    },
}

fn parse_bootstrap_options(
    no_bootstrap: bool,
    bootstrap_peers: Option<Vec<String>>,
) -> Vec<String> {
    if no_bootstrap {
        vec![]
    } else {
        match bootstrap_peers {
            Some(peers) => peers,
            None => BOOTSTRAP_PEERS.map(String::from).to_vec(),
        }
    }
}

pub async fn run_cli_main() -> Result<()> {
    let cli = CLI::parse();
    let workspace = Workspace::new(&std::env::current_dir()?, None)?;
    workspace.initialize_global_directories().await?;

    debug!("Running with args: {:#?}", cli.command);
    match cli.command {
        CLICommand::Daemon {
            keyname,
            address,
            bootstrap,
            no_bootstrap,
            address_book,
        } => {
            let bootstrap_peers = parse_bootstrap_options(no_bootstrap, bootstrap);
            commands::run_daemon(keyname, address, bootstrap_peers, address_book, &workspace)
                .await?;
        }
        CLICommand::Query { command } => match command {
            QueryCommand::Get {
                bootstrap,
                no_bootstrap,
                keyname,
                name,
            } => {
                let bootstrap_peers = parse_bootstrap_options(no_bootstrap, bootstrap);
                commands::run_query(keyname, name, bootstrap_peers, &workspace).await?;
            }
        },
    }
    Ok(())
}
