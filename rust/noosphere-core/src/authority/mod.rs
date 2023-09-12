//! Data types and helper routines related to general Noosphere authority
//! concepts.
//!
//! This includes key material generation, expressing capabilities and passing
//! around proof of authorization within the other corners of the API.

mod author;
mod authorization;
mod capability;
mod key_material;
mod walk;

pub use author::*;
pub use authorization::*;
pub use capability::*;
pub use key_material::*;
pub use walk::*;
