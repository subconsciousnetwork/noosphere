//! v0alpha1 stateless [axum] handlers.

#[cfg(doc)]
use axum;

mod did;
mod fetch;
mod identify;
mod push;
mod replicate;

pub use did::*;
pub use fetch::*;
pub use identify::*;
pub use push::*;
pub use replicate::*;
