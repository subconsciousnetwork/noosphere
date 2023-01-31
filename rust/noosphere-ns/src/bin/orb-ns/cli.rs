#![cfg(not(target_arch = "wasm32"))]

use clap::{Parser, Subcommand};
use noosphere_ns::{DHTConfig, Multiaddr};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Parser)]
#[clap(name = "orb-ns")]
pub struct CLI {
    #[clap(subcommand)]
    pub command: CLICommand,
}

#[derive(Subcommand)]
pub enum CLICommand {
    /// Run DHT nodes in the Noosphere Name System.
    Run {
        /// The path to the bootstrap configuration, a TOML file, containing
        /// entries for keyname/port pairs.
        #[clap(short, long)]
        config: Option<PathBuf>,

        /// If no configuration path provided, the name of the Noosphere keypair to use
        /// stored in `~/.noosphere/keys/`.
        #[clap(short, long)]
        key: Option<String>,

        /// If no configuration path provided, the listening port of this DHT node.
        #[clap(short, long)]
        port: Option<u16>,

        /// If no configuration path provided, the HTTP listening port of the
        /// API web server associated with this DHT node.
        #[clap(long)]
        api_port: Option<u16>,

        /// If no configuration path provided, a list of bootstrap peers to connect to
        /// instead of the default bootstrap peers.
        #[clap(short, long)]
        bootstrap: Option<Vec<Multiaddr>>,
    },

    /// Utility to create keys compatible with Noosphere.
    KeyGen {
        /// The name of the key to be stored in `~/.noosphere/keys/`.
        #[clap(short, long)]
        key: String,
    },
}

#[derive(Debug, Deserialize)]
pub struct CLIConfigFile {
    pub peers: Option<Vec<Multiaddr>>,
    pub nodes: Vec<CLIConfigFileNode>,
}

#[derive(Debug, Deserialize)]
pub struct CLIConfigFileNode {
    pub key: String,
    pub port: Option<u16>,
    pub api_port: Option<u16>,
    #[serde(default)]
    pub peers: Vec<Multiaddr>,
    #[serde(default)]
    pub dht_config: DHTConfig,
}
