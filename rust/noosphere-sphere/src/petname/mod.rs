//! Sphere petnames are shorthand names that are associated with DIDs. A petname
//! can be any string, and always refers to a DID. The DID, in turn, may be
//! resolved to a CID that represents the tip of history for the sphere that is
//! implicitly identified by the provided DID. Note though that a petname may refer
//! to no CID (`None`) when first added (as no peer may have been resolved yet).

mod read;
mod write;

pub use read::*;
pub use write::*;
