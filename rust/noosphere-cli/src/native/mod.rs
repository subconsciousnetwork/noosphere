pub mod commands;
pub mod workspace;

use anyhow::Result;

use noosphere_core::{data::Did, tracing::initialize_tracing};
use std::ffi::OsString;

use std::net::IpAddr;
use std::path::PathBuf;

use cid::Cid;

use clap::Parser;
use clap::Subcommand;
use url::Url;

use commands::key::key_create;
use commands::key::key_list;
use commands::sphere::sphere_create;
use commands::sphere::sphere_join;
use workspace::Workspace;

use self::commands::auth::auth_add;
use self::commands::auth::auth_list;
use self::commands::auth::auth_revoke;
use self::commands::config::config_get;
use self::commands::config::config_set;
use self::commands::save::save;
use self::commands::serve::serve;
use self::commands::status::status;
use self::commands::sync::sync;

#[derive(Debug, Parser)]
#[clap(name = "orb")]
#[clap(about = "A CLI tool for saving, syncing and sharing content to the Noosphere", long_about = Some(
r#"The orb CLI tool is a utility for saving, syncing and sharing content to the
Noosphere. In practical terms, this means it helps you with tasks such as key
management, creating and updating spheres, managing acccess to said spheres and
publishing the contents of those spheres to public networks."#))]
pub struct Cli {
    #[clap(subcommand)]
    pub command: OrbCommand,
}

#[derive(Debug, Subcommand)]
pub enum OrbCommand {
    Key {
        #[clap(subcommand)]
        command: KeyCommand,
    },

    Sphere {
        #[clap(subcommand)]
        command: SphereCommand,
    },

    Config {
        #[clap(subcommand)]
        command: ConfigCommand,
    },

    Auth {
        #[clap(subcommand)]
        command: AuthCommand,
    },

    /// Summon a gateway geist to manage the local sphere; it will accept
    /// push, fetch and other REST actions from any clients that are authorized
    /// to operate on its counterpart sphere. When it receives changes to its
    /// counterpart sphere, it will perform various actions such as publishing
    /// and/or querying the Noosphere Name System, generating static HTML and/or
    /// updating its own sphere with various related information of interest to
    /// the counterpart sphere
    Serve {
        /// Optional origin to allow CORS for
        #[clap(short, long)]
        cors_origin: Option<Url>,

        /// URL of a Kubo Gateway RPC API
        #[clap(short = 'I', long, default_value = "http://127.0.0.1:5001")]
        ipfs_api: Url,

        /// URL for a Noosphere name system RPC API
        #[clap(short = 'N', long, default_value = "http://127.0.0.1:6667")]
        name_resolver_api: Url,

        /// The IP address of the interface that the gateway should bind to
        #[clap(short, long, default_value = "127.0.0.1")]
        interface: IpAddr,

        /// The port that the gateway should listen on
        #[clap(short, long, default_value = "4433")]
        port: u16,
    },

    /// Show details about files in the sphere directory that have changed since
    /// the last time the sphere was saved
    Status,

    /// If a difftool is configured, show a diff between files on disk and saved versions in the sphere
    Diff {
        /// The specific file or files to show a diff of
        paths: Vec<PathBuf>,

        /// The base revision of the sphere to diff files against
        #[clap(short, long, value_name = "CID")]
        base: Option<Cid>,
    },

    /// Saves changed files to a sphere, creating and signing a new revision in
    /// the process; does nothing if there have been no changes to the files
    /// since the last revision
    Save,

    /// Synchronizes the local sphere with the copy in a configured gateway;
    /// note that this is a "conflict-free" sync that may cause local changes
    /// to be overwritten in cases where two or more clients have made changes
    /// to the same files
    Sync,

    /// Tell a configured gateway to update the published version of the sphere
    /// in the Noosphere name system
    Publish {
        /// The version of the sphere to publish; if none is specified, the
        /// latest saved version will be used
        #[clap(value_name = "CID")]
        version: Option<Cid>,
    },
}

/// Read and manage configuration values for a local sphere
/// TODO: Consider adding `config import` / `config export`
#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Set a configuration value for the local sphere
    Set {
        #[clap(subcommand)]
        command: ConfigSetCommand,
    },
    /// Retrieve a configuration value if one is set
    Get {
        #[clap(subcommand)]
        command: ConfigGetCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConfigSetCommand {
    /// Configure the URL of the gateway to use for publishing and sync
    GatewayUrl {
        /// The URL for a gateway API host that the owner key of this sphere is authorized to use
        url: Url,
    },

    /// If you are configuring your local sphere, the "counterpart" is the
    /// gateway's sphere DID. If you are configuring a gateway's sphere, the
    /// "counterpart" is the DID of your local sphere.
    Counterpart {
        /// The sphere identity (as a DID) of the counterpart
        did: String,
    },

    /// Configure a command to be used when diffing files
    Difftool {
        /// A command that can be used when diffing files
        tool: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConfigGetCommand {
    /// Read the configured gateway URL
    GatewayUrl,

    /// Read the configured counterpart DID
    Counterpart,

    /// Read the configured difftool command
    Difftool,
}

/// Create and securely manage personal keys
#[derive(Debug, Subcommand)]
pub enum KeyCommand {
    /// Generate and securely store a new named key; this key is the analog of
    /// a user account in the Noosphere.
    Create {
        /// The pet name for the newly created key; you will refer to it by this
        /// name when using it in other commands
        name: String,
    },

    /// Print the pet name and DID for all available keys
    #[clap(alias = "ls")]
    List {
        /// Output the list of available keys as formatted JSON
        #[clap(short = 'j', long)]
        as_json: bool,
    },
}

/// Create a new sphere or connect another device to an existing one
#[derive(Debug, Subcommand)]
pub enum SphereCommand {
    /// Initialize a new sphere and assign a key as its owner
    Create {
        /// The pet name of a key to assign as the owner of the sphere
        #[clap(short = 'k', long)]
        owner_key: String,

        /// An optional path to a directory where the sphere should be
        /// initialized; by default, the current working directory will
        /// be used
        path: Option<OsString>,
    },

    /// Join an existing sphere by its ID and set up a local working copy
    Join {
        /// The pet name of a key to use when requesting access to the sphere
        #[clap(short = 'k', long)]
        local_key: String,

        /// The identity of the authorization that allows the specified key
        /// to join the sphere (if already known)
        #[clap(short = 'a', long)]
        authorization: Option<String>,

        /// The identity of an existing sphere to join
        id: Did,

        /// An optional path to a directory where the sphere should be
        /// initialized; by default, the current working directory will
        /// be used
        path: Option<OsString>,
    },
}

/// Manage access to a sphere by holders of other keys
#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    /// Authorize a key to work on the sphere in the current directory
    Add {
        /// The DID of the key to authorize
        did: String,

        /// An optional name to give the key; if one is not specified, a random
        /// one will be assigned
        #[clap(short = 'n', long)]
        name: Option<String>,
    },

    /// Print the name and DID for all keys that the owner has authorized
    /// to work on this sphere
    List {
        /// Output the list of authorized keys as formatted JSON
        #[clap(short = 'j', long)]
        as_json: bool,
    },

    /// Revoke authorization to work on the sphere from a specified key
    Revoke {
        /// The name of a key to revoke authorization for
        name: String,
    },

    /// Rotate key authority from one key to another
    Rotate {},
}

pub async fn main() -> Result<()> {
    initialize_tracing();
    let args = Cli::parse();

    let current_working_directory = std::env::current_dir()?;
    let mut workspace = Workspace::new(&current_working_directory, None)?;

    match args.command {
        OrbCommand::Config { command } => match command {
            ConfigCommand::Set { command } => config_set(command, &workspace).await?,
            ConfigCommand::Get { command } => config_get(command, &workspace).await?,
        },
        OrbCommand::Key { command } => match command {
            KeyCommand::Create { name } => key_create(&name, &workspace).await?,
            KeyCommand::List { as_json } => key_list(as_json, &workspace).await?,
        },
        OrbCommand::Sphere { command } => match command {
            SphereCommand::Create { owner_key, path } => {
                if let Some(path) = path {
                    workspace = Workspace::new(&current_working_directory.join(path), None)?;
                }

                sphere_create(&owner_key, &workspace).await?;
            }
            SphereCommand::Join {
                local_key,
                authorization,
                id,
                path,
            } => {
                if let Some(path) = path {
                    workspace = Workspace::new(&current_working_directory.join(path), None)?;
                }

                sphere_join(&local_key, authorization, &id, &workspace).await?;
            }
        },
        OrbCommand::Status => status(&workspace).await?,
        OrbCommand::Diff { paths: _, base: _ } => todo!(),
        OrbCommand::Save => save(&workspace).await?,
        OrbCommand::Sync => sync(&workspace).await?,
        OrbCommand::Publish { version: _ } => todo!(),
        OrbCommand::Auth { command } => match command {
            AuthCommand::Add { did, name } => {
                auth_add(&did, name, &workspace).await?;
            }
            AuthCommand::List { as_json } => auth_list(as_json, &workspace).await?,
            AuthCommand::Revoke { name } => auth_revoke(&name, &workspace).await?,
            AuthCommand::Rotate {} => todo!(),
        },
        OrbCommand::Serve {
            cors_origin,
            ipfs_api,
            name_resolver_api,
            interface,
            port,
        } => {
            serve(
                interface,
                port,
                ipfs_api,
                name_resolver_api,
                cors_origin,
                &workspace,
            )
            .await?
        }
    };

    Ok(())
}
