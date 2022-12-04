mod memory;
mod tracking;

pub use memory::*;
pub use tracking::*;

#[cfg(not(target_arch = "wasm32"))]
mod native;

#[cfg(not(target_arch = "wasm32"))]
pub use native::*;

#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(target_arch = "wasm32")]
pub use web::*;

#[cfg(feature = "kubo-storage")]
mod kubo;

#[cfg(feature = "kubo-storage")]
pub use kubo::*;
