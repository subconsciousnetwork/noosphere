#![cfg(not(target_arch = "wasm32"))]

#[macro_use]
extern crate tracing;

#[macro_use]
extern crate lazy_static;

mod builder;
pub mod dht;
mod name_system;
mod records;
pub mod utils;
mod validator;

pub use builder::NameSystemBuilder;
pub use dht::DHTKeyMaterial;
pub use libp2p::multiaddr::Multiaddr;
pub use name_system::{NameSystem, BOOTSTRAP_PEERS};
pub use records::NSRecord;
pub use validator::Validator;
