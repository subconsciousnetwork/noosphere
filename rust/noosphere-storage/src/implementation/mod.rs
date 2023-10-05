mod memory;
mod tracking;

pub use memory::*;
pub use tracking::*;

#[cfg(not(target_arch = "wasm32"))]
mod sled;
#[cfg(not(target_arch = "wasm32"))]
pub use self::sled::*;

#[cfg(all(not(target_arch = "wasm32"), feature = "rocksdb"))]
mod rocks_db;
#[cfg(all(not(target_arch = "wasm32"), feature = "rocksdb"))]
pub use rocks_db::*;

#[cfg(target_arch = "wasm32")]
mod indexed_db;
#[cfg(target_arch = "wasm32")]
pub use indexed_db::*;

#[cfg(all(not(target_arch = "wasm32"), feature = "iroh"))]
mod iroh;
#[cfg(all(not(target_arch = "wasm32"), feature = "iroh"))]
pub use iroh::*;
