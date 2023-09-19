#![warn(missing_docs)]

//! This crate is a high-level entrypoint for embedders of the Noosphere
//! protocol. Embedders may use [NoosphereContext] to initialize a singleton
//! that enables manaing spheres, including creating new ones and joining
//! existing ones.
//!
//! ```rust,no_run
//! # use noosphere::{
//! #     NoosphereStorage, NoosphereSecurity, NoosphereNetwork, NoosphereContextConfiguration, NoosphereContext, sphere::SphereReceipt
//! # };
//! # use noosphere_core::{
//! #     context::{
//! #         HasMutableSphereContext, SphereContentWrite, SphereSync
//! #     },
//! #     data::ContentType
//! # };
//! # use url::Url;
//! # use anyhow::Result;
//! #
//! # #[tokio::main]
//! # pub async fn main() -> Result<()> {
//! let noosphere = NoosphereContext::new(NoosphereContextConfiguration {
//!     storage: NoosphereStorage::Scoped {
//!         path: "/path/to/block/storage".into(),
//!     },
//!     security: NoosphereSecurity::Insecure {
//!         path: "/path/to/key/storage".into(),
//!     },
//!     network: NoosphereNetwork::Http {
//!         gateway_api: Some(Url::parse("http://example.com")?),
//!         ipfs_gateway_url: None,
//!     },
//! })?;
//!
//! noosphere.create_key("my-key").await?;
//!
//! let SphereReceipt { identity, mnemonic } = noosphere.create_sphere("my-key").await?;
//!     
//! // identity is the sphere's DID
//! // mnemonic is a recovery phrase that must be stored securely by the user
//!
//! let mut sphere_channel = noosphere.get_sphere_channel(&identity).await?;
//! let sphere = sphere_channel.mutable();
//!
//! // Write something to the sphere's content space
//! sphere.write("foo", &ContentType::Text, "bar".as_bytes(), None).await?;
//! sphere.save(None).await?;
//! // Sync the sphere with the network via a Noosphere gateway
//! sphere.sync().await?;
//! #    Ok(())
//! # }
//! ```

#[macro_use]
extern crate tracing;

#[cfg(not(target_arch = "wasm32"))]
pub mod ffi;

#[cfg(target_arch = "wasm32")]
pub mod wasm;

pub mod key;

mod noosphere;
pub use crate::noosphere::*;

pub mod platform;
pub mod sphere;
pub mod storage;
