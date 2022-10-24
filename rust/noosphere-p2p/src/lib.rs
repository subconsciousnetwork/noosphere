#[macro_use]
extern crate tracing;

#[cfg(not(target_arch = "wasm32"))]
pub mod dht;
