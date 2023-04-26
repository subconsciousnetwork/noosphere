use crate::cli::{CLICommand, CLIPeers, CLIRecords, CLI};
use crate::runner::{NameSystemRunner, RunnerNodeConfig};
use anyhow::{anyhow, Result};
use clap::Parser;
use noosphere::key::{InsecureKeyStorage, KeyStorage};
use noosphere_ns::server::HttpClient;
use noosphere_ns::DhtClient;
use serde::Serialize;
use tracing::*;

/// A wrapper object containing a JSON string for rendering
/// to stdout, and potentially a handle to keep alive for
/// a long-running process (e.g. running the name system).
pub enum CommandResponse {
    Finalized {
        value: Option<String>,
    },
    LongRunning {
        value: Option<String>,
        runner: NameSystemRunner,
    },
}

impl CommandResponse {
    pub fn empty() -> Self {
        CommandResponse::Finalized { value: None }
    }

    pub fn value(&self) -> Option<&String> {
        match self {
            CommandResponse::Finalized { value } | CommandResponse::LongRunning { value, .. } => {
                value.as_ref()
            }
        }
    }

    pub async fn wait_until_completion(&mut self) -> Result<()> {
        match self {
            CommandResponse::LongRunning { runner, .. } => runner.await,
            _ => Err(anyhow!("Not a long running command.")),
        }
    }
}

/// Serializes a value to JSON.
fn jsonify<T: Serialize>(value: &T) -> Result<String> {
    serde_json::to_string(value).map_err(|e| e.into())
}

/// Wraps a serializable value in an `Result::Err`, resulting
/// in a JSON string that looks like:
/// `{ "Err": "something went wrong." }`
fn jsonify_err<T: Serialize>(value: T) -> Result<String> {
    serde_json::to_string(&Err::<T, T>(value)).map_err(|e| e.into())
}

/// Parse this process's arguments, and run until completion.
/// See [process_command].
pub async fn process_args(key_storage: &InsecureKeyStorage) -> Result<(), String> {
    let result = process_command(CLI::parse().command, key_storage).await;

    match result {
        Ok(command_res) => {
            if let Some(json_str) = command_res.value() {
                println!("{}", json_str);
            } else {
                println!("{{}}");
            }
            match command_res {
                mut cmd @ CommandResponse::LongRunning { .. } => {
                    let _ = cmd.wait_until_completion().await;
                }
                _ => {}
            }

            Ok(())
        }
        Err(json_str) => {
            error!("{}", json_str);
            Err(json_str)
        }
    }
}

/// Processes a [CLICommand], returning a [Result] containing
/// a [CommandResponse] on success (with JSON data and optionally a
/// long-running process to keep alive), or a JSON error on failure.
pub async fn process_command(
    command: CLICommand,
    key_storage: &InsecureKeyStorage,
) -> Result<CommandResponse, String> {
    async fn process_command_inner(
        command: CLICommand,
        key_storage: &InsecureKeyStorage,
    ) -> Result<CommandResponse> {
        match command {
            command @ CLICommand::Run { .. } => {
                let config = RunnerNodeConfig::try_from_command(command, key_storage).await?;
                let runner = NameSystemRunner::try_from_config(config).await?;
                let value = Some(jsonify(&runner)?);
                Ok::<CommandResponse, anyhow::Error>(CommandResponse::LongRunning { runner, value })
            }
            CLICommand::KeyGen { key } => {
                if key_storage.require_key(&key).await.is_ok() {
                    info!("Key \"{}\" already exists in `~/.noosphere/keys/`.", &key);
                } else {
                    key_storage.create_key(&key).await?;
                    info!("Key \"{}\" created in `~/.noosphere/keys/`.", &key);
                }
                Ok(CommandResponse::empty())
            }
            CLICommand::Status { api_url } => {
                let client = HttpClient::new(api_url).await?;
                let info = client.network_info().await?;
                Ok(CommandResponse::Finalized {
                    value: Some(jsonify(&info)?),
                })
            }
            CLICommand::Records(CLIRecords::Get { identity, api_url }) => {
                let client = HttpClient::new(api_url).await?;
                let maybe_record = client.get_record(&identity).await?;
                Ok(CommandResponse::Finalized {
                    value: Some(jsonify(&maybe_record)?),
                })
            }
            CLICommand::Records(CLIRecords::Put { record, api_url }) => {
                let client = HttpClient::new(api_url).await?;
                client.put_record(record, 1).await?;
                Ok(CommandResponse::empty())
            }
            CLICommand::Peers(CLIPeers::Ls { api_url }) => {
                let client = HttpClient::new(api_url).await?;
                let peers = client.peers().await?;
                Ok(CommandResponse::Finalized {
                    value: Some(jsonify(&peers)?),
                })
            }
            CLICommand::Peers(CLIPeers::Add { peer, api_url }) => {
                let client = HttpClient::new(api_url).await?;
                client.add_peers(vec![peer]).await?;
                Ok(CommandResponse::empty())
            }
        }
    }

    process_command_inner(command, key_storage)
        .await
        .map_err(|e| jsonify_err(e.to_string()).unwrap())
}
