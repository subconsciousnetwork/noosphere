#[cfg(not(target_arch = "wasm32"))]
#[macro_use]
extern crate tracing;

#[cfg(not(target_arch = "wasm32"))]
pub mod dht;

#[cfg(not(target_arch = "wasm32"))]
mod builder;
#[cfg(not(target_arch = "wasm32"))]
mod name_system;
#[cfg(not(target_arch = "wasm32"))]
mod records;

#[cfg(not(target_arch = "wasm32"))]
pub use builder::NameSystemBuilder;
#[cfg(not(target_arch = "wasm32"))]
pub use cid::Cid;
#[cfg(not(target_arch = "wasm32"))]
pub use libp2p::Multiaddr;
#[cfg(not(target_arch = "wasm32"))]
pub use name_system::NameSystem;
#[cfg(not(target_arch = "wasm32"))]
pub use records::NSRecord;
