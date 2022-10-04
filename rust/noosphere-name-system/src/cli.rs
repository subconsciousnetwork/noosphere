use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use noosphere::authority::restore_ed25519_key;
use noosphere_cli::native::workspace::Workspace;
use std::fs;
use toml;
/// @TODO these materials should be exposed in noosphere::authority
use ucan_key_support::ed25519::Ed25519KeyMaterial;

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
        #[clap(short, long)]
        pinned: Option<std::path::PathBuf>,
    },

    Query {
        #[clap(subcommand)]
        command: QueryCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum QueryCommand {
    Get { name: String },
}

pub async fn keyname_to_keymaterial(
    workspace: &Workspace,
    keyname: &String,
) -> Result<Ed25519KeyMaterial> {
    workspace
        .get_key_mnemonic(keyname.as_str())
        .await
        .and_then(|m| restore_ed25519_key(&m))
        .map_err(|_| anyhow!(format!("Could not find key with name {keyname:?}")))
}

pub fn read_pinned_file(
    path: &Option<std::path::PathBuf>,
) -> Result<Option<Vec<(String, String, String)>>> {
    match path {
        Some(p) => match fs::read_to_string(p) {
            Ok(toml_str) => match toml_str.parse::<toml::Value>() {
                Ok(parsed) => match parsed.as_table() {
                    Some(items) => Ok(Some(
                        items
                            .iter()
                            .filter_map(|(name, record)| {
                                let key_opt = record.get("key").and_then(|v| v.as_str());
                                let value_opt = record.get("value").and_then(|v| v.as_str());
                                if key_opt.is_some() && value_opt.is_some() {
                                    Some((
                                        String::from(name),
                                        String::from(key_opt.unwrap()),
                                        String::from(value_opt.unwrap()),
                                    ))
                                } else {
                                    println!("NONE!!");
                                    None
                                }
                            })
                            .collect::<Vec<(String, String, String)>>(),
                    )),
                    None => Ok(None),
                },
                Err(e) => Err(anyhow!(e.to_string())),
            },
            Err(_) => Err(anyhow!(format!("Could not read file at {p:?}"))),
        },
        None => Ok(None),
    }
}
