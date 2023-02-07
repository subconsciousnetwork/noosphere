#![cfg(not(target_arch = "wasm32"))]

use crate::cli_address::{deserialize_multiaddr, deserialize_socket_addr, parse_cli_address};
use clap::{Parser, Subcommand};
use noosphere_core::data::Did;
use noosphere_ns::{DHTConfig, Multiaddr, NSRecord};
use serde::Deserialize;
use std::net::SocketAddr;
use std::path::PathBuf;
use url::Url;

#[derive(Parser)]
#[command(author, version, about, long_about=None, name = "orb-ns")]
pub struct CLI {
    #[command(subcommand)]
    pub command: CLICommand,
}

#[derive(Subcommand)]
pub enum CLICommand {
    /// Run DHT nodes in the Noosphere Name System.
    Run {
        /// The path to the bootstrap configuration, a TOML file, containing
        /// entries for keyname/port pairs.
        #[arg(short, long)]
        config: Option<PathBuf>,

        /// If no configuration path provided, the name of the Noosphere keypair to use
        /// stored in `~/.noosphere/keys/`.
        #[arg(short, long)]
        key: Option<String>,

        /// If no configuration path provided, the listening address of this DHT node.
        #[arg(
            short,
            long,
            value_parser = parse_cli_address::<Multiaddr>
        )]
        listening_address: Option<Multiaddr>,

        /// If no configuration path provided, the HTTP listening port of the
        /// API web server associated with this DHT node.
        #[arg(
            long,
            value_parser = parse_cli_address::<SocketAddr>
        )]
        api_address: Option<SocketAddr>,

        /// If no configuration path provided, a list of bootstrap peers to connect to
        /// instead of the default bootstrap peers.
        #[arg(short, long)]
        bootstrap: Option<Vec<Multiaddr>>,
    },

    /// Utility to create keys compatible with Noosphere.
    KeyGen {
        /// The name of the key to be stored in `~/.noosphere/keys/`.
        #[arg(short, long)]
        key: String,
    },

    Status {
        #[arg(short, long, value_parser = parse_cli_address::<Url>)]
        api_url: Url,
    },

    #[command(subcommand)]
    Records(CLIRecords),

    #[command(subcommand)]
    Peers(CLIPeers),
}

#[derive(Subcommand)]
pub enum CLIRecords {
    Get {
        identity: Did,
        #[arg(short, long, value_parser = parse_cli_address::<Url>)]
        api_url: Url,
    },
    Put {
        record: NSRecord,
        #[arg(short, long, value_parser = parse_cli_address::<Url>)]
        api_url: Url,
    },
}

#[derive(Subcommand)]
pub enum CLIPeers {
    Ls {
        #[arg(short, long, value_parser = parse_cli_address::<Url>)]
        api_url: Url,
    },
    Add {
        peer: Multiaddr,
        #[arg(short, long, value_parser = parse_cli_address::<Url>)]
        api_url: Url,
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
    #[serde(default, deserialize_with = "deserialize_multiaddr")]
    pub listening_address: Option<Multiaddr>,
    #[serde(default, deserialize_with = "deserialize_socket_addr")]
    pub api_address: Option<SocketAddr>,
    #[serde(default)]
    pub peers: Vec<Multiaddr>,
    #[serde(default)]
    pub dht_config: DHTConfig,
}
