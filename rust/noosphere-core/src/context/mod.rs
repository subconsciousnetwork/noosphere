//! This module implements content, petname and other forms of acccess to
//! spheres. If you have storage and network primitives on your platform, you
//! can initialize a [SphereContext] and use it to work with and synchronize
//! spheres, as well as traverse the broader Noosphere data graph.
//!
//! In order to initialize a [SphereContext], you need a [Did] (like an ID) for
//! a Sphere, a [Storage] primitive and an [Author] (which represents whoever or
//! whatever is trying to access the Sphere inquestion).
//!
//! Once you have a [SphereContext], you can begin reading from, writing to and
//! traversing the Noosphere content graph.
//!
//! ```rust
//! # use anyhow::Result;
//! # use noosphere_core::context::{SphereCursor, HasMutableSphereContext, SphereContentWrite};
//! #
//! # #[cfg(feature = "helpers")]
//! # use noosphere_core::helpers::{simulated_sphere_context,SimulationAccess};
//! #
//! # #[cfg(feature = "helpers")]
//! # #[tokio::main(flavor = "multi_thread")]
//! # async fn main() -> Result<()> {
//! #   let (mut sphere_context, _) = simulated_sphere_context(SimulationAccess::ReadWrite, None).await?;
//! #
//! sphere_context.write("foo", "text/plain", "bar".as_ref(), None).await?;
//! sphere_context.save(None).await?;
//! #
//! #  Ok(())
//! # }
//! #
//! # #[cfg(not(feature = "helpers"))]
//! # fn main() {}
//! ```
//!
//! You can also use a [SphereContext] to access petnames in the sphere:
//!
//! ```rust
//! # use anyhow::Result;
//! # #[cfg(feature = "helpers")]
//! # use noosphere_core::{
//! #   helpers::{simulated_sphere_context,SimulationAccess},
//! #   data::Did,
//! #   context::{SphereCursor, HasMutableSphereContext, SpherePetnameWrite}
//! # };
//! #
//! # #[cfg(feature = "helpers")]
//! # #[tokio::main(flavor = "multi_thread")]
//! # async fn main() -> Result<()> {
//! #   let (mut sphere_context, _) = simulated_sphere_context(SimulationAccess::ReadWrite, None).await?;
//! #
//! sphere_context.set_petname("cdata", Some("did:key:example".into())).await?;
//! sphere_context.save(None).await?;
//! #
//! #   Ok(())
//! # }
//! #
//! # #[cfg(not(feature = "helpers"))]
//! # fn main() {}
//! ```
//!
//!

#![warn(missing_docs)]

#[cfg(doc)]
use crate::{authority::Author, data::Did};

#[cfg(doc)]
use noosphere_storage::Storage;

mod authority;
mod content;
#[allow(clippy::module_inception)]
mod context;
mod cursor;
mod has;
mod replication;
mod walker;

mod internal;
pub mod metadata;
mod petname;
mod sync;

pub use authority::*;
pub use content::*;
pub use context::*;
pub use cursor::*;
pub use has::*;
pub use metadata::*;
pub use petname::*;
pub use replication::*;
pub use sync::*;
pub use walker::*;
