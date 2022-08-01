use std::net::IpAddr;

use clap::Parser;
use clap::Subcommand;
use url::Url;

#[derive(Debug, Parser)]
#[clap(name = "ng")]
#[clap(about = "Noosphere Gateway", long_about = None)]
pub struct Cli {
    #[clap(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Generate the gateway's key and scaffold its configuration
    #[clap(arg_required_else_help = true)]
    Initialize {
        /// The owner of this gateway; must match the one recorded in the database
        #[clap(short, long, value_parser)]
        owner_did: String,
    },

    // #[clap(arg_required_else_help = true)]
    /// Start the gateway and serve clients
    Serve {
        /// The URL for an IPFS API that is accessible to this gateway
        // TODO: This should just be part of the config, so maybe move to initialize?
        // #[clap(
        //     short = 'I',
        //     long,
        //     value_parser,
        //     default_value = "http://127.0.0.1:5001"
        // )]
        // ipfs_api: Url,

        /// Optional origin to allow CORS for
        #[clap(short, long, value_parser)]
        cors_origin: Option<Url>,

        /// The IP address of the interface that the gateway should bind to
        #[clap(short, long, value_parser, default_value = "127.0.0.1")]
        interface: IpAddr,

        /// The port that the gateway should listen on
        #[clap(short, long, value_parser, default_value = "4433")]
        port: u16,
    },
}
