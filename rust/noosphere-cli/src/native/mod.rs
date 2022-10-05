pub mod commands;
pub mod workspace;

use anyhow::Result;
use std::ffi::OsString;

use std::path::PathBuf;

use cid::Cid;

use clap::Parser;
use clap::Subcommand;
use url::Url;

use commands::key::create_key;
use commands::key::list_keys;
use commands::sphere::initialize_sphere;
use commands::sphere::join_sphere;
use workspace::Workspace;

use self::commands::auth::auth_add;
use self::commands::auth::auth_list;
use self::commands::auth::auth_revoke;

// orb config set <key> <value> -> Set local configuration
// orb config get <key> -> Read local configuration
// orb config import [--replace] <toml file> -> Import settings from toml
// orb config export -> Export settings as toml

// orb sphere initialize
// orb sphere join

// orb auth add <did> [--as <name>]
// orb auth list
// orb auth revoke <did-or-name>

// orb history -> Show the history of changes to the sphere (like git log)
// orb history <file> -> Show the history of a file in the working tree

// orb status -> Show the list of things that have changed since last save (like git status)
// orb diff [<file>] -> If a difftool is configured, show a text diff (as appropriate) between the latest sphere revision and files in the working tree
// orb save -> Save all changes to the working tree as a revision to the sphere
// orb sync -> Bi-directionally sync with a configured gateway (if any); like get fetch + rebase + push
// orb publish [<cid>] -> Instruct the gateway to publish a version of the sphere to the DHT

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
    Config {
        #[clap(subcommand)]
        command: ConfigCommand,
    },

    Key {
        #[clap(subcommand)]
        command: KeyCommand,
    },

    Sphere {
        #[clap(subcommand)]
        command: SphereCommand,
    },

    Auth {
        #[clap(subcommand)]
        command: AuthCommand,
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
    /// Import configuration settings from a TOML file
    Import {
        /// A TOML file containing key-value configuration entries to set
        file: PathBuf,

        /// Replace all configurations with the values in the specified file
        #[clap(short, long)]
        replace: bool,
    },

    /// Print all current configuration values and exit
    Export,
}

#[derive(Debug, Subcommand)]
pub enum ConfigSetCommand {
    /// Configure the Noosphere gateway to use for publishing and sync
    Gateway {
        /// The URL for a gateway API host that the owner key of this sphere is authorized to use
        url: Url,
    },

    /// Configure a command to be used when diffing files
    Difftool {
        /// A command that can be used when diffing files
        command: OsString,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConfigGetCommand {
    /// Read the configured gateway URL
    Gateway,

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
    List {
        /// Output the list of available keys as formatted JSON
        #[clap(short = 'j', long)]
        as_json: bool,
    },
}

/// Create, join or share access to a sphere
#[derive(Debug, Subcommand)]
pub enum SphereCommand {
    /// Initialize a new sphere and assign a key as its owner
    Initialize {
        /// The pet name of a key to assign as the owner of the sphere
        #[clap(short = 'k', long)]
        owner_key: Option<String>,

        /// An optional path to a directory where the sphere should be
        /// initialized; by default, the current working directory will
        /// be used
        path: Option<OsString>,
    },

    /// Join an existing sphere by its ID and set up a local working copy
    Join {
        /// The pet name of a key to use when requesting access to the sphere
        #[clap(short = 'k', long)]
        local_key: Option<String>,

        /// An authorization token allowing the specified key to join the
        /// sphere (if already known)
        #[clap(short = 't', long)]
        token: Option<String>,

        /// The ID (specifically: a DID) of an existing sphere to join
        id: String,

        /// An optional path to a directory where the sphere should be
        /// initialized; by default, the current working directory will
        /// be used
        path: Option<OsString>,
    },

    /// Transfer ownership of the sphere in the current directory to another key
    Transfer {
        /// The pet name of the key to transfer ownership to
        new_owner_key: String,
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
}

pub async fn main() -> Result<()> {
    // println!("Hello, Orb!");
    let args = Cli::parse();

    let mut workspace = Workspace::new(&std::env::current_dir()?)?;

    // println!("{:#?}", args);
    // println!("{:#?}", working_paths);

    match args.command {
        OrbCommand::Config { command: _ } => todo!(),
        OrbCommand::Key { command } => match command {
            KeyCommand::Create { name } => create_key(name, &workspace).await?,
            KeyCommand::List { as_json } => list_keys(as_json, &workspace).await?,
        },
        OrbCommand::Sphere { command } => match command {
            SphereCommand::Initialize { owner_key, path } => {
                if let Some(path) = path {
                    workspace = Workspace::new(&workspace.root_path().join(path))?;
                }

                let owner_key = match owner_key {
                    Some(owner_key) => owner_key,
                    None => workspace.unambiguous_default_key_name().await?,
                };

                initialize_sphere(&owner_key, &workspace).await?;
            }
            SphereCommand::Join {
                local_key,
                token,
                id,
                path,
            } => {
                if let Some(path) = path {
                    workspace = Workspace::new(&workspace.root_path().join(path))?;
                }

                let local_key = match local_key {
                    Some(local_key) => local_key,
                    None => workspace.unambiguous_default_key_name().await?,
                };

                join_sphere(&local_key, token, &id, &workspace).await?;
            }
            SphereCommand::Transfer { new_owner_key: _ } => todo!(),
        },
        OrbCommand::Status => todo!(),
        OrbCommand::Diff { paths: _, base: _ } => todo!(),
        OrbCommand::Save => todo!(),
        OrbCommand::Sync => todo!(),
        OrbCommand::Publish { version: _ } => todo!(),
        OrbCommand::Auth { command } => match command {
            AuthCommand::Add { did, name } => auth_add(&did, name, &workspace).await?,
            AuthCommand::List { as_json } => auth_list(as_json, &workspace).await?,
            AuthCommand::Revoke { name } => auth_revoke(&name, &workspace).await?,
        },
    };

    Ok(())
}
