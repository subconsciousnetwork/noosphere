//! This crate implements the core functionality of the Noosphere CLI, otherwise
//! known as "orb". The Noosphere CLI has three goals:
//!
//! - Be a reference for those seeking to embed Noosphere in a client
//!   application
//! - Serve as a pedagogical tool for those seeking to explore the Noosphere
//!   protocol
//! - Provide swiss army knife-like utility for anyone using Noosphere protocol
//!   in their day-to-day lives
//!
//! Taken in its entirety, this crate implements various high-level routines
//! that are likely to be implemented by other apps that embed Noosphere,
//! including (but not limited to):
//!
//! - Saving, syncing, rendering and updating the content of spheres as it
//!   changes over time
//! - Following and unfollowing other spheres, and accessing their content
//! - Managing access to a sphere by other clients and devices

#![warn(missing_docs)]

#[cfg(not(target_arch = "wasm32"))]
#[macro_use]
extern crate tracing;

#[cfg(not(target_arch = "wasm32"))]
mod native;

#[cfg(not(target_arch = "wasm32"))]
pub use native::*;

#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(target_arch = "wasm32")]
pub use web::*;
