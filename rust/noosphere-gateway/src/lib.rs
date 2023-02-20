#[cfg(not(target_arch = "wasm32"))]
#[macro_use]
extern crate tracing;

#[cfg(not(target_arch = "wasm32"))]
mod authority;

#[cfg(not(target_arch = "wasm32"))]
mod extractor;

#[cfg(not(target_arch = "wasm32"))]
mod ipfs;

#[cfg(not(target_arch = "wasm32"))]
mod route;

#[cfg(not(target_arch = "wasm32"))]
mod gateway;

#[cfg(not(target_arch = "wasm32"))]
pub use gateway::*;
