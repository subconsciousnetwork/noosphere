//! Declarative definition for the end-user-facing CLI

use noosphere_core::data::Did;
use noosphere_gateway::DocTicket;

use std::net::IpAddr;

use clap::Parser;
use clap::Subcommand;
use url::Url;

#[allow(missing_docs)]
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

#[allow(missing_docs)]
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

        #[clap(long)]
        iroh_ticket: DocTicket,

        /// URL for a Noosphere name system RPC API
        #[clap(short = 'N', long, default_value = "http://127.0.0.1:6667")]
        name_resolver_api: Url,

        /// The IP address of the interface that the gateway should bind to
        #[clap(short, long, default_value = "127.0.0.1")]
        interface: IpAddr,

        /// The port that the gateway should listen on
        #[clap(short, long, default_value = "4433")]
        port: u16,

        /// If set, the amount of memory that the storage provider may use
        /// for caching in bytes.
        #[clap(long)]
        storage_memory_cache_limit: Option<usize>,
    },
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
    },

    /// Join an existing sphere by its ID and set up a local working copy
    Join {
        /// The pet name of a key to use when requesting access to the sphere
        #[clap(short = 'k', long)]
        local_key: String,

        /// The URL for a gateway API host that the owner key of this sphere is authorized to use
        #[clap(short = 'g', long)]
        gateway_url: Url,

        /// The identity of the authorization that allows the specified key
        /// to join the sphere (if already known)
        #[clap(short = 'a', long)]
        authorization: Option<String>,

        /// The identity of an existing sphere to join
        id: Did,

        /// The maximum depth to traverse through followed spheres when
        /// rendering updates
        #[clap(short = 'd', long)]
        render_depth: Option<u32>,
    },

    /// Show details about files in the sphere directory that have changed since
    /// the last time the sphere was saved
    Status {
        /// Only output the orb id
        #[clap(long)]
        id: bool,
    },

    /// Saves changed files to a sphere, creating and signing a new revision in
    /// the process; does nothing if there have been no changes to the files
    /// since the last revision
    Save {
        /// The maximum depth to traverse through followed spheres when
        /// rendering updates
        #[clap(short = 'd', long)]
        render_depth: Option<u32>,
    },

    /// Synchronizes the local sphere with the copy in a configured gateway;
    /// note that this is a "conflict-free" sync that may cause local changes
    /// to be overwritten in cases where two or more clients have made changes
    /// to the same files
    Sync {
        /// Automatically retry the attempt to sync this number of times
        #[clap(short = 'r', long, default_value = "0")]
        auto_retry: u32,

        /// The maximum depth to traverse through followed spheres when
        /// rendering updates
        #[clap(short = 'd', long)]
        render_depth: Option<u32>,
    },

    /// Force a render of local sphere content as well as the peer graph; note
    /// that this will overwrite any unsaved changes to local sphere content
    Render {
        /// The maximum depth to traverse through followed spheres when
        /// rendering updates
        #[clap(short = 'd', long)]
        render_depth: Option<u32>,
    },

    /// Print a changelog of sphere in a human readable format
    History,

    #[allow(missing_docs)]
    Follow {
        #[clap(subcommand)]
        command: FollowCommand,
    },

    #[allow(missing_docs)]
    Config {
        #[clap(subcommand)]
        command: ConfigCommand,
    },

    #[allow(missing_docs)]
    Auth {
        #[clap(subcommand)]
        command: AuthCommand,
    },
}

/// Read and manage configuration values for a local sphere
#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Set a configuration value for the local sphere
    Set {
        #[allow(missing_docs)]
        #[clap(subcommand)]
        command: ConfigSetCommand,
    },
    /// Retrieve a configuration value if one is set
    Get {
        #[allow(missing_docs)]
        #[clap(subcommand)]
        command: ConfigGetCommand,
    },
}

/// Write local-only metadata configuration related to this sphere
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
}

/// Read local-only metadata configuration related to this sphere
#[derive(Debug, Subcommand)]
pub enum ConfigGetCommand {
    /// Read the configured gateway URL
    GatewayUrl,

    /// Read the configured counterpart DID
    Counterpart,
}

/// Manage the devices/keys that are allowed to access this sphere
#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    /// Authorize a key to work on the sphere in the current directory
    Add {
        /// The DID of the key to authorize
        did: Did,

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

        /// Format the authorizations as a tree based on ancestry
        #[clap(short = 't', long)]
        tree: bool,
    },

    /// Revoke authorization to work on the sphere from a specified key
    Revoke {
        /// The name of a key to revoke authorization for
        name: String,
    },

    /// Rotate key authority from one key to another
    Rotate {},
}

/// Follow and/or unfollow other spheres
#[derive(Debug, Subcommand)]
pub enum FollowCommand {
    /// Follow a sphere, assigning it a personalized nickname
    Add {
        /// A personalized nickname for the sphere you are following
        name: Option<String>,

        /// The sphere ID that you wish to follow
        #[clap(short = 'i', long)]
        sphere_id: Option<Did>,
    },

    /// Unfollow a sphere, either by nickname or by sphere ID
    Remove {
        /// A short name of a sphere that you wish to unfollow. If you follow
        /// the same sphere by multiple names, this will only remove the name
        /// you specify
        #[clap(short = 'n', long)]
        by_name: Option<String>,

        /// The sphere ID that you wish to unfollow. If you follow this sphere
        /// by multiple names, all of them will be removed at once.
        #[clap(short = 'i', long)]
        by_sphere_id: Option<Did>,
    },

    /// Rename a sphere that you currently follow to something new
    Rename {
        /// The current nickname of a sphere that you follow
        from: String,

        /// The preferred nickname to rename the sphere to
        #[clap(short = 't', long)]
        to: Option<String>,
    },

    /// Show a list of all the spheres that you follow
    List {
        /// Output the list of peers as formatted JSON
        #[clap(short = 'j', long)]
        as_json: bool,
    },
}
