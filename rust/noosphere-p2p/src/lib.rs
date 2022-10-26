#[macro_use]
extern crate tracing;

#[cfg(not(target_arch = "wasm32"))]
pub mod dht;

#[cfg(not(target_arch = "wasm32"))]
mod address_book;
#[cfg(not(target_arch = "wasm32"))]
mod builder;
#[cfg(not(target_arch = "wasm32"))]
mod name_system;

#[cfg(not(target_arch = "wasm32"))]
pub use address_book::AddressBook;
#[cfg(not(target_arch = "wasm32"))]
pub use builder::NameSystemBuilder;
#[cfg(not(target_arch = "wasm32"))]
pub use name_system::NameSystem;
