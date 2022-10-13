#[cfg(not(target_arch = "wasm32"))]
#[macro_use]
extern crate tracing;

#[cfg(not(target_arch = "wasm32"))]
pub mod native;

#[cfg(target_arch = "wasm32")]
pub mod web;
