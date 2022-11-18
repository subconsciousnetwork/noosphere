#![cfg(not(target_arch = "wasm32"))]

use clap::{Parser, Subcommand};
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
    },

    /// Utility to create keys compatible with Noosphere.
    KeyGen {
        /// The name of the key to be stored in `~/.noosphere/keys/`.
        #[clap(short, long)]
        key: String,
    },
}

#[derive(Debug, Deserialize)]
pub struct CLIConfigNode {
    pub key: String,
    pub port: Option<u16>,
}

#[derive(Debug, Deserialize)]
pub struct CLIConfig {
    pub nodes: Vec<CLIConfigNode>,
}
