#![cfg(not(target_arch = "wasm32"))]

#[macro_use]
extern crate tracing;

#[macro_use]
extern crate lazy_static;

pub mod dht;
//mod name_resolver;
mod builder;
mod client;
pub mod helpers;
mod name_system;
mod records;
pub mod utils;

//#[cfg(feature = "api_server")]
pub mod server;

pub use builder::NameSystemBuilder;
pub use client::NameSystemClient;
pub use dht::{DhtConfig, NetworkInfo, Peer};
pub use libp2p::{multiaddr::Multiaddr, PeerId};
pub use name_system::{NameSystem, NameSystemKeyMaterial, BOOTSTRAP_PEERS};
pub use records::NsRecord;
