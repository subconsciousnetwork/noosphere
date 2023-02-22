//! Sphere petnames are shorthand names that are associated with DIDs. A petname
//! can be any string, and always refers to a DID. The DID, in turn, may be
//! resolved to a CID that represents the tip of history for the sphere that is
//! implicitly identified by the provided DID.

mod read;
mod write;

pub use read::*;
pub use write::*;
