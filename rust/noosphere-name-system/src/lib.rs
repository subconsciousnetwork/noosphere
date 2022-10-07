#[cfg(not(target_arch = "wasm32"))]
mod dht;
#[cfg(not(target_arch = "wasm32"))]
mod name_system;
#[cfg(not(target_arch = "wasm32"))]
mod utils;

#[cfg(not(target_arch = "wasm32"))]
pub use dht::DHTClient;
#[cfg(not(target_arch = "wasm32"))]
pub use name_system::NameSystem;
