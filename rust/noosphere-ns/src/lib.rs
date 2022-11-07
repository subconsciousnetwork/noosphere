#![cfg(not(target_arch = "wasm32"))]

#[macro_use]
extern crate tracing;

mod builder;
pub mod dht;
mod name_system;
mod records;
pub mod utils;

pub use builder::NameSystemBuilder;
pub use libp2p::multiaddr;
pub use name_system::NameSystem;
pub use records::NSRecord;
