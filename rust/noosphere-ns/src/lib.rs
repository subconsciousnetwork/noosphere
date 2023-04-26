#![cfg(not(target_arch = "wasm32"))]

#[macro_use]
extern crate tracing;

#[macro_use]
extern crate lazy_static;

mod builder;
pub mod dht;
mod dht_client;
pub mod helpers;
mod name_resolver;
mod name_system;
mod records;
pub mod utils;

//#[cfg(feature = "api_server")]
pub mod server;

pub use builder::NameSystemBuilder;
pub use dht::{DhtConfig, NetworkInfo, Peer};
pub use dht_client::DhtClient;
pub use libp2p::{multiaddr::Multiaddr, PeerId};
pub use name_resolver::NameResolver;
pub use name_system::{NameSystem, NameSystemKeyMaterial, BOOTSTRAP_PEERS};
pub use records::NsRecord;
