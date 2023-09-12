#[macro_use]
extern crate tracing;

#[cfg(not(target_arch = "wasm32"))]
mod authority;

#[cfg(not(target_arch = "wasm32"))]
mod try_or_reset;

#[cfg(not(target_arch = "wasm32"))]
mod extractor;

#[cfg(not(target_arch = "wasm32"))]
mod worker;

#[cfg(not(target_arch = "wasm32"))]
mod handlers;

#[cfg(not(target_arch = "wasm32"))]
mod error;

#[cfg(not(target_arch = "wasm32"))]
mod gateway;

#[cfg(not(target_arch = "wasm32"))]
pub use gateway::*;
