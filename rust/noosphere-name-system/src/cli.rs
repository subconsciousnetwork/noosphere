use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use futures::future::try_join_all;
use noosphere::authority::generate_ed25519_key;
use noosphere::authority::restore_ed25519_key;
use noosphere_cli::native::workspace::Workspace;
use noosphere_name_system::NameSystem;
use std::fs;
use tokio;
use tokio::signal;
use toml;
/// @TODO these materials should be exposed in noosphere::authority
use ucan_key_support::ed25519::Ed25519KeyMaterial;

const QUERY_TIMEOUT: u32 = 5 * 60;

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

async fn keyname_to_keymaterial(
    workspace: &Workspace,
    keyname: &String,
) -> Result<Ed25519KeyMaterial> {
    workspace
        .get_key_mnemonic(keyname.as_str())
        .await
        .and_then(|m| restore_ed25519_key(&m))
        .map_err(|_| anyhow!(format!("Could not find key with name {keyname:?}")))
}

fn read_pinned_file(
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

pub async fn run_cli_main() -> Result<()> {
    let cli = CLI::parse();
    let workspace = Workspace::new(&std::env::current_dir()?)?;
    workspace.initialize_global_directories().await?;

    match cli.command {
        CLICommand::Daemon { keyname, pinned } => {
            println!("Using keyname {}", keyname);
            let key_material = keyname_to_keymaterial(&workspace, &keyname).await?;
            let pinned_list = read_pinned_file(&pinned)?;
            let mut ns = NameSystem::new(&key_material, QUERY_TIMEOUT)?;
            ns.connect().await?;

            if let Some(pinned) = pinned_list {
                let ns_ref = &ns;
                let pending_responses: Vec<_> = pinned
                    .iter()
                    .map(|record| {
                        println!("Pinning {} key {}...", record.0, record.1);
                        ns_ref.set_record(
                            record.1.clone().into_bytes(),
                            record.2.clone().into_bytes(),
                        )
                    })
                    .collect();

                let pinned_names = try_join_all(pending_responses).await?;
                for pinned_name in pinned_names {
                    println!("Pinned {:#?}", pinned_name);
                }
            }
            signal::ctrl_c().await?;
            ns.disconnect().await?;
            Ok(())
        }
        CLICommand::Query { command } => match command {
            QueryCommand::Get { name } => {
                // Just querying, use an ephemeral key
                let key_material = generate_ed25519_key();
                let mut ns = NameSystem::new(&key_material, QUERY_TIMEOUT)?;
                ns.connect().await?;
                let result = ns.get_record(name.clone().into_bytes()).await?;
                match result {
                    Some(value) => {
                        println!(
                            "Found record for {}: {}",
                            name,
                            String::from_utf8(value).unwrap()
                        )
                    }
                    None => println!(
                        "Query completed successfully, but no results found for {}",
                        name
                    ),
                }
                Ok(())
            }
        },
    }
}
