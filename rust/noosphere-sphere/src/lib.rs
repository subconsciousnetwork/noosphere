//! This crate implements content, petname and other forms of acccess to
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
//! use anyhow::Result;
//! use noosphere_sphere::helpers::{simulated_sphere_context,SimulationAccess};
//!
//! use noosphere_sphere::{SphereCursor, HasMutableSphereContext, SphereContentWrite};
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!   let mut sphere_context = simulated_sphere_context(SimulationAccess::ReadWrite).await?;
//!
//!   sphere_context.write("/foo", "text/plain", "bar".as_ref(), None).await?;
//!   sphere_context.save(None).await?;
//!
//!   Ok(())
//! }
//! ```
//!
//! You can also use a [SphereContext] to access petnames in the sphere:
//!
//! ```rust
//! use anyhow::Result;
//! use noosphere_sphere::helpers::{simulated_sphere_context,SimulationAccess};
//! use noosphere_core::data::Did;
//!
//! use noosphere_sphere::{SphereCursor, HasMutableSphereContext, SpherePetnameWrite};
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!   let mut sphere_context = simulated_sphere_context(SimulationAccess::ReadWrite).await?;
//!
//!   sphere_context.set_petname("cdata", Some("did:key:example".into())).await?;
//!   sphere_context.save(None).await?;
//!
//!   Ok(())
//! }
//! ```
//!
//!

#[macro_use]
extern crate tracing;

#[cfg(doc)]
use noosphere_core::data::Did;

#[cfg(doc)]
use noosphere_core::authority::Author;

#[cfg(doc)]
use noosphere_storage::Storage;

mod content;
mod context;
mod cursor;
mod has;
mod walker;

pub mod helpers;

mod internal;
pub mod metadata;
mod petname;
mod sync;

pub use content::*;
pub use context::*;
pub use cursor::*;
pub use has::*;
pub use metadata::*;
pub use petname::*;
pub use sync::*;
pub use walker::*;
